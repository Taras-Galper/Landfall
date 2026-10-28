//! The colonial assembly, schools, custom houses and the war of independence.
mod common;
use landfall::assembly::*;
use landfall::game::*;
use landfall::types::UnitType;

#[test]
fn members_join() {
    let (mut g, cols) = common::with_colonies(3, 2, 4);
    let h = g.human;
    g.offer_members(h);
    let offered = g.players[h as usize].assembly.offered.clone();
    assert!(!offered.is_empty() && offered.len() <= 5, "{offered:?}");
    g.choose_member(h, M_BELLS);
    // statesmen ring the bells
    for &cid in &cols {
        let ids: Vec<Id> = g.workers(cid).map(|u| u.id).collect();
        g.colony_mut(cid).unwrap().stock[FOOD] = 500;
        for id in ids.iter().take(2) {
            g.unit_mut(*id).unwrap().loc = Loc::Colony { colony: cid, job: Job::Building(17) };
        }
    }
    for _ in 0..80 {
        g.end_turn();
        if g.has_member(h, M_BELLS) {
            break;
        }
    }
    assert!(g.has_member(h, M_BELLS), "bells {} cost {}", g.players[h as usize].assembly.bells, g.member_cost(h));
}

#[test]
fn school_teaches() {
    let (mut g, cols) = common::with_colonies(3, 1, 1);
    let h = g.human;
    let cid = cols[0];
    g.colony_mut(cid).unwrap().levels[BKind::Schoolhouse as usize] = 1;
    let teacher = g.spawn(UnitType::Colonist, h, P_EXPERT_FARMER, Loc::Colony { colony: cid, job: Job::Building(18) });
    let student = g.spawn(UnitType::Colonist, h, P_FREE, Loc::Colony { colony: cid, job: Job::Building(17) });
    let ids: Vec<Id> = g.workers(cid).map(|u| u.id).collect();
    for id in ids {
        if id != teacher && id != student {
            g.unit_mut(id).unwrap().prof = 5;
        }
    }
    g.colony_mut(cid).unwrap().stock[FOOD] = 500;
    for _ in 0..6 {
        g.end_turn();
    }
    assert_eq!(g.unit(student).unwrap().prof, P_EXPERT_FARMER, "the student should become an expert farmer");
}

#[test]
fn custom_house_sells() {
    let (mut g, cols) = common::with_colonies(3, 1, 2);
    let h = g.human;
    let cid = cols[0];
    g.colony_mut(cid).unwrap().levels[BKind::CustomHouse as usize] = 1;
    g.colony_mut(cid).unwrap().stock[FURS] = 150;
    let before = g.players[h as usize].gold;
    let earned = g.custom_house(cid);
    assert!(earned > 0 && g.players[h as usize].gold == before + earned);
    assert_eq!(g.colony(cid).unwrap().stock[FURS], 50);
}

fn rebels(g: &mut Game, cols: &[Id]) {
    for cid in cols {
        let pop = g.population(*cid) as i32;
        g.colony_mut(*cid).unwrap().liberty = pop * 200;
        let c = g.colony(*cid).unwrap().clone();
        for _ in 0..3 {
            g.spawn(UnitType::Soldier, 0, P_VETERAN, Loc::Map { x: c.x, y: c.y });
        }
        g.colony_mut(*cid).unwrap().levels[BKind::Fortification as usize] = 2;
    }
}

#[test]
fn war_of_independence_ends() {
    let (mut g, cols) = common::with_colonies(3, 3, 4);
    let h = g.human;
    assert!(g.can_declare(h).is_err());
    rebels(&mut g, &cols);
    assert!(g.rebel_percent(h) >= 50);
    g.crown_force = [8, 3, 2, 3];
    g.declare_independence(h).unwrap();
    assert!(g.rebellion.is_some());
    let mut turns = 0;
    while g.game_over.is_none() && turns < 200 {
        g.end_turn();
        turns += 1;
    }
    println!("after {turns} turns: {:?}", g.game_over);
    assert!(g.game_over.is_some(), "the war never ended");
}

#[test]
fn foreign_intervention_arrives() {
    let (mut g, cols) = common::with_colonies(3, 2, 4);
    let h = g.human;
    rebels(&mut g, &cols);
    g.crown_force = [20, 5, 4, 5];
    g.declare_independence(h).unwrap();
    let before = g.units.iter().filter(|u| u.nation == h && u.kind == UnitType::PatriotArmy).count();
    g.add_assembly_bells(h, 100_000);
    g.end_turn();
    let after = g.units.iter().filter(|u| u.nation == h && u.kind == UnitType::PatriotArmy).count();
    assert!(g.rebellion.as_ref().unwrap().intervened);
    assert!(after > before, "{before} -> {after}");
    assert!(g.messages.iter().any(|m| m.starts_with("INTERVENTION")));
}

#[test]
fn loyalists_slow_big_colonies() {
    let (mut g, cols) = common::with_colonies(3, 1, 8);
    let big = cols[0];
    g.difficulty = 4;
    g.colony_mut(big).unwrap().liberty = 0;
    let pop = g.population(big);
    let pen = g.loyalist_penalty(g.colony(big).unwrap());
    assert!(pop > 4 && pen >= 1, "pop {pop} penalty {pen}");
    g.colony_mut(big).unwrap().liberty = 200 * pop as i32;
    assert_eq!(g.loyalist_penalty(g.colony(big).unwrap()), 0);
}
