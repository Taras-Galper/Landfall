//! Shared fixtures: a fresh game with the human's colonies already founded.
#![allow(dead_code)]
use landfall::game::*;
use landfall::mapgen::{NewGame, World};
use landfall::rules::Rules;
use landfall::types::UnitType;

pub fn new_game(seed: u64) -> Game {
    let mut g = Game::new_game(Rules::standard(), &NewGame { human: 0, difficulty: 1, world: World::default(), seed });
    g.events.clear();
    g.messages.clear();
    g
}

/// Found up to `n` coastal colonies for nation 0, each with `pop` colonists.
pub fn with_colonies(seed: u64, n: usize, pop: usize) -> (Game, Vec<Id>) {
    let mut g = new_game(seed);
    let mut cols = Vec::new();
    'search: for y in 4..g.height as i32 - 4 {
        for x in 4..g.width as i32 - 4 {
            if cols.len() >= n {
                break 'search;
            }
            if g.is_water(x, y) || !g.is_coastal(x, y) || g.village_at(x, y).is_some() {
                continue;
            }
            if g.villages.iter().any(|v| (v.x as i32 - x).abs() <= 3 && (v.y as i32 - y).abs() <= 3) {
                continue;
            }
            if g.colonies.iter().any(|c| (c.x as i32 - x).abs() <= 4 && (c.y as i32 - y).abs() <= 4) {
                continue;
            }
            let u = g.spawn(UnitType::Colonist, 0, P_FREE, Loc::Map { x: x as i16, y: y as i16 });
            match g.found_colony(u) {
                Ok(cid) => {
                    for _ in 1..pop {
                        let w = g.spawn(UnitType::Colonist, 0, P_FREE, Loc::Map { x: x as i16, y: y as i16 });
                        g.join_colony(w).unwrap();
                    }
                    g.colony_mut(cid).unwrap().stock[FOOD] = 100;
                    cols.push(cid);
                }
                Err(_) => {
                    g.kill(u);
                    g.purge_dead();
                }
            }
        }
    }
    g.events.clear();
    g.messages.clear();
    (g, cols)
}
