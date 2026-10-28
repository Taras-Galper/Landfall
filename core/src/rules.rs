//! Game rules and the names of the setting, loaded from `data/rules.json`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One terrain row: name, movement cost, defence bonus, turns to improve,
/// and yields for the 9 outdoor jobs (farmer, sugar, tobacco, cotton,
/// trapper, lumberjack, ore miner, silver miner, fisherman).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TerrainRule {
    pub name: String,
    pub movement: u8,
    pub defence: u8,
    pub improve_turns: u8,
    pub yields: [u8; 9],
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuildingRule {
    pub name: String,
    pub hammers: u16,
    pub tools: u16,
    pub min_pop: u8,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UnitRule {
    pub name: String,
    pub movement: u8,
    pub attack: u8,
    pub combat: u8,
    pub holds: u8,
    pub hammers: u16,
    pub tools: u16,
}

/// Market behaviour of one good in the homeland.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CargoRule {
    pub name: String,
    pub start_lo: u8,
    pub start_hi: u8,
    pub low: u8,
    pub high: u8,
    pub burden: u8,
    pub rise: i32,
    pub fall: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobRule {
    pub name: String,
    pub expert_name: String,
    pub skill: u8,
    /// Price to train this expert in the homeland (None = not trainable).
    pub homeland_cost: Option<u32>,
}

/// A figure who can join the colonial assembly.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemberRule {
    pub name: String,
    /// 0 trade, 1 exploration, 2 military, 3 political, 4 religious.
    pub kind: u8,
    /// Likelihood of being offered before 1600, 1600-1700, after 1700.
    pub weights: [u8; 3],
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Rules {
    pub members: Vec<MemberRule>,
    /// Leader names of the four colonial powers.
    pub leaders: Vec<String>,
    /// Native peoples: (name, tech level 0..=3).
    pub tribes: Vec<(String, u8)>,
    /// 0..8 open land, 8..16 forest, 16 arctic, 17 ocean, 18 sea lane,
    /// 19 mountains, 20 hills.
    pub terrain: Vec<TerrainRule>,
    pub buildings: Vec<BuildingRule>,
    pub units: Vec<UnitRule>,
    pub cargo: Vec<CargoRule>,
    pub jobs: Vec<JobRule>,
    pub colony_names: HashMap<u8, Vec<String>>,
    pub resources: Vec<String>,
    /// Display names for nations, units, professions and institutions.
    /// Mechanics never depend on these.
    pub names: Names,
}

/// Everything the player reads that identifies the setting.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Names {
    /// 13 entries: the four colonial powers, the eight native peoples, the Crown's army.
    pub nations: Vec<String>,
    /// Adjectives for the four powers ("the Brennish").
    pub adjectives: Vec<String>,
    /// Name each power takes after winning independence.
    pub independent: Vec<String>,
    /// 23 unit kinds, in UnitType order.
    pub units: Vec<String>,
    /// 29 professions, in profession-code order.
    pub professions: Vec<String>,
    /// The body that great figures join.
    pub assembly: String,
    /// The old continent the colonists sail from.
    pub homeland: String,
    /// The ocean beyond the New World.
    pub far_ocean: String,
    /// How the most advanced peoples' rulers introduce themselves, per people (empty = generic).
    pub rulers: Vec<String>,
}

impl Rules {
    /// The rules that ship with the game.
    pub fn standard() -> Self {
        Self::from_json(include_str!("../../data/rules.json")).expect("data/rules.json is valid")
    }
    pub fn from_json(s: &str) -> Result<Self, String> {
        serde_json::from_str(s).map_err(|e| e.to_string())
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}
