//! Story events for the interface: milestones, native leaders speaking, the
//! Crown's announcements. The engine queues them; the interface shows them.

use crate::game::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A milestone reached for the first time (one of the MS_ constants).
    Milestone(usize),
    /// A native leader speaks: tribe index (0-7), pose (0 calm, 1 pleased, 2 talking, 3 hostile).
    Chief { tribe: usize, pose: usize, text: String },
    /// The Crown addresses the player.
    Crown { text: String },
    /// The Crown's tax raise awaits an answer (see `Game::tax_demand`).
    TaxDemand,
    /// Natives offer to buy goods from a colony.
    NativeTrade { tribe: usize, colony: Id, good: u8, amount: i32, price: i32 },
    /// An angry tribe demands goods (or gold if `good` is None) from a colony.
    NativeDemand { tribe: usize, colony: Id, good: Option<u8>, amount: i32 },
    /// Mercenaries offer their services for a price.
    Mercenaries { price: i32 },
    /// A battle the human took part in, for the combat display.
    Battle(Battle),
    /// A rival colonial power offers the human peace.
    PeaceOffer { nation: u8 },
    /// The player declared independence.
    Declared,
    /// A new member joined the human player's assembly.
    MemberJoined(usize),
    /// The war of independence ended (won?).
    WarOver { won: bool },
}

/// Who fought whom, at what odds, and who won.
#[derive(Debug, Clone, PartialEq)]
pub struct Battle {
    pub attacker: (crate::types::UnitType, u8, u8),
    /// None when colonists or a village defend without a unit.
    pub defender: Option<(crate::types::UnitType, u8, u8)>,
    pub defender_name: String,
    pub attack: f32,
    pub defence: f32,
    pub won: bool,
}

pub const MS_NEW_WORLD: usize = 1;
pub const MS_DISCOVERY: usize = 2;
pub const MS_COLONY: usize = 3;
pub const MS_NATIVES: usize = 4;
pub const MS_LEAGUE: usize = 5;
pub const MS_SUN_ROAD: usize = 6;
pub const MS_FAR_OCEAN: usize = 7;
pub const MS_VILLAGE: usize = 8;
pub const MS_SETTLERS: usize = 9;
pub const MS_CARGO: usize = 10;
pub const MS_RIVALS: usize = 11;
pub const MS_BURNING: usize = 12;
pub const MS_DESTROYED: usize = 13;
pub const MS_RAID: usize = 14;

impl Game {
    pub fn event(&mut self, e: Event) {
        self.events.push(e);
    }

    /// Queue a milestone event the first time it is reached.
    pub fn milestone(&mut self, n: usize) -> bool {
        let bit = 1u32 << n;
        if self.milestones & bit == 0 {
            self.milestones |= bit;
            self.events.push(Event::Milestone(n));
            true
        } else {
            false
        }
    }

    /// A chief speaks to the human player (others just get the log line).
    pub fn chief_says(&mut self, nation: u8, tribe: usize, pose: usize, text: String) {
        if nation == self.human {
            self.events.push(Event::Chief { tribe, pose, text });
        } else {
            self.msg(text);
        }
    }

    /// Water reachable from the western map edge without leaving the western
    /// half: the far ocean.
    fn far_ocean(&mut self) -> &Vec<bool> {
        if self.far_ocean_cache.len() != self.tiles.len() {
            let (w, h) = (self.width as i32, self.height as i32);
            let mut seen = vec![false; self.tiles.len()];
            let mut stack: Vec<(i32, i32)> = (2..h - 2).map(|y| (0, y)).collect();
            while let Some((x, y)) = stack.pop() {
                if x < 0 || y < 2 || y >= h - 2 || x >= w / 2 || !self.is_water(x, y) {
                    continue;
                }
                let i = (y * w + x) as usize;
                if seen[i] {
                    continue;
                }
                seen[i] = true;
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    stack.push((x + dx, y + dy));
                }
            }
            self.far_ocean_cache = seen;
        }
        &self.far_ocean_cache
    }

    /// Look around the human player's units for first contacts.
    pub fn scan_contacts(&mut self) {
        let h = self.human;
        if self.game_over.is_some() {
            return;
        }
        let spots: Vec<(i32, i32, bool)> = self
            .units
            .iter()
            .filter(|u| u.nation == h)
            .filter_map(|u| match u.loc {
                Loc::Map { x, y } => Some((x as i32, y as i32, u.kind.is_ship())),
                _ => None,
            })
            .chain(self.colonies.iter().filter(|c| c.nation == h).map(|c| (c.x as i32, c.y as i32, false)))
            .collect();
        if spots.is_empty() {
            return;
        }
        let w = self.width as i32;
        let mut ocean_seen = self.milestones & (1 << MS_FAR_OCEAN) != 0;
        for (x, y, ship) in spots {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if self.tile(nx, ny).is_none() {
                        continue;
                    }
                    // natives
                    let tribe = self
                        .village_at(nx, ny)
                        .map(|vi| self.villages[vi].tribe)
                        .or_else(|| self.units_at(nx, ny).find(|u| (4..12).contains(&u.nation)).map(|u| u.nation));
                    if let Some(t) = tribe {
                        let ti = t.saturating_sub(4) as usize;
                        if ti < 8 && self.met_tribes & (1 << ti) == 0 {
                            self.met_tribes |= 1 << ti;
                            self.meet_tribe(ti);
                        }
                    }
                    // rival colonial powers
                    let other = self
                        .units_at(nx, ny)
                        .find(|u| u.nation < 4 && u.nation != h)
                        .map(|u| u.nation)
                        .or_else(|| self.colony_at(nx, ny).filter(|c| c.nation < 4 && c.nation != h).map(|c| c.nation));
                    if let Some(n) = other {
                        let bit = 1u32 << (8 + n);
                        if self.met_tribes & bit == 0 {
                            self.met_tribes |= bit;
                            self.milestone(MS_RIVALS);
                            self.msg(format!("Your men meet the {}.", self.adjective(n)));
                        }
                    }
                    if !ocean_seen && (ship || dx != 0 || dy != 0) {
                        ocean_seen = self.milestones & (1 << MS_FAR_OCEAN) != 0;
                        let i = (ny * w + nx) as usize;
                        if self.far_ocean().get(i).copied().unwrap_or(false) && self.milestone(MS_FAR_OCEAN) {
                            { let o = self.rules.names.far_ocean.clone(); self.msg(format!("Your explorers have discovered the {o}!")); }
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn meet_tribe(&mut self, ti: usize) {
        let h = self.human;
        self.milestone(MS_NATIVES);
        match ti {
            0 => {
                self.milestone(MS_SUN_ROAD);
            }
            1 => {
                self.milestone(MS_LEAGUE);
            }
            _ => {}
        }
        let tribe = self.nation_name(4 + ti as u8).to_string();
        let ruler = self.rules.names.rulers.get(ti).cloned().unwrap_or_default();
        let villages = self.villages.iter().filter(|v| v.tribe as usize == 4 + ti).count();
        let tech = self.tribe_tech(4 + ti as u8) as usize;
        let greeting = match tech {
            3 if !ruler.is_empty() => format!(
                "I am {ruler}. The {tribe} rule {villages} cities in these lands. Strangers from {} may trade with us, if they remember whose land this is.",
                self.nation_name(h)
            ),
            2 | 3 => format!(
                "Strangers from across the sea, you stand before the {tribe}, masters of {villages} cities. Bring us your goods, and we shall see whether you are friends.",
            ),
            _ => format!(
                "Welcome, strangers from {}. We are the {tribe}, a people of {villages} villages. The land is wide; let us live in peace and trade as friends.",
                self.nation_name(h)
            ),
        };
        let pose = if tech >= 2 { 0 } else { 1 };
        self.events.push(Event::Chief { tribe: ti, pose, text: greeting });
    }
}
