//! The game engine for the browser.
//!
//! One entry point: `call` takes a JSON command and returns a JSON reply.
//! Every reply carries `ok`, an optional `error` or `msg`, any `prompt` the
//! player must answer, the news since the last call, and (for commands that
//! change the game) a fresh `state` view of what the human player can see.
//!
//! Built for `wasm32-unknown-unknown` it exports `alloc`, `dealloc`, `call`
//! and `reply_len`; natively the same `handle` function is used by tests.

use landfall::actions::is_carrier;
use landfall::colony::{BELLS, CROSSES, GOOD_NAMES, HAMMERS, N_OUT};
use landfall::assembly::MEMBER_EFFECTS;
use landfall::events::Event;
use landfall::game::*;
use landfall::mapgen::{NewGame, World};
use landfall::rules::Rules;
use landfall::types::UnitType;
use landfall::world::{MoveResult, Prompt, VillageAction};
use serde_json::{Value, json};
use std::cell::RefCell;


thread_local! {
    static GAME: RefCell<Option<Game>> = const { RefCell::new(None) };
    static REPLY: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn rules() -> Rules {
    Rules::standard()
}

// ---------------------------------------------------------------- wasm ABI

#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// # Safety
/// `ptr` must come from `alloc(len)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, len: usize) {
    unsafe { drop(Vec::from_raw_parts(ptr, 0, len.max(1))) };
}

/// # Safety
/// `ptr..ptr+len` must be a UTF-8 JSON command written into memory from `alloc`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn call(ptr: *const u8, len: usize) -> *const u8 {
    let input = unsafe { std::slice::from_raw_parts(ptr, len) };
    let text = String::from_utf8_lossy(input);
    let out = handle(&text);
    REPLY.with(|r| {
        let mut r = r.borrow_mut();
        *r = out.into_bytes();
        r.as_ptr()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn reply_len() -> usize {
    REPLY.with(|r| r.borrow().len())
}

// ---------------------------------------------------------------- dispatch

/// Run one JSON command and return the JSON reply.
pub fn handle(text: &str) -> String {
    let cmd: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => return json!({"ok": false, "error": format!("bad command: {e}")}).to_string(),
    };
    let name = cmd["cmd"].as_str().unwrap_or("").to_string();
    let reply = GAME.with(|cell| {
        let mut slot = cell.borrow_mut();
        match name.as_str() {
            "meta" => return Reply::data(meta(slot.as_ref())),
            "new_game" => {
                *slot = Some(new_game(&cmd));
                let g = slot.as_mut().unwrap();
                run_gotos(g);
                return Reply::changed("");
            }
            "load" => {
                return match Game::from_json(cmd["json"].as_str().unwrap_or(""), rules()) {
                    Ok(g) => {
                        *slot = Some(g);
                        Reply::changed("Game loaded.")
                    }
                    Err(e) => Reply::err(format!("That save can't be read: {e}")),
                };
            }
            _ => {}
        }
        let Some(g) = slot.as_mut() else { return Reply::err("No game in progress.") };
        command(g, &name, &cmd)
    });
    GAME.with(|cell| {
        let mut slot = cell.borrow_mut();
        let mut out = json!({"ok": reply.error.is_none()});
        if let Some(e) = &reply.error {
            out["error"] = json!(e);
        }
        if !reply.msg.is_empty() {
            out["msg"] = json!(reply.msg);
        }
        if let Some(p) = reply.prompt {
            out["prompt"] = p;
        }
        if let Some(d) = reply.data {
            out["data"] = d;
        }
        if let Some(g) = slot.as_mut() {
            let news: Vec<String> = std::mem::take(&mut g.messages);
            let events: Vec<Value> = std::mem::take(&mut g.events).into_iter().map(|e| event_json(g, e)).collect();
            g.sounds.clear();
            out["news"] = json!(news);
            out["events"] = json!(events);
            if reply.changed {
                out["state"] = state(g);
            }
        }
        out.to_string()
    })
}

#[derive(Default)]
struct Reply {
    error: Option<String>,
    msg: String,
    prompt: Option<Value>,
    data: Option<Value>,
    changed: bool,
}

impl Reply {
    fn err(e: impl Into<String>) -> Self {
        Reply { error: Some(e.into()), ..Default::default() }
    }
    fn changed(msg: impl Into<String>) -> Self {
        Reply { msg: msg.into(), changed: true, ..Default::default() }
    }
    fn data(d: Value) -> Self {
        Reply { data: Some(d), ..Default::default() }
    }
    fn from(r: Result<String, String>) -> Self {
        match r {
            Ok(m) => Reply::changed(m),
            Err(e) => Reply { error: Some(e), changed: true, ..Default::default() },
        }
    }
}

fn id(cmd: &Value, k: &str) -> Id {
    cmd[k].as_u64().unwrap_or(0) as Id
}
fn int(cmd: &Value, k: &str) -> i32 {
    cmd[k].as_i64().unwrap_or(0) as i32
}

fn new_game(cmd: &Value) -> Game {
    let w = &cmd["world"];
    let world = World {
        land: w["land"].as_u64().unwrap_or(1) as u8,
        shape: w["shape"].as_u64().unwrap_or(1) as u8,
        temperature: w["temperature"].as_u64().unwrap_or(1) as u8,
        climate: w["climate"].as_u64().unwrap_or(1) as u8,
    };
    let opts = NewGame {
        human: cmd["nation"].as_u64().unwrap_or(0).min(3) as u8,
        difficulty: cmd["difficulty"].as_u64().unwrap_or(1).min(4) as u8,
        world,
        seed: cmd["seed"].as_u64().unwrap_or(1),
    };
    let mut g = Game::new_game(rules(), &opts);
    g.messages.clear();
    g.milestone(landfall::events::MS_NEW_WORLD);
    g
}

/// Human units with go-to orders keep walking.
fn run_gotos(g: &mut Game) -> Option<Value> {
    let h = g.human;
    let ids: Vec<Id> = g
        .units
        .iter()
        .filter(|u| u.nation == h && matches!(u.orders, Orders::Goto { .. }) && u.moves > 0 && matches!(u.loc, Loc::Map { .. }))
        .map(|u| u.id)
        .collect();
    for id in ids {
        match g.continue_goto(id) {
            Some(MoveResult::Ask(p)) => return Some(prompt_json(g, p)),
            Some(MoveResult::Blocked(why)) => g.msg(why),
            _ => {}
        }
    }
    None
}

fn move_reply(g: &mut Game, r: Option<MoveResult>) -> Reply {
    match r {
        Some(MoveResult::Ask(p)) => Reply { prompt: Some(prompt_json(g, p)), changed: true, ..Default::default() },
        Some(MoveResult::Blocked(why)) => Reply { error: Some(why), changed: true, ..Default::default() },
        _ => Reply::changed(""),
    }
}

fn command(g: &mut Game, name: &str, cmd: &Value) -> Reply {
    let h = g.human;
    match name {
        "state" => Reply::changed(""),
        "save" => Reply::data(json!({"json": g.to_json(), "summary": format!("{}, {}", g.nation_name(h), g.year_label())})),

        // ---- the map
        "move" => {
            let u = id(cmd, "unit");
            if g.unit(u).is_none_or(|x| x.nation != h) {
                return Reply::err("That unit isn't yours.");
            }
            let r = g.try_move(u, int(cmd, "dx").clamp(-1, 1), int(cmd, "dy").clamp(-1, 1));
            move_reply(g, Some(r))
        }
        "goto" => {
            let u = id(cmd, "unit");
            if g.unit(u).is_none_or(|x| x.nation != h) {
                return Reply::err("That unit isn't yours.");
            }
            let r = g.goto(u, int(cmd, "x"), int(cmd, "y"));
            move_reply(g, r)
        }
        "path" => {
            let u = id(cmd, "unit");
            let Some(unit) = g.unit(u).cloned() else { return Reply::err("No such unit.") };
            let Some(path) = g.find_path(u, int(cmd, "x"), int(cmd, "y")) else { return Reply::data(json!({"path": null})) };
            let full = g.max_moves(unit.kind) as i32;
            let mut left = unit.moves as i32;
            let mut turn = 0;
            let start = g.unit_pos(u).unwrap_or((0, 0));
            let (mut fx, mut fy) = start;
            let steps: Vec<[i32; 3]> = path
                .iter()
                .filter(|&&p| p != start)
                .map(|&(x, y)| {
                    let unknown = g.tile(x, y).is_some_and(|t| !t.visible_to(h));
                    let cost = if unit.kind.is_ship() || unknown { 3 } else { g.move_cost(fx, fy, x, y) as i32 };
                    if left <= 0 {
                        turn += 1;
                        left = full;
                    }
                    left -= cost;
                    fx = x;
                    fy = y;
                    [x, y, turn]
                })
                .collect();
            Reply::data(json!({"path": steps}))
        }
        "orders" => {
            let u = id(cmd, "unit");
            if g.unit(u).is_none_or(|x| x.nation != h) {
                return Reply::err("That unit isn't yours.");
            }
            let o = match cmd["orders"].as_str().unwrap_or("") {
                "sentry" => Orders::Sentry,
                "fortify" => Orders::Fortify,
                "none" => Orders::None,
                _ => return Reply::err("Unknown orders."),
            };
            g.set_orders(u, o);
            Reply::changed("")
        }
        "skip" => {
            g.skip(id(cmd, "unit"));
            Reply::changed("")
        }
        "activate" => Reply::from(g.activate(id(cmd, "unit")).map(|_| String::new())),
        "improve" => {
            let road = cmd["what"].as_str() == Some("road");
            Reply::from(g.start_improvement(id(cmd, "unit"), road).map(|_| String::new()))
        }
        "disband" => Reply::from(g.disband(id(cmd, "unit")).map(|_| "Unit disbanded.".into())),
        "found" => {
            let u = id(cmd, "unit");
            if let Err(e) = g.can_found_colony(u) {
                return Reply::err(e);
            }
            let (x, y) = g.unit_pos(u).unwrap_or((0, 0));
            if let Some(claim) = g.land_claim(h, x, y) {
                match cmd["claim"].as_str() {
                    Some("pay") => {
                        if let Err(e) = g.settle_claim(h, &claim, true) {
                            return Reply::err(e);
                        }
                    }
                    Some("take") => {
                        let _ = g.settle_claim(h, &claim, false);
                    }
                    _ => {
                        return Reply {
                            prompt: Some(json!({
                                "type": "land_claim", "unit": u, "tribe": g.nation_name(4 + claim.tribe as u8), "price": claim.price,
                                "gold": g.human_player().gold,
                            })),
                            ..Default::default()
                        };
                    }
                }
            }
            match g.found_named(u, cmd["name"].as_str().unwrap_or("")) {
                Ok(cid) => Reply { data: Some(json!({"colony": cid})), changed: true, ..Default::default() },
                Err(e) => Reply::err(e),
            }
        }
        "tile" => {
            let (x, y) = (int(cmd, "x"), int(cmd, "y"));
            let Some(t) = g.tile(x, y) else { return Reply::err("Off the map.") };
            if !t.visible_to(h) {
                return Reply::data(json!({"explored": false}));
            }
            let yields: Vec<i32> = (0..9u8).map(|j| g.base_yield(x, y, j)).collect();
            let claim = g.land_claim(h, x, y).map(|c| json!({"tribe": g.nation_name(4 + c.tribe as u8), "price": c.price}));
            let moves = g.rules.terrain.get(g.terrain_row(t)).map(|r| r.movement).unwrap_or(1);
            Reply::data(json!({
                "explored": true, "terrain": g.terrain_name(x, y), "yields": yields, "claim": claim,
                "resource": g.resource_at(x, y).and_then(|r| g.rules.resources.get(r as usize).cloned()),
                "road": t.road(), "plowed": t.plowed(), "river": t.river(), "move_cost": moves,
                "defence": g.rules.terrain.get(g.terrain_row(t)).map(|r| r.defence).unwrap_or(0),
                "rumour": g.rumour.get((y * g.width as i32 + x) as usize).copied().unwrap_or(false),
            }))
        }
        "suggest_name" => Reply::data(json!({"name": g.next_colony_name(h)})),
        "join" => Reply::from(g.join_colony(id(cmd, "unit")).map(|_| "The colonist joins the colony.".into())),
        "attack" => {
            g.attack(id(cmd, "unit"), int(cmd, "x"), int(cmd, "y"));
            Reply::changed("")
        }
        "odds" => Reply::data(json!({"odds": g.attack_odds(id(cmd, "unit"), int(cmd, "x"), int(cmd, "y"))})),
        "sail_home" => {
            if g.rebellion.is_some() {
                return Reply::err(format!("{} is closed to the rebels.", g.rules.names.homeland));
            }
            g.sail_to_homeland(id(cmd, "unit"));
            Reply::changed("")
        }
        "landfall" => {
            g.landfall(id(cmd, "unit"), int(cmd, "x"), int(cmd, "y"));
            Reply::changed("")
        }
        "village" => {
            let u = id(cmd, "unit");
            let vi = cmd["village"].as_u64().unwrap_or(0) as usize;
            let a = match cmd["action"].as_str().unwrap_or("") {
                "speak" => VillageAction::SpeakWithChief,
                "learn" => VillageAction::LearnSkill,
                "mission" => VillageAction::EstablishMission,
                "trade" => VillageAction::Trade,
                "attack" => VillageAction::Attack,
                "tribute" => VillageAction::DemandTribute,
                _ => return Reply::err("Unknown action."),
            };
            if vi >= g.villages.len() || !g.village_actions(u).contains(&a) {
                return Reply::err("That unit can't do that here.");
            }
            g.village_action(u, vi, a);
            Reply::changed("")
        }
        "end_turn" => {
            if g.game_over.is_some() && !cmd["force"].as_bool().unwrap_or(false) {
                return Reply::err("The game is over.");
            }
            g.end_turn();
            let p = run_gotos(g);
            Reply { prompt: p, changed: true, ..Default::default() }
        }
        "run_gotos" => {
            let p = run_gotos(g);
            Reply { prompt: p, changed: true, ..Default::default() }
        }
        "set_route" => {
            let u = id(cmd, "unit");
            if cmd["clear"].as_bool() == Some(true) {
                g.set_route(u, None);
                return Reply::changed("Route cleared.");
            }
            let place = |v: &Value| -> Option<Place> {
                if v.as_str() == Some("homeland") { Some(Place::Homeland) } else { v.as_u64().map(|c| Place::Colony(c as Id)) }
            };
            let (Some(a), Some(b)) = (place(&cmd["a"]), place(&cmd["b"])) else { return Reply::err("Pick two stops.") };
            let goods = |v: &Value| -> Vec<u8> { v.as_array().map(|a| a.iter().filter_map(|x| x.as_u64()).filter(|x| *x < 16).map(|x| x as u8).collect()).unwrap_or_default() };
            let r = TradeRoute { a, b, a_to_b: goods(&cmd["a_to_b"]), b_to_a: goods(&cmd["b_to_a"]), to_b: false };
            let s = g.route_summary(&r);
            g.set_route(u, Some(r));
            Reply::changed(format!("Route set: {s}."))
        }
        "route_places" => {
            let u = id(cmd, "unit");
            let list: Vec<Value> = g
                .route_places(u)
                .into_iter()
                .map(|p| match p {
                    Place::Homeland => json!({"id": "homeland", "name": g.place_name(p)}),
                    Place::Colony(c) => json!({"id": c, "name": g.place_name(p)}),
                })
                .collect();
            Reply::data(json!({"places": list}))
        }

        // ---- colonies
        "colony" => match colony_view(g, id(cmd, "colony")) {
            Some(v) => Reply::data(v),
            None => Reply::err("No such colony."),
        },
        "assign" => {
            let c = id(cmd, "colony");
            let u = id(cmd, "unit");
            let job = match job_from(g, cmd, c, u) {
                Ok(j) => j,
                Err(e) => return Reply::err(e),
            };
            Reply::from(g.assign_job(c, u, job))
        }
        "preview" => {
            let c = id(cmd, "colony");
            let u = id(cmd, "unit");
            let job = match job_from(g, cmd, c, u) {
                Ok(j) => j,
                Err(e) => return Reply::err(e),
            };
            match g.preview_delta(c, u, job) {
                Ok(d) => Reply::data(json!({"delta": d.to_vec(), "job": job_json(job)})),
                Err(e) => Reply::err(e),
            }
        }
        "step_out" => Reply::from(g.step_outside(id(cmd, "unit"))),
        "board" => Reply::from(g.board(id(cmd, "unit"), id(cmd, "ship"))),
        "load_goods" => {
            let amount = cmd["amount"].as_i64().map(|a| a as i32);
            Reply::from(g.load_goods(id(cmd, "colony"), id(cmd, "carrier"), cmd["good"].as_u64().unwrap_or(99) as usize, amount))
        }
        "unload" => {
            let amount = cmd["amount"].as_i64().map(|a| a as i32);
            Reply::from(g.unload_goods(id(cmd, "carrier"), cmd["slot"].as_u64().unwrap_or(99) as usize, amount))
        }
        "build" => {
            let c = id(cmd, "colony");
            let b = build_from(&cmd["build"]);
            Reply::from(g.set_build(c, b))
        }
        "rush" => Reply::from(g.rush_build(id(cmd, "colony"))),
        "rename" => Reply::from(g.rename_colony(id(cmd, "colony"), cmd["name"].as_str().unwrap_or(""))),
        "equip" => {
            let Some(k) = kind_from(&cmd["kind"]) else { return Reply::err("Unknown equipment.") };
            let u = id(cmd, "unit");
            let in_homeland = g.unit(u).is_some_and(|x| x.loc == Loc::Homeland);
            let r = if in_homeland { g.equip_in_homeland(u, k) } else { g.equip(u, k) };
            let name = g.kind_name(k).to_string();
            Reply::from(r.map(|_| format!("Equipped as {name}.")))
        }
        "abandon" => Reply::from(g.abandon_colony(id(cmd, "colony")).map(|_| "Colony abandoned.".into())),

        // ---- the homeland
        "homeland" => Reply::data(homeland_view(g)),
        "buy" => {
            let ship = id(cmd, "ship");
            let gd = cmd["good"].as_u64().unwrap_or(99) as usize;
            let r = g.buy_cargo(ship, gd).map(|n| format!("Bought {n} {} at {} each.", GOOD_NAMES[gd.min(15)], g.ask_price(h, gd.min(15))));
            Reply::from(r)
        }
        "sell" => {
            let ship = id(cmd, "ship");
            let slot = cmd["slot"].as_u64().unwrap_or(99) as usize;
            let what = g.unit(ship).and_then(|u| u.cargo.get(slot).copied());
            let r = g.sell_cargo(ship, slot).map(|gold| match what {
                Some((gd, n)) => format!("Sold {n} {} for {gold} gold after tax.", GOOD_NAMES[gd as usize]),
                None => String::new(),
            });
            Reply::from(r)
        }
        "pay_arrears" => Reply::from(g.pay_arrears(h, cmd["good"].as_u64().unwrap_or(0) as usize).map(|c| format!("Paid {c} gold. The boycott is lifted."))),
        "recruit" => {
            let r = g.recruit(h, cmd["slot"].as_u64().unwrap_or(0).min(2) as usize).map(|id| {
                let n = g.unit(id).map(|u| g.unit_name(u)).unwrap_or_default();
                format!("Recruited a {n}.")
            });
            Reply::from(r)
        }
        "train" => {
            let p = cmd["prof"].as_u64().unwrap_or(99) as u8;
            let r = g.train(h, p).map(|_| format!("Trained a {}.", g.prof_name(p)));
            Reply::from(r)
        }
        "buy_unit" => {
            let Some(k) = kind_from(&cmd["kind"]) else { return Reply::err("Unknown unit.") };
            let r = g.buy_unit(h, k).map(|_| format!("Bought a {}.", g.kind_name(k)));
            Reply::from(r)
        }
        "disembark" => Reply::from(g.disembark_in_homeland(id(cmd, "unit"))),
        "sail_new_world" => Reply::from(g.sail_to_new_world(id(cmd, "ship")).map(|_| "The ship sails for the New World.".into())),

        // ---- answers to events
        "tax" => {
            if cmd["accept"].as_bool().unwrap_or(true) {
                g.accept_tax();
                Reply::changed("")
            } else {
                match g.refuse_tax() {
                    Ok(()) => Reply::changed(""),
                    Err(e) => {
                        g.accept_tax();
                        Reply { error: Some(e), changed: true, ..Default::default() }
                    }
                }
            }
        }
        "native_trade" => {
            let m = g.answer_native_trade(
                cmd["tribe"].as_u64().unwrap_or(0) as usize,
                id(cmd, "colony"),
                cmd["good"].as_u64().unwrap_or(0) as usize,
                int(cmd, "amount"),
                int(cmd, "price"),
                cmd["haggle"].as_bool().unwrap_or(false),
            );
            if cmd["decline"].as_bool() == Some(true) {
                return Reply::changed("");
            }
            Reply::changed(m)
        }
        "native_demand" => {
            let good = cmd["good"].as_u64().map(|x| x as u8);
            g.answer_demand(cmd["tribe"].as_u64().unwrap_or(0) as usize, id(cmd, "colony"), good, int(cmd, "amount"), cmd["give"].as_bool().unwrap_or(true));
            Reply::changed("")
        }
        "mercenaries" => {
            if cmd["hire"].as_bool() == Some(true) {
                Reply::from(g.hire_mercenaries(int(cmd, "price")).map(|_| "The mercenaries join your army.".into()))
            } else {
                Reply::changed("")
            }
        }
        "peace" => {
            let n = cmd["nation"].as_u64().unwrap_or(0) as u8;
            if cmd["accept"].as_bool() == Some(true) && n < 4 {
                g.make_peace(h, n);
            }
            Reply::changed("")
        }
        "declare_war" => {
            let n = cmd["nation"].as_u64().unwrap_or(9) as u8;
            if n >= 4 || n == h {
                return Reply::err("No such power.");
            }
            g.declare_war(h, n);
            Reply::changed("")
        }
        "propose_peace" => {
            let n = cmd["nation"].as_u64().unwrap_or(9) as u8;
            if n >= 4 || n == h {
                return Reply::err("No such power.");
            }
            let ok = g.propose_peace(h, n);
            let who = g.adjective(n).to_string();
            Reply::changed(if ok { format!("The {who} accept your offer of peace.") } else { format!("The {who} refuse. The war goes on.") })
        }
        "choose_member" => {
            let f = cmd["member"].as_u64().unwrap_or(99) as u8;
            if !g.players[h as usize].assembly.offered.contains(&f) {
                return Reply::err("That candidate isn't on offer.");
            }
            g.choose_member(h, f);
            Reply::changed("")
        }
        "declare" => Reply::from(g.declare_independence(h).map(|_| String::new())),
        "reports" => Reply::data(reports(g)),
        _ => Reply::err(format!("Unknown command '{name}'.")),
    }
}

fn job_from(g: &Game, cmd: &Value, cid: Id, unit: Id) -> Result<Job, String> {
    let j = &cmd["job"];
    if let Some(b) = j["building"].as_u64() {
        return Ok(Job::Building(b as u8));
    }
    let dir = j["dir"].as_u64().ok_or("Pick a square or a building.")? as u8;
    let job = match j["job"].as_u64() {
        Some(x) => x as u8,
        None => g.best_tile_job(cid, unit, dir).ok_or("Nothing can be produced there.")?,
    };
    Ok(Job::Tile { dir, job })
}

fn job_json(j: Job) -> Value {
    match j {
        Job::Tile { dir, job } => json!({"dir": dir, "job": job}),
        Job::Building(b) => json!({"building": b}),
    }
}

fn kind_from(v: &Value) -> Option<UnitType> {
    v.as_u64().and_then(|k| UnitType::from_u8(k as u8))
}

fn build_from(v: &Value) -> Option<Build> {
    if let Some(b) = v["building"].as_u64() {
        return BKINDS.get(b as usize).map(|k| Build::Building(*k));
    }
    kind_from(&v["unit"]).map(Build::Unit)
}

fn build_json(b: Build) -> Value {
    match b {
        Build::Building(k) => json!({"building": k as u8}),
        Build::Unit(u) => json!({"unit": u as u8}),
    }
}

fn orders_str(o: Orders) -> &'static str {
    match o {
        Orders::None => "none",
        Orders::Sentry => "sentry",
        Orders::Fortify | Orders::Fortified => "fortify",
        Orders::Plow { .. } => "plow",
        Orders::Road { .. } => "road",
        Orders::Goto { .. } => "goto",
        Orders::Route => "route",
    }
}

// ---------------------------------------------------------------- views

fn meta(g: Option<&Game>) -> Value {
    let r = match g {
        Some(g) => g.rules.clone(),
        None => rules(),
    };
    let buildings: Vec<Value> = BKINDS
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let levels: Vec<String> = (0..BUILDING_MAX[i] as usize)
                .map(|l| r.buildings.get(BUILDING_ROW[i] + l).map(|b| b.name.clone()).unwrap_or_default())
                .collect();
            json!({"kind": *k as u8, "levels": levels})
        })
        .collect();
    let job_buildings: Vec<Value> = (9..=18u8)
        .map(|j| {
            let (k, input, output) = job_building(j).unwrap();
            json!({"job": j, "building": k as u8, "input": input, "output": output})
        })
        .collect();
    json!({
        "width": landfall::mapgen::MAP_W, "height": landfall::mapgen::MAP_H,
        "goods": GOOD_NAMES.to_vec(),
        "jobs": JOB_NAMES.to_vec(),
        "job_good": JOB_GOOD.to_vec(),
        "job_buildings": job_buildings,
        "units": r.names.units, "professions": r.names.professions,
        "nations": r.names.nations, "adjectives": r.names.adjectives,
        "leaders": r.leaders, "independent": r.names.independent,
        "homeland": r.names.homeland, "far_ocean": r.names.far_ocean, "assembly": r.names.assembly,
        "terrain": r.terrain.iter().map(|t| t.name.clone()).collect::<Vec<_>>(),
        "buildings": buildings,
        "members": r.members.iter().enumerate().map(|(i, f)| json!({"name": f.name, "kind": f.kind, "effect": MEMBER_EFFECTS.get(i).copied().unwrap_or("")})).collect::<Vec<_>>(),
        "difficulties": ["Wayfarer", "Settler", "Pathfinder", "Magistrate", "Sovereign"],
        "resources": r.resources,
        "unit_stats": r.units.iter().map(|u| json!({"moves": u.movement, "attack": u.attack, "defence": u.combat, "holds": u.holds})).collect::<Vec<_>>(),
    })
}

fn unit_json(g: &Game, u: &Unit, mine: bool) -> Value {
    let pos = g.unit_pos(u.id);
    let mut v = json!({
        "id": u.id, "kind": u.kind as u8, "nation": u.nation, "name": g.unit_name(u),
        "x": pos.map(|p| p.0), "y": pos.map(|p| p.1), "prof": u.prof,
    });
    if mine {
        let loc = match u.loc {
            Loc::Map { .. } => json!({"type": "map"}),
            Loc::Aboard(s) => json!({"type": "aboard", "ship": s}),
            Loc::Colony { colony, job } => json!({"type": "colony", "colony": colony, "job": job_json(job)}),
            Loc::Homeland => json!({"type": "homeland"}),
            Loc::Sailing { to_homeland, turns } => json!({"type": "sailing", "to_homeland": to_homeland, "turns": turns}),
            Loc::Dead => json!({"type": "dead"}),
        };
        v["loc"] = loc;
        v["moves"] = json!(u.moves);
        v["max_moves"] = json!(g.max_moves(u.kind));
        v["orders"] = json!(orders_str(u.orders));
        v["done"] = json!(u.done);
        v["cargo"] = json!(u.cargo);
        v["tools"] = json!(u.tools);
        v["treasure"] = json!(u.treasure);
        v["damaged"] = json!(u.damaged);
        if is_carrier(u.kind) {
            v["holds"] = json!(g.holds(u.id));
            v["holds_used"] = json!(g.holds_used(u.id));
        }
        if let Orders::Goto { x, y } = u.orders {
            v["goto"] = json!([x, y]);
        }
        if let Some(r) = &u.route {
            v["route"] = json!(g.route_summary(r));
        }
    } else {
        v["fortified"] = json!(matches!(u.orders, Orders::Fortified | Orders::Fortify));
    }
    v
}

fn state(g: &Game) -> Value {
    let h = g.human;
    let (w, ht) = (g.width as i32, g.height as i32);
    // terrain of explored squares; 255 = unexplored
    let mut raw = Vec::with_capacity(g.tiles.len());
    let mut mask = Vec::with_capacity(g.tiles.len());
    let mut vis = Vec::with_capacity(g.tiles.len());
    for y in 0..ht {
        for x in 0..w {
            let t = g.tile(x, y).unwrap();
            let explored = t.visible_to(h);
            raw.push(if explored { t.raw } else { 255 });
            mask.push(if explored { t.mask } else { 0 });
            vis.push(u8::from(explored && g.visible(h, x, y)));
        }
    }
    let units: Vec<Value> = g
        .units
        .iter()
        .filter(|u| u.loc != Loc::Dead)
        .filter_map(|u| {
            if u.nation == h {
                return Some(unit_json(g, u, true));
            }
            match u.loc {
                Loc::Map { x, y } if g.visible(h, x as i32, y as i32) => Some(unit_json(g, u, false)),
                _ => None,
            }
        })
        .collect();
    let colonies: Vec<Value> = g
        .colonies
        .iter()
        .filter(|c| c.nation == h || g.tile(c.x as i32, c.y as i32).is_some_and(|t| t.visible_to(h)))
        .map(|c| {
            let mut v = json!({"id": c.id, "name": c.name, "nation": c.nation, "x": c.x, "y": c.y, "pop": g.population(c.id), "stockade": c.level(BKind::Fortification)});
            if c.nation == h {
                let p = g.production(c.id);
                v["food"] = json!(p.net(FOOD));
                v["building"] = json!(c.building.map(|b| g.build_name(b, c)));
                v["liberty"] = json!(g.liberty_percent(c));
                v["coastal"] = json!(g.is_coastal(c.x as i32, c.y as i32));
                v["stock"] = json!(c.stock.to_vec());
                v["capacity"] = json!(c.capacity());
                let mut warn = Vec::new();
                if p.net(FOOD) < 0 && c.stock[FOOD] + p.net(FOOD) * 3 < 0 {
                    let turns = (c.stock[FOOD] / (-p.net(FOOD)).max(1)).max(0);
                    warn.push(format!("Starving: food runs out in {turns} turns"));
                }
                let cap = c.capacity();
                for gd in 1..16 {
                    let n = p.net(gd);
                    if n > 0 && c.stock[gd] + n > cap {
                        warn.push(format!("{} will overflow the warehouse", GOOD_NAMES[gd]));
                    }
                }
                if c.building.is_none() {
                    warn.push("Nothing is being built".into());
                }
                v["warnings"] = json!(warn);
            }
            v
        })
        .collect();
    let villages: Vec<Value> = g
        .villages
        .iter()
        .enumerate()
        .filter(|(_, v)| g.tile(v.x as i32, v.y as i32).is_some_and(|t| t.visible_to(h)))
        .map(|(i, v)| {
            let ti = v.tribe.saturating_sub(4) as usize;
            json!({
                "index": i, "x": v.x, "y": v.y, "tribe": v.tribe, "capital": v.capital, "pop": v.pop,
                "mission": v.mission, "scouted": v.scouted & (1 << h) != 0,
                "teaches": if v.scouted & (1 << h) != 0 { v.teaches.map(|t| g.prof_name(t).to_string()) } else { None },
                "alarm": g.tribes.get(ti).map(|t| t.alarm[h as usize]).unwrap_or(0),
            })
        })
        .collect();
    let p = &g.players[h as usize];
    let players: Vec<Value> = (0..4u8)
        .map(|n| {
            let pl = &g.players[n as usize];
            json!({
                "nation": n, "name": g.nation_name(n), "adjective": g.adjective(n), "leader": pl.name,
                "colonies": g.colonies.iter().filter(|c| c.nation == n).count(),
                "known": n == h || g.knows(h, n), "at_war": n != h && g.at_war(h, n),
                "ai": n != h,
            })
        })
        .collect();
    let cong = &p.assembly;
    let rumours: Vec<[i32; 2]> = (0..ht)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| g.rumour.get((y * w + x) as usize).copied().unwrap_or(false) && g.tile(x, y).is_some_and(|t| t.visible_to(h)))
        .map(|(x, y)| [x, y])
        .collect();
    let resources: Vec<[i32; 3]> = (0..ht)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter_map(|(x, y)| {
            let t = g.tile(x, y)?;
            if !t.visible_to(h) {
                return None;
            }
            g.resource_at(x, y).map(|r| [x, y, r as i32])
        })
        .collect();
    json!({
        "turn": g.turn, "year": g.year, "autumn": g.autumn, "year_label": g.year_label(),
        "human": h, "difficulty": g.difficulty,
        "gold": p.gold, "tax": p.tax, "bells": p.bells, "crosses": p.crosses, "crosses_needed": p.crosses_needed,
        "width": w, "height": ht, "raw": raw, "mask": mask, "vis": vis,
        "rumours": rumours, "resources": resources,
        "units": units, "colonies": colonies, "villages": villages, "players": players,
        "next_active": g.next_active(None),
        "assembly": {
            "members": cong.members, "candidate": cong.candidate, "bells": cong.bells,
            "cost": g.member_cost(h), "offered": cong.offered,
        },
        "rebel_percent": g.rebel_percent(h),
        "can_declare": g.can_declare(h).err(),
        "rebellion": g.rebellion.as_ref().map(|r| json!({"nation": r.nation, "turn": r.declared_turn})),
        "crown_units": g.crown_units(),
        "tax_demand": g.tax_demand.as_ref().map(|t| json!({
            "rate": t.rate, "good": t.good, "good_name": GOOD_NAMES[t.good as usize],
            "colony": t.colony.and_then(|c| g.colony(c)).map(|c| c.name.clone()),
        })),
        "game_over": g.game_over.as_ref().map(|(won, text)| json!({"won": won, "text": text})),
        "score": g.score(h),
    })
}

fn colony_view(g: &Game, cid: Id) -> Option<Value> {
    let c = g.colony(cid)?;
    if c.nation != g.human {
        return None;
    }
    let p = g.production(cid);
    let (cx, cy) = (c.x as i32, c.y as i32);
    let mut net = [0; N_OUT];
    for (gd, n) in net.iter_mut().enumerate() {
        *n = if gd < 16 { p.net(gd) } else { p.made[gd] };
    }
    let workers: Vec<Value> = g
        .workers(cid)
        .map(|u| {
            let Loc::Colony { job, .. } = u.loc else { unreachable!() };
            let out = match job {
                Job::Tile { dir, job } => {
                    let (x, y) = g.colony_square(c, dir);
                    g.tile_output(c, u.prof, x, y, job)
                }
                Job::Building(j) => g.building_output(c, u.prof, j),
            };
            json!({"id": u.id, "name": g.unit_name(u), "prof": u.prof, "kind": u.kind as u8, "job": job_json(job), "output": out})
        })
        .collect();
    let field_food: i32 = g
        .workers(cid)
        .filter_map(|u| match u.loc {
            Loc::Colony { job: Job::Tile { dir, job }, .. } if JOB_GOOD[job as usize] == FOOD => {
                let (x, y) = g.colony_square(c, dir);
                Some(g.tile_output(c, u.prof, x, y, job))
            }
            _ => None,
        })
        .sum();
    let squares: Vec<Value> = (0..8u8)
        .map(|dir| {
            let (x, y) = g.colony_square(c, dir);
            let t = g.tile(x, y);
            let yields: Vec<i32> = (0..9u8).map(|j| if t.is_some() { g.tile_output(c, P_FREE, x, y, j) } else { 0 }).collect();
            json!({
                "dir": dir, "x": x, "y": y,
                "terrain": t.map(|_| g.terrain_name(x, y)),
                "raw": t.map(|t| t.raw), "mask": t.map(|t| t.mask),
                "taken": g.square_taken(x, y, None) && !g.workers(cid).any(|u| matches!(u.loc, Loc::Colony { job: Job::Tile { dir: d, .. }, .. } if d == dir)),
                "village": g.village_at(x, y).is_some(),
                "water": g.is_water(x, y),
                "yields": yields,
                "resource": g.resource_at(x, y),
            })
        })
        .collect();
    let options: Vec<Value> = g
        .build_options(cid)
        .into_iter()
        .map(|b| {
            let (hm, t) = g.build_cost(b, c);
            json!({"build": build_json(b), "name": g.build_name(b, c), "hammers": hm, "tools": t})
        })
        .collect();
    let current = c.building.map(|b| {
        let (hm, t) = g.build_cost(b, c);
        let rate = p.made[HAMMERS].max(0);
        let left = (hm - c.hammers).max(0);
        json!({
            "build": build_json(b), "name": g.build_name(b, c), "hammers": hm, "tools": t,
            "turns": if rate > 0 { Some((left + rate - 1) / rate) } else { None },
        })
    });
    let here: Vec<Value> = g
        .units_at(cx, cy)
        .filter(|u| u.nation == g.human && !matches!(u.loc, Loc::Colony { .. }))
        .map(|u| unit_json(g, u, true))
        .collect();
    let buildings: Vec<Value> = BKINDS.iter().map(|k| json!({"kind": *k as u8, "level": c.level(*k)})).collect();
    let teachers: Vec<Value> = g
        .workers(cid)
        .filter(|u| matches!(u.loc, Loc::Colony { job: Job::Building(18), .. }))
        .map(|u| json!({"id": u.id, "teach": u.teach}))
        .collect();
    Some(json!({
        "id": cid, "name": c.name, "x": cx, "y": cy, "stock": c.stock.to_vec(), "capacity": c.capacity(),
        "hammers": c.hammers, "liberty": c.liberty, "liberty_share": g.liberty_percent(c), "loyalist_penalty": g.loyalist_penalty(c),
        "net": net.to_vec(), "made": p.made.to_vec(), "used": p.used.to_vec(), "eaten": p.eaten, "bred": p.bred,
        "center": {"terrain": g.terrain_name(cx, cy), "raw": g.tile(cx, cy).map(|t| t.raw), "mask": g.tile(cx, cy).map(|t| t.mask), "food": p.made[FOOD] - field_food, "secondary": g.center_secondary(c)},
        "workers": workers, "squares": squares, "buildings": buildings,
        "building": current, "options": options, "rush_price": g.rush_price(cid),
        "units": here, "coastal": g.is_coastal(cx, cy), "teachers": teachers,
        "bells": p.made[BELLS], "crosses": p.made[CROSSES],
        "gold": g.human_player().gold,
    }))
}

fn homeland_view(g: &Game) -> Value {
    let h = g.human;
    let p = &g.players[h as usize];
    let market: Vec<Value> = (0..16)
        .map(|gd| json!({"good": gd, "name": GOOD_NAMES[gd], "bid": g.bid_price(h, gd), "ask": g.ask_price(h, gd), "boycott": g.boycotted(h, gd), "arrears": g.arrears(h, gd)}))
        .collect();
    let recruits: Vec<Value> = p.recruits.iter().map(|r| json!({"prof": r, "name": g.prof_name(*r)})).collect();
    let mut profs: Vec<u8> = (0..18u8).filter(|q| g.train_price(*q).is_some()).collect();
    profs.extend([P_HARDY_PIONEER, P_VETERAN, P_MISSIONARY]);
    let train: Vec<Value> = profs.iter().filter_map(|q| g.train_price(*q).map(|price| json!({"prof": q, "name": g.prof_name(*q), "price": price}))).collect();
    let kinds = [UnitType::Artillery, UnitType::Caravel, UnitType::Merchantman, UnitType::Galleon, UnitType::Privateer, UnitType::Frigate];
    let purchase: Vec<Value> = kinds
        .iter()
        .filter_map(|k| g.unit_price(h, *k).map(|price| json!({"kind": *k as u8, "name": g.kind_name(*k), "price": price})))
        .collect();
    let ask = |gd: usize| g.ask_price(h, gd);
    let equip = json!([
        {"kind": UnitType::Colonist as u8, "name": "No equipment (sell it back)", "price": 0},
        {"kind": UnitType::Soldier as u8, "name": g.kind_name(UnitType::Soldier), "price": 50 * ask(MUSKETS)},
        {"kind": UnitType::Dragoon as u8, "name": g.kind_name(UnitType::Dragoon), "price": 50 * (ask(MUSKETS) + ask(HORSES))},
        {"kind": UnitType::Pioneer as u8, "name": g.kind_name(UnitType::Pioneer), "price": 100 * ask(TOOLS)},
        {"kind": UnitType::Scout as u8, "name": g.kind_name(UnitType::Scout), "price": 50 * ask(HORSES)},
    ]);
    json!({
        "homeland": g.rules.names.homeland, "gold": p.gold, "tax": p.tax, "market": market,
        "recruits": recruits, "recruit_price": g.recruit_price(h), "crosses": p.crosses, "crosses_needed": p.crosses_needed,
        "train": train, "purchase": purchase, "equip": equip,
        "closed": g.rebellion.is_some(),
    })
}

fn reports(g: &Game) -> Value {
    let h = g.human;
    let colonies: Vec<Value> = g
        .colonies
        .iter()
        .filter(|c| c.nation == h)
        .map(|c| {
            let p = g.production(c.id);
            let net: Vec<i32> = (0..16).map(|gd| p.net(gd)).collect();
            json!({"id": c.id, "name": c.name, "pop": g.population(c.id), "liberty": g.liberty_percent(c), "net": net, "stock": c.stock.to_vec(),
                   "hammers": p.made[HAMMERS], "bells": p.made[BELLS], "crosses": p.made[CROSSES],
                   "building": c.building.map(|b| g.build_name(b, c)), "stockade": c.level(BKind::Fortification)})
        })
        .collect();
    let tribes: Vec<Value> = g
        .tribes
        .iter()
        .enumerate()
        .filter(|(i, _)| g.met_tribes & (1 << i) != 0)
        .map(|(i, t)| {
            json!({"name": g.nation_name(4 + i as u8), "alarm": t.alarm[h as usize], "extinct": t.extinct,
                   "villages": g.villages.iter().filter(|v| v.tribe as usize == 4 + i).count(), "tech": t.tech as u8})
        })
        .collect();
    let rivals: Vec<Value> = (0..4u8)
        .filter(|n| *n != h)
        .map(|n| {
            let known = g.knows(h, n);
            json!({"nation": n, "name": g.nation_name(n), "known": known, "at_war": g.at_war(h, n),
                   "colonies": if known { Some(g.colonies.iter().filter(|c| c.nation == n).count()) } else { None },
                   "strength": if known { Some(g.military_strength(n)) } else { None },
                   "tension": g.players[n as usize].tension[h as usize]})
        })
        .collect();
    let military: Vec<Value> = g
        .units
        .iter()
        .filter(|u| u.nation == h && g.can_attack(u.kind) && !u.kind.is_ship())
        .map(|u| json!({"name": g.unit_name(u), "x": g.unit_pos(u.id).map(|p| p.0), "y": g.unit_pos(u.id).map(|p| p.1)}))
        .collect();
    json!({
        "colonies": colonies, "tribes": tribes, "rivals": rivals, "military": military,
        "crown": g.crown_force, "score": g.score(h), "rebels": g.rebel_percent(h),
        "my_strength": g.military_strength(h),
    })
}

fn prompt_json(g: &Game, p: Prompt) -> Value {
    match p {
        Prompt::Attack { unit, x, y, target } => json!({"type": "attack", "unit": unit, "x": x, "y": y, "target": target, "odds": g.attack_odds(unit, x, y)}),
        Prompt::SailToHomeland { ship } => json!({"type": "sail_home", "unit": ship, "homeland": g.rules.names.homeland}),
        Prompt::Landfall { ship, x, y } => json!({"type": "landfall", "unit": ship, "x": x, "y": y}),
        Prompt::Village { unit, village } => {
            let v = &g.villages[village];
            let actions: Vec<Value> = g
                .village_actions(unit)
                .into_iter()
                .map(|a| {
                    let (k, label) = match a {
                        VillageAction::SpeakWithChief => ("speak", "Speak with the chief"),
                        VillageAction::LearnSkill => ("learn", "Live among the people and learn their skill"),
                        VillageAction::EstablishMission => ("mission", "Establish a mission"),
                        VillageAction::Trade => ("trade", "Trade"),
                        VillageAction::Attack => ("attack", "Attack the village"),
                        VillageAction::DemandTribute => ("tribute", "Demand tribute"),
                    };
                    json!({"action": k, "label": label})
                })
                .collect();
            let h = g.human;
            let alarm = g.tribes.get(v.tribe as usize - 4).map(|t| t.alarm[h as usize]).unwrap_or(0);
            json!({
                "type": "village", "unit": unit, "village": village, "tribe": g.nation_name(v.tribe),
                "capital": v.capital, "pop": v.pop, "teaches": v.teaches.map(|t| g.prof_name(t).to_string()),
                "mood": match alarm { 0..=24 => "content", 25..=49 => "uneasy", 50..=74 => "angry", _ => "hostile" },
                "actions": actions, "odds": g.attack_odds(unit, v.x as i32, v.y as i32),
            })
        }
    }
}

/// Milestone titles and lines written for this edition.
fn milestone(n: usize) -> (&'static str, &'static str) {
    use landfall::events::*;
    match n {
        MS_NEW_WORLD => ("A New World", "After weeks at sea the lookout cries land. A coast no map has drawn rises from the haze."),
        MS_DISCOVERY => ("First Footsteps", "Your people wade ashore and plant a banner on unknown ground."),
        MS_COLONY => ("A Colony Is Founded", "Axes ring and the first roofs go up. The settlement has a name, and a future."),
        MS_NATIVES => ("Those Who Were Here First", "Your explorers meet the people of this land. What happens next is up to you."),
        MS_LEAGUE => ("The Council Cities", "Stone towers and crowded markets: a great league of cities rules the interior."),
        MS_SUN_ROAD => ("The Sun Road", "A road of fitted stone runs over the mountains to the seat of an ancient sovereign."),
        MS_FAR_OCEAN => ("The Far Ocean", "From the last ridge, a second ocean stretches to the horizon. The continent has been crossed."),
        MS_VILLAGE => ("Lessons of the Land", "A colonist returns from a village, changed, with skills no school at home could teach."),
        MS_SETTLERS => ("A Land of Plenty", "Tales of the New World's riches sweep the homeland. Ships fill with eager settlers."),
        MS_CARGO => ("The First Cargo", "Your goods are sold on the homeland docks. The colony begins to pay its way."),
        MS_RIVALS => ("Rivals", "Another flag flies on this coast. The New World will not be yours alone."),
        MS_BURNING => ("A Village Burns", "Smoke rises over a native settlement. It will not soon be forgotten."),
        MS_DESTROYED => ("A People Destroyed", "The last village of a nation has fallen. Their name passes into memory."),
        MS_RAID => ("Raiders", "Warriors strike your colony and vanish into the forest."),
        _ => ("", ""),
    }
}

fn event_json(g: &Game, e: Event) -> Value {
    match e {
        Event::Milestone(n) => {
            let (title, text) = milestone(n);
            json!({"type": "milestone", "n": n, "title": title, "text": text})
        }
        Event::Chief { tribe, pose, text } => json!({"type": "chief", "tribe": g.nation_name(4 + tribe as u8), "tribe_index": tribe, "pose": pose, "text": text}),
        Event::Crown { text } => json!({"type": "crown", "text": text}),
        Event::TaxDemand => json!({"type": "tax"}),
        Event::NativeTrade { tribe, colony, good, amount, price } => json!({
            "type": "native_trade", "tribe": tribe, "tribe_name": g.nation_name(4 + tribe as u8), "colony": colony,
            "colony_name": g.colony(colony).map(|c| c.name.clone()), "good": good, "good_name": GOOD_NAMES[good as usize], "amount": amount, "price": price,
        }),
        Event::NativeDemand { tribe, colony, good, amount } => json!({
            "type": "native_demand", "tribe": tribe, "tribe_name": g.nation_name(4 + tribe as u8), "colony": colony,
            "colony_name": g.colony(colony).map(|c| c.name.clone()), "good": good, "good_name": good.map(|x| GOOD_NAMES[x as usize]), "amount": amount,
        }),
        Event::Mercenaries { price } => json!({"type": "mercenaries", "price": price, "gold": g.human_player().gold}),
        Event::Battle(b) => json!({
            "type": "battle", "attacker": g.kind_name(b.attacker.0), "attacker_nation": b.attacker.1,
            "defender": b.defender.map(|d| g.kind_name(d.0).to_string()), "defender_nation": b.defender.map(|d| d.1),
            "defender_name": b.defender_name, "attack": b.attack, "defence": b.defence, "won": b.won,
        }),
        Event::PeaceOffer { nation } => json!({"type": "peace_offer", "nation": nation, "name": g.adjective(nation)}),
        Event::Declared => json!({"type": "declared"}),
        Event::MemberJoined(f) => json!({"type": "member", "member": f, "name": g.member_name(f as u8), "effect": MEMBER_EFFECTS.get(f).copied().unwrap_or("")}),
        Event::WarOver { won } => json!({"type": "war_over", "won": won}),
    }
}
