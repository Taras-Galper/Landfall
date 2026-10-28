//! Life with the native nations beyond war: land claims, trade with the
//! colonies, demands from angry tribes and gifts from friendly ones.

use crate::colony::GOOD_NAMES;
use crate::assembly::*;
use crate::events::Event;
use crate::game::*;
use crate::types::UnitType;

/// Goods the natives will buy from a colony.
const WANTED: [usize; 8] = [CLOTH, RUM, CIGARS, COATS, TRADE_GOODS, MUSKETS, HORSES, TOOLS];

#[derive(Debug, Clone, PartialEq)]
pub struct Claim {
    pub tribe: usize,
    pub price: i32,
}

impl Game {
    /// Do the natives claim the land at (x, y)? Powers with the free-land member never pay.
    pub fn land_claim(&self, nation: u8, x: i32, y: i32) -> Option<Claim> {
        if nation >= 4 || self.has_member(nation, M_FREE_LAND) {
            return None;
        }
        let v = self
            .villages
            .iter()
            .filter(|v| (v.x as i32 - x).abs().max((v.y as i32 - y).abs()) <= 2)
            .min_by_key(|v| (v.x as i32 - x).abs().max((v.y as i32 - y).abs()))?;
        let tech = self.tribe_tech(v.tribe) as i32;
        let price = (60 + 40 * tech + 15 * v.pop as i32) * if v.capital { 2 } else { 1 };
        Some(Claim { tribe: v.tribe.saturating_sub(4) as usize, price })
    }

    /// Settle a land claim: pay for the land, or take it.
    pub fn settle_claim(&mut self, nation: u8, claim: &Claim, pay: bool) -> Result<(), String> {
        let n = nation as usize;
        if pay {
            if self.players[n].gold < claim.price {
                return Err(format!("The land costs {} gold.", claim.price));
            }
            self.players[n].gold -= claim.price;
            if let Some(t) = self.tribes.get_mut(claim.tribe) {
                t.alarm[n] = (t.alarm[n] - 10).max(0);
            }
            self.land_settled = true;
        } else if let Some(t) = self.tribes.get_mut(claim.tribe) {
            t.alarm[n] = (t.alarm[n] + 20).min(100);
        }
        Ok(())
    }

    /// Native affairs once a turn: trade, demands and gifts.
    pub(crate) fn native_relations(&mut self) {
        for ti in 0..self.tribes.len() {
            if self.tribes[ti].extinct {
                continue;
            }
            let tribe = 4 + ti as u8;
            let homes: Vec<(i32, i32)> = self.villages.iter().filter(|v| v.tribe == tribe).map(|v| (v.x as i32, v.y as i32)).collect();
            if homes.is_empty() {
                continue;
            }
            // colonies within reach of the tribe
            let near: Vec<(Id, u8)> = self
                .colonies
                .iter()
                .filter(|c| c.nation < 4)
                .filter(|c| homes.iter().any(|(x, y)| (c.x as i32 - x).abs().max((c.y as i32 - y).abs()) <= 6))
                .map(|c| (c.id, c.nation))
                .collect();
            if near.is_empty() {
                continue;
            }
            let (cid, nation) = near[self.rand(near.len() as u32) as usize];
            // a tribe the player hasn't met yet: the visit is the first meeting
            if nation == self.human && self.met_tribes & (1 << ti) == 0 {
                if self.chance(10) {
                    self.met_tribes |= 1 << ti;
                    self.meet_tribe(ti);
                }
                continue;
            }
            let alarm = self.tribes[ti].alarm[nation as usize];
            let name = self.nation_name(tribe).to_string();
            if alarm < 40 && self.chance(3) {
                // trade: buy something the colony has plenty of
                let stock = self.colony(cid).unwrap().stock;
                let goods: Vec<usize> = WANTED.iter().copied().filter(|&g| stock[g] >= 50).collect();
                if goods.is_empty() {
                    continue;
                }
                let g = goods[self.rand(goods.len() as u32) as usize];
                let amount = stock[g].min(100);
                let price = (self.bid_price(nation, g).max(1) * amount * (110 + self.tribe_tech(tribe) as i32 * 15) / 100).max(20);
                if nation == self.human {
                    self.events.push(Event::NativeTrade { tribe: ti, colony: cid, good: g as u8, amount, price });
                } else {
                    self.native_sale(ti, cid, g, amount, price);
                }
            } else if (40..75).contains(&alarm) && self.chance(2) {
                // demand: goods or gold, or else
                let stock = self.colony(cid).unwrap().stock;
                let g = (0..16).max_by_key(|&g| stock[g]).unwrap_or(0);
                let (good, amount) = if stock[g] >= 40 { (Some(g as u8), (stock[g] / 2).min(100)) } else { (None, (self.players[nation as usize].gold / 10).clamp(20, 300)) };
                if nation == self.human {
                    self.events.push(Event::NativeDemand { tribe: ti, colony: cid, good, amount });
                } else {
                    let give = self.chance(50);
                    self.answer_demand(ti, cid, good, amount, give);
                }
            } else if alarm < 15 && self.chance(2) {
                // gift
                let g = [FOOD, FURS, SUGAR, TOBACCO, COTTON][self.rand(5) as usize];
                let amount = 20 + self.rand(60) as i32;
                if let Some(c) = self.colony_mut(cid) {
                    c.stock[g] += amount;
                    let cname = c.name.clone();
                    if nation == self.human {
                        self.msg(format!("The {name} bring a gift of {amount} {} to {cname}.", GOOD_NAMES[g]));
                    }
                }
            }
        }
    }

    /// The natives pay for goods from a colony; muskets and horses arm their braves.
    pub fn native_sale(&mut self, ti: usize, cid: Id, good: usize, amount: i32, price: i32) {
        let Some(c) = self.colony_mut(cid) else { return };
        let amount = amount.min(c.stock[good]);
        c.stock[good] -= amount;
        let nation = c.nation;
        self.players[nation as usize].gold += price;
        if let Some(t) = self.tribes.get_mut(ti) {
            t.alarm[nation as usize] = (t.alarm[nation as usize] - 5).max(0);
        }
        let tribe = 4 + ti as u8;
        if good == MUSKETS || good == HORSES {
            let brave = self
                .units
                .iter()
                .find(|u| u.nation == tribe && matches!(u.kind, UnitType::Brave | UnitType::ArmedBrave | UnitType::MountedBrave))
                .map(|u| (u.id, u.kind));
            if let Some((id, k)) = brave {
                let new = match (good == MUSKETS, k) {
                    (true, UnitType::Brave) => UnitType::ArmedBrave,
                    (true, UnitType::MountedBrave) | (false, UnitType::ArmedBrave) => UnitType::MountedWarrior,
                    (false, UnitType::Brave) => UnitType::MountedBrave,
                    (_, k) => k,
                };
                self.unit_mut(id).unwrap().kind = new;
            }
        }
        if nation == self.human {
            let cname = self.colony(cid).map(|c| c.name.clone()).unwrap_or_default();
            self.msg(format!("The {} buy {amount} {} in {cname} for {price} gold.", self.nation_name(tribe), GOOD_NAMES[good]));
        }
    }

    /// Give in to a native demand, or refuse it.
    pub fn answer_demand(&mut self, ti: usize, cid: Id, good: Option<u8>, amount: i32, give: bool) {
        let Some(c) = self.colony(cid) else { return };
        let nation = c.nation as usize;
        let cname = c.name.clone();
        let tribe = self.nation_name(4 + ti as u8).to_string();
        if give {
            match good {
                Some(g) => {
                    let c = self.colony_mut(cid).unwrap();
                    c.stock[g as usize] = (c.stock[g as usize] - amount).max(0);
                }
                None => self.players[nation].gold = (self.players[nation].gold - amount).max(0),
            }
            if let Some(t) = self.tribes.get_mut(ti) {
                t.alarm[nation] = (t.alarm[nation] - 25).max(0);
            }
        } else {
            if let Some(t) = self.tribes.get_mut(ti) {
                t.alarm[nation] = (t.alarm[nation] + 20).min(100);
            }
            if nation as u8 == self.human {
                self.msg(format!("The {tribe} leave {cname} in anger."));
            }
        }
    }
}
