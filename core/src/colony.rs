//! Colony economics: what each colonist produces, what a colony nets per
//! turn, growth, starvation, construction, founding and joining colonies.

use crate::assembly::*;
use crate::game::*;
use crate::types::{Terrain, UnitType};

pub const N_OUT: usize = 20; // 16 goods + hammers, bells, crosses, teaching
pub const HAMMERS: usize = 16;
pub const BELLS: usize = 17;
pub const CROSSES: usize = 18;

pub const GOOD_NAMES: [&str; N_OUT] = [
    "Food", "Sugar", "Tobacco", "Cotton", "Furs", "Lumber", "Ore", "Silver", "Horses", "Rum",
    "Cigars", "Cloth", "Coats", "Trade Goods", "Tools", "Muskets", "Hammers", "Bells", "Crosses", "Teaching",
];

/// One turn's production for a colony.
#[derive(Debug, Clone, Default)]
pub struct Production {
    /// Gross production of every output.
    pub made: [i32; N_OUT],
    /// Raw materials used up by buildings.
    pub used: [i32; N_OUT],
    /// Food eaten.
    pub eaten: i32,
    /// Horses bred (food spent on them is included in `used[FOOD]`).
    pub bred: i32,
}

impl Production {
    pub fn net(&self, g: usize) -> i32 {
        let mut n = self.made[g] - self.used[g];
        if g == FOOD {
            n -= self.eaten;
        }
        if g == HORSES {
            n += self.bred;
        }
        n
    }
}

impl Game {
    /// Square worked by a colony job direction.
    pub fn colony_square(&self, colony: &Colony, dir: u8) -> (i32, i32) {
        let (dx, dy) = DIRS8[dir as usize % 8];
        (colony.x as i32 + dx, colony.y as i32 + dy)
    }

    /// Is this square already worked by some colony (other than `except` unit)?
    pub fn square_taken(&self, x: i32, y: i32, except: Option<Id>) -> bool {
        self.units.iter().any(|u| {
            if Some(u.id) == except {
                return false;
            }
            if let Loc::Colony { colony, job: Job::Tile { dir, .. } } = u.loc
                && let Some(c) = self.colony(colony) {
                    return self.colony_square(c, dir) == (x, y);
                }
            false
        }) || self.colony_at(x, y).is_some()
    }

    /// Raw yield of an outdoor job on a square (before expert/colonist modifiers).
    pub fn base_yield(&self, x: i32, y: i32, job: u8) -> i32 {
        let Some(t) = self.tile(x, y) else { return 0 };
        let water = t.terrain().is_water();
        if (job == 8) != water {
            return 0;
        }
        let row = self.terrain_row(t);
        let Some(rule) = self.rules.terrain.get(row) else { return 0 };
        let mut y_ = rule.yields[job as usize] as i32;
        if y_ == 0 {
            return 0;
        }
        let unforested_farm = !t.forested() && !t.hills() && !t.mountains();
        if t.plowed() && unforested_farm && job <= 3 {
            y_ += 1;
        }
        if t.road() && (5..=7).contains(&job) {
            y_ += 1;
        }
        if job != 8 {
            if t.major_river() {
                y_ += 2;
            } else if t.minor_river() {
                y_ += 1;
            }
        }
        if let Some(r) = self.resource_at(x, y)
            && let Some((good, bonus)) = resource_bonus(r)
                && JOB_GOOD[job as usize] == good {
                    y_ += bonus;
                }
        y_
    }

    /// What `unit` produces doing outdoor `job` on (x, y).
    pub fn tile_output(&self, colony: &Colony, prof: u8, x: i32, y: i32, job: u8) -> i32 {
        if job == 8 && colony.level(BKind::Docks) == 0 {
            return 0;
        }
        let base = self.base_yield(x, y, job);
        if base == 0 {
            return 0;
        }
        let mut out = base;
        if prof == job {
            out *= 2;
        } else if prof == P_CONVERT {
            out += 1;
        }
        if job == 4 && self.has_member(colony.nation, M_FUR_BONUS) {
            out = out * 3 / 2;
        }
        (out + self.liberty_bonus(colony)).max(1)
    }

    /// Output of a building job for a colonist of profession `prof`.
    pub fn building_output(&self, colony: &Colony, prof: u8, job: u8) -> i32 {
        let Some((kind, _, _)) = job_building(job) else { return 0 };
        let lvl = colony.level(kind);
        if lvl == 0 {
            return 0;
        }
        let mut out = match prof {
            P_CRIMINAL | P_CONVERT => 1,
            P_SERVANT => 2,
            p if p == job => 6,
            _ => 3,
        };
        if lvl >= 3 || (lvl >= 2 && self.has_member(colony.nation, M_FACTORY_SHOPS)) {
            out = out * 3 / 2;
        }
        if out > 0 {
            out = (out + self.liberty_bonus(colony)).max(1);
        }
        out
    }

    pub fn liberty_percent(&self, colony: &Colony) -> i32 {
        let pop = self.population(colony.id).max(1) as i32;
        ((colony.liberty / 200) * 100 / pop).clamp(0, 100)
    }

    fn liberty_bonus(&self, colony: &Colony) -> i32 {
        let bonus = match self.liberty_percent(colony) {
            100 => 2,
            50..=99 => 1,
            _ => 0,
        };
        bonus - self.loyalist_penalty(colony)
    }

    /// Too many loyalists in a colony slow its work: 1 point past the limit, 2 past twice it.
    pub fn loyalist_penalty(&self, colony: &Colony) -> i32 {
        let pop = self.population(colony.id) as i32;
        let loyalists = pop - pop * self.liberty_percent(colony) / 100;
        let limit = [10, 8, 6, 6, 4][self.difficulty.min(4) as usize];
        if loyalists > limit * 2 {
            2
        } else if loyalists > limit {
            1
        } else {
            0
        }
    }

    /// The good the colony square itself produces besides food.
    pub fn center_secondary(&self, colony: &Colony) -> Option<(usize, i32)> {
        let (x, y) = (colony.x as i32, colony.y as i32);
        [1u8, 2, 3, 4, 6]
            .iter()
            .map(|&j| (JOB_GOOD[j as usize], self.base_yield(x, y, j)))
            .filter(|(_, n)| *n > 0)
            .max_by_key(|(_, n)| *n)
    }

    pub fn production(&self, colony_id: Id) -> Production {
        let mut p = Production::default();
        let Some(c) = self.colony(colony_id) else { return p };
        let (cx, cy) = (c.x as i32, c.y as i32);

        // the colony square works itself
        p.made[FOOD] += (self.base_yield(cx, cy, 0).max(1) + self.liberty_bonus(c)).max(1);
        if let Some((g, n)) = self.center_secondary(c) {
            p.made[g] += n;
        }

        let mut pop = 0;
        let mut indoor: Vec<(u8, u8)> = Vec::new();
        for u in self.workers(colony_id) {
            pop += 1;
            let Loc::Colony { job, .. } = u.loc else { continue };
            match job {
                Job::Tile { dir, job } => {
                    let (x, y) = self.colony_square(c, dir);
                    p.made[JOB_GOOD[job as usize]] += self.tile_output(c, u.prof, x, y, job);
                }
                Job::Building(j) => indoor.push((u.prof, j)),
            }
        }
        p.eaten = pop * 2;

        // base production of town hall and chapel
        p.made[BELLS] += 1;
        p.made[CROSSES] += 1 + c.level(BKind::Church) as i32;

        // buildings, limited by available raw material
        for (prof, job) in indoor {
            let Some((_, input, output)) = job_building(job) else { continue };
            let mut out = self.building_output(c, prof, job);
            if let Some(raw) = input {
                let available = c.stock[raw] + p.made[raw] - p.used[raw];
                let lvl3 = job_building(job).map(|(k, _, _)| c.level(k) >= 3).unwrap_or(false);
                let need = if lvl3 { out * 2 / 3 } else { out };
                if need > available {
                    out = if lvl3 { available.max(0) * 3 / 2 } else { available.max(0) };
                }
                p.used[raw] += if lvl3 { out * 2 / 3 } else { out };
            }
            p.made[output] += out;
        }
        // printing press / newspaper
        let press = c.level(BKind::PrintingPress) as i32;
        if press > 0 {
            p.made[BELLS] += p.made[BELLS] * press / 2;
        }
        let n = c.nation;
        let mut bonus = 0;
        if self.has_member(n, M_BELLS) {
            bonus += 10;
        }
        if self.has_member(n, M_BELLS_TAX) {
            bonus += self.players[n as usize].tax as i32;
        }
        p.made[BELLS] += p.made[BELLS] * bonus / 100;
        if self.has_member(n, M_CROSSES) {
            p.made[CROSSES] += p.made[CROSSES] / 2;
        }

        // horses breed on surplus food
        let surplus = p.made[FOOD] - p.eaten;
        let horses = c.stock[HORSES];
        if horses >= 2 && surplus > 1 {
            let cap = (horses / 25 + 1) * (1 + c.level(BKind::Stables) as i32);
            let bred = (surplus / 2).min(cap).max(0);
            p.bred = bred;
            p.used[FOOD] += bred;
        }
        p
    }

    pub fn build_cost(&self, b: Build, colony: &Colony) -> (i32, i32) {
        match b {
            Build::Building(k) => {
                let lvl = colony.level(k);
                let row = BUILDING_ROW[k as usize] + lvl as usize;
                self.rules.buildings.get(row).map(|r| (r.hammers as i32, r.tools as i32)).unwrap_or((999, 0))
            }
            Build::Unit(kind) => {
                let r = &self.rules.units[kind as usize];
                (r.hammers.max(40) as i32, r.tools as i32)
            }
        }
    }

    pub fn build_name(&self, b: Build, colony: &Colony) -> String {
        match b {
            Build::Building(k) => {
                let row = BUILDING_ROW[k as usize] + colony.level(k) as usize;
                self.rules.buildings.get(row).map(|r| r.name.clone()).unwrap_or_default()
            }
            Build::Unit(kind) => self.kind_name(kind).to_string(),
        }
    }

    /// Everything this colony could start building now.
    pub fn build_options(&self, colony_id: Id) -> Vec<Build> {
        let Some(c) = self.colony(colony_id) else { return Vec::new() };
        let pop = self.population(colony_id) as u8;
        let mut v = Vec::new();
        for (i, &k) in BKINDS.iter().enumerate() {
            let lvl = c.levels[i];
            if lvl >= BUILDING_MAX[i] || k == BKind::Capitol {
                continue;
            }
            if k == BKind::CustomHouse && !self.has_member(c.nation, M_CUSTOM_HOUSE) {
                continue;
            }
            if k == BKind::Docks && !self.is_coastal(c.x as i32, c.y as i32) {
                continue;
            }
            let row = BUILDING_ROW[i] + lvl as usize;
            let min = self.rules.buildings.get(row).map(|r| r.min_pop).unwrap_or(1);
            if pop >= min {
                v.push(Build::Building(k));
            }
        }
        v.push(Build::Unit(UnitType::WagonTrain));
        if c.level(BKind::Armory) > 0 {
            v.push(Build::Unit(UnitType::Artillery));
        }
        if self.is_coastal(c.x as i32, c.y as i32) {
            if c.level(BKind::Docks) >= 2 {
                v.push(Build::Unit(UnitType::Caravel));
            }
            if c.level(BKind::Docks) >= 3 {
                v.extend([UnitType::Merchantman, UnitType::Galleon, UnitType::Privateer, UnitType::Frigate].map(Build::Unit));
            }
        }
        v
    }

    /// A sensible thing to build next (used for new colonies and the AI).
    pub fn default_build(&self, colony_id: Id) -> Option<Build> {
        let opts = self.build_options(colony_id);
        let c = self.colony(colony_id)?;
        let pref = [
            BKind::Fortification, BKind::Docks, BKind::Warehouse, BKind::Armory, BKind::Church,
            BKind::Schoolhouse, BKind::PrintingPress, BKind::Carpenter, BKind::Stables,
        ];
        for k in pref {
            if k == BKind::Docks && !self.is_coastal(c.x as i32, c.y as i32) {
                continue;
            }
            if k == BKind::Fortification && c.level(k) >= 1 {
                continue;
            }
            let b = Build::Building(k);
            if opts.contains(&b) {
                return Some(b);
            }
        }
        opts.into_iter().find(|b| matches!(b, Build::Building(_)))
    }

    /// Run one turn of production for a colony. Returns messages for its owner.
    pub fn produce(&mut self, colony_id: Id) -> Vec<String> {
        let mut msgs = Vec::new();
        let prod = self.production(colony_id);
        let Some(ci) = self.colonies.iter().position(|c| c.id == colony_id) else { return msgs };
        let nation = self.colonies[ci].nation;
        let name = self.colonies[ci].name.clone();

        let before = self.colonies[ci].stock;
        for g in 0..16 {
            self.colonies[ci].stock[g] += prod.net(g);
        }
        self.colonies[ci].hammers += prod.made[HAMMERS];
        self.colonies[ci].liberty += prod.made[BELLS];
        if let Some(p) = self.players.get_mut(nation as usize) {
            p.bells += prod.made[BELLS];
            p.crosses += prod.made[CROSSES];
        }
        self.add_assembly_bells(nation, prod.made[BELLS]);

        // growth / starvation
        let (cx, cy) = (self.colonies[ci].x, self.colonies[ci].y);
        if self.colonies[ci].stock[FOOD] >= 200 {
            self.colonies[ci].stock[FOOD] -= 200;
            let id = self.spawn(UnitType::Colonist, nation, P_FREE, Loc::Map { x: cx, y: cy });
            if self.players.get(nation as usize).is_none_or(|p| p.control != crate::types::Control::Human) {
                self.auto_join(id, colony_id);
            }
            msgs.push(format!("{name}: the colony has grown. A new colonist is born."));
        } else if self.colonies[ci].stock[FOOD] < 0 {
            self.colonies[ci].stock[FOOD] = 0;
            let victim = self.workers(colony_id).last().map(|u| u.id);
            if let Some(v) = victim {
                if self.population(colony_id) > 1 {
                    self.kill(v);
                    msgs.push(format!("{name}: famine! A colonist has starved to death."));
                } else {
                    msgs.push(format!("{name}: food stores are empty. Colonists will starve."));
                }
            }
        }

        // construction
        if let Some(b) = self.colonies[ci].building {
            let c = &self.colonies[ci];
            let (h, t) = self.build_cost(b, c);
            if c.hammers >= h {
                if c.stock[TOOLS] >= t {
                    let bname = self.build_name(b, c);
                    let c = &mut self.colonies[ci];
                    c.hammers -= h;
                    c.stock[TOOLS] -= t;
                    match b {
                        Build::Building(k) => c.levels[k as usize] += 1,
                        Build::Unit(kind) => {
                            self.spawn(kind, nation, P_FREE, Loc::Map { x: cx, y: cy });
                        }
                    }
                    msgs.push(format!("{name} has completed: {bname}."));
                    let next = self.default_build(colony_id);
                    let c = &mut self.colonies[ci];
                    c.building = if matches!(b, Build::Unit(_)) { Some(b) } else { next };
                } else {
                    msgs.push(format!("{name} needs {} tools to finish its {}.", t, self.build_name(b, &self.colonies[ci])));
                }
            }
        }

        // storage limits
        let cap = self.colonies[ci].capacity();
        let mut lost = Vec::new();
        for g in 1..16 {
            if self.colonies[ci].stock[g] > cap {
                // only warn the first time a good overflows
                if before[g] < cap {
                    lost.push(format!("{} {}", self.colonies[ci].stock[g] - cap, GOOD_NAMES[g]));
                }
                self.colonies[ci].stock[g] = cap;
            }
        }
        if !lost.is_empty() {
            msgs.push(format!("{name}: warehouse full, lost {}.", lost.join(", ")));
        }
        msgs
    }

    // ----- founding, joining, leaving ----------------------------------------

    pub fn can_found_colony(&self, unit_id: Id) -> Result<(i32, i32), String> {
        let u = self.unit(unit_id).ok_or("no unit")?;
        if !is_colonist(u.kind) {
            return Err("Only colonists can found a colony.".into());
        }
        let Loc::Map { x, y } = u.loc else { return Err("Unit must be on land.".into()) };
        let (x, y) = (x as i32, y as i32);
        let t = self.tile(x, y).ok_or("off map")?;
        if t.terrain().is_water() {
            return Err("Can't build on water.".into());
        }
        if t.terrain() == Terrain::Arctic {
            return Err("Nothing can grow in the arctic.".into());
        }
        if self.colony_at(x, y).is_some() {
            return Err("There is already a colony here.".into());
        }
        if self.village_at(x, y).is_some() {
            return Err("There is a native settlement here.".into());
        }
        for (dx, dy) in DIRS8 {
            if self.colony_at(x + dx, y + dy).is_some() {
                return Err("Too close to another colony.".into());
            }
        }
        Ok((x, y))
    }

    pub fn next_colony_name(&self, nation: u8) -> String {
        let used: Vec<&str> = self.colonies.iter().map(|c| c.name.as_str()).collect();
        if let Some(list) = self.rules.colony_names.get(&nation)
            && let Some(n) = list.iter().find(|n| !used.contains(&n.as_str())) {
                return n.clone();
            }
        format!("Colony {}", self.colonies.len() + 1)
    }

    /// Strip a unit's equipment into a colony's warehouse; it becomes a plain colonist.
    pub fn unequip_into(&mut self, unit_id: Id, colony_id: Id) {
        let Some(u) = self.unit(unit_id).cloned() else { return };
        let mut goods: Vec<(usize, i32)> = Vec::new();
        match u.kind {
            UnitType::Soldier => goods.push((MUSKETS, 50)),
            UnitType::Dragoon => goods.extend([(MUSKETS, 50), (HORSES, 50)]),
            UnitType::Scout => goods.push((HORSES, 50)),
            UnitType::Pioneer => goods.push((TOOLS, u.tools as i32)),
            _ => {}
        }
        if let Some(c) = self.colony_mut(colony_id) {
            for (g, n) in goods {
                c.stock[g] += n;
            }
        }
        if let Some(u) = self.unit_mut(unit_id)
            && is_colonist(u.kind) {
                u.kind = UnitType::Colonist;
                u.tools = 0;
            }
    }

    pub fn found_colony(&mut self, unit_id: Id) -> Result<Id, String> {
        let (x, y) = self.can_found_colony(unit_id)?;
        let nation = self.unit(unit_id).map(|u| u.nation).unwrap_or(0);
        let id = self.new_id();
        let name = self.next_colony_name(nation);
        let mut levels = [0u8; 17];
        for k in [BKind::TownHall, BKind::Carpenter, BKind::Blacksmith, BKind::Tobacconist, BKind::Weaver, BKind::Distiller, BKind::FurTrader] {
            levels[k as usize] = 1;
        }
        self.colonies.push(Colony {
            id,
            name: name.clone(),
            nation,
            x: x as i16,
            y: y as i16,
            stock: [0; 16],
            hammers: 0,
            levels,
            building: None,
            liberty: 0,
        });
        self.unequip_into(unit_id, id);
        self.auto_join(unit_id, id);
        let b = self.default_build(id);
        if let Some(c) = self.colony_mut(id) {
            c.building = b;
        }
        // the forest under a new colony stays as it is
        if let Some(i) = self.index(x, y) {
            self.tiles[i].mask |= 0x08; // colonies count as roads
            self.rumour[i] = false;
        }
        self.reveal(nation, x, y, 2);
        if let Some(p) = self.players.get_mut(nation as usize) {
            p.colonies_founded += 1;
        }
        // natives don't like colonies on their doorstep
        for vi in 0..self.villages.len() {
            let v = &self.villages[vi];
            let d = (v.x as i32 - x).abs().max((v.y as i32 - y).abs());
            if d <= 2 && nation < 4 && !self.has_member(nation, M_FREE_LAND) && !self.land_settled {
                let t = v.tribe.saturating_sub(4) as usize;
                if let Some(tr) = self.tribes.get_mut(t) {
                    let anger = if nation == 1 { 7 } else { 15 };
                    tr.alarm[nation as usize] = (tr.alarm[nation as usize] + anger).min(100);
                }
            }
        }
        self.land_settled = false;
        if nation == self.human {
            self.msg(format!("The colony of {name} has been founded."));
            self.milestone(crate::events::MS_COLONY);
        }
        Ok(id)
    }

    /// Best job for a colonist joining a colony.
    pub fn best_job(&self, colony_id: Id, prof: u8, except: Option<Id>) -> Job {
        let Some(c) = self.colony(colony_id) else { return Job::Building(17) };
        let prod = self.production(colony_id);
        // a newcomer eats too
        let working_here = except.is_some_and(|id| matches!(self.unit(id).map(|u| u.loc), Some(Loc::Colony { colony, .. }) if colony == colony_id));
        let food_net = prod.net(FOOD) - if working_here { 0 } else { 2 };

        // experts in building trades go to their building
        if (9..=17).contains(&prof)
            && let Some((k, _, _)) = job_building(prof)
                && c.level(k) > 0 && self.building_workers(colony_id, prof) < 3 {
                    return Job::Building(prof);
                }
        let mut best: Option<(i32, Job)> = None;
        for dir in 0..8u8 {
            let (x, y) = self.colony_square(c, dir);
            if self.square_taken(x, y, except) || self.tile(x, y).is_none() {
                continue;
            }
            if let Some(v) = self.village_at(x, y) {
                let _ = v;
                continue;
            }
            for job in 0..9u8 {
                let out = self.tile_output(c, prof, x, y, job);
                if out <= 0 {
                    continue;
                }
                let mut score = out * 10;
                if JOB_GOOD[job as usize] == FOOD && food_net < 2 {
                    score += if food_net < 0 { 80 } else { 40 };
                }
                if prof == job {
                    score += 50;
                }
                if JOB_GOOD[job as usize] == SILVER {
                    score += 20;
                }
                if best.is_none_or(|(s, _)| score > s) {
                    best = Some((score, Job::Tile { dir, job }));
                }
            }
        }
        if food_net >= 2 || best.is_none() {
            // a craft with raw materials available
            for job in [13u8, 17, 14, 9, 10, 11, 12] {
                let Some((k, input, _)) = job_building(job) else { continue };
                if c.level(k) == 0 || self.building_workers(colony_id, job) >= 3 {
                    continue;
                }
                let has_raw = input.is_none_or(|g| c.stock[g] + prod.made[g] >= 6);
                if has_raw && (best.is_none() || job == 13 && c.building.is_some() && prod.made[HAMMERS] == 0) {
                    return Job::Building(job);
                }
            }
        }
        best.map(|(_, j)| j).unwrap_or(Job::Building(17))
    }

    pub fn building_workers(&self, colony_id: Id, job: u8) -> usize {
        self.workers(colony_id).filter(|u| matches!(u.loc, Loc::Colony { job: Job::Building(j), .. } if j == job)).count()
    }

    pub fn auto_join(&mut self, unit_id: Id, colony_id: Id) {
        let prof = self.unit(unit_id).map(|u| u.prof).unwrap_or(P_FREE);
        let job = self.best_job(colony_id, prof, Some(unit_id));
        if let Some(u) = self.unit_mut(unit_id) {
            u.loc = Loc::Colony { colony: colony_id, job };
            u.orders = Orders::None;
            u.kind = UnitType::Colonist;
        }
    }

    /// A unit on the colony square joins the colony as a worker.
    pub fn join_colony(&mut self, unit_id: Id) -> Result<(), String> {
        let u = self.unit(unit_id).ok_or("no unit")?;
        if !is_colonist(u.kind) {
            return Err("Only colonists can join a colony.".into());
        }
        let Loc::Map { x, y } = u.loc else { return Err("Not on the map.".into()) };
        let c = self.colony_at(x as i32, y as i32).ok_or("No colony here.")?;
        if c.nation != u.nation {
            return Err("Not your colony.".into());
        }
        let cid = c.id;
        self.unequip_into(unit_id, cid);
        self.auto_join(unit_id, cid);
        Ok(())
    }

    /// A worker leaves the colony and stands on the colony square.
    pub fn leave_colony(&mut self, unit_id: Id) -> Result<(), String> {
        let u = self.unit(unit_id).ok_or("no unit")?;
        let Loc::Colony { colony, .. } = u.loc else { return Err("Not in a colony.".into()) };
        if self.population(colony) <= 1 {
            return Err("The last colonist can't leave; the colony would be abandoned.".into());
        }
        let c = self.colony(colony).ok_or("no colony")?;
        let (x, y) = (c.x, c.y);
        let full = self.max_moves(UnitType::Colonist);
        if let Some(u) = self.unit_mut(unit_id) {
            u.loc = Loc::Map { x, y };
            u.moves = full;
            u.done = false;
            u.orders = Orders::None;
        }
        Ok(())
    }

    /// Equip a unit standing on a colony square from the colony's warehouse.
    pub fn equip(&mut self, unit_id: Id, kind: UnitType) -> Result<(), String> {
        let u = self.unit(unit_id).ok_or("no unit")?.clone();
        if !is_colonist(u.kind) {
            return Err("Only colonists can be equipped.".into());
        }
        let Loc::Map { x, y } = u.loc else { return Err("Unit must be on the colony square.".into()) };
        let cid = self.colony_at(x as i32, y as i32).map(|c| c.id).ok_or("Unit must be in a colony.")?;
        self.unequip_into(unit_id, cid);
        let need: Vec<(usize, i32)> = match kind {
            UnitType::Soldier => vec![(MUSKETS, 50)],
            UnitType::Dragoon => vec![(MUSKETS, 50), (HORSES, 50)],
            UnitType::Scout => vec![(HORSES, 50)],
            UnitType::Pioneer => vec![(TOOLS, 20)],
            UnitType::Missionary => vec![],
            _ => vec![],
        };
        let c = self.colony(cid).unwrap();
        if kind == UnitType::Missionary && c.level(BKind::Church) == 0 {
            return Err("Missionaries need a church.".into());
        }
        for &(g, n) in &need {
            if c.stock[g] < n {
                return Err(format!("Needs {} {}.", n, GOOD_NAMES[g]));
            }
        }
        let tools_taken = if kind == UnitType::Pioneer { c.stock[TOOLS].min(100) } else { 0 };
        let c = self.colony_mut(cid).unwrap();
        for (g, n) in need {
            c.stock[g] -= if g == TOOLS { tools_taken } else { n };
        }
        if let Some(u) = self.unit_mut(unit_id) {
            u.kind = kind;
            u.tools = tools_taken as u8;
        }
        Ok(())
    }
}
