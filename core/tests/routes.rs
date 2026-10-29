//! Trade routes for wagon trains and ships.
mod common;
use landfall::game::*;
use landfall::types::UnitType;

#[test]
fn ship_route_to_the_homeland() {
    let (mut g, cols) = common::with_colonies(3, 1, 3);
    let h = g.human;
    let c = g.colony(cols[0]).unwrap().clone();
    g.colony_mut(c.id).unwrap().stock[FURS] = 100;
    g.colony_mut(c.id).unwrap().stock[TOOLS] = 0;
    let ship = g.spawn(UnitType::Merchantman, h, P_FREE, Loc::Map { x: c.x, y: c.y });
    g.set_route(ship, Some(TradeRoute { a: Place::Colony(c.id), b: Place::Homeland, a_to_b: vec![FURS as u8], b_to_a: vec![TOOLS as u8], to_b: false }));
    let gold0 = g.players[h as usize].gold;
    for _ in 0..40 {
        g.colony_mut(c.id).unwrap().stock[FOOD] = 100;
        g.end_turn();
    }
    let _ = gold0;
    assert!(!g.unit(ship).unwrap().cargo.iter().any(|c| c.0 == FURS as u8), "the furs were never sold");
    assert!(g.colony(c.id).unwrap().stock[TOOLS] > 0, "no tools came back");
}

#[test]
fn wagon_route_between_colonies() {
    let (mut g, cols) = common::with_colonies(3, 6, 2);
    let h = g.human;
    let mut pair = None;
    'outer: for &a in &cols {
        for &b in &cols {
            if a == b {
                continue;
            }
            let (ca, cb) = (g.colony(a).unwrap().clone(), g.colony(b).unwrap().clone());
            let w = g.spawn(UnitType::WagonTrain, h, P_FREE, Loc::Map { x: ca.x, y: ca.y });
            let ok = g.find_path(w, cb.x as i32, cb.y as i32).is_some_and(|p| p.len() < 25);
            g.kill(w);
            g.purge_dead();
            if ok {
                pair = Some((a, b));
                break 'outer;
            }
        }
    }
    let (a, b) = pair.expect("two colonies joined by land");
    let ca = g.colony(a).unwrap().clone();
    let w = g.spawn(UnitType::WagonTrain, h, P_FREE, Loc::Map { x: ca.x, y: ca.y });
    g.set_route(w, Some(TradeRoute { a: Place::Colony(a), b: Place::Colony(b), a_to_b: vec![FURS as u8], b_to_a: vec![], to_b: false }));
    let before = g.colony(b).unwrap().stock[FURS];
    for _ in 0..30 {
        g.colony_mut(a).unwrap().stock[FURS] = 200;
        for &c in &cols {
            g.colony_mut(c).unwrap().stock[FOOD] = 100;
        }
        g.end_turn();
    }
    assert!(g.colony(b).unwrap().stock[FURS] > before, "nothing delivered");
}
