//! Basic value types shared by the whole engine: unit kinds, terrain, map squares.

use serde::{Deserialize, Serialize};

/// Colonial powers 0..=3, native peoples 4..=11, the Crown's army 12.
pub type NationId = u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Control {
    Human,
    Ai,
    Withdrawn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum UnitType {
    Colonist, Soldier, Pioneer, Missionary, Dragoon, Scout, CrownRegular,
    PatriotCavalry, CrownCavalry, PatriotArmy, Treasure, Artillery,
    WagonTrain, Caravel, Merchantman, Galleon, Privateer, Frigate, ShipOfTheLine,
    Brave, ArmedBrave, MountedBrave, MountedWarrior,
}

impl UnitType {
    pub const COUNT: u8 = 23;

    pub fn from_u8(v: u8) -> Option<Self> {
        if v < Self::COUNT {
            // SAFETY: repr(u8), variants are contiguous from 0
            Some(unsafe { std::mem::transmute::<u8, UnitType>(v) })
        } else {
            None
        }
    }
    /// A plain name used only when the rules file doesn't name the unit.
    pub fn label(self) -> &'static str {
        [
            "Colonist", "Soldier", "Pioneer", "Missionary", "Dragoon", "Scout",
            "Crown Regular", "Patriot Cavalry", "Crown Cavalry", "Patriot Army",
            "Treasure", "Artillery", "Wagon Train", "Caravel", "Merchantman", "Galleon",
            "Privateer", "Frigate", "Ship of the Line", "Brave", "Armed Brave", "Mounted Brave",
            "Mounted Warrior",
        ][self as usize]
    }
    pub fn is_ship(self) -> bool {
        matches!(
            self,
            UnitType::Caravel | UnitType::Merchantman | UnitType::Galleon
                | UnitType::Privateer | UnitType::Frigate | UnitType::ShipOfTheLine
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tech {
    SemiNomadic,
    Agrarian,
    Advanced,
    Civilized,
}

/// Terrain base types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Terrain {
    Tundra, Desert, Plains, Prairie, Grassland, Savannah, Marsh, Swamp,
    Arctic, Ocean, SeaLane,
}

impl Terrain {
    pub fn name(self) -> &'static str {
        [
            "Tundra", "Desert", "Plains", "Prairie", "Grassland", "Savannah", "Marsh", "Swamp",
            "Arctic", "Ocean", "Sea Lane",
        ][self as usize]
    }
    pub fn forest_name(self) -> &'static str {
        match self {
            Terrain::Tundra => "Boreal Forest",
            Terrain::Desert => "Scrub Forest",
            Terrain::Plains => "Mixed Forest",
            Terrain::Prairie => "Broadleaf Forest",
            Terrain::Grassland => "Conifer Forest",
            Terrain::Savannah => "Tropical Forest",
            Terrain::Marsh => "Wetland Forest",
            Terrain::Swamp => "Rain Forest",
            other => other.name(),
        }
    }
    pub fn is_water(self) -> bool {
        matches!(self, Terrain::Ocean | Terrain::SeaLane)
    }
}

/// One map square, packed into four bytes.
///
/// `raw`: bits 0-4 terrain (0-7 open land, 8-23 forest, 24 arctic, 25 ocean,
/// 26 sea lane), bits 5-7 relief and rivers (1 hills, 2 river, 3 hills with
/// river, 5 mountains, 6 major river, 7 mountains with major river).
/// `mask`: 0x08 road, 0x20 far ocean, 0x40 plowed. `seen`: bit 4+n set once
/// power n has explored the square.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Tile {
    pub raw: u8,
    pub mask: u8,
    pub path: u8,
    pub seen: u8,
}

impl Tile {
    pub fn terrain(self) -> Terrain {
        match self.raw & 0x1F {
            t @ 0..=7 => base8(t),
            t @ 8..=15 => base8(t - 8),
            t @ 16..=23 => base8(t - 16),
            24 => Terrain::Arctic,
            26 => Terrain::SeaLane,
            _ => Terrain::Ocean,
        }
    }
    pub fn forested(self) -> bool {
        (8..24).contains(&(self.raw & 0x1F))
    }
    fn relief(self) -> u8 {
        self.raw >> 5
    }
    pub fn hills(self) -> bool {
        matches!(self.relief(), 1 | 3)
    }
    pub fn mountains(self) -> bool {
        matches!(self.relief(), 5 | 7)
    }
    pub fn minor_river(self) -> bool {
        matches!(self.relief(), 2 | 3)
    }
    pub fn major_river(self) -> bool {
        matches!(self.relief(), 6 | 7)
    }
    pub fn river(self) -> bool {
        self.minor_river() || self.major_river()
    }
    pub fn road(self) -> bool {
        self.mask & 0x08 != 0
    }
    pub fn plowed(self) -> bool {
        self.mask & 0x40 != 0
    }
    /// Part of the far ocean on the other side of the continent.
    pub fn far_ocean(self) -> bool {
        self.mask & 0x20 != 0
    }
    pub fn visible_to(self, nation: NationId) -> bool {
        nation < 4 && self.seen & (0x10 << nation) != 0
    }
    pub fn describe(self) -> String {
        let t = self.terrain();
        let mut s = if self.mountains() {
            "Mountains".to_string()
        } else if self.hills() {
            "Hills".to_string()
        } else if self.forested() {
            t.forest_name().to_string()
        } else {
            t.name().to_string()
        };
        if self.major_river() {
            s.push_str(", major river");
        } else if self.minor_river() {
            s.push_str(", river");
        }
        if self.road() {
            s.push_str(", road");
        }
        if self.plowed() {
            s.push_str(", plowed");
        }
        s
    }
}

fn base8(t: u8) -> Terrain {
    [
        Terrain::Tundra, Terrain::Desert, Terrain::Plains, Terrain::Prairie,
        Terrain::Grassland, Terrain::Savannah, Terrain::Marsh, Terrain::Swamp,
    ][t as usize]
}
