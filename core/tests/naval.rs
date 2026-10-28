//! War at sea.
mod common;
use landfall::game::*;
use landfall::naval::SeaResult;
use landfall::types::UnitType;

fn open_sea(g: &Game) -> (i32, i32) {
    for y in 10..60 {
        for x in 45..54 {
            if (0..3).all(|dx| g.is_water(x + dx, y) && g.units_at(x + dx, y).next().is_none()) {
                return (x, y);
            }
        }
    }
    panic!("no open sea");
}

#[test]
fn frigate_against_merchantman() {
    let base = common::new_game(3);
    let (h, foe) = (0u8, 1u8);
    let (x, y) = open_sea(&base);
    let mut tally = std::collections::HashMap::new();
    for seed in 0..200u64 {
        let mut g = base.clone();
        g.rng = g.rng.wrapping_add(seed * 7919);
        let f = g.spawn(UnitType::Frigate, foe, P_FREE, Loc::Map { x: x as i16, y: y as i16 });
        let m = g.spawn(UnitType::Merchantman, h, P_FREE, Loc::Map { x: x as i16 + 1, y: y as i16 });
        g.unit_mut(m).unwrap().cargo.push((FURS as u8, 100));
        g.unit_mut(f).unwrap().moves = 18;
        let r = g.naval_battle(f, x + 1, y).expect("battle");
        let key = match &r {
            SeaResult::Evaded => "evaded",
            SeaResult::Damaged { .. } => "damaged",
            SeaResult::Sunk => "sunk",
            SeaResult::AttackerDamaged { .. } => "att_damaged",
            SeaResult::AttackerSunk => "att_sunk",
        };
        *tally.entry(key).or_insert(0) += 1;
        match r {
            SeaResult::Damaged { .. } => {
                let u = g.unit(m).unwrap();
                assert!(u.cargo.is_empty() && u.damaged && u.repair > 0);
                assert!(g.unit(f).unwrap().cargo.iter().any(|c| c.0 == FURS as u8));
            }
            SeaResult::Sunk => assert!(g.unit(m).is_none()),
            _ => {}
        }
        assert!(g.players[foe as usize].at_war[h as usize], "attacking means war");
    }
    println!("{tally:?}");
    assert!(tally.get("sunk").copied().unwrap_or(0) + tally.get("damaged").copied().unwrap_or(0) > 60);
    assert!(tally.get("evaded").copied().unwrap_or(0) > 10);
}

#[test]
fn privateers_do_not_declare_war() {
    let mut g = common::new_game(3);
    let (h, foe) = (0u8, 1u8);
    g.players[foe as usize].at_war = [false; 12];
    g.players[h as usize].at_war = [false; 12];
    let (x, y) = open_sea(&g);
    let p = g.spawn(UnitType::Privateer, foe, P_FREE, Loc::Map { x: x as i16, y: y as i16 });
    g.spawn(UnitType::Caravel, h, P_FREE, Loc::Map { x: x as i16 + 1, y: y as i16 });
    g.naval_battle(p, x + 1, y);
    assert!(!g.players[foe as usize].at_war[h as usize]);
}
