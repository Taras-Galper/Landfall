//! Player commands that any interface can issue: putting colonists to work,
//! loading and unloading, boarding ships, building, renaming, founding.
//!
//! Every rule lives here, so interfaces only show the game and send commands.

use crate::colony::{GOOD_NAMES, Production, N_OUT};
use crate::game::*;
use crate::types::UnitType;

pub fn is_carrier(k: UnitType) -> bool {
    k.is_ship() || k == UnitType::WagonTrain
}

impl Game {
    fn own_unit(&self, id: Id) -> Result<&Unit, String> {
        let u = self.unit(id).ok_or("No such unit.")?;
        if u.nation != self.human {
            return Err("That unit isn't yours.".into());
        }
        Ok(u)
    }

    fn own_colony(&self, id: Id) -> Result<&Colony, String> {
        let c = self.colony(id).ok_or("No such colony.")?;
        if c.nation != self.human {
            return Err("That colony isn't yours.".into());
        }
        Ok(c)
    }

    /// Is the unit inside the colony, on its square, or aboard a carrier there?
    fn at_colony(&self, unit: &Unit, cid: Id) -> bool {
        let Some(c) = self.colony(cid) else { return false };
        match unit.loc {
            Loc::Colony { colony, .. } => colony == cid,
            Loc::Map { x, y } => (x, y) == (c.x, c.y),
            Loc::Aboard(s) => self.unit_pos(s) == Some((c.x as i32, c.y as i32)),
            _ => false,
        }
    }

    /// Put a colonist to work at `job` in colony `cid`. A colonist already
    /// working a square someone else wants swaps with them.
    pub fn assign_job(&mut self, cid: Id, unit: Id, job: Job) -> Result<String, String> {
        let u = self.own_unit(unit)?.clone();
        let c = self.own_colony(cid)?.clone();
        if !self.at_colony(&u, cid) {
            return Err("The colonist must be at the colony.".into());
        }
        if !is_colonist(u.kind) {
            return Err("Only colonists can work in a colony.".into());
        }
        match job {
            Job::Building(j) => {
                let (k, _, _) = job_building(j).ok_or("No such job.")?;
                if c.level(k) == 0 {
                    return Err("The colony doesn't have that building yet.".into());
                }
                let here = self.building_workers(cid, j)
                    - usize::from(matches!(u.loc, Loc::Colony { job: Job::Building(jj), .. } if jj == j));
                if here >= 3 {
                    return Err("That building is full (3 workers).".into());
                }
            }
            Job::Tile { dir, job: tj } => {
                if dir > 7 || tj > 8 {
                    return Err("No such square.".into());
                }
                let (sx, sy) = self.colony_square(&c, dir);
                if self.square_taken(sx, sy, Some(unit)) {
                    return Err("Another colony works that square.".into());
                }
                if self.village_at(sx, sy).is_some() {
                    return Err("Natives live there.".into());
                }
                if self.tile(sx, sy).is_none() {
                    return Err("That square is off the map.".into());
                }
                // someone else on that square: swap, or send them to the best free job
                let other = self
                    .workers(cid)
                    .find(|w| w.id != unit && matches!(w.loc, Loc::Colony { job: Job::Tile { dir: d, .. }, .. } if d == dir))
                    .map(|w| w.id);
                if let Some(w) = other {
                    let theirs_to = match u.loc {
                        Loc::Colony { colony, job: old } if colony == cid => Some(old),
                        _ => None,
                    };
                    match theirs_to {
                        Some(old) => self.unit_mut(w).unwrap().loc = Loc::Colony { colony: cid, job: old },
                        None => {
                            let prof = self.unit(w).map(|x| x.prof).unwrap_or(P_FREE);
                            // take them off the square first so best_job doesn't pick it
                            self.unit_mut(w).unwrap().loc = Loc::Colony { colony: cid, job: Job::Building(17) };
                            let mut alt = self.best_job(cid, prof, Some(w));
                            if alt == (Job::Tile { dir, job: tj }) || matches!(alt, Job::Tile { dir: d, .. } if d == dir) {
                                alt = Job::Building(17);
                            }
                            self.unit_mut(w).unwrap().loc = Loc::Colony { colony: cid, job: alt };
                        }
                    }
                }
            }
        }
        if !matches!(u.loc, Loc::Colony { .. }) {
            self.unequip_into(unit, cid);
        }
        if let Some(um) = self.unit_mut(unit) {
            um.loc = Loc::Colony { colony: cid, job };
            um.kind = UnitType::Colonist;
            um.orders = Orders::None;
        }
        let name = match job {
            Job::Tile { job, .. } | Job::Building(job) => JOB_NAMES[job as usize],
        };
        Ok(format!("Now working as {name}."))
    }

    /// The best outdoor job for this colonist on square `dir`, if anything grows there.
    pub fn best_tile_job(&self, cid: Id, unit: Id, dir: u8) -> Option<u8> {
        let c = self.colony(cid)?;
        let prof = self.unit(unit)?.prof;
        let (sx, sy) = self.colony_square(c, dir);
        (0..9u8)
            .map(|j| (j, self.tile_output(c, prof, sx, sy, j) + if j == prof { 1 } else { 0 }))
            .filter(|(_, o)| *o > 0)
            .max_by_key(|(j, o)| (*o, JOB_GOOD[*j as usize] == FOOD))
            .map(|(j, _)| j)
    }

    /// What the colony would produce if `unit` took `job` (production preview).
    pub fn preview_job(&self, cid: Id, unit: Id, job: Job) -> Result<Production, String> {
        let mut g = self.clone();
        g.messages.clear();
        g.assign_job(cid, unit, job)?;
        Ok(g.production(cid))
    }

    /// Net change in every output if `unit` took `job`.
    pub fn preview_delta(&self, cid: Id, unit: Id, job: Job) -> Result<[i32; N_OUT], String> {
        let before = self.production(cid);
        let after = self.preview_job(cid, unit, job)?;
        let mut d = [0; N_OUT];
        for (g, v) in d.iter_mut().enumerate() {
            *v = if g < 16 { after.net(g) - before.net(g) } else { after.made[g] - before.made[g] };
        }
        Ok(d)
    }

    /// A worker steps out of the colony onto its square.
    pub fn step_outside(&mut self, unit: Id) -> Result<String, String> {
        let u = self.own_unit(unit)?.clone();
        match u.loc {
            Loc::Colony { .. } => {
                self.leave_colony(unit)?;
                Ok("The colonist steps outside.".into())
            }
            Loc::Aboard(s) => {
                let (x, y) = self.unit_pos(s).ok_or("The ship isn't on the map.")?;
                if self.colony_at(x, y).is_none() {
                    return Err("Unload passengers in a colony, or sail next to land.".into());
                }
                self.place(unit, x, y);
                if let Some(m) = self.unit_mut(unit) {
                    m.orders = Orders::None;
                }
                Ok("Unloaded.".into())
            }
            _ => Err("Already outside.".into()),
        }
    }

    /// A person boards a ship (in a colony, or in port in the homeland).
    pub fn board(&mut self, unit: Id, ship: Id) -> Result<String, String> {
        let m = self.own_unit(unit)?.clone();
        let s = self.own_unit(ship)?.clone();
        if is_carrier(m.kind) || unit == ship {
            return Err("Ships and wagons can't go aboard.".into());
        }
        if s.kind == UnitType::WagonTrain {
            return Err("Wagon trains carry goods, not people.".into());
        }
        if !s.kind.is_ship() {
            return Err("That isn't a ship.".into());
        }
        let same_place = match (m.loc, s.loc) {
            (Loc::Homeland, Loc::Homeland) => true,
            (Loc::Aboard(a), _) if a == ship => return Ok("Already aboard.".into()),
            (_, Loc::Map { x, y }) => {
                let col = self.colony_at(x as i32, y as i32).map(|c| c.id);
                match m.loc {
                    Loc::Map { x: mx, y: my } => (mx, my) == (x, y),
                    Loc::Colony { colony, .. } => Some(colony) == col,
                    Loc::Aboard(other) => self.unit_pos(other) == Some((x as i32, y as i32)) && col.is_some(),
                    _ => false,
                }
            }
            _ => false,
        };
        if !same_place {
            return Err("The ship must be in the same place.".into());
        }
        if self.holds_used(ship) >= self.holds(ship) {
            return Err("The ship is full.".into());
        }
        if matches!(m.loc, Loc::Colony { .. }) {
            self.leave_colony(unit)?;
        }
        if let Some(u) = self.unit_mut(unit) {
            u.loc = Loc::Aboard(ship);
            u.orders = Orders::Sentry;
        }
        Ok("Boarded the ship.".into())
    }

    /// Back onto the docks from a ship in port.
    pub fn disembark_in_homeland(&mut self, unit: Id) -> Result<String, String> {
        let u = self.own_unit(unit)?.clone();
        let Loc::Aboard(s) = u.loc else { return Err("Not aboard a ship.".into()) };
        if self.unit(s).map(|x| x.loc) != Some(Loc::Homeland) {
            return Err("The ship isn't in port.".into());
        }
        self.unit_mut(unit).unwrap().loc = Loc::Homeland;
        Ok("Back on the docks.".into())
    }

    /// Load up to `amount` (default: as much as fits, at most 100) of `good`
    /// from the colony warehouse onto a carrier at the colony.
    pub fn load_goods(&mut self, cid: Id, carrier: Id, good: usize, amount: Option<i32>) -> Result<String, String> {
        if good >= 16 {
            return Err("No such goods.".into());
        }
        let c = self.own_colony(cid)?.clone();
        let u = self.own_unit(carrier)?.clone();
        if !is_carrier(u.kind) {
            return Err("Only ships and wagon trains carry goods.".into());
        }
        if self.unit_pos(carrier) != Some((c.x as i32, c.y as i32)) {
            return Err("The carrier must be in the colony.".into());
        }
        let have = c.stock[good];
        if have <= 0 {
            return Err(format!("No {} in store.", GOOD_NAMES[good]));
        }
        let slot = u.cargo.iter().position(|(cg, n)| *cg as usize == good && *n < 100);
        let room = match slot {
            Some(s) => 100 - u.cargo[s].1 as i32,
            None if self.holds_used(carrier) < self.holds(carrier) => 100,
            None => return Err("No room left aboard.".into()),
        };
        let n = have.min(room).min(amount.unwrap_or(100).max(1));
        self.colony_mut(cid).unwrap().stock[good] -= n;
        let um = self.unit_mut(carrier).unwrap();
        match slot {
            Some(s) => um.cargo[s].1 += n as u8,
            None => um.cargo.push((good as u8, n as u8)),
        }
        Ok(format!("Loaded {n} {}.", GOOD_NAMES[good]))
    }

    /// Unload cargo slot `slot` of a carrier into the colony it is in.
    pub fn unload_goods(&mut self, carrier: Id, slot: usize, amount: Option<i32>) -> Result<String, String> {
        let u = self.own_unit(carrier)?.clone();
        let (x, y) = self.unit_pos(carrier).ok_or("The carrier isn't on the map.")?;
        let cid = self.colony_at(x, y).filter(|c| c.nation == self.human).map(|c| c.id).ok_or("Goods can only be unloaded in your colonies.")?;
        let &(g, n) = u.cargo.get(slot).ok_or("Nothing in that hold.")?;
        let take = amount.unwrap_or(n as i32).clamp(1, n as i32);
        self.colony_mut(cid).unwrap().stock[g as usize] += take;
        let um = self.unit_mut(carrier).unwrap();
        if take >= n as i32 {
            um.cargo.remove(slot);
        } else {
            um.cargo[slot].1 -= take as u8;
        }
        Ok(format!("Unloaded {take} {}.", GOOD_NAMES[g as usize]))
    }

    pub fn set_build(&mut self, cid: Id, b: Option<Build>) -> Result<String, String> {
        self.own_colony(cid)?;
        if let Some(b) = b
            && !self.build_options(cid).contains(&b)
        {
            return Err("That can't be built here yet.".into());
        }
        let c = self.colony_mut(cid).unwrap();
        c.building = b;
        Ok(String::new())
    }

    /// Price to finish the current construction right now.
    pub fn rush_price(&self, cid: Id) -> Option<i32> {
        let c = self.colony(cid)?;
        let b = c.building?;
        let (h, t) = self.build_cost(b, c);
        let rem_h = (h - c.hammers).max(0);
        let rem_t = (t - c.stock[TOOLS]).max(0);
        Some(rem_h * 13 + rem_t * self.ask_price(c.nation, TOOLS) * 2)
    }

    pub fn rush_build(&mut self, cid: Id) -> Result<String, String> {
        let c = self.own_colony(cid)?.clone();
        let b = c.building.ok_or("Nothing is being built.")?;
        let price = self.rush_price(cid).unwrap_or(0);
        if price == 0 {
            return Ok("It will be finished at the end of the turn.".into());
        }
        if self.human_player().gold < price {
            return Err(format!("Finishing it now costs {price} gold."));
        }
        let (h, t) = self.build_cost(b, &c);
        self.human_player_mut().gold -= price;
        let cm = self.colony_mut(cid).unwrap();
        cm.hammers = cm.hammers.max(h);
        cm.stock[TOOLS] = cm.stock[TOOLS].max(t);
        Ok(format!("Paid {price} gold. Finished at the end of the turn."))
    }

    pub fn rename_colony(&mut self, cid: Id, name: &str) -> Result<String, String> {
        self.own_colony(cid)?;
        let name = name.trim();
        if name.is_empty() || name.len() > 40 {
            return Err("Give the colony a name of up to 40 letters.".into());
        }
        if self.colonies.iter().any(|c| c.id != cid && c.name.eq_ignore_ascii_case(name)) {
            return Err(format!("There is already a colony called {name}."));
        }
        self.colony_mut(cid).unwrap().name = name.to_string();
        Ok(String::new())
    }

    /// Found a colony and give it a name (the suggested one if `name` is empty or taken).
    pub fn found_named(&mut self, unit: Id, name: &str) -> Result<Id, String> {
        self.own_unit(unit)?;
        let cid = self.found_colony(unit)?;
        let name = name.trim();
        if !name.is_empty() && name.len() <= 40 && !self.colonies.iter().any(|c| c.id != cid && c.name.eq_ignore_ascii_case(name)) {
            self.colony_mut(cid).unwrap().name = name.to_string();
        }
        Ok(cid)
    }

    /// Wake a unit (clear sentry/fortify) so it can be given orders.
    pub fn activate(&mut self, unit: Id) -> Result<(), String> {
        self.own_unit(unit)?;
        let u = self.unit_mut(unit).unwrap();
        u.done = false;
        if matches!(u.orders, Orders::Sentry | Orders::Fortified | Orders::Fortify | Orders::Goto { .. }) {
            u.orders = Orders::None;
            u.plan.clear();
        }
        Ok(())
    }

    /// Accept or bargain over a native offer to buy goods.
    pub fn answer_native_trade(&mut self, tribe: usize, cid: Id, good: usize, amount: i32, price: i32, haggle: bool) -> String {
        if !haggle {
            self.native_sale(tribe, cid, good, amount, price);
            return String::new();
        }
        if self.chance(50) {
            self.native_sale(tribe, cid, good, amount, price * 5 / 4);
            String::new()
        } else {
            "Offended by your greed, the traders leave.".into()
        }
    }
}

impl Game {
    /// Chance (0..1) that `attacker` wins against whatever defends (x, y), as the combat rules compute it.
    pub fn attack_odds(&self, attacker: Id, x: i32, y: i32) -> Option<f32> {
        let a = self.unit(attacker)?;
        if a.kind.is_ship() {
            return None;
        }
        let att = self.attack_strength(a);
        if let Some(vi) = self.village_at(x, y) {
            let v = &self.villages[vi];
            let tech = self.tribe_tech(v.tribe) as i32 as f32;
            let mut def = (1.0 + tech * 0.5) * 1.5;
            if a.nation == 2 {
                def /= 1.5;
            }
            if v.capital {
                def *= 1.5;
            }
            return Some(att / (att + def));
        }
        let colony = self.colony_at(x, y);
        let defender = self.defender_at(x, y).filter(|d| colony.is_none() || self.unit(*d).is_some_and(|u| self.is_military(u)));
        let def = match defender {
            Some(d) => self.defence_strength(self.unit(d)?, x, y),
            None => {
                let c = colony?;
                1.0 + 0.5 * (1 + c.level(BKind::Fortification)) as f32
            }
        };
        Some(att / (att + def))
    }
}
