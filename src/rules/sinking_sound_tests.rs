//! The process owner binds native sound IDs to fixed SOUNDMD names, retaining
//! valid values across root/LANG/mode/map readers (712FF1/7130A5/6699C8).

use crate::rules::ini_parser::IniFile;
use crate::rules::sound_ini::SoundRegistry;

/// Consume the actual original selected readers, without equating native
/// fixture-relative Voc ordinals to Rust's independently built catalogue.
#[test]
fn original_radar_reference_reads_match_production_layers_and_retention() {
    use crate::rules::native_processing::{RulesLayerKind, RulesLayerStack};
    use crate::rules::ruleset::RuleSet;
    let native = crate::test_fixture::json("tools/input_oracle/radar_availability_audio.json");
    let sounds = SoundRegistry::from_ini(&crate::test_fixture::ini_sections(
        &native["retail"]["sound_sections"],
    ));
    let project = |layers: &RulesLayerStack| {
        let processed = layers.process().unwrap();
        let mut rules = RuleSet::from_processed_rules(&processed).unwrap();
        rules.bind_type_sound_references(processed.ini(), &sounds);
        rules
    };
    let names = |references: &serde_json::Value| {
        ["RadarOn", "RadarOff"].map(|key| {
            let id = references[key].as_i64().unwrap();
            if id == -1 {
                return None;
            }
            Some(
                native["retail"]["sounds"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|sound| sound["index"].as_i64() == Some(id))
                    .unwrap()["name"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
            )
        })
    };
    let actual = |rules: &RuleSet| {
        [
            rules.general.radar_on.clone(),
            rules.general.radar_off.clone(),
        ]
    };
    let mut layers = RulesLayerStack::new(IniFile::from_str(""));
    assert_eq!(
        actual(&project(&layers)),
        names(&native["retail"]["reference_constructor"])
    );
    let controls = native["retail"]["reader_controls"].as_array().unwrap();
    assert_eq!(controls.len(), 14);
    for row in controls {
        assert_eq!(
            actual(&project(&layers)),
            names(&row["before"]),
            "{} before",
            row["name"]
        );
        layers.push(
            RulesLayerKind::Scenario,
            crate::test_fixture::ini_sections(&row["input"]),
        );
        assert_eq!(
            actual(&project(&layers)),
            names(&row["after"]),
            "{} after",
            row["name"]
        );
    }

    // Native created NARADR before these selected field reads. The existing
    // processor receives that same declared registration prior, not an orphan
    // type body implicitly allocated by a later layer.
    let mut layers = RulesLayerStack::new(IniFile::from_str("[BuildingTypes]\n0=NARADR\n"));
    for row in native["retail"]["reference_layers"].as_array().unwrap() {
        if row["absent"] == true {
            assert_eq!(row["file"], "LANGRULE.INI");
            continue;
        }
        assert_eq!(actual(&project(&layers)), names(&row["before"]));
        let kind = match row["file"].as_str().unwrap() {
            "RULESMD.INI" => RulesLayerKind::RulesMd,
            "LANGRULE.INI" => RulesLayerKind::LangRule,
            "MPBattleMD.ini" => RulesLayerKind::GameMode,
            "XMP03T4.MAP" => RulesLayerKind::Scenario,
            other => panic!("unexpected original radar source {other}"),
        };
        layers.push(kind, crate::test_fixture::ini_sections(&row["input"]));
        let rules = project(&layers);
        assert_eq!(actual(&rules), names(&row["after"]), "{}", row["file"]);
        let radar = rules.object("NARADR").unwrap();
        assert_eq!(radar.radar, row["provider"]["radar"].as_bool().unwrap());
        assert_eq!(radar.spy_sat, row["provider"]["spy_sat"].as_bool().unwrap());
        assert_eq!(
            radar.strength,
            row["provider"]["strength"].as_i64().unwrap() as i32
        );
    }
}
