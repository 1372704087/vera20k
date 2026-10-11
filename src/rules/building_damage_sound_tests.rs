//! Damage-sound catalog binding through the actual retained rules passes.
//! These reader regressions do not establish receiver arithmetic parity.
//! Original gamemd SHA2561cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c:
//! type712EB7..712F07 and Rules66AC98..66ACDD use ReadString528A10 (128 bytes)
//! then VocFindIndex7514D0, retaining previous valid IDs on failed resolution.

use super::RuleSet;
use crate::rules::ini_parser::IniFile;
use crate::rules::process_owner::{NativeRulesProcessOwner, NativeScenarioRulesPrefix};
use crate::rules::sound_ini::SoundRegistry;
use std::sync::Arc;

fn sounds() -> SoundRegistry {
    SoundRegistry::from_ini(&IniFile::from_str(
        "[SoundList]\n0=TypeDamage\n1=GlobalDamage\n2=ReplacementDamage\n3=Dummy\n\
         [TypeDamage]\nSounds=type1\n\
         [GlobalDamage]\nSounds=global1\n\
         [ReplacementDamage]\nSounds=replacement1\n\
         [UnlistedDamage]\nSounds=unlisted1\n",
    ))
}

#[test]
fn damage_sound_binds_exact_keys_to_registered_names_from_silent_defaults() {
    let sounds = sounds();
    for (text, expected_type, expected_global) in [
        ("", None, None),
        (
            "[GAWEAP]\nDamageSound=\n[AudioVisual]\nBuildingDamageSound=\n",
            None,
            None,
        ),
        (
            "[GAWEAP]\nDamageSound=UnlistedDamage\n[AudioVisual]\nBuildingDamageSound=MissingDamage\n",
            None,
            None,
        ),
        (
            "[GAWEAP]\nDamageSound=  tYpEdAmAgE  \n[AudioVisual]\nBuildingDamageSound= gLoBaLdAmAgE \n",
            Some("TypeDamage"),
            Some("GlobalDamage"),
        ),
        (
            "[GAWEAP]\ndamagesound=TypeDamage\n[AudioVisual]\nbuildingdamagesound=GlobalDamage\n",
            None,
            None,
        ),
        (
            "[gaweap]\nDamageSound=TypeDamage\n[audiovisual]\nBuildingDamageSound=GlobalDamage\n",
            None,
            None,
        ),
        (
            "[General]\nDamageSound=TypeDamage\nBuildingDamageSound=GlobalDamage\n",
            None,
            None,
        ),
    ] {
        let ini = IniFile::from_str(&format!("[BuildingTypes]\n0=GAWEAP\n{text}"));
        let mut rules = RuleSet::from_ini(&ini).unwrap();
        assert_eq!(rules.object("GAWEAP").unwrap().damage_sound(), None);
        assert_eq!(rules.general.building_damage_sound(), None);
        rules.bind_type_sound_references(&ini, &sounds);
        assert_eq!(
            (
                rules.object("GAWEAP").unwrap().damage_sound(),
                rules.general.building_damage_sound(),
            ),
            (expected_type, expected_global),
            "{text:?}"
        );
    }
}

#[test]
fn damage_sound_references_resolve_and_retain_through_production_rules() {
    let sounds = Arc::new(sounds());
    for (type_name, global_name, lang, mode, map, expected_type, expected_global) in [
        ("UnlistedDamage", "MissingDamage", "", "", "", None, None),
        (
            "tYpEdAmAgE",
            "gLoBaLdAmAgE",
            "",
            "",
            "",
            Some("TypeDamage"),
            Some("GlobalDamage"),
        ),
        (
            "TypeDamage",
            "GlobalDamage",
            "[GAWEAP]\nDamageSound=\n[AudioVisual]\nBuildingDamageSound=MissingDamage\n",
            "[GAWEAP]\nDamageSound=UnlistedDamage\n[AudioVisual]\nBuildingDamageSound=\n",
            "[GAWEAP]\ndamagesound=ReplacementDamage\n[AudioVisual]\nbuildingdamagesound=ReplacementDamage\n",
            Some("TypeDamage"),
            Some("GlobalDamage"),
        ),
        (
            "TypeDamage",
            "GlobalDamage",
            "[GAWEAP]\nDamageSound=ReplacementDamage\n[AudioVisual]\nBuildingDamageSound=ReplacementDamage\n",
            "[GAWEAP]\nDamageSound=MissingDamage\n[AudioVisual]\nBuildingDamageSound=UnlistedDamage\n",
            "[GAWEAP]\nDamageSound=\n[AudioVisual]\nBuildingDamageSound=\n",
            Some("ReplacementDamage"),
            Some("ReplacementDamage"),
        ),
        (
            "TypeDamage",
            "GlobalDamage",
            "[GAWEAP]\nDamageSound=Dummy\n[AudioVisual]\nBuildingDamageSound=Dummy\n",
            "",
            "",
            Some("Dummy"),
            Some("Dummy"),
        ),
    ] {
        let root = IniFile::from_str(&format!(
            "[BuildingTypes]\n0=GAWEAP\n[GAWEAP]\nStrength=100\nDamageSound={type_name}\n\
             [AudioVisual]\nBuildingDamageSound={global_name}\n"
        ));
        let mut owner = NativeRulesProcessOwner::from_cold_start_sources(
            root,
            Some(IniFile::from_str(lang)),
            IniFile::empty(),
            Arc::clone(&sounds),
        )
        .unwrap();
        let (rules, _, _, _) = owner
            .load_scenario(
                NativeScenarioRulesPrefix::NonCampaign(Some(&IniFile::from_str(mode))),
                &IniFile::from_str(map),
            )
            .unwrap()
            .into_parts();
        assert_eq!(
            (
                rules.object("GAWEAP").unwrap().damage_sound(),
                rules.general.building_damage_sound(),
            ),
            (expected_type, expected_global),
            "root type={type_name:?}, global={global_name:?}, LANGRULE={lang:?}, mode={mode:?}, map={map:?}"
        );
    }
}

#[test]
fn damage_sound_reads_one_bounded_name_before_catalog_lookup() {
    let sounds = sounds();
    for (value, expected) in [
        (format!("{}TypeDamage", " ".repeat(120)), None),
        (format!("TypeDamage{}", " ".repeat(128)), Some("TypeDamage")),
        ("TypeDamage,GlobalDamage".to_owned(), None),
    ] {
        // Physical lexical loading trims values first. These original reader
        // controls explicitly supply the cached value at the ReadString entry.
        let base = IniFile::from_str(
            "[BuildingTypes]\n0=GAWEAP\n[GAWEAP]\nDamageSound=TypeDamage\n\
             [AudioVisual]\nBuildingDamageSound=TypeDamage\n",
        );
        let mut type_section = base.section("GAWEAP").unwrap().clone();
        type_section.set("DamageSound", &value);
        let mut audio_visual = base.section("AudioVisual").unwrap().clone();
        audio_visual.set("BuildingDamageSound", &value);
        let ini = IniFile::from_sections_for_test([
            base.section("BuildingTypes").unwrap().clone(),
            type_section,
            audio_visual,
        ]);
        let mut rules = RuleSet::from_ini(&ini).unwrap();
        rules.bind_type_sound_references(&ini, &sounds);
        assert_eq!(
            (
                rules.object("GAWEAP").unwrap().damage_sound(),
                rules.general.building_damage_sound(),
            ),
            (expected, expected),
            "{value:?}"
        );
    }
}

#[test]
fn retail_damage_sound_references_bind_through_the_production_process_owner() {
    let Some((root, art)) = crate::rules::retail_ini_fixture::retail_rules_and_art() else {
        return;
    };
    let Some(sound_ini) = crate::rules::retail_ini_fixture::retail_ini("soundmd.ini") else {
        return;
    };
    let mut owner = NativeRulesProcessOwner::from_cold_start_sources(
        root,
        None,
        art,
        Arc::new(SoundRegistry::from_ini(&sound_ini)),
    )
    .unwrap();
    let (rules, _, _, _) = owner
        .load_scenario(
            NativeScenarioRulesPrefix::NonCampaign(None),
            &IniFile::empty(),
        )
        .unwrap()
        .into_parts();
    assert_eq!(
        rules.general.building_damage_sound(),
        Some("BuildingDamaged")
    );
    assert_eq!(
        rules.object("CAMISC03").unwrap().damage_sound(),
        Some("BuildingMetalDamaged")
    );
    assert_eq!(rules.object("GAWEAP").unwrap().damage_sound(), None);
}

#[test]
fn damage_sound_global_reference_retention_matches_whole_native_reader_passes() {
    let Some((root, art)) = crate::rules::retail_ini_fixture::retail_rules_and_art() else {
        return;
    };
    let Some(sound_ini) = crate::rules::retail_ini_fixture::retail_ini("soundmd.ini") else {
        return;
    };
    let corpus: serde_json::Value = serde_json::from_str(crate::test_fixture::text(
        "tools/spatial_oracle/building_damage_sound_joined.json",
    ))
    .unwrap();
    assert_eq!(corpus["selected_profile"], "all");
    assert_eq!(
        corpus["native_sha256"],
        "1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c"
    );
    let passes = corpus["threshold_controls"]["global_later_passes"]
        .as_array()
        .unwrap();
    assert_eq!(passes.len(), 3, "all executed later reader controls");
    let name = |index: &serde_json::Value| -> Option<&str> {
        let index = index.as_i64().unwrap();
        if index == -1 {
            None
        } else {
            Some(
                corpus["selected_sounds"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|sound| sound["fixture_index"].as_i64().unwrap() == index)
                    .unwrap()["name"]
                    .as_str()
                    .unwrap(),
            )
        }
    };
    let sounds = Arc::new(SoundRegistry::from_ini(&sound_ini));
    let mut previous = name(&passes[0]["before"]);
    for end in 0..=passes.len() {
        // Original full6691E0 performs each later AV read in place. Exercise
        // the same exact values as ordered LANG/mode/map production passes;
        // no public/raw GeneralRules writer or partial rebinding is used.
        let layers: Vec<_> = passes[..end]
            .iter()
            .map(|pass| {
                IniFile::from_str(&format!(
                    "[AudioVisual]\nBuildingDamageSound={}\n",
                    pass["raw"].as_str().unwrap()
                ))
            })
            .collect();
        let mut owner = NativeRulesProcessOwner::from_cold_start_sources(
            root.clone(),
            layers.first().cloned(),
            art.clone(),
            Arc::clone(&sounds),
        )
        .unwrap();
        let rules = owner
            .load_scenario(
                NativeScenarioRulesPrefix::NonCampaign(layers.get(1)),
                layers.get(2).unwrap_or(&IniFile::empty()),
            )
            .unwrap()
            .into_parts()
            .0;
        if end == 0 {
            assert_eq!(rules.general.building_damage_sound(), previous);
        } else {
            let native = &passes[end - 1];
            let label = native["label"].as_str().unwrap();
            assert_eq!(previous, name(&native["before"]), "{label}: prior binding");
            assert_ne!(native["returned"].as_u64().unwrap(), 0, "{label}");
            let expected = name(&native["after"]);
            assert_eq!(
                rules.general.building_damage_sound(),
                expected,
                "{label}: retained production AV reader"
            );
            previous = expected;
        }
    }
}
