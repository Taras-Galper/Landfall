//! Long games with the computer players running everything.
mod common;
use landfall::game::*;
use landfall::mapgen::{NewGame, World};
use landfall::rules::Rules;

#[test]
fn computer_players_build_empires() {
    for (world, seed) in [(World::default(), 9u64), (World { land: 0, shape: 0, temperature: 1, climate: 1 }, 5), (World { land: 2, shape: 1, temperature: 2, climate: 2 }, 21)] {
        let mut g = Game::new_game(Rules::standard(), &NewGame { human: 0, difficulty: 2, world, seed });
        for _ in 0..220 {
            g.end_turn();
            g.messages.clear();
            g.events.clear();
        }
        let ai_cols: usize = (1..4u8).map(|n| g.colonies.iter().filter(|c| c.nation == n).count()).sum();
        println!("{}: {ai_cols} computer colonies by {}", world.describe(), g.year_label());
        // the computer players are weaker on small islands (a known gap)
        let need = if world.shape == 0 { 4 } else { 6 };
        assert!(ai_cols >= need, "computer players should expand ({ai_cols} colonies)");
    }
}

#[test]
fn saves_round_trip() {
    let (mut g, _) = common::with_colonies(3, 2, 3);
    for _ in 0..30 {
        g.end_turn();
    }
    let j = g.to_json();
    let back = Game::from_json(&j, Rules::standard()).unwrap();
    assert_eq!(back.colonies.len(), g.colonies.len());
    assert_eq!(back.units.len(), g.units.len());
    assert_eq!(back.to_json(), j);
}
