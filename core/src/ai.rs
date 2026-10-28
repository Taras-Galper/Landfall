//! The computer players' economy: trade with the homeland, recruiting colonists,
//! arming for war and spreading out to new colony sites.

use crate::game::*;
use crate::types::UnitType;

/// Goods a computer player ships home for sale.
const EXPORTS: [u8; 10] = [1, 2, 3, 4, 7, 9, 10, 11, 12, 13];

impl Game {
    /// Enough land and food around a square to be worth a colony.
    pub(crate) fn ai_good_site(&self, x: i32, y: i32) -> bool {
        let land = DIRS8.iter().filter(|(dx, dy)| !self.is_water(x + dx, y + dy) && self.tile(x + dx, y + dy).is_some()).count();
        let food: i32 = DIRS8.iter().map(|(dx, dy)| self.base_yield(x + dx, y + dy, 0)).sum::<i32>() + self.base_yield(x, y, 0);
        land >= 4 && food >= 8
    }

    /// Could a colony go here (ignoring who would found it)?
    pub(crate) fn site_free(&self, x: i32, y: i32) -> bool {
        let Some(t) = self.tile(x, y) else { return false };
        !t.terrain().is_water()
            && t.terrain() != crate::types::Terrain::Arctic
            && self.colony_at(x, y).is_none()
            && self.village_at(x, y).is_none()
            && !DIRS8.iter().any(|(dx, dy)| self.colony_at(x + dx, y + dy).is_some() || self.village_at(x + dx, y + dy).is_some())
    }

    /// The best colony site within `radius` of (x, y): good land, coastal preferred, near.
    pub(crate) fn ai_find_site(&self, x: i32, y: i32, radius: i32, coastal: bool) -> Option<(i32, i32)> {
        let mut best: Option<(i32, i32, i32)> = None;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let (sx, sy) = (x + dx, y + dy);
                if !self.site_free(sx, sy) || !self.ai_good_site(sx, sy) {
                    continue;
                }
                let is_coast = self.is_coastal(sx, sy);
                if coastal && !is_coast {
                    continue;
                }
                let food: i32 = DIRS8.iter().map(|(ddx, ddy)| self.base_yield(sx + ddx, sy + ddy, 0)).sum();
                let score = food * 2 + if is_coast { 10 } else { 0 } - dx.abs().max(dy.abs()) * 3
                    - if self.colonies.iter().any(|c| (c.x as i32 - sx).abs().max((c.y as i32 - sy).abs()) < 3) { 30 } else { 0 };
                if best.is_none_or(|(b, _, _)| score > b) {
                    best = Some((score, sx, sy));
                }
            }
        }
        best.map(|(_, a, b)| (a, b))
    }

    /// Walk a settler towards a good site and found the colony there.
    /// Returns false if no site is in reach.
    pub(crate) fn ai_settle(&mut self, id: Id, x: i32, y: i32, coastal: bool) -> bool {
        if self.site_free(x, y) && self.ai_good_site(x, y) && (!coastal || self.is_coastal(x, y)) && self.can_found_colony(id).is_ok() {
            return self.found_colony(id).is_ok();
        }
        let Some((tx, ty)) = self.ai_find_site(x, y, 8, coastal) else { return false };
        if let Some(path) = self.find_path(id, tx, ty)
            && let Some(&(px, py)) = path.first()
            && !self.units_at(px, py).any(|u| u.nation != self.unit(id).map(|v| v.nation).unwrap_or(0))
            && self.village_at(px, py).is_none()
            && self.colony_at(px, py).is_none()
        {
            self.place(id, px, py);
            if (px, py) == (tx, ty) && self.can_found_colony(id).is_ok() {
                let _ = self.found_colony(id);
            }
            return true;
        }
        false
    }

    pub(crate) fn ai_economy(&mut self, n: u8) {
        if self.rebellion.as_ref().is_some_and(|r| r.nation == n) {
            return;
        }
        let colonies: Vec<(Id, i16, i16, bool)> = self
            .colonies
            .iter()
            .filter(|c| c.nation == n)
            .map(|c| (c.id, c.x, c.y, self.is_coastal(c.x as i32, c.y as i32)))
            .collect();
        // trade the game doesn't model in detail (furs, smuggling, fishing
        // fleets) keeps the computer empires solvent; harder levels pay more
        let pop: i32 = colonies.iter().map(|c| self.population(c.0) as i32).sum();
        let scale = [6, 8, 10, 12, 15][self.difficulty.min(4) as usize];
        let income = (colonies.len() as i32 * 6 + pop * 3) * scale / 10;
        self.players[n as usize].gold += income;
        let gold = self.players[n as usize].gold;
        let threatened = (0..4u8).any(|e| self.at_war(n, e)) || self.players[n as usize].tension.iter().any(|t| *t > 50);

        // ---- the homeland: recruit, arm, buy ----
        let docks = self.units.iter().filter(|u| u.nation == n && u.loc == Loc::Homeland && !u.kind.is_ship()).count();
        if docks < 6 && gold > self.recruit_price(n) + 100 && self.chance(40) {
            let slot = self.rand(3) as usize;
            let _ = self.recruit(n, slot);
        }
        if threatened && self.players[n as usize].gold > 1500 && self.chance(20) {
            // turn a colonist on the docks into a soldier, or buy artillery
            let recruit = self.units.iter().find(|u| u.nation == n && u.loc == Loc::Homeland && u.kind == UnitType::Colonist).map(|u| u.id);
            match recruit {
                Some(id) => {
                    let kind = if self.players[n as usize].gold > 2500 { UnitType::Dragoon } else { UnitType::Soldier };
                    let _ = self.equip_in_homeland(id, kind);
                }
                None => {
                    let _ = self.buy_unit(n, UnitType::Artillery);
                }
            }
        }
        // a crowded dock goes over with the next merchant convoy
        if docks >= 3 {
            let coastal: Vec<Id> = colonies.iter().filter(|c| c.3).map(|c| c.0).collect();
            let waiting = self.units.iter().find(|u| u.nation == n && u.loc == Loc::Homeland && !u.kind.is_ship()).map(|u| u.id);
            if let (Some(w), false) = (waiting, coastal.is_empty()) {
                let cid = coastal[self.rand(coastal.len() as u32) as usize];
                let (x, y) = self.colony(cid).map(|c| (c.x as i32, c.y as i32)).unwrap();
                self.place(w, x, y);
                if self.unit(w).is_some_and(|u| u.kind == UnitType::Colonist) && self.population(cid) < 6 {
                    let _ = self.join_colony(w);
                } else if self.unit(w).is_some_and(|u| u.kind == UnitType::Colonist) {
                    self.ai_settlers.push(w);
                }
            }
        }
        // a bigger fleet once the colonies are established
        let ships = self.units.iter().filter(|u| u.nation == n && u.kind.is_ship()).count();
        if ships < 1 + colonies.len() / 4 && self.players[n as usize].gold > 4000 && self.chance(10) {
            let _ = self.buy_unit(n, UnitType::Merchantman);
        }
        if threatened && ships < 3 && self.players[n as usize].gold > 7000 && self.chance(5) {
            let _ = self.buy_unit(n, UnitType::Frigate);
        }

        // ---- ships: put idle ones on a route between a colony and the homeland ----
        let idle: Vec<Id> = self
            .units
            .iter()
            .filter(|u| u.nation == n && u.kind.is_ship() && u.route.is_none() && u.repair == 0)
            .filter(|u| !crate::naval::is_warship(u.kind))
            .filter(|u| matches!(u.loc, Loc::Map { .. } | Loc::Homeland))
            .map(|u| u.id)
            .collect();
        for sid in idle {
            // land any passengers first (handled by ai_ships)
            if self.aboard(sid).count() > 0 && matches!(self.unit(sid).map(|u| u.loc), Some(Loc::Map { .. })) {
                continue;
            }
            // the coastal colony with the most to sell
            let best = colonies
                .iter()
                .filter(|c| c.3)
                .max_by_key(|c| {
                    self.colony(c.0).map(|col| EXPORTS.iter().map(|&g| col.stock[g as usize]).sum::<i32>()).unwrap_or(0)
                })
                .map(|c| c.0);
            let Some(home) = best else { continue };
            let bring = if self.players[n as usize].gold > 800 { vec![TOOLS as u8] } else { Vec::new() };
            self.set_route(
                sid,
                Some(TradeRoute { a: Place::Colony(home), b: Place::Homeland, a_to_b: EXPORTS.to_vec(), b_to_a: bring, to_b: false }),
            );
        }

        // ---- colonies: crowded ones send settlers out ----
        for (cid, cx, cy, _) in &colonies {
            let wanted = 2 + self.turn as usize / 12;
            if self.population(*cid) < 4 || colonies.len() >= wanted || !self.chance(15) {
                continue;
            }
            let worker = self
                .units
                .iter()
                .find(|u| matches!(u.loc, Loc::Colony { colony, .. } if colony == *cid) && u.prof == P_FREE)
                .map(|u| u.id);
            if let Some(w) = worker
                && self.leave_colony(w).is_ok()
            {
                self.place(w, *cx as i32, *cy as i32);
                if let Some(u) = self.unit_mut(w) {
                    u.orders = Orders::None;
                }
                self.ai_settlers.push(w);
            }
        }
    }

    /// Computer passengers arriving at one of their colonies go ashore.
    pub(crate) fn ai_disembark(&mut self, ship: Id) {
        let Some(s) = self.unit(ship) else { return };
        let n = s.nation;
        if n == self.human {
            return;
        }
        let Some((x, y)) = self.unit_pos(ship) else { return };
        let Some(cid) = self.colony_at(x, y).filter(|c| c.nation == n).map(|c| c.id) else { return };
        let people: Vec<Id> = self.aboard(ship).map(|u| u.id).collect();
        for p in people {
            self.place(p, x, y);
            let kind = self.unit(p).map(|u| u.kind);
            if kind == Some(UnitType::Colonist) && self.population(cid) < 8 {
                let _ = self.join_colony(p);
            }
        }
    }

    /// Computer passengers waiting on the docks board a ship leaving the homeland.
    pub(crate) fn ai_board_in_homeland(&mut self, ship: Id) {
        let Some(s) = self.unit(ship) else { return };
        let n = s.nation;
        if n == self.human {
            return;
        }
        let waiting: Vec<Id> = self.units.iter().filter(|u| u.nation == n && u.loc == Loc::Homeland && !u.kind.is_ship()).map(|u| u.id).collect();
        for w in waiting {
            if self.holds_used(ship) >= self.holds(ship) {
                break;
            }
            if let Some(u) = self.unit_mut(w) {
                u.loc = Loc::Aboard(ship);
                u.orders = Orders::Sentry;
            }
        }
    }
}
