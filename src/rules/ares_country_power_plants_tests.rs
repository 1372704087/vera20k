//! Ares 3.0 `[Country] AI.PowerPlants=` (HouseTypeExt), which
//! `GetPowerplants @ 0x1001BD20` answers while the list is non-empty.

use super::*;
use crate::rules::ruleset::RuleSet;

fn processed(ini: &str) -> RuleSet {
    let processed = RulesLayerStack::new(IniFile::from_str(ini))
        .process()
        .expect("country power plant rules parse");
    RuleSet::from_processed_rules(&processed).expect("country power plant projection")
}

const REGISTRY: &str = "\
[BuildingTypes]\n0=GAPOWR\n1=NAAPWR\n2=NAPOWR\n\
[GAPOWR]\n[NAAPWR]\n[NAPOWR]\n";

#[test]
fn ares_country_power_plants_keep_registered_building_identities() {
    let rules = processed(&format!(
        "{REGISTRY}[Countries]\n0=Greece\n\
         [Greece]\nAI.PowerPlants=NAAPWR,UNKNOWN,NAPOWR\n"
    ));
    let plants: Option<&[String]> = rules.country_power_plants("Greece");
    assert_eq!(
        plants,
        Some(&[String::from("NAAPWR"), String::from("NAPOWR")][..])
    );
}

#[test]
fn ares_country_without_power_plants_defers_to_the_side_plant() {
    let rules = processed(&format!("{REGISTRY}[Countries]\n0=Greece\n[Greece]\n"));
    assert_eq!(rules.country_power_plants("Greece"), None);
    // An unallocated country keeps every caller's stock path too.
    assert_eq!(rules.country_power_plants("Nowhere"), None);
}

#[test]
fn ares_country_parent_inherits_power_plants() {
    let rules = processed(&format!(
        "{REGISTRY}[Countries]\n0=Greece\n1=Rome\n\
         [Greece]\nAI.PowerPlants=NAAPWR\n\
         [Rome]\nParentCountry=Greece\n"
    ));
    assert_eq!(
        rules.country_power_plants("Rome"),
        Some(&[String::from("NAAPWR")][..])
    );
}
