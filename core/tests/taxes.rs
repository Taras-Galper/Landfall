//! Tax raises, harbour protests and boycotts.
mod common;
use landfall::events::Event;
use landfall::game::*;
use landfall::types::UnitType;

#[test]
fn protest_and_boycott() {
    let (mut g, cols) = common::with_colonies(3, 2, 2);
    let h = g.human;
    let cid = cols[1];
    g.colony_mut(cid).unwrap().stock[FURS] = 5000;
    let tax0 = g.players[h as usize].tax;
    g.raise_tax(h, FURS);
    assert!(g.events.contains(&Event::TaxDemand));
    assert_eq!(g.players[h as usize].tax, tax0, "nothing changes until the human answers");
    let d = g.tax_demand.clone().unwrap();
    assert_eq!(d.colony, Some(cid), "the colony with most furs");
    g.refuse_tax().unwrap();
    assert_eq!(g.colony(cid).unwrap().stock[FURS], 0);
    assert!(g.boycotted(h, FURS));
    assert_eq!(g.players[h as usize].tax, tax0);
    let ship = g.spawn(UnitType::Merchantman, h, P_FREE, Loc::Homeland);
    g.unit_mut(ship).unwrap().cargo.push((FURS as u8, 100));
    assert!(g.sell_cargo(ship, 0).is_err());
    assert!(g.buy_cargo(ship, FURS).is_err());
    g.players[h as usize].gold = 100_000;
    g.pay_arrears(h, FURS).unwrap();
    assert!(g.sell_cargo(ship, 0).is_ok());
    g.raise_tax(h, SUGAR);
    g.accept_tax();
    assert!(g.players[h as usize].tax > tax0);
}
