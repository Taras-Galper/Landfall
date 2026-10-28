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
