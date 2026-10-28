//! Sound cues. The engine queues them; the interface plays a matching sound.

use crate::game::Game;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cue {
    Cannon,
    Musket,
    Victory,
    Defeat,
    ShipSunk,
    Drums,
    WarCry,
    Cheer,
    Bells,
    Build,
    Sail,
    Fanfare,
    Treasure,
    Burning,
    Click,
}

impl Cue {
    pub const ALL: [Cue; 15] = [
        Cue::Cannon,
        Cue::Musket,
        Cue::Victory,
        Cue::Defeat,
        Cue::ShipSunk,
        Cue::Drums,
        Cue::WarCry,
        Cue::Cheer,
        Cue::Bells,
        Cue::Build,
        Cue::Sail,
        Cue::Fanfare,
        Cue::Treasure,
        Cue::Burning,
        Cue::Click,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Cue::Cannon => "cannon",
            Cue::Musket => "musket",
            Cue::Victory => "victory",
            Cue::Defeat => "defeat",
            Cue::ShipSunk => "ship_sunk",
            Cue::Drums => "drums",
            Cue::WarCry => "war_cry",
            Cue::Cheer => "cheer",
            Cue::Bells => "bells",
            Cue::Build => "build",
            Cue::Sail => "sail",
            Cue::Fanfare => "fanfare",
            Cue::Treasure => "treasure",
            Cue::Burning => "burning",
            Cue::Click => "click",
        }
    }
}

/// Queue a sound if the human player would hear it.
pub fn cue(g: &mut Game, c: Cue, audible: bool) {
    if audible && g.sounds.len() < 8 {
        g.sounds.push(c);
    }
}
