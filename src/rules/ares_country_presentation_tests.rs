//! Ares 3.0 CountryExt presentation defaults, reads and ParentCountry copy.

use super::*;
use crate::rules::ruleset::RuleSet;

fn processed(ini: &str) -> RuleSet {
    let processed = RulesLayerStack::new(IniFile::from_str(ini))
        .process()
        .expect("country presentation rules parse");
    RuleSet::from_processed_rules(&processed).expect("country presentation projection")
}

const COUNTRIES: &str = "\
[Countries]\n\
0=C0\n1=C1\n2=C2\n3=C3\n4=C4\n5=C5\n6=C6\n7=C7\n8=C8\n9=C9\n10=Custom\n";

#[test]
fn ares_country_defaults_match_the_installed_initializer() {
    let rules = processed(&format!("{COUNTRIES}[Custom]\n"));
    assert_eq!(rules.country_flag("C0"), Some("usai.pcx"));
    assert_eq!(
        rules.country_status_key("C0"),
        Some("STT:PlayerSideAmerica")
    );
    assert_eq!(
        rules.country_taunt_pattern("C0"),
        Some("taunts\\tauam%02i.wav")
    );
    assert_eq!(rules.country_flag("C9"), Some("yrii.pcx"));
    assert_eq!(
        rules.country_status_key("C9"),
        Some("STT:PlayerSideYuriCountry")
    );
    assert!(rules.country_observer_flag_alt_palette("C9"));
    // The unknown branch at 0x1001C413.
    assert_eq!(rules.country_flag("Custom"), Some("rani.pcx"));
    assert_eq!(rules.country_status_key("Custom"), Some("GUI:Unknown"));
    assert_eq!(
        rules.country_taunt_pattern("Custom"),
        Some("taunts\\tauam%02i.wav")
    );
    assert_eq!(rules.country_list_index("Custom"), 100);
    assert_eq!(rules.country_random_weight("Custom"), 1);
    assert_eq!(rules.country_ai_random_weight("Custom"), 1);
}

#[test]
fn ares_country_reads_flags_status_taunts_order_and_weights() {
    let rules = processed(&format!(
        "{COUNTRIES}\
         [Custom]\n\
         File.Flag=MyFlag.PCX\n\
         MenuText.Status=STT:Custom\n\
         File.Taunt=Taunts\\custom~~.wav\n\
         File.ObserverFlag=obsflag.shp\n\
         File.ObserverBackground=obsbkg.shp\n\
         File.ObserverFlagAltPalette=yes\n\
         ListIndex=3\n\
         RandomSelectionWeight=4\n\
         AIRandomSelectionWeight=7\n"
    ));
    assert_eq!(rules.country_flag("Custom"), Some("MyFlag.PCX"));
    assert_eq!(rules.country_status_key("Custom"), Some("STT:Custom"));
    assert_eq!(
        rules.country_taunt_pattern("Custom"),
        Some("Taunts\\custom~~.wav")
    );
    assert_eq!(rules.country_observer_flag("Custom"), Some("obsflag.shp"));
    assert_eq!(
        rules.country_observer_background("Custom"),
        Some("obsbkg.shp")
    );
    assert!(rules.country_observer_flag_alt_palette("Custom"));
    assert_eq!(rules.country_list_index("Custom"), 3);
    assert_eq!(rules.country_random_weight("Custom"), 4);
    assert_eq!(rules.country_ai_random_weight("Custom"), 7);
}

#[test]
fn ares_country_parent_copies_presentation_and_child_keys_override() {
    let rules = processed(
        "[Countries]\n0=Parent\n1=Child\n\
         [Parent]\n\
         File.Flag=parent.pcx\n\
         MenuText.Status=STT:Parent\n\
         ListIndex=7\n\
         RandomSelectionWeight=2\n\
         [Child]\n\
         ParentCountry=Parent\n\
         File.Taunt=Taunts\\child~~.wav\n",
    );
    assert_eq!(rules.country_flag("Child"), Some("parent.pcx"));
    assert_eq!(rules.country_status_key("Child"), Some("STT:Parent"));
    assert_eq!(
        rules.country_taunt_pattern("Child"),
        Some("Taunts\\child~~.wav")
    );
    assert_eq!(rules.country_list_index("Child"), 8);
    assert_eq!(rules.country_random_weight("Child"), 2);
    assert_eq!(rules.country_ai_random_weight("Child"), 2);
}

#[test]
fn ares_country_empty_flag_clears_the_stock_default() {
    let mut countries = IniSection::new("Countries".into());
    for index in 0..11 {
        let id = if index == 10 {
            "Custom".to_owned()
        } else {
            format!("C{index}")
        };
        countries.set(&index.to_string(), &id);
    }
    let mut custom = IniSection::new("Custom".into());
    custom.set("File.Flag", "");
    let processed = RulesLayerStack::new(IniFile::from_sections_for_test([countries, custom]))
        .process()
        .expect("cleared flag rules parse");
    let rules = RuleSet::from_processed_rules(&processed).expect("cleared flag projection");
    assert_eq!(rules.country_flag("Custom"), None);
    assert_eq!(
        rules.country_flag_surface("Custom").as_deref(),
        Some("rani.pcx")
    );
    // `Game_GetFlagSurface` resumes on the `rani.pcx` branch (`0x4E3567`) when
    // the Country has no loaded flag surface, and only indices outside the
    // HouseType array keep the stock selection chain.
    assert_eq!(rules.country_insignia_pcx(10).as_deref(), Some("rani.pcx"));
    // Stock indices keep their initializer file: the hook answers from the
    // same field, so `0x4E3567` is only reached when that file is cleared.
    assert_eq!(rules.country_insignia_pcx(0).as_deref(), Some("usai.pcx"));
    assert_eq!(rules.country_insignia_pcx(11), None);
}

#[test]
fn ares_custom_country_insignia_replaces_the_stock_loading_chain() {
    let rules = processed(&format!("{COUNTRIES}[Custom]\nFile.Flag=MyFlag.PCX\n"));
    assert_eq!(
        rules.country_insignia_pcx(10).as_deref(),
        Some("MyFlag.PCX")
    );
    assert_eq!(rules.country_insignia_pcx(9).as_deref(), Some("yrii.pcx"));
}
