//! Ares 3.0 `[Country] ParaDrop.*` and `Parachute.Anim=` overrides, read by
//! `HouseTypeExt::LoadFromRulesFile @ 0x1001C530` and consulted by the three
//! country resolvers before the SideExt values.

use super::*;
use crate::rules::ruleset::RuleSet;

fn processed(ini: &str) -> RuleSet {
    let processed = RulesLayerStack::new(IniFile::from_str(ini))
        .process()
        .expect("country paradrop rules parse");
    RuleSet::from_processed_rules(&processed).expect("country paradrop projection")
}

const REGISTRY: &str = "\
[InfantryTypes]\n0=ENG\n\
[VehicleTypes]\n0=MTANK\n\
[AircraftTypes]\n0=CARGO\n1=PLANEZ\n\
[Animations]\n0=PARA1\n";

#[test]
fn ares_country_paradrop_overrides_resolve_by_identity() {
    let rules = processed(&format!(
        "{REGISTRY}[Countries]\n0=Greece\n\
         [Greece]\n\
         Parachute.Anim=PARA1\n\
         ParaDrop.Aircraft=PLANEZ\n\
         ParaDrop.Types=MTANK,ENG,NOTHING\n\
         ParaDrop.Num=2,3\n"
    ));
    assert_eq!(rules.country_parachute_anim("Greece"), Some("PARA1"));
    assert_eq!(rules.country_paradrop_aircraft("Greece"), Some("PLANEZ"));
    let expected: Option<Option<(&[String], &[i32])>> = Some(Some((
        &[String::from("MTANK"), String::from("ENG")][..],
        &[2, 3][..],
    )));
    // `remove_non_paradroppables` drops the unregistered third name.
    assert_eq!(rules.country_paradrop_lists("Greece"), expected);
}

#[test]
fn ares_country_paradrop_absence_defers_and_types_without_counts_send_nothing() {
    let unset = processed(&format!("{REGISTRY}[Countries]\n0=Greece\n[Greece]\n"));
    assert_eq!(unset.country_paradrop_lists("Greece"), None);
    assert_eq!(unset.country_paradrop_aircraft("Greece"), None);
    assert_eq!(unset.country_parachute_anim("Greece"), None);

    // `GetParadropContent @ 0x1001BC20` returns false for a non-empty type list
    // with no counts, so the launch sends nothing instead of deferring.
    let no_counts = processed(&format!(
        "{REGISTRY}[Countries]\n0=Greece\n[Greece]\nParaDrop.Types=ENG\n"
    ));
    assert_eq!(no_counts.country_paradrop_lists("Greece"), Some(None));

    // A key naming nothing registered stores the null/-1 value, which every
    // resolver treats as "defer to the side".
    let unknown = processed(&format!(
        "{REGISTRY}[Countries]\n0=Greece\n[Greece]\n\
         Parachute.Anim=NOPE\nParaDrop.Aircraft=NOPE\n"
    ));
    assert_eq!(unknown.country_parachute_anim("Greece"), None);
    assert_eq!(unknown.country_paradrop_aircraft("Greece"), None);
}

#[test]
fn ares_country_parent_inherits_paradrop_overrides() {
    let rules = processed(&format!(
        "{REGISTRY}[Countries]\n0=Greece\n1=Rome\n\
         [Greece]\n\
         Parachute.Anim=PARA1\n\
         ParaDrop.Aircraft=PLANEZ\n\
         ParaDrop.Types=MTANK\n\
         ParaDrop.Num=4\n\
         [Rome]\n\
         ParentCountry=Greece\n"
    ));
    assert_eq!(rules.country_parachute_anim("Rome"), Some("PARA1"));
    assert_eq!(rules.country_paradrop_aircraft("Rome"), Some("PLANEZ"));
    let expected: Option<Option<(&[String], &[i32])>> =
        Some(Some((&[String::from("MTANK")][..], &[4][..])));
    assert_eq!(rules.country_paradrop_lists("Rome"), expected);
}
