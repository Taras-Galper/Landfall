//! Trade routes: wagon trains and ships shuttling goods between two stops.

use crate::colony::GOOD_NAMES;
use crate::game::*;
use crate::world::{MoveResult, Prompt};

impl Game {
    pub fn place_name(&self, p: Place) -> String {
        match p {
            Place::Homeland => self.rules.names.homeland.clone(),
            Place::Colony(id) => self.colony(id).map(|c| c.name.clone()).unwrap_or_else(|| "(lost colony)".into()),
        }
    }

    pub fn route_summary(&self, r: &TradeRoute) -> String {
        let goods = |v: &Vec<u8>| -> String {
            if v.is_empty() {
                "nothing".into()
            } else {
                v.iter().map(|g| GOOD_NAMES[*g as usize]).collect::<Vec<_>>().join(", ")
            }
        };
        format!(
            "{} -> {} ({}), back with {}",
            self.place_name(r.a),
            self.place_name(r.b),
            goods(&r.a_to_b),
            goods(&r.b_to_a)
        )
    }

    pub fn set_route(&mut self, id: Id, route: Option<TradeRoute>) {
        if let Some(u) = self.unit_mut(id) {
            u.orders = if route.is_some() { Orders::Route } else { Orders::None };
            u.route = route;
        }
    }

    fn at_place(&self, id: Id, p: Place) -> bool {
        let Some(u) = self.unit(id) else { return false };
        match p {
            Place::Homeland => u.loc == Loc::Homeland,
            Place::Colony(cid) => {
                let Some(c) = self.colony(cid) else { return false };
                u.loc == Loc::Map { x: c.x, y: c.y }
            }
        }
    }

    /// Unload what this stop wants, load what it sends. Returns false if the stop is gone.
    fn exchange(&mut self, id: Id, stop: Place, deliver: &[u8], pick_up: &[u8]) -> bool {
        let nation = self.unit(id).map(|u| u.nation).unwrap_or(0);
        match stop {
            Place::Colony(cid) => {
                if self.colony(cid).is_none_or(|c| c.nation != nation) {
                    return false;
                }
                self.ai_disembark(id);
                // unload
                let cargo = self.unit(id).unwrap().cargo.clone();
                let mut keep = Vec::new();
                for (g, n) in cargo {
                    if deliver.contains(&g) {
                        self.colony_mut(cid).unwrap().stock[g as usize] += n as i32;
                    } else {
                        keep.push((g, n));
                    }
                }
                self.unit_mut(id).unwrap().cargo = keep;
                // load
                for &g in pick_up {
                    loop {
                        let have = self.colony(cid).unwrap().stock[g as usize];
                        if have <= 0 {
                            break;
                        }
                        let u = self.unit(id).unwrap();
                        let slot = u.cargo.iter().position(|(cg, n)| *cg == g && *n < 100);
                        let room = match slot {
                            Some(s) => 100 - u.cargo[s].1 as i32,
                            None if self.holds_used(id) < self.holds(id) => 100,
                            None => break,
                        };
                        let n = have.min(room);
                        self.colony_mut(cid).unwrap().stock[g as usize] -= n;
                        let um = self.unit_mut(id).unwrap();
                        match slot {
                            Some(s) => um.cargo[s].1 += n as u8,
                            None => um.cargo.push((g, n as u8)),
                        }
                    }
                }
            }
            Place::Homeland => {
                if self.rebellion.as_ref().is_some_and(|r| r.nation == nation) {
                    return false;
                }
                // sell deliveries
                loop {
                    let cargo = self.unit(id).unwrap().cargo.clone();
                    let Some(i) = cargo.iter().position(|(g, _)| deliver.contains(g)) else { break };
                    if self.sell_cargo(id, i).is_err() {
                        break;
                    }
                }
                // buy what we were told to bring back, if we can afford it
                // one hold (100) of each good on the list, as far as money allows
                for &g in pick_up {
                    let _ = self.buy_cargo(id, g as usize);
                }
                self.ai_board_in_homeland(id);
            }
        }
        true
    }

    /// Run one turn of a trade route for a unit. Returns a message for its owner if it stopped.
    pub fn run_route(&mut self, id: Id) -> Option<String> {
        for _ in 0..4 {
            let u = self.unit(id)?.clone();
            let mut r = u.route.clone()?;
            if u.orders != Orders::Route {
                return None;
            }
            let (target, deliver, pick_up) =
                if r.to_b { (r.b, r.a_to_b.clone(), r.b_to_a.clone()) } else { (r.a, r.b_to_a.clone(), r.a_to_b.clone()) };
            if self.at_place(id, target) {
                if !self.exchange(id, target, &deliver, &pick_up) {
                    self.set_route(id, None);
                    return Some(format!("{}: the trade route to {} is cut.", self.unit_name(&u), self.place_name(target)));
                }
                r.to_b = !r.to_b;
                self.unit_mut(id).unwrap().route = Some(r);
                continue;
            }
            // travel
            match (target, u.loc) {
                (_, Loc::Sailing { .. }) => return None,
                (Place::Colony(_), Loc::Homeland) => {
                    let _ = self.sail_to_new_world(id);
                    return None;
                }
                (Place::Homeland, Loc::Map { .. }) if u.kind.is_ship() => {
                    // head for the high seas
                    let (x, y) = self.unit_pos(id)?;
                    if self.tile(x, y).is_some_and(|t| t.raw & 0x1F == 26) {
                        self.sail_to_homeland(id);
                        return None;
                    }
                    let lane = self.nearest_sea_lane(x, y)?;
                    self.set_orders(id, Orders::Goto { x: lane.0 as i16, y: lane.1 as i16 });
                    let res = self.continue_goto(id);
                    if let Some(um) = self.unit_mut(id) {
                        um.orders = Orders::Route;
                    }
                    if let Some(MoveResult::Ask(Prompt::SailToHomeland { .. })) = res {
                        self.sail_to_homeland(id);
                    }
                    return None;
                }
                (Place::Colony(cid), Loc::Map { .. }) => {
                    let c = self.colony(cid)?;
                    let (cx, cy) = (c.x as i32, c.y as i32);
                    if self.find_path(id, cx, cy).is_none() {
                        self.set_route(id, None);
                        return Some(format!("{}: no way to reach {}. Route cancelled.", self.unit_name(&u), self.place_name(target)));
                    }
                    self.set_orders(id, Orders::Goto { x: cx as i16, y: cy as i16 });
                    let res = self.continue_goto(id);
                    if let Some(um) = self.unit_mut(id) {
                        um.orders = Orders::Route;
                    }
                    if let Some(MoveResult::Ask(Prompt::SailToHomeland { .. })) = res {
                        // passed through the high seas: keep going
                    }
                    if !self.at_place(id, target) {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        None
    }

    fn nearest_sea_lane(&self, x: i32, y: i32) -> Option<(i32, i32)> {
        let mut best: Option<(i32, (i32, i32))> = None;
        for ty in 0..self.height as i32 {
            for tx in 0..self.width as i32 {
                if self.tile(tx, ty).is_some_and(|t| t.raw & 0x1F == 26) {
                    let d = (tx - x).abs().max((ty - y).abs());
                    if best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, (tx, ty)));
                    }
                }
            }
        }
        best.map(|(_, p)| p)
    }

    /// Advance every unit that has a trade route (called at the start of each turn).
    pub fn run_routes(&mut self) {
        let ids: Vec<Id> = self.units.iter().filter(|u| u.orders == Orders::Route).map(|u| u.id).collect();
        for id in ids {
            let nation = self.unit(id).map(|u| u.nation).unwrap_or(0);
            if let Some(m) = self.run_route(id)
                && nation == self.human {
                    self.msg(m);
                }
        }
    }

    /// Places a carrier can use as stops.
    pub fn route_places(&self, id: Id) -> Vec<Place> {
        let Some(u) = self.unit(id) else { return Vec::new() };
        let mut v: Vec<Place> = self
            .colonies
            .iter()
            .filter(|c| c.nation == u.nation)
            .filter(|c| !u.kind.is_ship() || self.is_coastal(c.x as i32, c.y as i32))
            .map(|c| Place::Colony(c.id))
            .collect();
        if u.kind.is_ship() && self.rebellion.as_ref().is_none_or(|r| r.nation != u.nation) {
            v.push(Place::Homeland);
        }
        v
    }
}
