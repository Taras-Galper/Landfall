//! The live game state and the rules that act on it.
//!
//! `Game` is created by `Game::new_game` (or loaded from a JSON save) and is
//! what the interface shows and the turn engine updates. Colony economics are
//! in `colony.rs`; end-of-turn processing, the homeland, natives, combat and
//! the computer players in `world.rs`.

use crate::rules::Rules;
use crate::types::{Control, Tech, Terrain, Tile, UnitType};
use serde::{Deserialize, Serialize};

pub type Id = u32;

pub const FOOD: usize = 0;
pub const SUGAR: usize = 1;
pub const TOBACCO: usize = 2;
pub const COTTON: usize = 3;
pub const FURS: usize = 4;
pub const LUMBER: usize = 5;
pub const ORE: usize = 6;
pub const SILVER: usize = 7;
pub const HORSES: usize = 8;
pub const RUM: usize = 9;
pub const CIGARS: usize = 10;
pub const CLOTH: usize = 11;
pub const COATS: usize = 12;
pub const TRADE_GOODS: usize = 13;
pub const TOOLS: usize = 14;
pub const MUSKETS: usize = 15;

// Profession codes.
pub const P_EXPERT_FARMER: u8 = 0x00;
pub const P_FISHERMAN: u8 = 0x08;
pub const P_STATESMAN: u8 = 0x11;
pub const P_FREE_OLD: u8 = 0x13;
pub const P_HARDY_PIONEER: u8 = 0x14;
pub const P_VETERAN: u8 = 0x15;
pub const P_SEASONED_SCOUT: u8 = 0x16;
pub const P_VETERAN_DRAGOON: u8 = 0x17;
pub const P_MISSIONARY: u8 = 0x18;
pub const P_SERVANT: u8 = 0x19;
pub const P_CRIMINAL: u8 = 0x1A;
pub const P_CONVERT: u8 = 0x1B;
pub const P_FREE: u8 = 0x1C;

/// Jobs inside a colony. 0..=8 work a map square, 9..=18 work in a building.
pub const JOB_NAMES: [&str; 19] = [
    "Farmer", "Sugar Planter", "Tobacco Planter", "Cotton Planter", "Fur Trapper", "Lumberjack",
    "Ore Miner", "Silver Miner", "Fisherman", "Distiller", "Tobacconist", "Weaver", "Fur Trader",
    "Carpenter", "Blacksmith", "Gunsmith", "Preacher", "Statesman", "Teacher",
];
/// Good produced by outdoor jobs 0..=8.
pub const JOB_GOOD: [usize; 9] = [FOOD, SUGAR, TOBACCO, COTTON, FURS, LUMBER, ORE, SILVER, FOOD];

/// The 8 squares around a colony: N, E, S, W, NW, NE, SE, SW.
pub const DIRS8: [(i32, i32); 8] = [(0, -1), (1, 0), (0, 1), (-1, 0), (-1, -1), (1, -1), (1, 1), (-1, 1)];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Job {
    /// Working the square at colony + DIRS8[dir], doing outdoor job `job` (0..=8).
    Tile { dir: u8, job: u8 },
    /// Working in a building, job 9..=18.
    Building(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Loc {
    Map { x: i16, y: i16 },
    Aboard(Id),
    Colony { colony: Id, job: Job },
    /// In port / on the docks in the homeland.
    Homeland,
    Sailing { to_homeland: bool, turns: u8 },
    Dead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Orders {
    None,
    Sentry,
    Fortify,
    Fortified,
    Plow { turns: u8 },
    Road { turns: u8 },
    /// Walking / sailing to a square over one or more turns.
    Goto { x: i16, y: i16 },
    /// Following its trade route.
    Route,
}

/// A stop on a trade route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Place {
    Colony(Id),
    Homeland,
}

/// A two-stop trade route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradeRoute {
    pub a: Place,
    pub b: Place,
    /// Goods loaded at A and delivered to B.
    pub a_to_b: Vec<u8>,
    /// Goods loaded at B and delivered to A.
    pub b_to_a: Vec<u8>,
    /// Currently travelling towards B.
    pub to_b: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Unit {
    pub id: Id,
    pub kind: UnitType,
    pub nation: u8,
    pub prof: u8,
    pub loc: Loc,
    /// Movement left this turn, in thirds of a point.
    pub moves: u8,
    pub orders: Orders,
    /// Goods carried by ships and wagon trains: (good, amount).
    pub cargo: Vec<(u8, u8)>,
    pub tools: u8,
    pub treasure: u32,
    pub damaged: bool,
    /// Skipped / waiting this turn.
    pub done: bool,
    /// Where a ship left the map to sail to the homeland.
    pub sea_exit: Option<(i16, i16)>,
    /// Turns spent teaching in a schoolhouse.
    #[serde(default)]
    pub teach: u8,
    #[serde(default)]
    pub route: Option<TradeRoute>,
    /// Turns until a damaged ship is repaired.
    #[serde(default)]
    pub repair: u8,
    /// Squares still to cross on a go-to journey (the human's units follow
    /// the route they were given and stop if it turns out to be blocked).
    #[serde(default)]
    pub plan: Vec<(i16, i16)>,
    /// Sail on to the homeland when the current journey reaches the sea lanes.
    #[serde(default)]
    pub then_home: bool,
    /// Where to head after arriving in the New World from the homeland.
    #[serde(default)]
    pub arrive_at: Option<(i16, i16)>,
}

/// Building kinds, each with up to 3 levels. Row in the rules' `buildings` for
/// (kind, level >= 1) is `BUILDING_ROW[kind] + level - 1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[repr(u8)]
pub enum BKind {
    Fortification, Armory, Docks, TownHall, Schoolhouse, Warehouse, Stables, CustomHouse,
    PrintingPress, Weaver, Tobacconist, Distiller, Capitol, FurTrader, Carpenter, Church, Blacksmith,
}
pub const BKINDS: [BKind; 17] = [
    BKind::Fortification, BKind::Armory, BKind::Docks, BKind::TownHall, BKind::Schoolhouse,
    BKind::Warehouse, BKind::Stables, BKind::CustomHouse, BKind::PrintingPress, BKind::Weaver,
    BKind::Tobacconist, BKind::Distiller, BKind::Capitol, BKind::FurTrader, BKind::Carpenter,
    BKind::Church, BKind::Blacksmith,
];
pub const BUILDING_ROW: [usize; 17] = [0, 3, 6, 9, 12, 15, 17, 18, 19, 21, 24, 27, 30, 32, 35, 37, 39];
pub const BUILDING_MAX: [u8; 17] = [3, 3, 3, 3, 3, 2, 1, 1, 2, 3, 3, 3, 2, 3, 2, 2, 3];
/// Slot in types::BUILDING_BITS for each BKind.

/// Which building a building job (9..=18) needs, and what it converts.
pub fn job_building(job: u8) -> Option<(BKind, Option<usize>, usize)> {
    // (building, raw input, output). Outputs >= 16: 16 hammers, 17 bells, 18 crosses.
    Some(match job {
        9 => (BKind::Distiller, Some(SUGAR), RUM),
        10 => (BKind::Tobacconist, Some(TOBACCO), CIGARS),
        11 => (BKind::Weaver, Some(COTTON), CLOTH),
        12 => (BKind::FurTrader, Some(FURS), COATS),
        13 => (BKind::Carpenter, Some(LUMBER), 16),
        14 => (BKind::Blacksmith, Some(ORE), TOOLS),
        15 => (BKind::Armory, Some(TOOLS), MUSKETS),
        16 => (BKind::Church, None, 18),
        17 => (BKind::TownHall, None, 17),
        18 => (BKind::Schoolhouse, None, 19),
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Build {
    Building(BKind),
    Unit(UnitType),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Colony {
    pub id: Id,
    pub name: String,
    pub nation: u8,
    pub x: i16,
    pub y: i16,
    pub stock: [i32; 16],
    pub hammers: i32,
    pub levels: [u8; 17],
    pub building: Option<Build>,
    /// Liberty bells accumulated (200 per rebel colonist).
    pub liberty: i32,
}

impl Colony {
    pub fn level(&self, k: BKind) -> u8 {
        self.levels[k as usize]
    }
    pub fn capacity(&self) -> i32 {
        100 * (1 + self.level(BKind::Warehouse) as i32)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Village {
    pub x: i16,
    pub y: i16,
    pub tribe: u8,
    pub pop: u8,
    pub capital: bool,
    /// Bit per colonial power that has spoken with the chief.
    pub scouted: u8,
    /// Skill taught here (profession code); None once taught.
    pub teaches: Option<u8>,
    pub mission: Option<u8>,
    /// Gold carried, plundered on destruction.
    pub wealth: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tribe {
    pub tech: Tech,
    /// Tension towards each colonial power, 0..=100.
    pub alarm: [i32; 4],
    pub extinct: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub name: String,
    pub control: Control,
    pub gold: i32,
    pub tax: u8,
    pub bells: i32,
    pub crosses: i32,
    pub crosses_needed: i32,
    pub recruits: [u8; 3],
    pub prices: [u8; 16],
    pub volume: [i32; 16],
    pub entry: (i16, i16),
    pub artillery_bought: u8,
    pub colonies_founded: u16,
    pub at_war: [bool; 12],
    /// Ill will towards each colonial power, 0..=100.
    #[serde(default)]
    pub tension: [i32; 4],
    /// Turns spent at war with each colonial power.
    #[serde(default)]
    pub war_turns: [u16; 4],
    /// Goods boycotted in the homeland after a harbour protest.
    #[serde(default)]
    pub boycott: [bool; 16],
    #[serde(default)]
    pub assembly: Assembly,
}

/// Assembly progress for one player.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Assembly {
    /// Members who have joined (indices into the rules' `members`).
    pub members: Vec<u8>,
    /// The candidate the player is currently gathering bells for.
    pub candidate: Option<u8>,
    /// Bells towards the candidate.
    pub bells: i32,
    /// Candidates currently on offer.
    #[serde(default)]
    pub offered: Vec<u8>,
}

impl Assembly {
    pub fn has(&self, f: u8) -> bool {
        self.members.contains(&f)
    }
}

/// State of a declared rebellion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rebellion {
    pub nation: u8,
    pub declared_turn: u32,
    /// How many colonies the rebels held when they declared.
    pub colonies_at_start: usize,
    /// Liberty bells rung since the declaration (brings foreign intervention).
    #[serde(default)]
    pub bells: i32,
    /// Has a foreign power intervened yet?
    #[serde(default)]
    pub intervened: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub width: usize,
    pub height: usize,
    pub tiles: Vec<Tile>,
    /// Prime resource index per square (the rules' `resources`), 255 = none.
    pub resource: Vec<u8>,
    pub rumour: Vec<bool>,
    pub year: u16,
    pub autumn: bool,
    pub turn: u32,
    pub human: u8,
    pub difficulty: u8,
    pub players: Vec<Player>,
    pub units: Vec<Unit>,
    pub colonies: Vec<Colony>,
    pub villages: Vec<Village>,
    pub tribes: Vec<Tribe>,
    pub next_id: Id,
    pub rng: u64,
    /// Messages for the human player, shown and cleared by the UI.
    pub messages: Vec<String>,
    /// The Crown's army still in the homeland: regulars, cavalry, warships, artillery.
    #[serde(default)]
    pub crown_force: [i32; 4],
    #[serde(default)]
    pub rebellion: Option<Rebellion>,
    /// Set when the game has ended: (won, text).
    #[serde(default)]
    pub game_over: Option<(bool, String)>,
    /// Milestones already reached (bit n = milestone n).
    #[serde(default)]
    pub milestones: u32,
    /// Native peoples (bits 0-7) and colonial powers (bits 8-11) the human has met.
    #[serde(default)]
    pub met_tribes: u32,
    /// Story events waiting for the interface.
    #[serde(skip)]
    pub events: Vec<crate::events::Event>,
    #[serde(skip)]
    pub far_ocean_cache: Vec<bool>,
    /// Sounds waiting for the interface.
    #[serde(skip)]
    pub sounds: Vec<crate::sound::Cue>,
    /// Turn the last mercenary offer was made.
    #[serde(default)]
    pub mercenary_turn: u32,
    /// Set when the human paid for the land under the next colony.
    #[serde(skip)]
    pub land_settled: bool,
    /// A tax raise the human has not yet answered.
    #[serde(default)]
    pub tax_demand: Option<crate::taxes::TaxDemand>,
    /// Computer colonists sent out to found new colonies.
    #[serde(default)]
    pub ai_settlers: Vec<Id>,
    #[serde(skip)]
    pub rules: Rules,
}

/// The Crown's expeditionary army fights as its own nation.
pub const CROWN: u8 = 12;

pub fn is_colonist(kind: UnitType) -> bool {
    use UnitType::*;
    matches!(kind, Colonist | Soldier | Pioneer | Missionary | Dragoon | Scout)
}

pub fn is_native(kind: UnitType) -> bool {
    use UnitType::*;
    matches!(kind, Brave | ArmedBrave | MountedBrave | MountedWarrior)
}

impl Game {
    /// Scatter prime resources and rumours over the map.
    pub fn place_resources(&mut self, seed: u64) {
        let hash = |x: i32, y: i32, k: u64| -> u64 {
            let mut h = seed ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ k;
            h ^= h >> 31;
            h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
            h ^= h >> 29;
            h % 1000
        };
        for row in 1..self.height as i32 - 1 {
            for col in 1..self.width as i32 - 1 {
                let i = row as usize * self.width + col as usize;
                let t = self.tiles[i];
                let water = t.terrain().is_water();
                if t.raw & 0x1F == 26 {
                    continue;
                }
                if !water && hash(col, row, 1) < 30 {
                    self.rumour[i] = true;
                    continue;
                }
                if hash(col, row, 2) >= 110 {
                    continue;
                }
                if water {
                    let near_land = DIRS8.iter().any(|(dx, dy)| self.tile(col + dx, row + dy).is_some_and(|n| !n.terrain().is_water()));
                    if near_land {
                        self.resource[i] = 7;
                    }
                    continue;
                }
                self.resource[i] = resource_for(t);
            }
        }
    }

    pub fn resource_at(&self, x: i32, y: i32) -> Option<u8> {
        let i = self.index(x, y)?;
        let r = self.resource[i];
        (r != 255).then_some(r)
    }

    pub fn new_id(&mut self) -> Id {
        self.next_id += 1;
        self.next_id
    }

    pub fn rand(&mut self, n: u32) -> u32 {
        // xorshift64*
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        let v = self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D);
        if n == 0 { 0 } else { ((v >> 33) % n as u64) as u32 }
    }

    pub fn chance(&mut self, percent: u32) -> bool {
        self.rand(100) < percent
    }

    pub fn msg(&mut self, s: impl Into<String>) {
        self.messages.push(s.into());
    }

    // ----- map helpers ----------------------------------------------------

    pub fn index(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height)
            .then(|| y as usize * self.width + x as usize)
    }

    pub fn tile(&self, x: i32, y: i32) -> Option<Tile> {
        self.index(x, y).map(|i| self.tiles[i])
    }

    pub fn is_water(&self, x: i32, y: i32) -> bool {
        self.tile(x, y).is_none_or(|t| t.terrain().is_water())
    }

    pub fn is_coastal(&self, x: i32, y: i32) -> bool {
        DIRS8.iter().any(|(dx, dy)| self.tile(x + dx, y + dy).is_some_and(|t| t.terrain().is_water()))
    }

    /// Index into rules.terrain for a square.
    pub fn terrain_row(&self, t: Tile) -> usize {
        if t.mountains() {
            19
        } else if t.hills() {
            20
        } else {
            match t.terrain() {
                Terrain::Arctic => 16,
                Terrain::Ocean => 17,
                Terrain::SeaLane => 18,
                other => other as usize + if t.forested() { 8 } else { 0 },
            }
        }
    }

    pub fn terrain_name(&self, x: i32, y: i32) -> String {
        let Some(t) = self.tile(x, y) else { return String::new() };
        let mut s = t.describe();
        if let Some(r) = self.resource_at(x, y)
            && let Some(n) = self.rules.resources.get(r as usize) {
                s = format!("{s}, {n}");
            }
        if self.rumour.get(self.index(x, y).unwrap_or(0)).copied().unwrap_or(false) {
            s.push_str(", Rumours");
        }
        s
    }

    pub fn reveal(&mut self, nation: u8, x: i32, y: i32, radius: i32) {
        if nation > 3 {
            return;
        }
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if let Some(i) = self.index(x + dx, y + dy) {
                    self.tiles[i].seen |= 0x10 << nation;
                }
            }
        }
    }

    pub fn visible(&self, nation: u8, x: i32, y: i32) -> bool {
        self.tile(x, y).is_some_and(|t| t.visible_to(nation))
    }

    // ----- lookups --------------------------------------------------------

    pub fn idx(&self, id: Id) -> Option<usize> {
        self.units.iter().position(|u| u.id == id)
    }

    pub fn unit(&self, id: Id) -> Option<&Unit> {
        self.units.iter().find(|u| u.id == id)
    }

    pub fn unit_mut(&mut self, id: Id) -> Option<&mut Unit> {
        self.units.iter_mut().find(|u| u.id == id)
    }

    pub fn colony(&self, id: Id) -> Option<&Colony> {
        self.colonies.iter().find(|c| c.id == id)
    }

    pub fn colony_mut(&mut self, id: Id) -> Option<&mut Colony> {
        self.colonies.iter_mut().find(|c| c.id == id)
    }

    pub fn colony_at(&self, x: i32, y: i32) -> Option<&Colony> {
        self.colonies.iter().find(|c| c.x as i32 == x && c.y as i32 == y)
    }

    pub fn village_at(&self, x: i32, y: i32) -> Option<usize> {
        self.villages.iter().position(|v| v.x as i32 == x && v.y as i32 == y)
    }

    pub fn units_at(&self, x: i32, y: i32) -> impl Iterator<Item = &Unit> {
        self.units.iter().filter(move |u| u.loc == Loc::Map { x: x as i16, y: y as i16 })
    }

    pub fn unit_pos(&self, id: Id) -> Option<(i32, i32)> {
        let u = self.unit(id)?;
        match u.loc {
            Loc::Map { x, y } => Some((x as i32, y as i32)),
            Loc::Aboard(s) => self.unit_pos(s),
            Loc::Colony { colony, .. } => self.colony(colony).map(|c| (c.x as i32, c.y as i32)),
            _ => None,
        }
    }

    pub fn aboard(&self, ship: Id) -> impl Iterator<Item = &Unit> {
        self.units.iter().filter(move |u| u.loc == Loc::Aboard(ship))
    }

    pub fn workers(&self, colony: Id) -> impl Iterator<Item = &Unit> {
        self.units.iter().filter(move |u| matches!(u.loc, Loc::Colony { colony: c, .. } if c == colony))
    }

    pub fn population(&self, colony: Id) -> usize {
        self.workers(colony).count()
    }

    pub fn tribe_tech(&self, tribe: u8) -> Tech {
        self.tribes.get(tribe.saturating_sub(4) as usize).map(|t| t.tech).unwrap_or(Tech::SemiNomadic)
    }

    pub fn max_moves(&self, kind: UnitType) -> u8 {
        self.rules.units.get(kind as usize).map(|r| r.movement * 3).unwrap_or(3)
    }

    pub fn holds(&self, ship: Id) -> usize {
        self.unit(ship).and_then(|u| self.rules.units.get(u.kind as usize)).map(|r| r.holds as usize).unwrap_or(0)
    }

    pub fn holds_used(&self, ship: Id) -> usize {
        let goods = self.unit(ship).map(|u| u.cargo.len()).unwrap_or(0);
        goods + self.aboard(ship).count()
    }

    /// Display name of a nation (0..4 powers, 4..12 native peoples, 12 the Crown's army).
    pub fn nation_name(&self, n: u8) -> &str {
        self.rules.names.nations.get(n.min(12) as usize).map(|s| s.as_str()).unwrap_or("")
    }
    pub fn adjective(&self, n: u8) -> &str {
        self.rules.names.adjectives.get(n as usize).map(|s| s.as_str()).unwrap_or("")
    }
    pub fn independent_name(&self, n: u8) -> &str {
        self.rules.names.independent.get(n as usize).map(|s| s.as_str()).unwrap_or("new nation")
    }
    pub fn kind_name(&self, k: UnitType) -> &str {
        self.rules.names.units.get(k as usize).map(|s| s.as_str()).unwrap_or(k.label())
    }
    pub fn prof_name(&self, p: u8) -> &str {
        self.rules.names.professions.get(p as usize).map(|s| s.as_str()).unwrap_or("Free Colonist")
    }

    pub fn unit_name(&self, u: &Unit) -> String {
        let base = self.kind_name(u.kind);
        if is_colonist(u.kind) {
            let p = self.prof_name(u.prof);
            if u.prof == P_FREE || u.prof == P_FREE_OLD || p == base {
                if u.kind == UnitType::Colonist { "Free Colonist".into() } else { base.into() }
            } else if u.kind == UnitType::Colonist {
                // an expert whose trade needs equipment, without it
                match u.prof {
                    P_VETERAN | P_VETERAN_DRAGOON => format!("{p} (unarmed)"),
                    P_HARDY_PIONEER => format!("{p} (no tools)"),
                    P_SEASONED_SCOUT => format!("{p} (no horses)"),
                    _ => p.into(),
                }
            } else {
                format!("{base} ({p})")
            }
        } else if u.kind == UnitType::Treasure {
            format!("Treasure ({} gold)", u.treasure)
        } else if u.repair > 0 {
            format!("{base} (repairs: {} turns)", u.repair)
        } else {
            base.into()
        }
    }

    pub fn village_skill(&self, x: i32, y: i32, tribe: u8) -> u8 {
        let t = self.tile(x, y).unwrap_or(Tile { raw: 0, mask: 0, path: 0, seen: 0 });
        let tech = self.tribe_tech(tribe);
        if t.mountains() {
            return 7;
        }
        if t.hills() {
            return 6;
        }
        if self.is_coastal(x, y) && (x + y) % 3 == 0 {
            return P_FISHERMAN;
        }
        if t.forested() {
            return if (x * 7 + y) % 2 == 0 { 4 } else { 5 };
        }
        match (t.terrain(), tech) {
            (_, Tech::Civilized) if (x + y) % 2 == 0 => 7,
            (Terrain::Grassland, _) => 2,
            (Terrain::Prairie, _) => 3,
            (Terrain::Savannah | Terrain::Swamp, _) => 1,
            (Terrain::Tundra | Terrain::Marsh, _) => 6,
            _ => P_EXPERT_FARMER,
        }
    }

    // ----- turn bookkeeping ------------------------------------------------

    pub fn year_label(&self) -> String {
        if self.year >= 1600 {
            format!("{} {}", if self.autumn { "Autumn" } else { "Spring" }, self.year)
        } else {
            format!("{}", self.year)
        }
    }

    pub fn human_player(&self) -> &Player {
        &self.players[self.human as usize]
    }

    pub fn human_player_mut(&mut self) -> &mut Player {
        let h = self.human as usize;
        &mut self.players[h]
    }

    /// Next unit of the human player that still needs orders.
    pub fn next_active(&self, after: Option<Id>) -> Option<Id> {
        let cands: Vec<&Unit> = self
            .units
            .iter()
            .filter(|u| {
                u.nation == self.human
                    && matches!(u.loc, Loc::Map { .. })
                    && u.moves > 0
                    && !u.done
                    && u.orders == Orders::None
                    && u.kind != UnitType::Treasure
            })
            .collect();
        if cands.is_empty() {
            return None;
        }
        let start = after.and_then(|a| cands.iter().position(|u| u.id == a)).map(|p| p + 1).unwrap_or(0);
        Some(cands[start % cands.len()].id)
    }

    pub fn kill(&mut self, id: Id) {
        // anything aboard goes down too
        let aboard: Vec<Id> = self.aboard(id).map(|u| u.id).collect();
        for a in aboard {
            self.kill(a);
        }
        if let Some(u) = self.unit_mut(id) {
            u.loc = Loc::Dead;
        }
    }

    pub fn purge_dead(&mut self) {
        self.units.retain(|u| u.loc != Loc::Dead);
    }

    pub fn spawn(&mut self, kind: UnitType, nation: u8, prof: u8, loc: Loc) -> Id {
        let id = self.new_id();
        let tools = if kind == UnitType::Pioneer { 100 } else { 0 };
        self.units.push(Unit {
            id,
            kind,
            nation,
            prof,
            loc,
            moves: 0,
            orders: Orders::None,
            cargo: Vec::new(),
            tools,
            treasure: 0,
            damaged: false,
            done: false,
            sea_exit: None,
            teach: 0,
            route: None,
            repair: 0,
            plan: Vec::new(),
            then_home: false,
            arrive_at: None,
        });
        id
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(s: &str, rules: Rules) -> Result<Self, String> {
        let mut g: Game = serde_json::from_str(s).map_err(|e| e.to_string())?;
        g.rules = rules;
        Ok(g)
    }
}

/// Which prime resource (index into the rules' `resources`) a square gets.
fn resource_for(t: Tile) -> u8 {
    if t.mountains() {
        return 12; // silver
    }
    if t.hills() {
        return 13; // ore
    }
    let forest = t.forested();
    match t.terrain() {
        Terrain::Tundra => if forest { 8 } else { 6 },
        Terrain::Desert => if forest { 9 } else { 1 },
        Terrain::Plains => if forest { 8 } else { 2 },
        Terrain::Prairie => if forest { 9 } else { 3 },
        Terrain::Grassland => if forest { 10 } else { 4 },
        Terrain::Savannah => if forest { 9 } else { 5 },
        Terrain::Marsh => 6,
        Terrain::Swamp => if forest { 10 } else { 6 },
        _ => 255,
    }
}

/// Good boosted by each resource and by how much.
pub fn resource_bonus(r: u8) -> Option<(usize, i32)> {
    Some(match r {
        1 | 2 => (FOOD, 2),
        3 => (COTTON, 3),
        4 => (TOBACCO, 3),
        5 => (SUGAR, 3),
        6 => (ORE, 2),
        7 => (FOOD, 4),
        8 | 9 => (FURS, 3),
        10 | 11 => (LUMBER, 4),
        12 => (SILVER, 4),
        13 => (ORE, 3),
        _ => return None,
    })
}
