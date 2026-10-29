//! Travel orders into unexplored waters and lands.
mod common;
use landfall::game::*;
use landfall::world::MoveResult;

#[test]
fn ships_head_into_the_unknown_and_stop_at_land() {
    let mut g = common::new_game(7);
    let ship = g.units.iter().find(|u| u.nation == 0 && u.kind.is_ship()).unwrap().id;
    let (sx, sy) = g.unit_pos(ship).unwrap();
    // a square far to the west that nobody has seen
    let (tx, ty) = (6, sy);
    assert!(!g.tile(tx, ty).unwrap().visible_to(0));
    assert!(g.find_path(ship, tx, ty).is_some(), "unexplored squares count as open");
    let mut blocked = None;
    if let Some(MoveResult::Blocked(why)) = g.goto(ship, tx, ty) {
        blocked = Some(why);
    }
    for _ in 0..40 {
        if blocked.is_some() || g.unit(ship).unwrap().orders == Orders::None {
            break;
        }
        g.end_turn();
        // the turn engine leaves human go-to orders to the interface
        if let Some(MoveResult::Blocked(why)) = g.continue_goto(ship) {
            blocked = Some(why);
        }
    }
    let (x, y) = g.unit_pos(ship).unwrap();
    assert!(g.is_water(x, y) || g.colony_at(x, y).is_some(), "the ship never sails onto land");
    assert!((x, y) != (sx, sy), "the ship moved");
    println!("stopped at ({x},{y}): {blocked:?}");
    if g.is_water(tx, ty) && blocked.is_none() {
        assert_eq!((x, y), (tx, ty));
    } else {
        assert!(blocked.is_some_and(|w| w.contains("waits for orders") || w.contains("blocked")));
    }
}

#[test]
fn ships_sent_home_and_back_to_a_port() {
    let (mut g, cols) = common::with_colonies(3, 1, 3);
    let c = g.colony(cols[0]).unwrap().clone();
    let ship = g.spawn(landfall::types::UnitType::Caravel, 0, P_FREE, Loc::Map { x: c.x, y: c.y });
    // reveal the way out so the route is known
    for y in 0..g.height as i32 {
        for x in 0..g.width as i32 {
            g.reveal(0, x, y, 0);
        }
    }
    let msg = g.send_home(ship).expect("a route to the sea lanes");
    println!("{msg}");
    let mut reached = false;
    for _ in 0..30 {
        if g.unit(ship).unwrap().loc == Loc::Homeland {
            reached = true;
            break;
        }
        g.end_turn();
        let _ = g.continue_goto(ship);
    }
    assert!(reached, "the ship never reached the homeland: {:?}", g.unit(ship).unwrap().loc);
    // back across, straight to the colony
    g.sail_to_port(ship, Some(c.id)).unwrap();
    let mut home = false;
    for _ in 0..40 {
        g.end_turn();
        let _ = g.continue_goto(ship);
        if g.unit_pos(ship) == Some((c.x as i32, c.y as i32)) {
            home = true;
            break;
        }
    }
    assert!(home, "the ship should sail on to {} after arriving", c.name);
}

#[test]
fn partial_cargo() {
    let (mut g, cols) = common::with_colonies(3, 1, 2);
    let c = g.colony(cols[0]).unwrap().clone();
    g.colony_mut(c.id).unwrap().stock[FURS] = 80;
    let ship = g.spawn(landfall::types::UnitType::Merchantman, 0, P_FREE, Loc::Map { x: c.x, y: c.y });
    g.load_goods(c.id, ship, FURS, Some(30)).unwrap();
    assert_eq!(g.unit(ship).unwrap().cargo, vec![(FURS as u8, 30)]);
    assert_eq!(g.colony(c.id).unwrap().stock[FURS], 50);
    g.unload_goods(ship, 0, Some(10)).unwrap();
    assert_eq!(g.unit(ship).unwrap().cargo, vec![(FURS as u8, 20)]);
    // in the homeland: sell part, buy part
    g.unit_mut(ship).unwrap().loc = Loc::Homeland;
    let gold = g.players[0].gold;
    g.sell_cargo_part(ship, 0, Some(5)).unwrap();
    assert_eq!(g.unit(ship).unwrap().cargo, vec![(FURS as u8, 15)]);
    assert!(g.players[0].gold > gold);
    g.players[0].gold = 10_000;
    assert_eq!(g.buy_cargo_part(ship, TOOLS, Some(25)).unwrap(), 25);
    assert!(g.unit(ship).unwrap().cargo.contains(&(TOOLS as u8, 25)));
}
