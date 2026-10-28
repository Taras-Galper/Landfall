//! War at sea: ships attacking ships, evasion, damage, sinking, plunder,
//! and repairs at a drydock or in the homeland.

use crate::colony::GOOD_NAMES;
use crate::game::*;
use crate::types::UnitType;

/// Ships built to fight.
pub fn is_warship(k: UnitType) -> bool {
    matches!(k, UnitType::Privateer | UnitType::Frigate | UnitType::ShipOfTheLine)
}

/// The outcome of a sea battle, for the interface.
#[derive(Debug, Clone, PartialEq)]
pub enum SeaResult {
    Evaded,
    Damaged { to: String },
    Sunk,
    AttackerDamaged { to: String },
    AttackerSunk,
}

impl Game {
    fn ship_label(&self, id: Id) -> String {
        match self.unit(id) {
            Some(u) => format!("{} {}", self.nation_name(u.nation), self.kind_name(u.kind)),
            None => "ship".into(),
        }
    }

    /// A ship attacks whatever ship is defending (x, y).
    pub fn naval_battle(&mut self, attacker: Id, x: i32, y: i32) -> Option<SeaResult> {
        let a = self.unit(attacker)?.clone();
        // the strongest ship on the square defends
        let defender = self
            .units_at(x, y)
            .filter(|u| u.kind.is_ship() && u.nation != a.nation)
            .max_by(|p, q| {
                self.defence_strength(p, x, y).partial_cmp(&self.defence_strength(q, x, y)).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|u| u.id)?;
        let d = self.unit(defender)?.clone();
        if let Some(am) = self.unit_mut(attacker) {
            am.moves = 0;
            am.orders = Orders::None;
        }
        // privateers sail under no flag: no war is declared
        if a.kind != UnitType::Privateer && a.nation < 4 && d.nation < 4 {
            self.declare_war(a.nation, d.nation);
        }
        let involved = a.nation == self.human || d.nation == self.human;
        let a_name = if a.kind == UnitType::Privateer && d.nation == self.human && a.nation != self.human {
            "A privateer".to_string()
        } else {
            self.ship_label(attacker)
        };
        let d_name = self.ship_label(defender);
        crate::sound::cue(self, crate::sound::Cue::Cannon, involved);

        // merchant ships try to slip away
        if !is_warship(d.kind) {
            let evade = 25 + 5 * (self.max_moves(d.kind) as i32 / 3) - 5 * (self.max_moves(a.kind) as i32 / 3);
            if self.chance(evade.clamp(10, 50) as u32) {
                if involved {
                    self.msg(format!("The {d_name} evades the {a_name}."));
                }
                return Some(SeaResult::Evaded);
            }
        }
        let att = self.attack_strength(&a);
        let def = self.defence_strength(&d, x, y);
        let win = self.rand(1000) as f32 / 1000.0 < att / (att + def);
        if involved {
            self.events.push(crate::events::Event::Battle(crate::events::Battle {
                attacker: (a.kind, a.nation, a.prof),
                defender: Some((d.kind, d.nation, d.prof)),
                defender_name: d_name.clone(),
                attack: att,
                defence: def,
                won: win,
            }));
        }
        let (winner, loser) = if win { (attacker, defender) } else { (defender, attacker) };
        // the winner plunders what it can carry
        let plunder = self.unit(loser).map(|u| u.cargo.clone()).unwrap_or_default();
        let mut taken = Vec::new();
        for (g, n) in plunder {
            if self.holds_used(winner) < self.holds(winner) && self.unit(winner).is_some_and(|w| is_warship(w.kind)) {
                self.unit_mut(winner).unwrap().cargo.push((g, n));
                taken.push(format!("{n} {}", GOOD_NAMES[g as usize]));
            }
        }
        let odds = if win { att / (att + def) } else { def / (att + def) };
        let sink = self.rand(1000) as f32 / 1000.0 < 0.25 + 0.3 * odds;
        let outcome = self.wreck(loser, sink);
        if involved {
            let line = match (&outcome, win) {
                (None, true) => format!("The {a_name} sinks the {d_name}!"),
                (None, false) => format!("The {a_name} is sunk by the {d_name}!"),
                (Some(to), true) => format!("The {a_name} damages the {d_name}. It limps to {to} for repairs."),
                (Some(to), false) => format!("The {d_name} beats off the {a_name}, which limps to {to} for repairs."),
            };
            self.msg(line);
            if !taken.is_empty() {
                self.msg(format!("Cargo seized: {}.", taken.join(", ")));
            }
        }
        if win {
            self.purge_dead();
            Some(match outcome {
                None => SeaResult::Sunk,
                Some(to) => SeaResult::Damaged { to },
            })
        } else {
            self.purge_dead();
            Some(match outcome {
                None => SeaResult::AttackerSunk,
                Some(to) => SeaResult::AttackerDamaged { to },
            })
        }
    }

    /// A ship loses a battle: cargo and passengers are lost; the ship sinks or
    /// goes for repairs. Returns where it went, or None if it sank.
    fn wreck(&mut self, id: Id, sink: bool) -> Option<String> {
        let passengers: Vec<Id> = self.aboard(id).map(|u| u.id).collect();
        for p in passengers {
            self.kill(p);
        }
        let nation = self.unit(id)?.nation;
        if let Some(u) = self.unit_mut(id) {
            u.cargo.clear();
            u.route = None;
            u.orders = Orders::None;
        }
        if sink {
            self.kill(id);
            return None;
        }
        let (sx, sy) = self.unit_pos(id).unwrap_or((0, 0));
        // nearest colony of ours with a drydock
        let dock = self
            .colonies
            .iter()
            .filter(|c| c.nation == nation && c.level(BKind::Docks) >= 2)
            .min_by_key(|c| (c.x as i32 - sx).abs().max((c.y as i32 - sy).abs()))
            .map(|c| (c.x, c.y, c.name.clone()));
        let rebel = self.rebellion.as_ref().is_some_and(|r| r.nation == nation);
        match dock {
            Some((x, y, name)) => {
                let u = self.unit_mut(id)?;
                u.loc = Loc::Map { x, y };
                u.damaged = true;
                u.repair = 3;
                u.moves = 0;
                Some(name)
            }
            None if nation < 4 && !rebel => {
                let u = self.unit_mut(id)?;
                u.loc = Loc::Homeland;
                u.damaged = true;
                u.repair = 6;
                Some(self.rules.names.homeland.clone())
            }
            None => {
                self.kill(id);
                None
            }
        }
    }

    /// Repairs progress each turn.
    pub(crate) fn repair_ships(&mut self) {
        let mut done = Vec::new();
        for u in &mut self.units {
            if u.repair > 0 {
                u.repair -= 1;
                u.moves = 0;
                if u.repair == 0 {
                    u.damaged = false;
                    done.push((u.nation, u.kind));
                }
            }
        }
        for (n, k) in done {
            if n == self.human {
                self.msg(format!("Your {} has been repaired.", self.kind_name(k)));
            }
        }
    }

    /// Computer warships look for a fight with an enemy ship next to them.
    pub(crate) fn ai_naval(&mut self, n: u8) {
        let ships: Vec<Id> = self
            .units
            .iter()
            .filter(|u| u.nation == n && is_warship(u.kind) && u.repair == 0 && matches!(u.loc, Loc::Map { .. }))
            .map(|u| u.id)
            .collect();
        for sid in ships {
            let Some((x, y)) = self.unit_pos(sid) else { continue };
            let kind = self.unit(sid).map(|u| u.kind).unwrap();
            let mut target = None;
            for (dx, dy) in DIRS8 {
                let (nx, ny) = (x + dx, y + dy);
                if !self.is_water(nx, ny) {
                    continue;
                }
                let enemy = self.units_at(nx, ny).find(|u| {
                    u.kind.is_ship()
                        && u.nation != n
                        && u.nation < 13
                        && (self.players.get(n as usize).is_some_and(|p| p.at_war.get(u.nation as usize).copied().unwrap_or(false))
                            || (kind == UnitType::Privateer && u.nation < 4 && !is_warship(u.kind))
                            || (n == CROWN || u.nation == CROWN))
                });
                if enemy.is_some() {
                    target = Some((nx, ny));
                    break;
                }
            }
            if let Some((tx, ty)) = target {
                // privateers pick on lone merchants only now and then
                if kind == UnitType::Privateer && !self.chance(35) {
                    continue;
                }
                self.naval_battle(sid, tx, ty);
            }
        }
    }
}
