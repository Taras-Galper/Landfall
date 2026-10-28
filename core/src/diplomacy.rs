//! Relations between the colonial powers: war and peace, tension, the
//! Crown's wars, peace offers, and computer armies on the march.

use crate::events::Event;
use crate::game::*;
use crate::types::UnitType;

impl Game {
    pub fn at_war(&self, a: u8, b: u8) -> bool {
        a != b && self.players.get(a as usize).is_some_and(|p| p.at_war.get(b as usize).copied().unwrap_or(false))
    }

    pub fn declare_war(&mut self, a: u8, b: u8) {
        if a >= 4 || b >= 4 || a == b || self.at_war(a, b) {
            return;
        }
        self.players[a as usize].at_war[b as usize] = true;
        self.players[b as usize].at_war[a as usize] = true;
        self.players[a as usize].war_turns[b as usize] = 0;
        self.players[b as usize].war_turns[a as usize] = 0;
        if a == self.human || b == self.human || self.knows(self.human, a) || self.knows(self.human, b) {
            self.msg(format!("WAR! The {} declare war on the {}.", self.adjective(a), self.adjective(b)));
        }
        crate::sound::cue(self, crate::sound::Cue::Drums, a == self.human || b == self.human);
    }

    pub fn make_peace(&mut self, a: u8, b: u8) {
        if a >= 4 || b >= 4 || !self.at_war(a, b) {
            return;
        }
        self.players[a as usize].at_war[b as usize] = false;
        self.players[b as usize].at_war[a as usize] = false;
        self.players[a as usize].tension[b as usize] = 20;
        self.players[b as usize].tension[a as usize] = 20;
        if a == self.human || b == self.human || self.knows(self.human, a) || self.knows(self.human, b) {
            self.msg(format!("PEACE: the {} and the {} sign a peace treaty.", self.adjective(a), self.adjective(b)));
        }
    }

    /// Has the human met this colonial power?
    pub fn knows(&self, h: u8, n: u8) -> bool {
        h == n || (n < 4 && self.met_tribes & (1 << (8 + n)) != 0)
    }

    /// Military strength of a colonial power on this side of the ocean.
    pub fn military_strength(&self, n: u8) -> i32 {
        self.units
            .iter()
            .filter(|u| u.nation == n && matches!(u.loc, Loc::Map { .. } | Loc::Colony { .. }))
            .map(|u| match u.kind {
                UnitType::Soldier | UnitType::PatriotArmy => 2,
                UnitType::Dragoon | UnitType::PatriotCavalry | UnitType::Artillery => 3,
                UnitType::Frigate | UnitType::Privateer => 2,
                UnitType::ShipOfTheLine => 4,
                _ => 0,
            })
            .sum()
    }

    /// The human asks a computer player for peace. True if it accepts.
    pub fn propose_peace(&mut self, from: u8, to: u8) -> bool {
        if !self.at_war(from, to) {
            return true;
        }
        let tension = self.players[to as usize].tension[from as usize];
        let mine = self.military_strength(to) + 1;
        let theirs = self.military_strength(from) + 1;
        let weary = self.players[to as usize].war_turns[from as usize] as i32;
        let odds = 30 + weary * 2 + (theirs - mine) * 4 - tension / 3;
        if self.chance(odds.clamp(5, 90) as u32) {
            self.make_peace(from, to);
            true
        } else {
            false
        }
    }

    /// Once a turn: tension rises with crowding, wars are declared and ended.
    pub fn diplomacy_turn(&mut self) {
        let rebel = self.rebellion.as_ref().map(|r| r.nation);
        // crowding: colonies close to another power's colonies
        for a in 0..4u8 {
            for b in 0..4u8 {
                if a == b {
                    continue;
                }
                let close = self
                    .colonies
                    .iter()
                    .filter(|c| c.nation == a)
                    .filter(|c| {
                        self.colonies
                            .iter()
                            .any(|d| d.nation == b && (d.x - c.x).abs().max((d.y - c.y).abs()) <= 4)
                    })
                    .count() as i32;
                let t = &mut self.players[a as usize].tension[b as usize];
                *t = (*t + close - 1).clamp(0, 100);
                if self.players[a as usize].at_war[b as usize] {
                    self.players[a as usize].war_turns[b as usize] = self.players[a as usize].war_turns[b as usize].saturating_add(1);
                }
            }
        }
        // the computer players decide
        for a in 0..4u8 {
            if a == self.human || self.players[a as usize].control == crate::types::Control::Withdrawn {
                continue;
            }
            for b in 0..4u8 {
                if a == b || Some(a) == rebel || Some(b) == rebel && b != self.human {
                    continue;
                }
                let at_war = self.at_war(a, b);
                let tension = self.players[a as usize].tension[b as usize];
                let mine = self.military_strength(a);
                let theirs = self.military_strength(b);
                if !at_war && tension > 60 && mine > theirs && self.chance(3) {
                    self.declare_war(a, b);
                } else if at_war {
                    let weary = self.players[a as usize].war_turns[b as usize] as i32;
                    let wants_peace = weary > 12 && (mine <= theirs || self.chance(30));
                    if wants_peace && self.chance(6) {
                        if b == self.human {
                            if !self.events.iter().any(|e| matches!(e, Event::PeaceOffer { nation } if *nation == a)) {
                                self.events.push(Event::PeaceOffer { nation: a });
                            }
                        } else if self.chance(60) {
                            self.make_peace(a, b);
                        }
                    }
                }
            }
        }
        // the Crown drags its subjects into wars between the old powers
        let h = self.human;
        if rebel.is_none() && self.year >= 1550 && self.chance(1) && self.chance(60) {
            let enemies: Vec<u8> = (0..4u8).filter(|&n| n != h && !self.at_war(h, n) && self.knows(h, n)).collect();
            if !enemies.is_empty() {
                let e = enemies[self.rand(enemies.len() as u32) as usize];
                self.events.push(Event::Crown {
                    text: format!(
                        "We are at war with {}! Our loyal subjects in the New World will take up arms against the {} at once.",
                        self.nation_name(e), self.adjective(e)
                    ),
                });
                self.declare_war(h, e);
            }
        }
    }

    /// Computer armies at war: attack weak neighbours and march on enemy colonies.
    pub(crate) fn ai_war(&mut self, n: u8) {
        let enemies: Vec<u8> = (0..4u8).filter(|&e| self.at_war(n, e)).collect();
        if enemies.is_empty() {
            return;
        }
        let army: Vec<Id> = self
            .units
            .iter()
            .filter(|u| u.nation == n && matches!(u.loc, Loc::Map { .. }))
            .filter(|u| matches!(u.kind, UnitType::Soldier | UnitType::Dragoon | UnitType::Artillery))
            .map(|u| u.id)
            .collect();
        for id in army {
            let Some((x, y)) = self.unit_pos(id) else { continue };
            let Some(u) = self.unit(id).cloned() else { continue };
            // keep one defender in every colony
            if let Some(c) = self.colony_at(x, y)
                && c.nation == n
            {
                let defenders = self
                    .units_at(x, y)
                    .filter(|v| v.nation == n && matches!(v.kind, UnitType::Soldier | UnitType::Dragoon | UnitType::Artillery))
                    .count();
                if defenders <= 1 {
                    continue;
                }
            }
            // a good fight next door?
            let att = self.attack_strength(&u);
            let mut best: Option<(f32, i32, i32)> = None;
            for (dx, dy) in DIRS8 {
                let (nx, ny) = (x + dx, y + dy);
                if self.is_water(nx, ny) {
                    continue;
                }
                let enemy_here = self.colony_at(nx, ny).is_some_and(|c| enemies.contains(&c.nation))
                    || self.units_at(nx, ny).any(|v| enemies.contains(&v.nation) && !v.kind.is_ship());
                if !enemy_here {
                    continue;
                }
                let def = self
                    .units_at(nx, ny)
                    .filter(|v| !v.kind.is_ship())
                    .map(|v| self.defence_strength(v, nx, ny))
                    .fold(1.0f32, f32::max);
                let odds = att / (att + def);
                if best.is_none_or(|(b, _, _)| odds > b) {
                    best = Some((odds, nx, ny));
                }
            }
            if let Some((odds, nx, ny)) = best
                && odds > 0.45
            {
                self.attack(id, nx, ny);
                continue;
            }
            // march on the nearest enemy colony within reach
            let target = self
                .colonies
                .iter()
                .filter(|c| enemies.contains(&c.nation))
                .map(|c| ((c.x as i32 - x).abs().max((c.y as i32 - y).abs()), c.x as i32, c.y as i32))
                .filter(|(d, _, _)| *d <= 10)
                .min();
            let force = self
                .units
                .iter()
                .filter(|v| v.nation == n && matches!(v.kind, UnitType::Soldier | UnitType::Dragoon | UnitType::Artillery))
                .count();
            if let Some((_, tx, ty)) = target
                && force >= 3
                && let Some(path) = self.find_path(id, tx, ty)
                && let Some(&(px, py)) = path.first()
                && (px, py) != (tx, ty)
                && !self.units_at(px, py).any(|v| v.nation != n)
            {
                self.place(id, px, py);
            }
        }
        self.purge_dead();
    }
}
