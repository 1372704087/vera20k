//! Original Rules66A7B3..66A822: the two message sound references use the
//! fixed SOUNDMD catalog and retain the last valid ID across reached passes.

use super::RuleSet;
use crate::rules::ini_parser::IniFile;
use crate::rules::process_owner::{NativeRulesProcessOwner, NativeScenarioRulesPrefix};
use crate::rules::sound_ini::SoundRegistry;
use std::sync::Arc;

#[test]
fn message_sounds_use_exact_bounded_catalog_readers_and_constructor_silence() {
    let sounds = SoundRegistry::from_ini(&IniFile::from_str(
        "[SoundList]\n0=Insert\n1=Typed\n[Insert]\nSounds=insert\n[Typed]\nSounds=typed\n\
         [Unregistered]\nSounds=unregistered\n",
    ));
    for (source, expected) in [
        ("", (None, None)),
        (
            "[AudioVisual]\nIncomingMessage=\nMessageCharTyped=\n",
            (None, None),
        ),
        (
            "[AudioVisual]\nIncomingMessage=Unregistered\nMessageCharTyped=Missing\n",
            (None, None),
        ),
        (
            "[AudioVisual]\nIncomingMessage= iNsErT \nMessageCharTyped= tYpEd \n",
            (Some("Insert"), Some("Typed")),
        ),
        (
            "[AudioVisual]\nincomingmessage=Insert\nmessagechartyped=Typed\n",
            (None, None),
        ),
        (
            "[audiovisual]\nIncomingMessage=Insert\nMessageCharTyped=Typed\n",
            (None, None),
        ),
        (
            "[General]\nIncomingMessage=Insert\nMessageCharTyped=Typed\n",
            (None, None),
        ),
        (
            "[AudioVisual]\nIncomingMessage=Insert,Typed\nMessageCharTyped=Typed,Insert\n",
            (None, None),
        ),
    ] {
        let ini = IniFile::from_str(source);
        let mut rules = RuleSet::from_ini(&ini).unwrap();
        assert_eq!(rules.general.incoming_message_sound(), None);
        assert_eq!(rules.general.message_char_typed_sound(), None);
        rules.bind_type_sound_references(&ini, &sounds);
        assert_eq!(
            (
                rules.general.incoming_message_sound(),
                rules.general.message_char_typed_sound()
            ),
            expected,
            "{source:?}"
        );
    }
}

#[test]
fn message_sounds_retain_valid_ids_through_the_production_rules_layers() {
    let sounds = Arc::new(SoundRegistry::from_ini(&IniFile::from_str(
        "[SoundList]\n0=First\n1=Second\n[First]\nSounds=first\n[Second]\nSounds=second\n",
    )));
    let source = |name: &str| {
        IniFile::from_str(&format!(
            "[AudioVisual]\nIncomingMessage={name}\nMessageCharTyped={name}\n"
        ))
    };
    let mut owner = NativeRulesProcessOwner::from_cold_start_sources(
        source("First"),
        Some(source("sEcOnD")),
        IniFile::empty(),
        sounds,
    )
    .unwrap();
    let (rules, _, _, _) = owner
        .load_scenario(
            NativeScenarioRulesPrefix::NonCampaign(Some(&source("Unknown"))),
            &source(""),
        )
        .unwrap()
        .into_parts();
    assert_eq!(rules.general.incoming_message_sound(), Some("Second"));
    assert_eq!(rules.general.message_char_typed_sound(), Some("Second"));
}

#[test]
fn message_sound_readers_cut_cached_bytes_before_trimming_and_lookup() {
    let sounds = SoundRegistry::from_ini(&IniFile::from_str("[SoundList]\n0=Insert\n"));
    for (value, expected) in [
        (format!("{}Insert", " ".repeat(123)), None),
        (format!("Insert{}", " ".repeat(128)), Some("Insert")),
    ] {
        let base = IniFile::from_str("[AudioVisual]\nIncomingMessage=Insert\n");
        let mut section = base.section("AudioVisual").unwrap().clone();
        // Bypass only lexical line trimming to supply the native cached
        // ReadString input; its128-byte buffer cuts to127 before strtrim.
        section.set("IncomingMessage", &value);
        section.set("MessageCharTyped", &value);
        let ini = IniFile::from_sections_for_test([section]);
        let mut rules = RuleSet::from_ini(&ini).unwrap();
        rules.bind_type_sound_references(&ini, &sounds);
        assert_eq!(
            rules.general.incoming_message_sound(),
            expected,
            "{value:?}"
        );
        assert_eq!(
            rules.general.message_char_typed_sound(),
            expected,
            "{value:?}"
        );
    }
}

#[test]
fn paid_opening_rules_bind_the_stock_message_cues_from_fixed_soundmd() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules_for_map("XMP03T4.MAP")
    else {
        return;
    };
    // Original physical native reader observations: House advice report and
    // naval_occupants.json's AudioVisual record. This checks the production
    // RULES/LANGRULE/mode/map and fixed SOUNDMD binding, not audible output.
    assert_eq!(
        retail.rules.general.incoming_message_sound(),
        Some("MessageText")
    );
    assert_eq!(
        retail.rules.general.message_char_typed_sound(),
        Some("TextBleep")
    );
}
