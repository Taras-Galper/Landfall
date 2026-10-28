//! The Crown's taxes, and the colonists' answer: refusing a raise by
//! dumping a cargo into the harbour, which boycotts that good in the homeland
//! until the arrears are paid.

use crate::colony::GOOD_NAMES;
use crate::events::Event;
use crate::game::*;

/// A tax raise waiting for the human's answer.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TaxDemand {
    pub rate: u8,
    pub good: u8,
    /// The colony that would hold the party (most of that good).
    pub colony: Option<Id>,
}

impl Game {
    /// The Crown wants more. Computer players pay; the human is asked.
    pub fn raise_tax(&mut self, nation: u8, good: usize) {
        let raise = 1 + self.rand(4) as u8;
        let rate = (self.players[nation as usize].tax + raise).min(70);
        if nation != self.human {
            self.players[nation as usize].tax = rate;
            return;
        }
        if self.tax_demand.is_some() {
            return;
        }
        let colony = self
            .colonies
            .iter()
            .filter(|c| c.nation == nation && c.stock[good] > 0)
            .max_by_key(|c| c.stock[good])
            .map(|c| c.id);
        self.tax_demand = Some(TaxDemand { rate, good: good as u8, colony });
        let gname = GOOD_NAMES[good];
        self.event(Event::Crown {
            text: format!(
                "Our treasury has need of the profits from your {gname} trade. We are raising the tax rate to {rate}%. Your loyalty has been noted."
            ),
        });
        self.event(Event::TaxDemand);
    }

    /// The human accepts the new rate.
    pub fn accept_tax(&mut self) {
        if let Some(d) = self.tax_demand.take() {
            let h = self.human as usize;
            self.players[h].tax = d.rate;
            self.msg(format!("The tax rate is now {}%.", d.rate));
        }
    }

    /// The human refuses: the goods go into the harbour and are boycotted.
    pub fn refuse_tax(&mut self) -> Result<(), String> {
        let d = self.tax_demand.clone().ok_or("No tax demand.")?;
        let cid = d.colony.ok_or("No colony holds those goods.")?;
        self.tax_demand = None;
        let g = d.good as usize;
        let h = self.human;
        let pop = self.population(cid) as i32;
        let Some(c) = self.colony_mut(cid) else { return Err("The colony is gone.".into()) };
        let dumped = c.stock[g];
        c.stock[g] = 0;
        // the act of defiance stirs the town
        c.liberty += 100 * pop.max(1);
        let name = c.name.clone();
        self.players[h as usize].boycott[g] = true;
        self.msg(format!(
            "PROTEST! Colonists in {name} dump {dumped} {} into the harbour rather than pay the Crown's tax. {} are now boycotted in {}.",
            GOOD_NAMES[g],
            GOOD_NAMES[g],
            self.rules.names.homeland
        ));
        crate::sound::cue(self, crate::sound::Cue::Cheer, true);
        Ok(())
    }

    pub fn boycotted(&self, nation: u8, good: usize) -> bool {
        self.players.get(nation as usize).is_some_and(|p| p.boycott.get(good).copied().unwrap_or(false))
    }

    /// Back taxes owed before a boycotted good may be traded again.
    pub fn arrears(&self, nation: u8, good: usize) -> i32 {
        (self.bid_price(nation, good).max(1) * 100).max(200)
    }

    pub fn pay_arrears(&mut self, nation: u8, good: usize) -> Result<i32, String> {
        if !self.boycotted(nation, good) {
            return Err("That good is not boycotted.".into());
        }
        let cost = self.arrears(nation, good);
        if self.players[nation as usize].gold < cost {
            return Err(format!("The arrears are {cost} gold."));
        }
        self.players[nation as usize].gold -= cost;
        self.players[nation as usize].boycott[good] = false;
        Ok(cost)
    }

    /// Every boycott is forgiven.
    pub fn lift_boycotts(&mut self, nation: u8) {
        if let Some(p) = self.players.get_mut(nation as usize) {
            p.boycott = [false; 16];
        }
    }
}
