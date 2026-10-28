//! The rules file covers everything the engine needs.
use landfall::rules::Rules;

#[test]
fn standard_rules_are_complete() {
    let r = Rules::standard();
    assert_eq!(r.terrain.len(), 21);
    assert_eq!(r.units.len(), 23);
    assert_eq!(r.cargo.len(), 16);
    assert_eq!(r.members.len(), 25);
    assert_eq!(r.tribes.len(), 8);
    assert_eq!(r.names.nations.len(), 13);
    assert_eq!(r.names.units.len(), 23);
    assert_eq!(r.names.professions.len(), 29);
    for n in 0..4u8 {
        assert!(r.colony_names.get(&n).is_some_and(|v| v.len() >= 20));
    }
    let back = Rules::from_json(&r.to_json()).unwrap();
    assert_eq!(back.members.len(), r.members.len());
}
