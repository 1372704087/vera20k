//! Original CreateSound712E75..712EB7 and AudioVisual6691E0 scalar readers.
//! Constructor defaults are -1, independent of the authored stock values.
//! These checks exercise fixed-catalog binding and reached rules-pass history;
//! paid delivery and actual PCM output have separate production witnesses.

use super::RuleSet;
use crate::rules::ini_parser::IniFile;
use crate::rules::process_owner::{NativeRulesProcessOwner, NativeScenarioRulesPrefix};
use crate::rules::sound_ini::SoundRegistry;
use std::sync::Arc;

fn sounds() -> SoundRegistry {
    SoundRegistry::from_ini(&IniFile::from_str(
        "[SoundList]\n0=First\n1=Second\n[First]\nSounds=first\n\
         [Second]\nSounds=second\n[Unlisted]\nSounds=unlisted\n",
    ))
}

fn names(rules: &RuleSet) -> [Option<&str>; 4] {
    [
        rules.object("BORIS").unwrap().create_sound(),
        rules.general.create_unit_sound(),
        rules.general.create_infantry_sound(),
        rules.general.create_aircraft_sound(),
    ]
}

fn references(value: &str) -> String {
    format!(
        "[BORIS]\nCreateSound={value}\n[AudioVisual]\nCreateUnitSound={value}\n\
         CreateInfantrySound={value}\nCreateAircraftSound={value}\n"
    )
}

#[test]
fn creation_sounds_require_exact_keys_and_registered_bounded_scalar_names() {
    let sounds = sounds();
    for (source, expected) in [
        (String::new(), None),
        (references(""), None),
        (references("Unknown"), None),
        (references("Unlisted"), None),
        (references("First,Second"), None),
        (references(" fIrSt "), Some("First")),
        (
            "[BORIS]\ncreatesound=First\n[AudioVisual]\ncreateunitsound=First\n\
             createinfantrysound=First\ncreateaircraftsound=First\n"
                .to_owned(),
            None,
        ),
        (
            "[boris]\nCreateSound=First\n[audiovisual]\nCreateUnitSound=First\n\
             CreateInfantrySound=First\nCreateAircraftSound=First\n"
                .to_owned(),
            None,
        ),
        (
            "[General]\nCreateSound=First\nCreateUnitSound=First\n\
             CreateInfantrySound=First\nCreateAircraftSound=First\n"
                .to_owned(),
            None,
        ),
    ] {
        let ini = IniFile::from_str(&format!("[InfantryTypes]\n0=BORIS\n{source}"));
        let mut rules = RuleSet::from_ini(&ini).unwrap();
        assert_eq!(names(&rules), [None; 4], "unbound constructor silence");
        rules.bind_type_sound_references(&ini, &sounds);
        assert_eq!(names(&rules), [expected; 4], "{source:?}");
    }
}

#[test]
fn creation_sounds_retain_the_last_valid_id_through_actual_rules_passes() {
    let sounds = Arc::new(sounds());
    for (lang, mode, map, expected) in [
        (
            references(""),
            references("Unknown"),
            references("Unlisted"),
            "First",
        ),
        (
            references("Second"),
            references(""),
            references("Unknown"),
            "Second",
        ),
        (
            String::new(),
            references("Unknown"),
            references(" sEcOnD "),
            "Second",
        ),
    ] {
        let root = IniFile::from_str(&format!(
            "[InfantryTypes]\n0=BORIS\n{}",
            references("First")
        ));
        let mut owner = NativeRulesProcessOwner::from_cold_start_sources(
            root,
            Some(IniFile::from_str(&lang)),
            IniFile::empty(),
            Arc::clone(&sounds),
        )
        .unwrap();
        let (rules, _, _, _) = owner
            .load_scenario(
                NativeScenarioRulesPrefix::NonCampaign(Some(&IniFile::from_str(&mode))),
                &IniFile::from_str(&map),
            )
            .unwrap()
            .into_parts();
        assert_eq!(names(&rules), [Some(expected); 4]);
    }
}

#[test]
fn creation_sounds_cut_cached_bytes_before_trimming_and_catalog_lookup() {
    let sounds = sounds();
    for (value, expected) in [
        (format!("{}First", " ".repeat(123)), None),
        (format!("First{}", " ".repeat(128)), Some("First")),
    ] {
        let base = IniFile::from_str(&format!(
            "[InfantryTypes]\n0=BORIS\n{}",
            references("First")
        ));
        let mut ty = base.section("BORIS").unwrap().clone();
        ty.set("CreateSound", &value);
        let mut general = base.section("AudioVisual").unwrap().clone();
        for key in [
            "CreateUnitSound",
            "CreateInfantrySound",
            "CreateAircraftSound",
        ] {
            general.set(key, &value);
        }
        let ini = IniFile::from_sections_for_test([
            base.section("InfantryTypes").unwrap().clone(),
            ty,
            general,
        ]);
        let mut rules = RuleSet::from_ini(&ini).unwrap();
        rules.bind_type_sound_references(&ini, &sounds);
        assert_eq!(names(&rules), [expected; 4], "{value:?}");
    }
}

#[test]
fn stock_boris_creation_cue_binds_from_the_real_scenario_and_fixed_sound_catalog() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules_for_map("XMP03T4.MAP")
    else {
        return;
    };
    // Original full type reader/physical RULESMD, absent LANGRULE, then mode
    // and map passes resolved BorisCreated. All three Rules fallbacks remain
    // the constructor -1. The cue is not inferred from the type's name.
    assert_eq!(
        names(&retail.rules),
        [Some("BorisCreated"), None, None, None]
    );
}
