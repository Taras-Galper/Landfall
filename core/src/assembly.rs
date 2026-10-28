//! The colonial assembly (the great figures who join it), schools, custom houses,
//! the War of Independence and the end of the game.

use crate::game::*;
use crate::types::{Control, UnitType};

// Assembly members, in the order of `members` in the rules file.
pub const M_FACTORY_SHOPS: u8 = 0;
pub const M_STEADY_PRICES: u8 = 1;
pub const M_FREE_LAND: u8 = 2;
pub const M_CUSTOM_HOUSE: u8 = 3;
pub const M_FOREIGN_INTEL: u8 = 4;
pub const M_FAST_SHIPS: u8 = 5;
pub const M_SEE_COLONIES: u8 = 6;
pub const M_GOOD_RUMOURS: u8 = 7;
pub const M_FUR_BONUS: u8 = 8;
pub const M_FREE_STOCKADE: u8 = 9;
pub const M_PLUNDER: u8 = 10;
pub const M_PROMOTION: u8 = 11;
pub const M_MINUTEMEN: u8 = 12;
pub const M_PRIVATEERS: u8 = 13;
pub const M_FRIGATE: u8 = 14;
pub const M_BELLS: u8 = 15;
pub const M_PEACE: u8 = 16;
pub const M_BELLS_TAX: u8 = 17;
pub const M_LIBERTY_BOOST: u8 = 18;
pub const M_NO_WARS: u8 = 19;
pub const M_CHOOSE_IMMIGRANTS: u8 = 20;
pub const M_CROSSES: u8 = 21;
pub const M_MISSIONS: u8 = 22;
pub const M_CONQUEST_CONVERTS: u8 = 23;
pub const M_FREE_CONVERTS: u8 = 24;

/// One-line description of each member's effect.
pub const MEMBER_EFFECTS: [&str; 25] = [
    "Level-2 shops work as well as factories.",
    "Market prices fall half as fast when you sell.",
    "Natives don't mind new colonies near their villages.",
    "Allows custom houses, which sell surplus goods automatically.",
    "Reveals the land around every foreign colony.",
    "Ships move one square further and reach the homeland a turn sooner.",
    "Reveals the land around every colony in the New World.",
    "Rumours are always good, and explorers see further.",
    "Fur trappers produce 50% more.",
    "Every colony of 3 or more gets a free stockade.",
    "Conquered villages yield twice the plunder; treasure ships home free.",
    "Soldiers and dragoons are promoted after every victory.",
    "Colonists take up stockpiled muskets to defend their colony.",
    "Privateers fight 50% harder.",
    "A frigate joins your fleet.",
    "Liberty bell production +10%.",
    "All native tension is forgiven.",
    "Liberty bell production + the tax rate.",
    "Rebel support +20% in every colony.",
    "Wars between the old powers no longer spill into the New World.",
    "No more criminals or servants among the immigrants.",
    "Cross production +50%.",
    "Missions convert natives twice as fast.",
    "Defeated villages yield converts.",
    "All native converts become free colonists.",
];

impl Game {
    pub fn has_member(&self, nation: u8, f: u8) -> bool {
        nation < 4 && self.players.get(nation as usize).is_some_and(|p| p.assembly.has(f))
    }

    pub fn member_name(&self, f: u8) -> String {
        self.rules.members.get(f as usize).map(|r| r.name.clone()).unwrap_or_else(|| format!("Member {f}"))
    }

    /// Bells needed for the next member.
    pub fn member_cost(&self, nation: u8) -> i32 {
        let n = self.players[nation as usize].assembly.members.len() as i32;
        (120 + 60 * n + 45 * n * n) * (4 + self.difficulty as i32) / 5
    }

    /// Up to five candidates, one per field, weighted by era.
    pub fn offer_members(&mut self, nation: u8) {
        let era = if self.year < 1600 { 0 } else if self.year < 1700 { 1 } else { 2 };
        let mut offered = Vec::new();
        for kind in 0..5u8 {
            let pool: Vec<(u8, u32)> = self
                .rules
                .members
                .iter()
                .enumerate()
                .filter(|(i, f)| f.kind == kind && !self.players[nation as usize].assembly.has(*i as u8))
                .map(|(i, f)| (i as u8, (f.weights[era] as u32).max(if era == 2 { 1 } else { 0 })))
                .filter(|(_, w)| *w > 0)
                .collect();
            let total: u32 = pool.iter().map(|(_, w)| w).sum();
            if total == 0 {
                continue;
            }
            let mut r = self.rand(total);
            for (i, w) in pool {
                if r < w {
                    offered.push(i);
                    break;
                }
                r -= w;
            }
        }
        self.players[nation as usize].assembly.offered = offered;
    }

    pub fn choose_member(&mut self, nation: u8, f: u8) {
        let c = &mut self.players[nation as usize].assembly;
        c.candidate = Some(f);
    }

    /// Bells from a colony also go towards the next member.
    pub fn add_assembly_bells(&mut self, nation: u8, bells: i32) {
        if let Some(p) = self.players.get_mut(nation as usize) {
            p.assembly.bells += bells;
        }
        if let Some(r) = self.rebellion.as_mut()
            && r.nation == nation
        {
            r.bells += bells;
        }
    }

    fn assembly_turn(&mut self) {
        for n in 0..4u8 {
            if self.players[n as usize].control == Control::Withdrawn {
                continue;
            }
            if self.players[n as usize].assembly.offered.is_empty() && self.players[n as usize].assembly.candidate.is_none() {
                self.offer_members(n);
            }
            if self.players[n as usize].control == Control::Ai && self.players[n as usize].assembly.candidate.is_none() {
                let offered = self.players[n as usize].assembly.offered.clone();
                if !offered.is_empty() {
                    let pick = offered[self.rand(offered.len() as u32) as usize];
                    self.choose_member(n, pick);
                }
            }
            let Some(f) = self.players[n as usize].assembly.candidate else { continue };
            let cost = self.member_cost(n);
            if self.players[n as usize].assembly.bells < cost {
                continue;
            }
            let c = &mut self.players[n as usize].assembly;
            c.bells -= cost;
            c.members.push(f);
            c.candidate = None;
            c.offered.clear();
            self.offer_members(n);
            if n == self.human {
                self.event(crate::events::Event::MemberJoined(f as usize));
                self.msg(format!(
                    "{} joins the {}! {}",
                    self.member_name(f),
                    self.rules.names.assembly,
                    MEMBER_EFFECTS.get(f as usize).copied().unwrap_or("")
                ));
            }
            self.member_joined(n, f);
        }
    }

    /// One-time effects when a member joins.
    fn member_joined(&mut self, n: u8, f: u8) {
        match f {
            M_FOREIGN_INTEL | M_SEE_COLONIES => {
                let spots: Vec<(i32, i32)> = self
                    .colonies
                    .iter()
                    .filter(|c| c.nation != n && (f == M_SEE_COLONIES || c.nation < 4))
                    .map(|c| (c.x as i32, c.y as i32))
                    .collect();
                for (x, y) in spots {
                    self.reveal(n, x, y, if f == M_SEE_COLONIES { 2 } else { 1 });
                }
            }
            M_FREE_STOCKADE => self.free_stockades(n),
            M_STEADY_PRICES => self.lift_boycotts(n),
            M_FRIGATE => {
                let (x, y) = self.players[n as usize].entry;
                let (x, y) = if self.is_water(x as i32, y as i32) { (x as i32, y as i32) } else { (self.width as i32 - 2, y as i32) };
                let id = self.spawn(UnitType::Frigate, n, P_FREE, Loc::Map { x: x as i16, y: y as i16 });
                let full = self.max_moves(UnitType::Frigate);
                self.unit_mut(id).unwrap().moves = full;
            }
            M_PEACE => {
                for t in &mut self.tribes {
                    t.alarm[n as usize] = 0;
                }
            }
            M_LIBERTY_BOOST => {
                let ids: Vec<Id> = self.colonies.iter().filter(|c| c.nation == n).map(|c| c.id).collect();
                for cid in ids {
                    let pop = self.population(cid) as i32;
                    if let Some(c) = self.colony_mut(cid) {
                        c.liberty += pop * 40;
                    }
                }
            }
            M_FREE_CONVERTS => {
                for u in &mut self.units {
                    if u.nation == n && u.prof == P_CONVERT {
                        u.prof = P_FREE;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn free_stockades(&mut self, n: u8) {
        if !self.has_member(n, M_FREE_STOCKADE) {
            return;
        }
        let ids: Vec<Id> = self.colonies.iter().filter(|c| c.nation == n).map(|c| c.id).collect();
        for cid in ids {
            if self.population(cid) >= 3
                && let Some(c) = self.colony_mut(cid)
                    && c.levels[BKind::Fortification as usize] == 0 {
                        c.levels[BKind::Fortification as usize] = 1;
                    }
        }
    }

    // ----- schools -------------------------------------------------------------------

    fn school_turn(&mut self) {
        let cids: Vec<Id> = self.colonies.iter().map(|c| c.id).collect();
        for cid in cids {
            let Some(c) = self.colony(cid) else { continue };
            let level = c.level(BKind::Schoolhouse);
            let (name, nation) = (c.name.clone(), c.nation);
            if level == 0 {
                continue;
            }
            let teachers: Vec<(Id, u8)> = self
                .workers(cid)
                .filter(|u| matches!(u.loc, Loc::Colony { job: Job::Building(18), .. }))
                .map(|u| (u.id, u.prof))
                .collect();
            for (tid, prof) in teachers {
                let skill = self.rules.jobs.get(prof as usize).map(|j| j.skill).unwrap_or(4);
                if skill == 0 || skill > 3 || skill > level {
                    continue;
                }
                let needed = [4u8, 6, 8][skill as usize - 1];
                let t = self.unit_mut(tid).unwrap();
                t.teach += 1;
                if t.teach < needed {
                    continue;
                }
                // the least skilled colonist outside the school learns
                let student = self
                    .workers(cid)
                    .filter(|u| u.id != tid && !matches!(u.loc, Loc::Colony { job: Job::Building(18), .. }))
                    .filter(|u| matches!(u.prof, P_FREE | P_FREE_OLD | P_SERVANT | P_CRIMINAL))
                    .min_by_key(|u| match u.prof {
                        P_FREE | P_FREE_OLD => 0,
                        P_SERVANT => 1,
                        _ => 2,
                    })
                    .map(|u| (u.id, u.prof));
                self.unit_mut(tid).unwrap().teach = 0;
                let Some((sid, sprof)) = student else { continue };
                let new_prof = match sprof {
                    P_CRIMINAL => P_SERVANT,
                    P_SERVANT => P_FREE,
                    _ => prof,
                };
                self.unit_mut(sid).unwrap().prof = new_prof;
                if nation == self.human {
                    self.msg(format!("{name}: a student has become a {}.", self.prof_name(new_prof)));
                }
            }
        }
    }

    // ----- custom houses ----------------------------------------------------------------

    /// Sells surplus export goods from colonies with a custom house. Returns gold earned.
    pub fn custom_house(&mut self, cid: Id) -> i32 {
        let Some(c) = self.colony(cid) else { return 0 };
        let n = c.nation;
        if n > 3 || c.level(BKind::CustomHouse) == 0 {
            return 0;
        }
        let tax = if self.rebellion.as_ref().is_some_and(|r| r.nation == n) { 0 } else { self.players[n as usize].tax as i32 };
        let mut gold = 0;
        for g in [SUGAR, TOBACCO, COTTON, FURS, SILVER, RUM, CIGARS, CLOTH, COATS] {
            let have = self.colony(cid).unwrap().stock[g];
            if have > 50 {
                let amount = have - 50;
                gold += amount * self.bid_price(n, g) * (100 - tax) / 100;
                self.colony_mut(cid).unwrap().stock[g] = 50;
            }
        }
        self.players[n as usize].gold += gold;
        gold
    }

    // ----- rebellion ----------------------------------------------------------------------

    /// Share of colonists (0..=100) who support independence.
    pub fn rebel_percent(&self, nation: u8) -> i32 {
        let mut pop = 0;
        let mut rebels = 0;
        for c in self.colonies.iter().filter(|c| c.nation == nation) {
            let p = self.population(c.id) as i32;
            pop += p;
            rebels += (c.liberty / 200).min(p);
        }
        if pop == 0 { 0 } else { rebels * 100 / pop }
    }

    pub fn can_declare(&self, nation: u8) -> Result<(), String> {
        if self.rebellion.is_some() {
            return Err("Independence has already been declared.".into());
        }
        let r = self.rebel_percent(nation);
        if r < 50 {
            return Err(format!("Only {r}% of your colonists support independence. You need 50%."));
        }
        Ok(())
    }

    pub fn declare_independence(&mut self, nation: u8) -> Result<(), String> {
        self.can_declare(nation)?;
        let colonies = self.colonies.iter().filter(|c| c.nation == nation).count();
        self.rebellion = Some(Rebellion { nation, declared_turn: self.turn, colonies_at_start: colonies, bells: 0, intervened: false });
        if nation == self.human {
            self.event(crate::events::Event::Declared);
        }
        self.players[nation as usize].tax = 0;
        // anything in or bound for the homeland is seized by the Crown
        let mut seized = 0;
        for u in &mut self.units {
            if u.nation == nation && matches!(u.loc, Loc::Homeland | Loc::Sailing { .. }) {
                u.loc = Loc::Dead;
                seized += 1;
            }
        }
        self.purge_dead();
        // soldiers in colonies with strong rebel support become the patriot army
        let mut promoted = 0;
        let ids: Vec<Id> = self.units.iter().filter(|u| u.nation == nation).map(|u| u.id).collect();
        for id in ids {
            let Some((x, y)) = self.unit_pos(id) else { continue };
            let strong = self.colony_at(x, y).is_some_and(|c| self.liberty_percent(c) >= 50);
            if !strong {
                continue;
            }
            let u = self.unit_mut(id).unwrap();
            match u.kind {
                UnitType::Soldier => {
                    u.kind = UnitType::PatriotArmy;
                    promoted += 1;
                }
                UnitType::Dragoon => {
                    u.kind = UnitType::PatriotCavalry;
                    promoted += 1;
                }
                _ => {}
            }
        }
        let f = self.crown_force;
        self.msg(format!(
            "INDEPENDENCE! The Crown sends {} regulars, {} cavalry and {} artillery aboard {} ships of the line to crush your rebellion.",
            f[0], f[1], f[3], f[2]
        ));
        if seized > 0 {
            { let home = self.rules.names.homeland.clone(); self.msg(format!("{seized} of your ships and colonists in {home} have been seized by the Crown.")); }
        }
        if promoted > 0 {
            { let army = self.kind_name(UnitType::PatriotArmy).to_string(); self.msg(format!("{promoted} units join the {army}.")); }
        }
        Ok(())
    }

    fn crown_turn(&mut self) {
        // before the war the Crown keeps building up its army
        if self.rebellion.is_none() {
            let h = self.human;
            let rebels = self.rebel_percent(h);
            if rebels > 20 && self.chance(4) {
                let k = self.rand(3) as usize;
                let slot = [0, 1, 3][k];
                let n = 1 + self.rand(2) as i32;
                self.crown_force[slot] += n;
                let what = ["regulars", "cavalry", "", "artillery"][slot];
                self.msg(format!("Rumours of rebellion reach the Crown. It adds {n} {what} to its expeditionary army."));
            }
            return;
        }
        let rebel = self.rebellion.as_ref().unwrap().nation;
        self.intervention(rebel);

        // landings
        let reserve = self.crown_force[0] + self.crown_force[1] + self.crown_force[3];
        if reserve > 0 && (self.turn.is_multiple_of(2) || self.crown_units() == 0) {
            let targets: Vec<(i32, i32, String)> = self
                .colonies
                .iter()
                .filter(|c| c.nation == rebel && self.is_coastal(c.x as i32, c.y as i32))
                .map(|c| (c.x as i32, c.y as i32, c.name.clone()))
                .collect();
            if !targets.is_empty() {
                let (tx, ty, name) = targets[self.rand(targets.len() as u32) as usize].clone();
                let spots: Vec<(i32, i32)> = DIRS8
                    .iter()
                    .map(|(dx, dy)| (tx + dx, ty + dy))
                    .filter(|&(x, y)| {
                        !self.is_water(x, y)
                            && self.colony_at(x, y).is_none()
                            && self.village_at(x, y).is_none()
                            && !self.units_at(x, y).any(|u| u.nation != CROWN)
                    })
                    .collect();
                if let Some(&(lx, ly)) = spots.first() {
                    let mut landed = 0;
                    for _ in 0..6 {
                        let kind = if self.crown_force[0] > 0 && (landed % 3 != 2 || self.crown_force[3] == 0) {
                            self.crown_force[0] -= 1;
                            UnitType::CrownRegular
                        } else if self.crown_force[3] > 0 {
                            self.crown_force[3] -= 1;
                            UnitType::Artillery
                        } else if self.crown_force[1] > 0 {
                            self.crown_force[1] -= 1;
                            UnitType::CrownCavalry
                        } else {
                            break;
                        };
                        self.spawn(kind, CROWN, P_FREE, Loc::Map { x: lx as i16, y: ly as i16 });
                        landed += 1;
                    }
                    if landed > 0 && rebel == self.human {
                        self.msg(format!("The Crown's army lands {landed} units near {name}!"));
                        self.reveal(rebel, lx, ly, 1);
                        crate::sound::cue(self, crate::sound::Cue::Cannon, true);
                        // soldiers of fortune smell an opportunity
                        if self.mercenary_turn + 20 <= self.turn && self.chance(40) {
                            self.mercenary_turn = self.turn;
                            let price = 2000 + 500 * self.difficulty as i32;
                            if self.players[rebel as usize].gold >= price {
                                self.events.push(crate::events::Event::Mercenaries { price });
                            }
                        }
                    }
                }
            }
        }

        // the Crown's army marches on the nearest rebel colony
        let ids: Vec<Id> = self.units.iter().filter(|u| u.nation == CROWN && matches!(u.loc, Loc::Map { .. })).map(|u| u.id).collect();
        for id in ids {
            let Some((x, y)) = self.unit_pos(id) else { continue };
            if let Some(u) = self.unit_mut(id) {
                u.moves = 3;
            }
            let mut target = None;
            for (dx, dy) in DIRS8 {
                let (nx, ny) = (x + dx, y + dy);
                if self.colony_at(nx, ny).is_some_and(|c| c.nation == rebel) {
                    target = Some((nx, ny));
                    break;
                }
                if target.is_none() && self.units_at(nx, ny).any(|u| u.nation == rebel && !u.kind.is_ship()) && !self.is_water(nx, ny) {
                    target = Some((nx, ny));
                }
            }
            if let Some((ax, ay)) = target {
                self.attack(id, ax, ay);
                continue;
            }
            let home = self
                .colonies
                .iter()
                .filter(|c| c.nation == rebel)
                .min_by_key(|c| (c.x as i32 - x).abs().max((c.y as i32 - y).abs()))
                .map(|c| (c.x as i32, c.y as i32));
            if let Some((hx, hy)) = home
                && let Some(path) = self.find_path(id, hx, hy)
                    && let Some(&(nx, ny)) = path.first()
                        && (nx, ny) != (hx, hy) && !self.units_at(nx, ny).any(|u| u.nation != CROWN) {
                            self.place(id, nx, ny);
                        }
        }
        self.purge_dead();

        // victory or defeat?
        let rb = self.rebellion.clone().unwrap();
        let reserve = self.crown_force[0] + self.crown_force[1] + self.crown_force[3];
        let held = self.colonies.iter().filter(|c| c.nation == rb.nation).count();
        let lost = self.colonies.iter().filter(|c| c.nation == CROWN).count();
        if self.game_over.is_none() {
            if reserve == 0 && self.crown_units() == 0 {
                let score = self.score(rb.nation);
                self.game_over = Some((
                    true,
                    format!(
                        "VICTORY! The Crown's army has been defeated. The {} is free! Final score: {score}.",
                        self.independent_name(rb.nation)
                    ),
                ));
                if rb.nation == self.human {
                    self.event(crate::events::Event::WarOver { won: true });
                }
            } else if held == 0 || lost * 2 >= rb.colonies_at_start.max(1) {
                let score = self.score(rb.nation);
                self.game_over = Some((
                    false,
                    format!("DEFEAT. The Crown's army has crushed the rebellion. Final score: {score}."),
                ));
                if rb.nation == self.human {
                    self.event(crate::events::Event::WarOver { won: false });
                }
            }
        }
    }

    pub fn crown_units(&self) -> usize {
        self.units.iter().filter(|u| u.nation == CROWN && matches!(u.loc, Loc::Map { .. })).count()
    }

    pub fn score(&self, n: u8) -> i32 {
        let pop: i32 = self.colonies.iter().filter(|c| c.nation == n).map(|c| self.population(c.id) as i32).sum();
        let rebels = self.rebel_percent(n);
        let members = self.players.get(n as usize).map(|p| p.assembly.members.len() as i32).unwrap_or(0);
        let gold = self.players.get(n as usize).map(|p| p.gold / 1000).unwrap_or(0);
        let won = self.game_over.as_ref().is_some_and(|(w, _)| *w);
        let mut s = pop * 2 + rebels + members * 5 + gold;
        if won {
            s += 200 * (1 + self.difficulty as i32);
        }
        s
    }

    /// Everything the assembly, schools, custom houses and the Crown do at the end of a turn.
    pub fn end_of_turn_politics(&mut self) {
        self.school_turn();
        let cids: Vec<Id> = self.colonies.iter().map(|c| c.id).collect();
        for cid in cids {
            let gold = self.custom_house(cid);
            let _ = gold;
        }
        for n in 0..4u8 {
            self.free_stockades(n);
        }
        self.assembly_turn();
        self.diplomacy_turn();
        self.crown_turn();
        if self.year >= 1850 && self.game_over.is_none() {
            let score = self.score(self.human);
            self.game_over = Some((false, format!("The year 1850: the colonial era is over. Final score: {score}.")));
        }
    }
}

impl Game {
    /// Once enough liberty bells ring after the declaration, a rival power
    /// of the Crown sends troops to help the rebels.
    fn intervention(&mut self, rebel: u8) {
        let Some(r) = self.rebellion.as_ref() else { return };
        if r.intervened {
            return;
        }
        let needed = [2000, 3000, 4000, 5000, 6000][self.difficulty.min(4) as usize];
        if r.bells < needed {
            return;
        }
        let Some(target) = self
            .colonies
            .iter()
            .filter(|c| c.nation == rebel && self.is_coastal(c.x as i32, c.y as i32))
            .max_by_key(|c| self.population(c.id))
            .map(|c| (c.x, c.y, c.name.clone()))
        else {
            return;
        };
        // who intervenes: Corvaine, unless the rebels are Corvaine themselves
        let helper = if rebel == 1 { 2 } else { 1 };
        self.rebellion.as_mut().unwrap().intervened = true;
        let (x, y, name) = target;
        let force = [
            (UnitType::PatriotArmy, 4),
            (UnitType::PatriotCavalry, 2),
            (UnitType::Artillery, 2),
        ];
        for (k, n) in force {
            for _ in 0..n {
                let id = self.spawn(k, rebel, P_VETERAN, Loc::Map { x, y });
                self.unit_mut(id).unwrap().orders = Orders::Fortified;
            }
        }
        // and a pair of ships of the line offshore
        if let Some((sx, sy)) = DIRS8.iter().map(|(dx, dy)| (x as i32 + dx, y as i32 + dy)).find(|&(a, b)| self.is_water(a, b)) {
            for _ in 0..2 {
                self.spawn(UnitType::ShipOfTheLine, rebel, P_FREE, Loc::Map { x: sx as i16, y: sy as i16 });
            }
        }
        if rebel == self.human {
            self.msg(format!(
                "INTERVENTION! The {} declare war on the Crown and land an army at {name}: 4 regiments, 2 cavalry, 2 artillery and 2 ships of the line join your cause.",
                self.adjective(helper)
            ));
            crate::sound::cue(self, crate::sound::Cue::Cheer, true);
        }
    }

    /// The human hires the mercenaries on offer.
    pub fn hire_mercenaries(&mut self, price: i32) -> Result<(), String> {
        let h = self.human;
        if self.players[h as usize].gold < price {
            return Err("We cannot afford them.".into());
        }
        let Some((x, y)) = self.colonies.iter().filter(|c| c.nation == h).max_by_key(|c| self.population(c.id)).map(|c| (c.x, c.y)) else {
            return Err("We have no colony to receive them.".into());
        };
        self.players[h as usize].gold -= price;
        for k in [UnitType::Soldier, UnitType::Soldier, UnitType::Dragoon, UnitType::Artillery] {
            self.spawn(k, h, P_VETERAN, Loc::Map { x, y });
        }
        self.msg("Hired soldiers march into your largest colony: 2 veteran soldiers, a dragoon and artillery.");
        Ok(())
    }
}
