//! Movement, orders, end of turn, the homeland, natives, combat, rumours and the
//! simple computer players.

use crate::colony::GOOD_NAMES;
use crate::assembly::*;
use crate::game::*;
use crate::types::{Control, Tech, UnitType};

/// What happened when the player tried to move a unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveResult {
    Moved,
    Blocked(String),
    /// The UI must ask the player what to do.
    Ask(Prompt),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prompt {
    Attack { unit: Id, x: i32, y: i32, target: String },
    Village { unit: Id, village: usize },
    SailToHomeland { ship: Id },
    Landfall { ship: Id, x: i32, y: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VillageAction {
    SpeakWithChief,
    LearnSkill,
    EstablishMission,
    Trade,
    Attack,
    DemandTribute,
}

impl Game {
    // ----- movement -----------------------------------------------------------

    fn road_like(&self, x: i32, y: i32) -> bool {
        self.tile(x, y).is_some_and(|t| t.road()) || self.colony_at(x, y).is_some()
    }

    pub fn move_cost(&self, fx: i32, fy: i32, tx: i32, ty: i32) -> u8 {
        if self.road_like(fx, fy) && self.road_like(tx, ty) {
            return 1;
        }
        let (Some(a), Some(b)) = (self.tile(fx, fy), self.tile(tx, ty)) else { return 3 };
        if a.river() && b.river() {
            return 1;
        }
        let row = self.terrain_row(b);
        self.rules.terrain.get(row).map(|r| r.movement * 3).unwrap_or(3)
    }

    fn foreign_units_at(&self, nation: u8, x: i32, y: i32) -> bool {
        self.units_at(x, y).any(|u| u.nation != nation)
    }

    pub fn can_attack(&self, kind: UnitType) -> bool {
        self.rules.units.get(kind as usize).is_some_and(|r| r.attack > 0)
    }

    /// Spend movement for a step; false if the unit runs out before getting there.
    fn pay_move(&mut self, id: Id, cost: u8) -> bool {
        let full = self.unit(id).map(|u| self.max_moves(u.kind)).unwrap_or(3);
        let moves = self.unit(id).map(|u| u.moves).unwrap_or(0);
        if moves >= cost || moves >= full {
            if let Some(u) = self.unit_mut(id) {
                u.moves = moves.saturating_sub(cost);
            }
            return true;
        }
        // not enough movement left: a chance to make it anyway
        let ok = self.rand(cost as u32) < moves as u32;
        if let Some(u) = self.unit_mut(id) {
            u.moves = 0;
        }
        ok
    }

    pub fn try_move(&mut self, id: Id, dx: i32, dy: i32) -> MoveResult {
        let Some(u) = self.unit(id).cloned() else { return MoveResult::Blocked("No unit.".into()) };
        let Loc::Map { x, y } = u.loc else { return MoveResult::Blocked("Unit is not on the map.".into()) };
        let (x, y) = (x as i32, y as i32);
        let (nx, ny) = (x + dx, y + dy);
        if self.tile(nx, ny).is_none() {
            return MoveResult::Blocked("The edge of the world.".into());
        }
        if u.repair > 0 {
            return MoveResult::Blocked(format!("The ship is under repair for {} more turns.", u.repair));
        }
        if u.moves == 0 {
            return MoveResult::Blocked("No movement points left.".into());
        }
        let target_water = self.is_water(nx, ny);
        let colony = self.colony_at(nx, ny).map(|c| (c.id, c.nation, c.name.clone()));

        if u.kind.is_ship() {
            if target_water {
                if self.foreign_units_at(u.nation, nx, ny) {
                    if self.can_attack(u.kind) {
                        return MoveResult::Ask(Prompt::Attack { unit: id, x: nx, y: ny, target: self.describe_target(nx, ny) });
                    }
                    return MoveResult::Blocked("Another ship is in the way.".into());
                }
                if !self.pay_move(id, 3) {
                    return MoveResult::Moved;
                }
                self.place(id, nx, ny);
                let sea_lane = self.tile(nx, ny).is_some_and(|t| t.raw & 0x1F == 26);
                let from_lane = self.tile(x, y).is_some_and(|t| t.raw & 0x1F == 26);
                // ask when a ship first reaches the high seas, not at every step across them
                if sea_lane && !from_lane && u.nation == self.human {
                    return MoveResult::Ask(Prompt::SailToHomeland { ship: id });
                }
                return MoveResult::Moved;
            }
            if let Some((cid, cn, _)) = colony {
                if cn == u.nation {
                    self.pay_move(id, 3);
                    self.place(id, nx, ny);
                    let _ = cid;
                    return MoveResult::Moved;
                }
                return MoveResult::Blocked("That colony belongs to another nation.".into());
            }
            if self.aboard(id).count() > 0 {
                if self.village_at(nx, ny).is_some() || self.foreign_units_at(u.nation, nx, ny) {
                    return MoveResult::Blocked("Can't land there.".into());
                }
                return MoveResult::Ask(Prompt::Landfall { ship: id, x: nx, y: ny });
            }
            return MoveResult::Blocked("Ships can't sail on land.".into());
        }

        // land units
        if target_water {
            let ship = self
                .units_at(nx, ny)
                .find(|s| s.kind.is_ship() && s.nation == u.nation)
                .map(|s| s.id);
            if let Some(sid) = ship {
                if u.kind == UnitType::WagonTrain || u.kind == UnitType::Treasure && self.unit(sid).map(|s| s.kind) != Some(UnitType::Galleon) {
                    return MoveResult::Blocked("That ship can't carry this unit.".into());
                }
                if self.holds_used(sid) >= self.holds(sid) {
                    return MoveResult::Blocked("The ship is full.".into());
                }
                if let Some(um) = self.unit_mut(id) {
                    um.loc = Loc::Aboard(sid);
                    um.moves = 0;
                    um.orders = Orders::Sentry;
                }
                return MoveResult::Moved;
            }
            return MoveResult::Blocked("Land units can't enter the water.".into());
        }
        if let Some(vi) = self.village_at(nx, ny) {
            if is_native(u.kind) {
                return MoveResult::Blocked("".into());
            }
            return MoveResult::Ask(Prompt::Village { unit: id, village: vi });
        }
        if let Some((_, cn, cname)) = &colony
            && *cn != u.nation {
                if self.can_attack(u.kind) {
                    return MoveResult::Ask(Prompt::Attack { unit: id, x: nx, y: ny, target: cname.clone() });
                }
                return MoveResult::Blocked(format!("{cname} belongs to the {}.", self.nation_name(*cn)));
            }
        if colony.is_none() && self.foreign_units_at(u.nation, nx, ny) {
            if self.can_attack(u.kind) {
                return MoveResult::Ask(Prompt::Attack { unit: id, x: nx, y: ny, target: self.describe_target(nx, ny) });
            }
            return MoveResult::Blocked("Someone else is standing there.".into());
        }
        let cost = self.move_cost(x, y, nx, ny);
        if !self.pay_move(id, cost) {
            return MoveResult::Moved;
        }
        self.place(id, nx, ny);

        // treasure reaching a coastal colony gets shipped home by the Crown
        if u.kind == UnitType::Treasure
            && let Some((_, cn, cname)) = colony
                && cn == u.nation && self.is_coastal(nx, ny) {
                    let gold = if self.has_member(u.nation, M_PLUNDER) { u.treasure } else { u.treasure / 2 };
                    self.players[u.nation as usize].gold += gold as i32;
                    self.kill(id);
                    self.purge_dead();
                    if u.nation == self.human {
                        { let home = self.rules.names.homeland.clone(); self.msg(format!("The Crown ships the treasure from {cname} to {home} for a 50% fee. You receive {gold} gold.")); }
                    }
                }

        let i = self.index(nx, ny).unwrap();
        if self.rumour[i] && u.nation == self.human {
            self.rumour[i] = false;
            self.explore_rumour(id, nx, ny);
        }
        MoveResult::Moved
    }

    fn describe_target(&self, x: i32, y: i32) -> String {
        match self.units_at(x, y).next() {
            Some(t) => format!("{} {}", self.nation_name(t.nation), self.unit_name(t)),
            None => "the enemy".into(),
        }
    }

    /// Put a unit on a square and look around.
    pub fn place(&mut self, id: Id, x: i32, y: i32) {
        let Some(u) = self.unit_mut(id) else { return };
        u.loc = Loc::Map { x: x as i16, y: y as i16 };
        let (nation, kind) = (u.nation, u.kind);
        let mut radius = match kind {
            UnitType::Scout | UnitType::Dragoon => 2,
            k if k.is_ship() => 1,
            _ => 1,
        };
        if !kind.is_ship() && self.has_member(nation, M_GOOD_RUMOURS) {
            radius += 1;
        }
        self.reveal(nation, x, y, radius);
        if let Some(i) = self.index(x, y)
            && nation < 4 {
                self.tiles[i].path = (self.tiles[i].path & 0x0F) | (nation << 4);
            }
    }

    pub fn landfall(&mut self, ship: Id, x: i32, y: i32) {
        let ids: Vec<Id> = self.aboard(ship).map(|u| u.id).collect();
        for id in ids {
            self.place(id, x, y);
            if let Some(u) = self.unit_mut(id) {
                u.moves = 0;
                u.orders = Orders::None;
            }
            let i = self.index(x, y).unwrap();
            if self.rumour[i] && self.unit(id).is_some_and(|u| u.nation == self.human) {
                self.rumour[i] = false;
                self.explore_rumour(id, x, y);
            }
        }
        if self.unit(ship).is_some_and(|s| s.nation == self.human) {
            self.milestone(crate::events::MS_DISCOVERY);
        }
        if let Some(s) = self.unit_mut(ship) {
            s.moves = 0;
        }
    }

    pub fn sail_to_homeland(&mut self, ship: Id) {
        let pos = self.unit_pos(ship);
        if let Some(s) = self.unit_mut(ship) {
            s.sea_exit = pos.map(|(x, y)| (x as i16, y as i16));
            s.loc = Loc::Sailing { to_homeland: true, turns: 3 };
            s.moves = 0;
        }
        if self.unit(ship).is_some_and(|s| self.has_member(s.nation, M_FAST_SHIPS))
            && let Some(s) = self.unit_mut(ship) {
                s.loc = Loc::Sailing { to_homeland: true, turns: 2 };
            }
    }

    // ----- go to ------------------------------------------------------------------

    /// Can this unit pass through (x, y) on the way to (tx, ty)?
    /// Has the human never seen this square? Routes for the human's units
    /// treat such squares as open, since nothing is known about them.
    fn unknown_to(&self, u: &Unit, x: i32, y: i32) -> bool {
        u.nation == self.human && self.tile(x, y).is_some_and(|t| !t.visible_to(u.nation))
    }

    fn passable(&self, u: &Unit, x: i32, y: i32, tx: i32, ty: i32) -> bool {
        let Some(t) = self.tile(x, y) else { return false };
        if self.unknown_to(u, x, y) {
            return true;
        }
        let dest = (x, y) == (tx, ty);
        let water = t.terrain().is_water();
        let own_colony = self.colony_at(x, y).is_some_and(|c| c.nation == u.nation);
        if u.kind.is_ship() {
            if !(water || own_colony) {
                return false;
            }
        } else if water {
            return false;
        }
        if dest {
            return true;
        }
        if self.village_at(x, y).is_some() || self.colony_at(x, y).is_some_and(|c| c.nation != u.nation) {
            return false;
        }
        !self.units_at(x, y).any(|o| o.nation != u.nation)
    }

    /// Cheapest route (in movement thirds) from the unit to (tx, ty), excluding the start.
    pub fn find_path(&self, id: Id, tx: i32, ty: i32) -> Option<Vec<(i32, i32)>> {
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;
        let u = self.unit(id)?;
        let (sx, sy) = self.unit_pos(id)?;
        if (sx, sy) == (tx, ty) || !self.passable(u, tx, ty, tx, ty) {
            return None;
        }
        let w = self.width as i32;
        let n = self.width * self.height;
        let mut dist = vec![u32::MAX; n];
        let mut prev = vec![usize::MAX; n];
        let start = (sy * w + sx) as usize;
        let goal = (ty * w + tx) as usize;
        dist[start] = 0;
        let mut heap = BinaryHeap::new();
        heap.push(Reverse((0u32, start)));
        while let Some(Reverse((d, i))) = heap.pop() {
            if i == goal {
                break;
            }
            if d > dist[i] {
                continue;
            }
            let (x, y) = (i as i32 % w, i as i32 / w);
            for (dx, dy) in DIRS8 {
                let (nx, ny) = (x + dx, y + dy);
                if !self.passable(u, nx, ny, tx, ty) {
                    continue;
                }
                let cost = if u.kind.is_ship() || self.unknown_to(u, nx, ny) { 3 } else { self.move_cost(x, y, nx, ny) as u32 };
                let ni = (ny * w + nx) as usize;
                // among equally short routes, prefer the one closest to the straight line
                let off = {
                    let (lx, ly) = ((tx - sx) as f32, (ty - sy) as f32);
                    let len = (lx * lx + ly * ly).sqrt().max(1.0);
                    (((nx - sx) as f32 * ly - (ny - sy) as f32 * lx).abs() / len * 4.0).min(30.0) as u32
                };
                let nd = d + cost.max(1) * 100 + off;
                if nd < dist[ni] {
                    dist[ni] = nd;
                    prev[ni] = i;
                    heap.push(Reverse((nd, ni)));
                }
            }
        }
        if dist[goal] == u32::MAX {
            return None;
        }
        let mut path = Vec::new();
        let mut cur = goal;
        while cur != start {
            path.push((cur as i32 % w, cur as i32 / w));
            cur = prev[cur];
        }
        path.reverse();
        Some(path)
    }

    /// Give a go-to order and start walking. Returns a prompt/blocker if the
    /// journey is interrupted this turn. Squares nobody has explored count as
    /// open; if the way turns out to be blocked the unit stops and waits.
    pub fn goto(&mut self, id: Id, tx: i32, ty: i32) -> Option<MoveResult> {
        let Some(path) = self.find_path(id, tx, ty) else {
            return Some(MoveResult::Blocked("There is no way to get there.".into()));
        };
        self.set_orders(id, Orders::Goto { x: tx as i16, y: ty as i16 });
        if let Some(u) = self.unit_mut(id) {
            u.plan = path.iter().map(|&(x, y)| (x as i16, y as i16)).collect();
        }
        self.continue_goto(id)
    }

    fn stop_goto(&mut self, id: Id) {
        self.set_orders(id, Orders::None);
        if let Some(u) = self.unit_mut(id) {
            u.plan.clear();
        }
    }

    /// Advance a unit with a go-to order as far as its movement allows.
    pub fn continue_goto(&mut self, id: Id) -> Option<MoveResult> {
        for _ in 0..64 {
            let u = self.unit(id)?.clone();
            let Orders::Goto { x: tx, y: ty } = u.orders else { return None };
            let (tx, ty) = (tx as i32, ty as i32);
            if u.moves == 0 {
                return None;
            }
            let pos = self.unit_pos(id)?;
            if !matches!(u.loc, Loc::Map { .. }) || pos == (tx, ty) {
                self.stop_goto(id);
                return None;
            }
            let human = u.nation == self.human;
            // the human's units keep to the route they were given; others plan afresh each step
            let next = match u.plan.first() {
                Some(&(x, y)) if human && (x as i32 - pos.0).abs() <= 1 && (y as i32 - pos.1).abs() <= 1 => (x as i32, y as i32),
                _ => {
                    let Some(path) = self.find_path(id, tx, ty) else {
                        self.stop_goto(id);
                        return Some(MoveResult::Blocked("The way is blocked.".into()));
                    };
                    if human {
                        self.unit_mut(id).unwrap().plan = path.iter().map(|&(x, y)| (x as i16, y as i16)).collect();
                    }
                    path[0]
                }
            };
            // a square on the route has turned out to be impassable: stop and wait for orders
            let dest = (tx, ty);
            if human && !self.passable(&u, next.0, next.1, dest.0, dest.1) {
                let what = if u.kind.is_ship() { "Land ahead" } else { "The way ahead is blocked" };
                let name = self.unit_name(&u);
                self.stop_goto(id);
                return Some(MoveResult::Blocked(format!("{what}. The {name} stops and waits for orders.")));
            }
            match self.try_move(id, next.0 - pos.0, next.1 - pos.1) {
                MoveResult::Moved => {
                    let now = self.unit_pos(id);
                    if now == Some(pos) {
                        return None; // ran out of movement trying
                    }
                    if let Some(u) = self.unit_mut(id)
                        && u.plan.first().is_some_and(|&(x, y)| Some((x as i32, y as i32)) == now)
                    {
                        u.plan.remove(0);
                    }
                    if now == Some(dest) {
                        self.stop_goto(id);
                        return None;
                    }
                }
                MoveResult::Blocked(why) if human => {
                    self.stop_goto(id);
                    let name = self.unit(id).map(|u| self.unit_name(u)).unwrap_or_default();
                    return Some(MoveResult::Blocked(format!("{why} The {name} stops and waits for orders.")));
                }
                other => {
                    self.stop_goto(id);
                    return Some(other);
                }
            }
        }
        None
    }

    // ----- orders ---------------------------------------------------------------

    pub fn set_orders(&mut self, id: Id, o: Orders) {
        if let Some(u) = self.unit_mut(id) {
            u.orders = o;
            u.plan.clear();
        }
    }

    pub fn skip(&mut self, id: Id) {
        if let Some(u) = self.unit_mut(id) {
            u.done = true;
        }
    }

    pub fn start_improvement(&mut self, id: Id, road: bool) -> Result<(), String> {
        let u = self.unit(id).ok_or("no unit")?.clone();
        if u.kind != UnitType::Pioneer {
            return Err("Only pioneers can improve land.".into());
        }
        if u.tools < 20 {
            return Err("The pioneer needs at least 20 tools.".into());
        }
        let Loc::Map { x, y } = u.loc else { return Err("Not on the map.".into()) };
        let t = self.tile(x as i32, y as i32).ok_or("off map")?;
        if t.terrain().is_water() {
            return Err("Can't improve water.".into());
        }
        let row = self.terrain_row(t);
        let mut turns = self.rules.terrain.get(row).map(|r| r.improve_turns).unwrap_or(4).max(1);
        if road {
            if t.road() || self.colony_at(x as i32, y as i32).is_some() {
                return Err("There is already a road here.".into());
            }
            turns = turns.div_ceil(2);
        } else if !t.forested() && (t.plowed() || t.hills() || t.mountains()) {
            return Err("This land can't be plowed further.".into());
        }
        if u.prof == P_HARDY_PIONEER {
            turns = turns.div_ceil(2);
        }
        self.set_orders(id, if road { Orders::Road { turns } } else { Orders::Plow { turns } });
        Ok(())
    }

    fn finish_improvement(&mut self, id: Id, road: bool) {
        let Some((x, y)) = self.unit_pos(id) else { return };
        let Some(i) = self.index(x, y) else { return };
        let nation = self.unit(id).map(|u| u.nation).unwrap_or(0);
        let what;
        if road {
            self.tiles[i].mask |= 0x08;
            what = "built a road";
        } else if self.tiles[i].forested() {
            let raw = self.tiles[i].raw;
            let base = raw & 0x1F;
            let cleared = if (8..16).contains(&base) { base - 8 } else { base - 16 };
            self.tiles[i].raw = (raw & 0xE0) | cleared;
            if self.resource[i] != 255 && matches!(self.resource[i], 8..=11) {
                self.resource[i] = 255;
            }
            // lumber goes to a nearby colony
            let near = self
                .colonies
                .iter()
                .position(|c| c.nation == nation && (c.x as i32 - x).abs() <= 1 && (c.y as i32 - y).abs() <= 1);
            if let Some(ci) = near {
                self.colonies[ci].stock[LUMBER] += 20;
            }
            what = "cleared the forest";
        } else {
            self.tiles[i].mask |= 0x40;
            what = "plowed the land";
        }
        let mut became_colonist = false;
        if let Some(u) = self.unit_mut(id) {
            u.tools = u.tools.saturating_sub(20);
            u.orders = Orders::None;
            if u.tools == 0 {
                u.kind = UnitType::Colonist;
                became_colonist = true;
            }
        }
        if nation == self.human {
            self.msg(format!("Pioneer has {what}.{}", if became_colonist { " Out of tools." } else { "" }));
        }
    }

    // ----- end of turn ------------------------------------------------------------

    pub fn end_turn(&mut self) {
        // pioneers and fortification
        let ids: Vec<Id> = self.units.iter().map(|u| u.id).collect();
        for id in &ids {
            let Some(u) = self.unit(*id) else { continue };
            match u.orders {
                Orders::Fortify => self.set_orders(*id, Orders::Fortified),
                Orders::Plow { turns } | Orders::Road { turns } => {
                    let road = matches!(u.orders, Orders::Road { .. });
                    if turns <= 1 {
                        self.finish_improvement(*id, road);
                    } else {
                        let o = if road { Orders::Road { turns: turns - 1 } } else { Orders::Plow { turns: turns - 1 } };
                        self.set_orders(*id, o);
                    }
                }
                _ => {}
            }
        }

        // colonies
        let cids: Vec<(Id, u8)> = self.colonies.iter().map(|c| (c.id, c.nation)).collect();
        for (cid, nation) in cids {
            let msgs = self.produce(cid);
            if nation == self.human {
                self.messages.extend(msgs);
            }
            self.reveal_colony(cid);
        }

        // ships at sea
        for id in &ids {
            let Some(u) = self.unit(*id) else { continue };
            if let Loc::Sailing { to_homeland, turns } = u.loc {
                let nation = u.nation;
                if turns > 1 {
                    self.unit_mut(*id).unwrap().loc = Loc::Sailing { to_homeland, turns: turns - 1 };
                } else if to_homeland {
                    self.unit_mut(*id).unwrap().loc = Loc::Homeland;
                    if nation == self.human {
                        let n = self.unit_name(self.unit(*id).unwrap());
                        self.msg(format!("Your {n} has arrived in {}.", self.rules.names.homeland));
                    }
                } else {
                    let (x, y) = self.arrival_point(*id);
                    self.place(*id, x, y);
                    if nation == self.human {
                        let n = self.unit_name(self.unit(*id).unwrap());
                        self.msg(format!("Your {n} has arrived in the New World."));
                    }
                }
            }
        }

        self.repair_ships();
        self.immigration();
        self.natives_turn();
        self.native_relations();
        self.ai_turn();
        self.end_of_turn_politics();

        // markets recover slowly
        for p in &mut self.players {
            for v in &mut p.volume {
                *v -= v.signum() * 10.min(v.abs());
            }
        }
        for t in &mut self.tribes {
            for a in &mut t.alarm {
                if *a > 0 {
                    *a -= 1;
                }
            }
        }

        // calendar
        if self.year < 1600 {
            self.year += 1;
        } else if self.autumn {
            self.year += 1;
            self.autumn = false;
        } else {
            self.autumn = true;
        }
        self.turn += 1;


        // fresh movement
        let fast_ships: Vec<bool> = (0..4u8).map(|n| self.has_member(n, M_FAST_SHIPS)).collect();
        for u in &mut self.units {
            u.moves = self.rules.units.get(u.kind as usize).map(|r| r.movement * 3).unwrap_or(3);
            if u.kind.is_ship() && u.nation < 4 && fast_ships[u.nation as usize] {
                u.moves += 3;
            }
            u.done = false;
        }
        self.purge_dead();
        // trade routes move at the start of the new turn
        self.run_routes();
        self.purge_dead();
    }

    fn reveal_colony(&mut self, cid: Id) {
        if let Some(c) = self.colony(cid) {
            let (n, x, y) = (c.nation, c.x as i32, c.y as i32);
            self.reveal(n, x, y, 2);
        }
    }

    fn arrival_point(&self, ship: Id) -> (i32, i32) {
        let u = self.unit(ship);
        let nation = u.map(|u| u.nation).unwrap_or(0);
        let want = u
            .and_then(|u| u.sea_exit)
            .unwrap_or(self.players[nation as usize].entry);
        let (wx, wy) = (want.0 as i32, want.1 as i32);
        if self.is_water(wx, wy) && self.tile(wx, wy).is_some() {
            return (wx, wy);
        }
        // nearest sea lane on the same row, searching from the east edge
        for x in (0..self.width as i32).rev() {
            if self.tile(x, wy).is_some_and(|t| t.raw & 0x1F == 26) {
                return (x, wy);
            }
        }
        (self.width as i32 - 2, wy.clamp(1, self.height as i32 - 2))
    }

    // ----- the homeland ---------------------------------------------------------

    pub fn ask_price(&self, nation: u8, good: usize) -> i32 {
        let bid = self.players[nation as usize].prices[good] as i32;
        let burden = self.rules.cargo.get(good).map(|c| c.burden as i32).unwrap_or(1);
        bid + burden.max(1)
    }

    pub fn bid_price(&self, nation: u8, good: usize) -> i32 {
        self.players[nation as usize].prices[good] as i32
    }

    fn market_traffic(&mut self, nation: u8, good: usize, amount: i32) {
        let (rise, fall, low, high) = self
            .rules
            .cargo
            .get(good)
            .map(|c| (c.rise.max(1), c.fall.max(1), c.low as i32, c.high as i32))
            .unwrap_or((4, 4, 1, 20));
        let fall = if self.has_member(nation, M_STEADY_PRICES) || nation == 3 { fall * 2 } else { fall };
        let rise = if nation == 3 { rise * 2 } else { rise };
        let p = &mut self.players[nation as usize];
        p.volume[good] += amount;
        if p.volume[good] >= fall * 100 {
            p.volume[good] = 0;
            let bid = p.prices[good] as i32;
            if bid > low.max(1) {
                p.prices[good] = (bid - 1) as u8;
            }
        } else if p.volume[good] <= -rise * 100 {
            p.volume[good] = 0;
            let bid = p.prices[good] as i32;
            if bid < high.max(2) {
                p.prices[good] = (bid + 1) as u8;
            }
        }
    }

    /// Sell one cargo slot of a ship docked in the homeland.
    pub fn sell_cargo(&mut self, ship: Id, slot: usize) -> Result<i32, String> {
        let u = self.unit(ship).ok_or("no ship")?;
        if u.loc != Loc::Homeland {
            return Err("The ship is not in port in the homeland.".into());
        }
        let (good, amount) = *u.cargo.get(slot).ok_or("empty hold")?;
        let nation = u.nation;
        if self.boycotted(nation, good as usize) {
            return Err(format!("{} are boycotted. Pay the arrears first.", GOOD_NAMES[good as usize]));
        }
        let gross = self.bid_price(nation, good as usize) * amount as i32;
        let tax = self.players[nation as usize].tax as i32;
        let net = gross * (100 - tax) / 100;
        if nation == self.human {
            self.milestone(crate::events::MS_CARGO);
        }
        self.players[nation as usize].gold += net;
        self.unit_mut(ship).unwrap().cargo.remove(slot);
        self.market_traffic(nation, good as usize, amount as i32);
        if self.chance(3) && tax < 70 {
            self.raise_tax(nation, good as usize);
        }
        Ok(net)
    }

    /// Buy up to 100 of a good into a ship in the homeland.
    pub fn buy_cargo(&mut self, ship: Id, good: usize) -> Result<i32, String> {
        let u = self.unit(ship).ok_or("no ship")?;
        if u.loc != Loc::Homeland {
            return Err("The ship is not in port in the homeland.".into());
        }
        let nation = u.nation;
        if self.boycotted(nation, good) {
            return Err(format!("{} are boycotted. Pay the arrears first.", GOOD_NAMES[good]));
        }
        // top up a partial hold of the same good, or use a free hold
        let slot = u.cargo.iter().position(|(g, n)| *g as usize == good && *n < 100);
        let room = match slot {
            Some(s) => 100 - u.cargo[s].1 as i32,
            None => {
                if self.holds_used(ship) >= self.holds(ship) {
                    return Err("The ship's holds are full.".into());
                }
                100
            }
        };
        let price = self.ask_price(nation, good);
        let gold = self.players[nation as usize].gold;
        let amount = room.min(gold / price.max(1));
        if amount <= 0 {
            return Err("Not enough gold.".into());
        }
        self.players[nation as usize].gold -= amount * price;
        let u = self.unit_mut(ship).unwrap();
        match slot {
            Some(s) => u.cargo[s].1 += amount as u8,
            None => u.cargo.push((good as u8, amount as u8)),
        }
        self.market_traffic(nation, good, -amount);
        Ok(amount)
    }

    pub fn recruit_price(&self, nation: u8) -> i32 {
        let p = &self.players[nation as usize];
        (200 + (p.crosses_needed - p.crosses).max(0) * 4).clamp(150, 2000)
    }

    pub fn random_immigrant(&mut self) -> u8 {
        let r = self.rand(100);
        match r {
            0..=39 => P_FREE,
            40..=54 => P_SERVANT,
            55..=64 => P_CRIMINAL,
            65..=68 => P_HARDY_PIONEER,
            69..=71 => P_VETERAN,
            72..=73 => P_SEASONED_SCOUT,
            74..=75 => P_MISSIONARY,
            _ => self.rand(18) as u8,
        }
    }

    pub fn recruit(&mut self, nation: u8, slot: usize) -> Result<Id, String> {
        let price = self.recruit_price(nation);
        if self.players[nation as usize].gold < price {
            return Err(format!("Recruiting costs {price} gold."));
        }
        self.players[nation as usize].gold -= price;
        let prof = self.players[nation as usize].recruits[slot.min(2)];
        let mut next = self.random_immigrant();
        if self.has_member(nation, M_CHOOSE_IMMIGRANTS) && matches!(next, P_CRIMINAL | P_SERVANT) {
            next = P_FREE;
        }
        let p = &mut self.players[nation as usize];
        p.recruits[slot.min(2)] = next;
        p.crosses = (p.crosses - p.crosses_needed / 3).max(0);
        Ok(self.spawn(UnitType::Colonist, nation, prof, Loc::Homeland))
    }

    pub fn train_price(&self, prof: u8) -> Option<i32> {
        match prof {
            P_HARDY_PIONEER => Some(1200),
            P_VETERAN => Some(2000),
            P_MISSIONARY => Some(1400),
            p => self.rules.jobs.get(p as usize).and_then(|j| j.homeland_cost).map(|c| c as i32),
        }
    }

    pub fn train(&mut self, nation: u8, prof: u8) -> Result<Id, String> {
        let price = self.train_price(prof).ok_or("That expert can't be trained in the homeland.")?;
        if self.players[nation as usize].gold < price {
            return Err(format!("Training costs {price} gold."));
        }
        self.players[nation as usize].gold -= price;
        Ok(self.spawn(UnitType::Colonist, nation, prof, Loc::Homeland))
    }

    pub fn unit_price(&self, nation: u8, kind: UnitType) -> Option<i32> {
        Some(match kind {
            UnitType::Artillery => 500 + 100 * self.players[nation as usize].artillery_bought as i32,
            UnitType::Caravel => 1000,
            UnitType::Merchantman => 2000,
            UnitType::Galleon => 3000,
            UnitType::Privateer => 2000,
            UnitType::Frigate => 5000,
            _ => return None,
        })
    }

    pub fn buy_unit(&mut self, nation: u8, kind: UnitType) -> Result<Id, String> {
        let price = self.unit_price(nation, kind).ok_or("Not for sale.")?;
        if self.players[nation as usize].gold < price {
            return Err(format!("{} costs {price} gold.", self.kind_name(kind)));
        }
        self.players[nation as usize].gold -= price;
        if kind == UnitType::Artillery {
            self.players[nation as usize].artillery_bought += 1;
        }
        Ok(self.spawn(kind, nation, P_FREE, Loc::Homeland))
    }

    /// Equip a colonist on the docks, buying the goods at market prices.
    pub fn equip_in_homeland(&mut self, id: Id, kind: UnitType) -> Result<(), String> {
        let u = self.unit(id).ok_or("no unit")?.clone();
        if u.loc != Loc::Homeland || !is_colonist(u.kind) {
            return Err("Only colonists on the docks can be equipped.".into());
        }
        let nation = u.nation;
        // sell back current equipment first
        let mut refund = 0;
        match u.kind {
            UnitType::Soldier => refund += 50 * self.bid_price(nation, MUSKETS),
            UnitType::Dragoon => refund += 50 * (self.bid_price(nation, MUSKETS) + self.bid_price(nation, HORSES)),
            UnitType::Scout => refund += 50 * self.bid_price(nation, HORSES),
            UnitType::Pioneer => refund += u.tools as i32 * self.bid_price(nation, TOOLS),
            _ => {}
        }
        let cost = match kind {
            UnitType::Soldier => 50 * self.ask_price(nation, MUSKETS),
            UnitType::Dragoon => 50 * (self.ask_price(nation, MUSKETS) + self.ask_price(nation, HORSES)),
            UnitType::Scout => 50 * self.ask_price(nation, HORSES),
            UnitType::Pioneer => 100 * self.ask_price(nation, TOOLS),
            _ => 0,
        };
        if self.players[nation as usize].gold + refund < cost {
            return Err(format!("That costs {cost} gold."));
        }
        self.players[nation as usize].gold += refund - cost;
        let u = self.unit_mut(id).unwrap();
        u.kind = kind;
        u.tools = if kind == UnitType::Pioneer { 100 } else { 0 };
        Ok(())
    }

    pub fn sail_to_new_world(&mut self, ship: Id) -> Result<(), String> {
        let fast = self.unit(ship).is_some_and(|s| self.has_member(s.nation, M_FAST_SHIPS));
        let u = self.unit_mut(ship).ok_or("no ship")?;
        if u.loc != Loc::Homeland || !u.kind.is_ship() {
            return Err("Only ships in port can sail.".into());
        }
        if u.repair > 0 {
            return Err(format!("The ship is being repaired ({} turns).", u.repair));
        }
        u.loc = Loc::Sailing { to_homeland: false, turns: if fast { 2 } else { 3 } };
        Ok(())
    }

    fn immigration(&mut self) {
        for n in 0..4u8 {
            let p = &self.players[n as usize];
            if p.control == Control::Withdrawn || p.crosses < p.crosses_needed {
                continue;
            }
            let slot = self.rand(3) as usize;
            let prof = self.players[n as usize].recruits[slot];
            let mut next = self.random_immigrant();
            if self.has_member(n, M_CHOOSE_IMMIGRANTS) && matches!(next, P_CRIMINAL | P_SERVANT) {
                next = P_FREE;
            }
            let p = &mut self.players[n as usize];
            p.recruits[slot] = next;
            p.crosses -= p.crosses_needed;
            p.crosses_needed += if n == 0 { 12 + p.crosses_needed / 15 } else { 20 + p.crosses_needed / 10 };
            if n == self.human {
                self.spawn(UnitType::Colonist, n, prof, Loc::Homeland);
                self.msg(format!(
                    "Religious unrest in {}! A {} has joined you on the docks.", self.rules.names.homeland,
                    self.prof_name(prof)
                ));
            } else {
                // computer players: the immigrant goes straight to a colony
                let target = self.colonies.iter().filter(|c| c.nation == n).map(|c| c.id).next();
                if let Some(cid) = target {
                    let id = self.spawn(UnitType::Colonist, n, prof, Loc::Homeland);
                    self.auto_join(id, cid);
                }
            }
        }
    }

    // ----- rumours -----------------------------------------------------------------

    fn explore_rumour(&mut self, id: Id, x: i32, y: i32) {
        let Some(u) = self.unit(id).cloned() else { return };
        let scout = u.kind == UnitType::Scout;
        let seasoned = scout && u.prof == P_SEASONED_SCOUT;
        let nation = u.nation;
        let roll = if self.has_member(nation, M_GOOD_RUMOURS) { 28 + self.rand(51) } else { self.rand(100) };
        match roll {
            0..=27 => self.msg("The rumours prove to be nothing but tall tales."),
            28..=52 => {
                let mut gold = 30 + self.rand(270) as i32;
                if seasoned {
                    gold *= 2;
                }
                self.players[nation as usize].gold += gold;
                self.msg(format!("You find a small native trinket cache worth {gold} gold."));
            }
            53..=62 => {
                self.reveal(nation, x, y, 6);
                self.msg("Natives share tales of the surrounding lands. Your maps have improved.");
            }
            63..=70 => {
                let c = self.spawn(UnitType::Colonist, nation, P_FREE, Loc::Map { x: x as i16, y: y as i16 });
                let _ = c;
                self.msg("You find the survivors of a lost colony. A free colonist joins you.");
            }
            71..=75 => {
                let amount = 300 + self.rand(1200);
                let t = self.spawn(UnitType::Treasure, nation, P_FREE, Loc::Map { x: x as i16, y: y as i16 });
                if let Some(tu) = self.unit_mut(t) {
                    tu.treasure = amount;
                }
                self.msg(format!(
                    "You discover the ruins of a lost civilization! Treasure worth {amount} gold. Bring it to a coastal colony."
                ));
            }
            76..=78 => {
                for _ in 0..8 {
                    let p = self.random_immigrant();
                    self.spawn(UnitType::Colonist, nation, p, Loc::Homeland);
                }
                self.msg(format!("Tales of a land of plenty sweep the docks! Eight new colonists await you in {}.", self.rules.names.homeland));
                if nation == self.human {
                    self.milestone(crate::events::MS_SETTLERS);
                }
            }
            79..=85 => {
                if let Some(t) = self.nearest_tribe(x, y) {
                    let tr = &mut self.tribes[t];
                    tr.alarm[nation.min(3) as usize] = (tr.alarm[nation.min(3) as usize] + 40).min(100);
                    self.msg(format!(
                        "Your colonists have disturbed a {} burial ground. The tribe is angered.",
                        self.nation_name(t as u8 + 4)
                    ));
                } else {
                    self.msg("The rumours prove to be nothing but tall tales.");
                }
            }
            86..=92 if !seasoned => {
                self.kill(id);
                self.purge_dead();
                self.msg("Your expedition has vanished without a trace.");
            }
            _ => {
                let gold = 10 + self.rand(60) as i32;
                self.players[nation as usize].gold += gold;
                self.msg(format!("You find {gold} gold in an abandoned camp."));
            }
        }
    }

    fn nearest_tribe(&self, x: i32, y: i32) -> Option<usize> {
        self.villages
            .iter()
            .min_by_key(|v| (v.x as i32 - x).abs() + (v.y as i32 - y).abs())
            .map(|v| v.tribe.saturating_sub(4) as usize)
    }

    // ----- natives -------------------------------------------------------------

    pub fn village_actions(&self, unit: Id) -> Vec<VillageAction> {
        let Some(u) = self.unit(unit) else { return Vec::new() };
        use VillageAction::*;
        match u.kind {
            UnitType::Scout => vec![SpeakWithChief, Attack],
            UnitType::Colonist => vec![LearnSkill],
            UnitType::Missionary => vec![EstablishMission],
            UnitType::WagonTrain => vec![Trade],
            UnitType::Soldier | UnitType::Dragoon | UnitType::Artillery | UnitType::PatriotArmy | UnitType::PatriotCavalry => {
                vec![Attack, DemandTribute]
            }
            _ => Vec::new(),
        }
    }

    pub fn village_action(&mut self, unit: Id, vi: usize, action: VillageAction) {
        let Some(u) = self.unit(unit).cloned() else { return };
        let Some(v) = self.villages.get(vi).cloned() else { return };
        let tribe_i = v.tribe.saturating_sub(4) as usize;
        let tribe = self.nation_name(v.tribe).to_string();
        let nation = u.nation.min(3);
        let alarm = self.tribes.get(tribe_i).map(|t| t.alarm[nation as usize]).unwrap_or(0);
        if let Some(um) = self.unit_mut(unit) {
            um.moves = 0;
        }
        if u.nation == self.human && action != VillageAction::Attack {
            self.milestone(crate::events::MS_VILLAGE);
        }
        match action {
            VillageAction::SpeakWithChief => {
                if v.scouted & (1 << nation) != 0 {
                    self.chief_says(nation, tribe_i, 0, "You have already heard our words, scout. Go now.".to_string());
                    return;
                }
                self.villages[vi].scouted |= 1 << nation;
                let r = self.rand(100);
                if alarm > 60 && r < 30 {
                    self.kill(unit);
                    self.purge_dead();
                    self.chief_says(nation, tribe_i, 3, format!("The {tribe} have no patience for spies. Your scout is put to death!"));
                } else if r < 45 {
                    let gold = (50 + self.rand(150) as i32) * (1 + self.tribe_tech(v.tribe) as i32);
                    self.players[nation as usize].gold += gold;
                    self.chief_says(nation, tribe_i, 1, format!("Welcome, friend. Accept this gift of {gold} gold as a sign of the friendship of the {tribe}."));
                } else if r < 80 {
                    self.reveal(nation, v.x as i32, v.y as i32, 6);
                    self.chief_says(nation, tribe_i, 2, "Sit by our fire, scout, and hear of the lands around us: the rivers, the hills and the villages of our neighbours. (Your maps have improved.)".to_string());
                } else {
                    if u.prof != P_SEASONED_SCOUT && self.chance(35) {
                        self.unit_mut(unit).unwrap().prof = P_SEASONED_SCOUT;
                        self.chief_says(nation, tribe_i, 2, "You walk lightly on the land. Our hunters will teach you their ways. (Your scout is now a Seasoned Scout.)".to_string());
                    } else {
                        self.chief_says(nation, tribe_i, 0, format!("The {tribe} chief listens politely and sends your scout on his way."));
                    }
                }
            }
            VillageAction::LearnSkill => {
                let Some(skill) = v.teaches else {
                    self.chief_says(nation, tribe_i, 0, "We have already taught your people what we know.".to_string());
                    return;
                };
                match u.prof {
                    P_FREE | P_FREE_OLD | P_SERVANT => {
                        if alarm > 60 && self.chance(25) {
                            self.kill(unit);
                            self.purge_dead();
                            self.chief_says(nation, tribe_i, 3, format!("You are not welcome among the {tribe}! Your colonist is killed."));
                            return;
                        }
                        self.unit_mut(unit).unwrap().prof = skill;
                        self.villages[vi].teaches = None;
                        self.chief_says(nation, tribe_i, 1, format!(
                            "Live among us and learn our ways. (Your colonist becomes an {}.)",
                            self.prof_name(skill)
                        ));
                    }
                    P_CRIMINAL => self.chief_says(nation, tribe_i, 0, "We do not teach thieves and outcasts. Send us someone worthy.".to_string()),
                    _ => self.msg("This colonist is already an expert."),
                }
            }
            VillageAction::EstablishMission => {
                if v.mission.is_some() {
                    self.msg(format!("There is already a mission among the {tribe}."));
                    return;
                }
                self.villages[vi].mission = Some(nation);
                self.kill(unit);
                self.purge_dead();
                self.chief_says(nation, tribe_i, 1, format!("Your holy man may build his mission among the {tribe}. Our people will listen to his words."));
            }
            VillageAction::Trade => {
                let Some(&(good, amount)) = u.cargo.first() else {
                    self.msg("The wagon train has nothing to sell.");
                    return;
                };
                let base = self.bid_price(nation, good as usize).max(1) * amount as i32;
                let pct = 80 + self.rand(120) as i32 + 15 * self.tribe_tech(v.tribe) as i32;
                let gold = base * pct / 100;
                self.players[nation as usize].gold += gold;
                self.unit_mut(unit).unwrap().cargo.remove(0);
                if let Some(t) = self.tribes.get_mut(tribe_i) {
                    t.alarm[nation as usize] = (t.alarm[nation as usize] - 5).max(0);
                }
                self.chief_says(nation, tribe_i, 1, format!("The {tribe} will buy your {} {} for {gold} gold.", amount, GOOD_NAMES[good as usize]));
            }
            VillageAction::DemandTribute => {
                let gold = (v.wealth as i32 / 3).max(10);
                if self.chance(50) {
                    self.players[nation as usize].gold += gold;
                    self.villages[vi].wealth = v.wealth.saturating_sub(gold as u32);
                    self.chief_says(nation, tribe_i, 0, format!("Take your {gold} gold and leave our village. The {tribe} will remember this."));
                } else {
                    self.chief_says(nation, tribe_i, 3, format!("The {tribe} pay no tribute to strangers! Leave our lands."));
                }
                if let Some(t) = self.tribes.get_mut(tribe_i) {
                    let anger = if nation == 1 { 7 } else { 15 };
                    t.alarm[nation as usize] = (t.alarm[nation as usize] + anger).min(100);
                }
            }
            VillageAction::Attack => {
                let (x, y) = (v.x as i32, v.y as i32);
                self.attack(unit, x, y);
            }
        }
    }

    fn natives_turn(&mut self) {
        // villages spawn braves, grow, and missions make converts
        for vi in 0..self.villages.len() {
            let v = self.villages[vi].clone();
            let ti = v.tribe.saturating_sub(4) as usize;
            if self.tribes.get(ti).is_none_or(|t| t.extinct) {
                continue;
            }
            let braves = self.units.iter().filter(|u| u.nation == v.tribe && matches!(u.loc, Loc::Map { .. })).count();
            let villages = self.villages.iter().filter(|w| w.tribe == v.tribe).count();
            if braves < villages && v.pop >= 2 && self.chance(8) {
                let kind = if self.tribe_tech(v.tribe) == Tech::SemiNomadic && self.chance(30) {
                    UnitType::MountedBrave
                } else {
                    UnitType::Brave
                };
                let id = self.spawn(kind, v.tribe, P_FREE, Loc::Map { x: v.x, y: v.y });
                let full = self.max_moves(kind);
                self.unit_mut(id).unwrap().moves = full;
            }
            if v.pop < 12 && self.chance(2) {
                self.villages[vi].pop += 1;
            }
            let odds = v.mission.map(|n| if self.has_member(n, M_MISSIONS) { 6 } else { 3 }).unwrap_or(0);
            if let Some(n) = v.mission
                && self.chance(odds)
                    && let Some((x, y)) = self.free_land_near(v.x as i32, v.y as i32) {
                        self.spawn(UnitType::Colonist, n, P_CONVERT, Loc::Map { x: x as i16, y: y as i16 });
                        if n == self.human {
                            self.msg(format!("A {} convert has come to your mission.", self.nation_name(v.tribe)));
                        }
                    }
        }

        // braves wander and, when angry, attack
        let braves: Vec<Id> = self.units.iter().filter(|u| is_native(u.kind) && matches!(u.loc, Loc::Map { .. })).map(|u| u.id).collect();
        for id in braves {
            let Some((x, y)) = self.unit_pos(id) else { continue };
            let tribe = self.unit(id).unwrap().nation;
            let ti = tribe.saturating_sub(4) as usize;
            // attack an angry neighbour?
            let mut attacked = false;
            for (dx, dy) in DIRS8 {
                let (nx, ny) = (x + dx, y + dy);
                let target_nation = self
                    .colony_at(nx, ny)
                    .map(|c| c.nation)
                    .or_else(|| self.units_at(nx, ny).find(|u| u.nation < 4 && !u.kind.is_ship()).map(|u| u.nation))
                    .filter(|n| *n < 4);
                if let Some(tn) = target_nation {
                    let alarm = self.tribes.get(ti).map(|t| t.alarm[tn as usize]).unwrap_or(0);
                    if alarm >= 70 && self.chance(25) {
                        self.attack(id, nx, ny);
                        attacked = true;
                        break;
                    }
                }
            }
            if attacked {
                continue;
            }
            // wander, staying near home
            let dir = DIRS8[self.rand(8) as usize];
            let (nx, ny) = (x + dir.0, y + dir.1);
            let home = self
                .villages
                .iter()
                .filter(|v| v.tribe == tribe)
                .map(|v| (v.x as i32 - nx).abs().max((v.y as i32 - ny).abs()))
                .min()
                .unwrap_or(99);
            let free = !self.is_water(nx, ny)
                && self.colony_at(nx, ny).is_none()
                && self.village_at(nx, ny).is_none()
                && !self.units_at(nx, ny).any(|u| u.nation != tribe);
            if free && home <= 4 {
                self.place(id, nx, ny);
            } else if home > 6 {
                self.kill(id);
            }
        }
        self.purge_dead();
    }

    fn free_land_near(&self, x: i32, y: i32) -> Option<(i32, i32)> {
        DIRS8
            .iter()
            .map(|(dx, dy)| (x + dx, y + dy))
            .find(|&(nx, ny)| !self.is_water(nx, ny) && self.colony_at(nx, ny).is_none() && self.village_at(nx, ny).is_none())
    }

    /// Dismiss a unit for good.
    pub fn disband(&mut self, id: Id) -> Result<(), String> {
        let u = self.unit(id).ok_or("No unit.")?;
        if matches!(u.loc, Loc::Colony { .. }) {
            return Err("Take the colonist out of the colony first.".into());
        }
        let passengers: Vec<Id> = self.aboard(id).map(|p| p.id).collect();
        for p in passengers {
            self.kill(p);
        }
        self.kill(id);
        self.purge_dead();
        Ok(())
    }

    /// Give up a colony: its people walk out, its goods are lost.
    pub fn abandon_colony(&mut self, cid: Id) -> Result<(), String> {
        let c = self.colony(cid).ok_or("No colony.")?;
        if c.level(BKind::Fortification) > 0 {
            return Err("The stockade's garrison will not allow the colony to be abandoned.".into());
        }
        let (x, y, name, nation) = (c.x, c.y, c.name.clone(), c.nation);
        for u in &mut self.units {
            if matches!(u.loc, Loc::Colony { colony, .. } if colony == cid) {
                u.loc = Loc::Map { x, y };
                u.orders = Orders::None;
            }
        }
        // routes that stopped there are cut
        for u in &mut self.units {
            if u.route.as_ref().is_some_and(|r| r.a == Place::Colony(cid) || r.b == Place::Colony(cid)) {
                u.route = None;
                if u.orders == Orders::Route {
                    u.orders = Orders::None;
                }
            }
        }
        self.colonies.retain(|c| c.id != cid);
        if nation == self.human {
            self.msg(format!("{name} has been abandoned."));
        }
        Ok(())
    }

    // ----- combat -------------------------------------------------------------------

    pub(crate) fn attack_strength(&self, u: &Unit) -> f32 {
        let r = &self.rules.units[u.kind as usize];
        let mut s = r.attack as f32;
        if matches!(u.prof, P_VETERAN | P_VETERAN_DRAGOON) && matches!(u.kind, UnitType::Soldier | UnitType::Dragoon) {
            s *= 1.5;
        }
        if u.damaged {
            s *= 0.5;
        }
        if u.kind == UnitType::Privateer && self.has_member(u.nation, M_PRIVATEERS) {
            s *= 1.5;
        }
        s.max(0.5)
    }

    pub(crate) fn defence_strength(&self, u: &Unit, x: i32, y: i32) -> f32 {
        let r = &self.rules.units[u.kind as usize];
        let mut s = (r.combat as f32).max(1.0);
        if matches!(u.prof, P_VETERAN | P_VETERAN_DRAGOON) && matches!(u.kind, UnitType::Soldier | UnitType::Dragoon) {
            s *= 1.5;
        }
        if u.orders == Orders::Fortified {
            s *= 1.5;
        }
        if u.damaged {
            s *= 0.5;
        }
        if let Some(c) = self.colony_at(x, y) {
            s *= 1.0 + 0.5 * (1 + c.level(BKind::Fortification)) as f32;
            if self.rebellion.as_ref().is_some_and(|r| r.nation == u.nation) && self.liberty_percent(c) >= 50 {
                s *= 1.5;
            }
        } else if let Some(t) = self.tile(x, y) {
            let d = self.rules.terrain.get(self.terrain_row(t)).map(|r| r.defence).unwrap_or(0);
            s *= 1.0 + d as f32 * 0.125;
        }
        s
    }

    /// Best defender standing on a square.
    pub(crate) fn defender_at(&self, x: i32, y: i32) -> Option<Id> {
        self.units_at(x, y)
            .max_by(|a, b| {
                self.defence_strength(a, x, y)
                    .partial_cmp(&self.defence_strength(b, x, y))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|u| u.id)
    }

    pub(crate) fn is_military(&self, u: &Unit) -> bool {
        self.can_attack(u.kind)
    }

    pub fn attack(&mut self, attacker: Id, x: i32, y: i32) {
        let Some(a) = self.unit(attacker).cloned() else { return };
        if a.kind.is_ship() {
            self.naval_battle(attacker, x, y);
            return;
        }
        if let Some(am) = self.unit_mut(attacker) {
            am.moves = 0;
            am.orders = Orders::None;
        }
        let att = self.attack_strength(&a);
        let a_name = format!("{} {}", self.nation_name(a.nation), self.unit_name(&a));
        let human_involved = |g: &Game, n: u8| n == g.human;

        // native settlement
        if let Some(vi) = self.village_at(x, y) {
            let v = self.villages[vi].clone();
            let tribe = self.nation_name(v.tribe).to_string();
            let tech = self.tribe_tech(v.tribe) as i32 as f32;
            let mut def = (1.0 + tech * 0.5) * 1.5;
            if a.nation == 2 {
                def /= 1.5; // Castamar's soldiers fight natives 50% harder
            }
            if v.capital {
                def *= 1.5;
            }
            let ti = v.tribe.saturating_sub(4) as usize;
            if let Some(t) = self.tribes.get_mut(ti) {
                t.alarm[a.nation.min(3) as usize] = 100;
            }
            let won = self.rand(1000) as f32 / 1000.0 < att / (att + def);
            if a.nation == self.human {
                self.events.push(crate::events::Event::Battle(crate::events::Battle {
                    attacker: (a.kind, a.nation, a.prof),
                    defender: None,
                    defender_name: format!("the {tribe} village"),
                    attack: att,
                    defence: def,
                    won,
                }));
                crate::sound::cue(self, crate::sound::Cue::Musket, true);
            }
            if won {
                self.villages[vi].pop = v.pop.saturating_sub(1);
                if self.villages[vi].pop == 0 {
                    let mut gold = v.wealth as i32 + self.rand(200) as i32;
                    if self.has_member(a.nation, M_PLUNDER) {
                        gold *= 2;
                    }
                    self.players[a.nation as usize].gold += gold;
                    self.villages.remove(vi);
                    self.msg(format!("The {tribe} settlement has been destroyed! You plunder {gold} gold."));
                    if !self.villages.iter().any(|w| w.tribe == v.tribe) {
                        if let Some(t) = self.tribes.get_mut(ti) {
                            t.extinct = true;
                        }
                        self.msg(format!("The {tribe} nation has been wiped out."));
                    }
                } else {
                    self.msg(format!("Your attack on the {tribe} succeeds. The village is weakened."));
                }
                if self.has_member(a.nation, M_CONQUEST_CONVERTS)
                    && let Some(p) = self.unit_pos(attacker) {
                        self.spawn(UnitType::Colonist, a.nation, P_CONVERT, Loc::Map { x: p.0 as i16, y: p.1 as i16 });
                    }
                self.maybe_promote(attacker);
            } else {
                self.msg(format!("The {tribe} drive off your attack."));
                self.lose(attacker, None);
            }
            self.purge_dead();
            return;
        }

        let colony = self.colony_at(x, y).map(|c| (c.id, c.nation, c.name.clone()));
        let defender = self.defender_at(x, y).filter(|d| {
            colony.is_none() || self.unit(*d).is_some_and(|u| self.is_military(u))
        });
        let (def, d_nation, d_name) = match defender {
            Some(d) => {
                let du = self.unit(d).unwrap();
                (self.defence_strength(du, x, y), du.nation, format!("{} {}", self.nation_name(du.nation), self.unit_name(du)))
            }
            None => match &colony {
                // colonists take up arms
                Some((cid, n, name)) => {
                    let c = self.colony(*cid).unwrap();
                    let s = 1.0 * (1.0 + 0.5 * (1 + c.level(BKind::Fortification)) as f32);
                    let minutemen = self.has_member(c.nation, M_MINUTEMEN);
                    let s = if minutemen && c.stock[MUSKETS] >= 50 { s * 2.0 } else { s };
                    (s, *n, format!("the colonists of {name}"))
                }
                None => return,
            },
        };
        if a.nation < 4 && d_nation < 4 {
            self.declare_war(a.nation, d_nation);
        }
        if a.nation < 4 && d_nation < 12 {
            self.players[a.nation as usize].at_war[d_nation as usize] = true;
        }
        if d_nation < 4 && a.nation < 12 {
            self.players[d_nation as usize].at_war[a.nation as usize] = true;
        }
        // patriots fighting the Crown's troops in the open get an ambush bonus
        let mut att = att;
        if self.rebellion.as_ref().is_some_and(|r| r.nation == a.nation) && d_nation == CROWN && colony.is_none() {
            att *= 1.5;
        }
        let involved = human_involved(self, a.nation) || human_involved(self, d_nation);
        let win = self.rand(1000) as f32 / 1000.0 < att / (att + def);
        if involved {
            let defender_unit = defender.and_then(|d| self.unit(d)).map(|u| (u.kind, u.nation, u.prof));
            self.events.push(crate::events::Event::Battle(crate::events::Battle {
                attacker: (a.kind, a.nation, a.prof),
                defender: defender_unit,
                defender_name: d_name.clone(),
                attack: att,
                defence: def,
                won: win,
            }));
            let cue = if a.kind == UnitType::Artillery || colony.is_some() { crate::sound::Cue::Cannon } else { crate::sound::Cue::Musket };
            crate::sound::cue(self, cue, true);
        }
        if win {
            match (defender, &colony) {
                (Some(d), _) => {
                    if involved {
                        self.msg(format!("{a_name} defeats {d_name}."));
                    }
                    self.lose(d, Some(a.nation));
                    self.maybe_promote(attacker);
                }
                (None, Some((cid, cn, name))) => {
                    if is_native(a.kind) {
                        let gold = (self.players[*cn as usize].gold / 10).min(300);
                        self.players[*cn as usize].gold -= gold;
                        let c = self.colony_mut(*cid).unwrap();
                        let g = (0..16).max_by_key(|&g| c.stock[g]).unwrap_or(0);
                        let stolen = c.stock[g] / 2;
                        c.stock[g] -= stolen;
                        if involved {
                            self.msg(format!(
                                "{a_name} raid {name}! They steal {stolen} {} and {gold} gold.",
                                GOOD_NAMES[g]
                            ));
                        }
                        if *cn == self.human {
                            self.milestone(crate::events::MS_RAID);
                        }
                    } else {
                        self.capture_colony(*cid, a.nation);
                        self.place(attacker, x, y);
                        if involved {
                            self.msg(format!("{name} has been captured by the {}!", self.nation_name(a.nation)));
                        }
                    }
                }
                _ => {}
            }
        } else {
            if involved {
                self.msg(format!("{a_name} is defeated by {d_name}."));
            }
            self.lose(attacker, Some(d_nation));
        }
        self.purge_dead();
    }

    fn maybe_promote(&mut self, id: Id) {
        let always = self.unit(id).is_some_and(|u| self.has_member(u.nation, M_PROMOTION));
        let lucky = self.chance(20);
        if let Some(u) = self.unit_mut(id) {
            let soldier = matches!(u.kind, UnitType::Soldier | UnitType::Dragoon);
            let green = matches!(u.prof, P_FREE | P_FREE_OLD | P_SERVANT | P_CRIMINAL);
            if soldier && (always || (green && lucky)) {
                u.prof = P_VETERAN;
            }
        }
    }

    /// A unit loses a battle.
    fn lose(&mut self, id: Id, winner: Option<u8>) {
        let Some(u) = self.unit(id).cloned() else { return };
        let kind = u.kind;
        let captured_by = winner.filter(|w| *w < 4 && u.nation < 4);
        let new_kind = match kind {
            UnitType::Soldier => Some(UnitType::Colonist),
            UnitType::Dragoon => Some(UnitType::Soldier),
            UnitType::PatriotCavalry => Some(UnitType::PatriotArmy),
            UnitType::CrownCavalry => Some(UnitType::CrownRegular),
            UnitType::Artillery if !u.damaged => {
                self.unit_mut(id).unwrap().damaged = true;
                return;
            }
            UnitType::MountedWarrior => Some(UnitType::ArmedBrave),
            UnitType::MountedBrave => Some(UnitType::Brave),
            UnitType::Colonist | UnitType::Pioneer | UnitType::Scout | UnitType::Missionary | UnitType::WagonTrain | UnitType::Treasure => {
                if let Some(w) = captured_by {
                    let um = self.unit_mut(id).unwrap();
                    um.nation = w;
                    if kind == UnitType::Scout || kind == UnitType::Pioneer {
                        um.kind = UnitType::Colonist;
                    }
                    return;
                }
                None
            }
            _ => None,
        };
        match new_kind {
            Some(k) => {
                let um = self.unit_mut(id).unwrap();
                um.kind = k;
                if k == UnitType::Colonist {
                    um.tools = 0;
                }
            }
            None => self.kill(id),
        }
    }

    fn capture_colony(&mut self, cid: Id, new_owner: u8) {
        if let Some(c) = self.colony_mut(cid) {
            c.nation = new_owner;
        }
        let (x, y) = self.colony(cid).map(|c| (c.x, c.y)).unwrap();
        for u in &mut self.units {
            if matches!(u.loc, Loc::Colony { colony, .. } if colony == cid) {
                u.nation = new_owner;
            } else if u.loc == (Loc::Map { x, y }) && u.nation != new_owner {
                u.loc = Loc::Dead;
            }
        }
    }

    // ----- computer players -----------------------------------------------------

    fn ai_turn(&mut self) {
        let ai: Vec<u8> = (0..4u8).filter(|&n| n != self.human && self.players[n as usize].control == Control::Ai).collect();
        for n in ai {
            // keep every colony building something
            let cids: Vec<Id> = self.colonies.iter().filter(|c| c.nation == n).map(|c| c.id).collect();
            for cid in &cids {
                if self.colony(*cid).is_some_and(|c| c.building.is_none()) {
                    let b = self.default_build(*cid);
                    self.colony_mut(*cid).unwrap().building = b;
                }
            }
            self.ai_economy(n);
            self.ai_ships(n);
            self.ai_naval(n);
            self.ai_war(n);
            let ids: Vec<Id> = self
                .units
                .iter()
                .filter(|u| u.nation == n && matches!(u.loc, Loc::Map { .. }) && !u.kind.is_ship())
                .map(|u| u.id)
                .collect();
            for id in ids {
                let Some(u) = self.unit(id).cloned() else { continue };
                let Some((x, y)) = self.unit_pos(id) else { continue };
                let has_colony = self.colonies.iter().any(|c| c.nation == n);
                let at_home = self.colony_at(x, y).is_some_and(|c| c.nation == n);
                if !has_colony && is_colonist(u.kind) {
                    if !self.ai_settle(id, x, y, true) && !self.ai_settle(id, x, y, false) {
                        self.ai_wander(id, x, y);
                    }
                    continue;
                }
                let settler = self.ai_settlers.contains(&id);
                match u.kind {
                    UnitType::Colonist if settler => {
                        // look for a fresh site a few squares from home
                        let before = self.colonies.len();
                        if !self.ai_settle(id, x, y, false) {
                            self.ai_wander(id, x, y);
                            if self.chance(5) {
                                self.ai_settlers.retain(|s| *s != id);
                            }
                        }
                        if self.colonies.len() > before {
                            self.ai_settlers.retain(|s| *s != id);
                        }
                    }
                    UnitType::Colonist => {
                        if at_home {
                            let _ = self.join_colony(id);
                        } else if self.can_found_colony(id).is_ok() && self.ai_good_site(x, y) && self.chance(20) {
                            let _ = self.found_colony(id);
                        } else if self.chance(50) {
                            self.ai_step_home(id, x, y);
                        } else {
                            self.ai_wander(id, x, y);
                        }
                    }
                    UnitType::Soldier | UnitType::Dragoon | UnitType::Artillery => {
                        if at_home {
                            if u.orders != Orders::Fortified {
                                self.set_orders(id, Orders::Fortify);
                            }
                        } else {
                            self.ai_step_home(id, x, y);
                        }
                    }
                    UnitType::Pioneer => {
                        if matches!(u.orders, Orders::Plow { .. } | Orders::Road { .. }) {
                            continue;
                        }
                        let near = self.colonies.iter().any(|c| {
                            c.nation == n && (c.x as i32 - x).abs() <= 1 && (c.y as i32 - y).abs() <= 1
                        });
                        if near && !at_home && self.start_improvement(id, false).is_ok() {
                            continue;
                        }
                        if at_home && u.tools < 20 {
                            let _ = self.join_colony(id);
                        } else if near {
                            self.ai_wander(id, x, y);
                        } else {
                            self.ai_step_home(id, x, y);
                        }
                    }
                    UnitType::Scout | UnitType::Missionary => self.ai_wander(id, x, y),
                    _ => {}
                }
            }
        }
        self.purge_dead();
    }

    /// Ships carrying colonists head for the nearest free coast and land them.
    fn ai_ships(&mut self, n: u8) {
        let ships: Vec<Id> = self
            .units
            .iter()
            .filter(|u| u.nation == n && u.kind.is_ship() && matches!(u.loc, Loc::Map { .. }))
            .map(|u| u.id)
            .collect();
        for sid in ships {
            if self.aboard(sid).count() == 0 || self.unit(sid).is_some_and(|u| u.orders == Orders::Route) {
                continue;
            }
            let Some((sx, sy)) = self.unit_pos(sid) else { continue };
            // nearest good landing square
            let mut best: Option<(i32, i32, i32)> = None;
            let reach = self.width.max(self.height) as i32;
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    let (x, y) = (sx + dx, sy + dy);
                    let Some(t) = self.tile(x, y) else { continue };
                    if t.terrain().is_water()
                        || t.terrain() == crate::types::Terrain::Arctic
                        || self.colony_at(x, y).is_some()
                        || self.village_at(x, y).is_some()
                        || self.units_at(x, y).any(|u| u.nation != n)
                        || !self.is_coastal(x, y)
                    {
                        continue;
                    }
                    // land at a decent colony site, or next to our own colony
                    let own_colony_near = self.colonies.iter().any(|c| c.nation == n && (c.x as i32 - x).abs().max((c.y as i32 - y).abs()) <= 1);
                    if !own_colony_near && !(self.site_free(x, y) && self.ai_good_site(x, y)) {
                        continue;
                    }
                    let d = dx.abs().max(dy.abs());
                    if best.is_none_or(|(_, _, bd)| d < bd) {
                        best = Some((x, y, d));
                    }
                }
            }
            let Some((tx, ty, _)) = best else { continue };
            let steps = self.max_moves(self.unit(sid).unwrap().kind) / 3;
            let (cx, cy) = self.unit_pos(sid).unwrap();
            if (tx - cx).abs() <= 1 && (ty - cy).abs() <= 1 {
                self.landfall(sid, tx, ty);
                continue;
            }
            // sail for the water beside the landing site
            let water = DIRS8
                .iter()
                .map(|(dx, dy)| (tx + dx, ty + dy))
                .filter(|&(wx, wy)| self.is_water(wx, wy) && self.tile(wx, wy).is_some())
                .min_by_key(|&(wx, wy)| (wx - cx).abs().max((wy - cy).abs()));
            let Some((wx, wy)) = water else { continue };
            let Some(path) = self.find_path(sid, wx, wy) else { continue };
            for &(px, py) in path.iter().take(steps as usize) {
                if self.units_at(px, py).any(|u| u.nation != n) {
                    break;
                }
                self.place(sid, px, py);
            }
            let (cx, cy) = self.unit_pos(sid).unwrap();
            if (tx - cx).abs() <= 1 && (ty - cy).abs() <= 1 {
                self.landfall(sid, tx, ty);
            }
        }
    }

    /// One step towards the nearest own colony.
    fn ai_step_home(&mut self, id: Id, x: i32, y: i32) {
        let nation = self.unit(id).map(|u| u.nation).unwrap_or(0);
        let home = self
            .colonies
            .iter()
            .filter(|c| c.nation == nation)
            .min_by_key(|c| (c.x as i32 - x).abs().max((c.y as i32 - y).abs()))
            .map(|c| (c.x as i32, c.y as i32));
        let Some((hx, hy)) = home else { return self.ai_wander(id, x, y) };
        let (dx, dy) = ((hx - x).signum(), (hy - y).signum());
        for (ddx, ddy) in [(dx, dy), (dx, 0), (0, dy)] {
            if (ddx, ddy) == (0, 0) {
                continue;
            }
            let (nx, ny) = (x + ddx, y + ddy);
            let ok = !self.is_water(nx, ny)
                && self.village_at(nx, ny).is_none()
                && self.colony_at(nx, ny).is_none_or(|c| c.nation == nation)
                && !self.units_at(nx, ny).any(|u| u.nation != nation);
            if ok {
                self.place(id, nx, ny);
                return;
            }
        }
        self.ai_wander(id, x, y);
    }

    fn ai_wander(&mut self, id: Id, x: i32, y: i32) {
        let (dx, dy) = DIRS8[self.rand(8) as usize];
        let (nx, ny) = (x + dx, y + dy);
        let nation = self.unit(id).map(|u| u.nation).unwrap_or(0);
        let ok = !self.is_water(nx, ny)
            && self.village_at(nx, ny).is_none()
            && self.colony_at(nx, ny).is_none_or(|c| c.nation == nation)
            && !self.units_at(nx, ny).any(|u| u.nation != nation);
        if ok {
            self.place(id, nx, ny);
        }
    }
}
