//! Land claims, trade and first contact with the native peoples.
mod common;
use landfall::events::*;
use landfall::game::*;
use landfall::types::UnitType;
use landfall::world::VillageAction;

#[test]
fn land_claims_and_trade() {
    let mut g = common::new_game(4);
    let v = g.villages[0].clone();
    let spot = DIRS8.iter().map(|(dx, dy)| (v.x as i32 + 2 * dx, v.y as i32 + 2 * dy)).find(|&(x, y)| g.tile(x, y).is_some() && !g.is_water(x, y));
    let Some((x, y)) = spot else { return };
    let claim = g.land_claim(0, x, y).expect("land near a village is claimed");
    assert!(claim.price > 0);
    g.players[0].gold = 10_000;
    let alarm0 = g.tribes[claim.tribe].alarm[0];
    g.settle_claim(0, &claim, true).unwrap();
    assert!(g.players[0].gold < 10_000);
    assert!(g.tribes[claim.tribe].alarm[0] <= alarm0);

    let c = g.spawn(UnitType::Colonist, 0, P_FREE, Loc::Map { x: x as i16, y: y as i16 });
    if let Ok(cid) = g.found_colony(c) {
        g.colony_mut(cid).unwrap().stock[MUSKETS] = 100;
        let gold = g.players[0].gold;
        g.native_sale((v.tribe - 4) as usize, cid, MUSKETS, 100, 500);
        assert_eq!(g.players[0].gold, gold + 500);
    }
    let mut seen = 0;
    for _ in 0..200 {
        for c in g.colonies.iter_mut().filter(|c| c.nation == 0) {
            c.stock[CLOTH] = 100;
            c.stock[FOOD] = 100;
        }
        g.end_turn();
        seen += g.events.iter().filter(|e| matches!(e, Event::NativeTrade { .. } | Event::NativeDemand { .. })).count();
        g.events.clear();
        g.messages.clear();
    }
    assert!(seen > 0, "natives should come to trade");
}

#[test]
fn first_contact_and_the_chief() {
    let mut g = common::new_game(7);
    let v = g.villages.iter().find(|v| v.tribe == 5).unwrap().clone();
    let (x, y) = DIRS8.iter().map(|(dx, dy)| (v.x as i32 + dx, v.y as i32 + dy)).find(|&(x, y)| !g.is_water(x, y) && g.village_at(x, y).is_none()).unwrap();
    let s = g.spawn(UnitType::Scout, 0, P_FREE, Loc::Map { x: x as i16, y: y as i16 });
    g.scan_contacts();
    assert!(g.events.contains(&Event::Milestone(MS_NATIVES)));
    assert!(g.events.contains(&Event::Milestone(MS_LEAGUE)));
    assert!(g.events.iter().any(|e| matches!(e, Event::Chief { tribe: 1, .. })));
    g.events.clear();
    g.scan_contacts();
    assert!(g.events.is_empty(), "contact happens once");
    let vi = g.village_at(v.x as i32, v.y as i32).unwrap();
    g.village_action(s, vi, VillageAction::SpeakWithChief);
    assert!(g.events.contains(&Event::Milestone(MS_VILLAGE)));
}
