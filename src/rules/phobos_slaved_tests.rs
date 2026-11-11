//! `Slaved.OwnerWhenMasterKilled=` on an `InfantryType`.

use crate::rules::ini_parser::IniFile;
use crate::rules::phobos_slaved::{PhobosSlaved, SlaveChangeOwnerType};

fn read(keys: &str) -> PhobosSlaved {
    PhobosSlaved::from_section(IniFile::from_str(&format!("[Q]\n{keys}")).section_or_empty("Q"))
}

/// `Ext/InfantryType/Body.h:39`: the constructor default is `Killer`.
#[test]
fn an_absent_key_defaults_to_killer() {
    assert_eq!(
        read("").owner_when_master_killed,
        SlaveChangeOwnerType::Killer
    );
    assert_eq!(
        SlaveChangeOwnerType::default(),
        SlaveChangeOwnerType::Killer
    );
}

/// `Utilities/Enum.h:178-184`: the four names parse, case-insensitively, and
/// their native discriminants are the exact switch values — `Neutral` is
/// `4`, not `3`.
#[test]
fn every_name_parses_to_its_native_discriminant() {
    for (raw, kind, native) in [
        ("killer", SlaveChangeOwnerType::Killer, 0),
        ("master", SlaveChangeOwnerType::Master, 1),
        ("suicide", SlaveChangeOwnerType::Suicide, 2),
        ("neutral", SlaveChangeOwnerType::Neutral, 4),
    ] {
        let parsed = SlaveChangeOwnerType::parse(raw);
        assert_eq!(parsed, Some(kind), "{raw}");
        assert_eq!(parsed.unwrap().native_value(), native, "{raw}");
        assert_eq!(
            read(&format!("Slaved.OwnerWhenMasterKilled={raw}\n")).owner_when_master_killed,
            kind,
            "{raw} reads"
        );
    }
    assert_eq!(
        SlaveChangeOwnerType::parse("KILLER"),
        Some(SlaveChangeOwnerType::Killer)
    );
    assert_eq!(
        SlaveChangeOwnerType::parse("NeUtRaL"),
        Some(SlaveChangeOwnerType::Neutral)
    );
}

/// `Valueable<SlaveChangeOwnerType>::Read`: an unknown name is a parse
/// failure and leaves the current value (`Killer`) in place. There is no
/// `<default>`/`<none>` reset token on a `Valueable`.
#[test]
fn an_unknown_or_blank_read_keeps_killer() {
    for raw in ["nonsense", "3", "", "0"] {
        assert_eq!(
            read(&format!("Slaved.OwnerWhenMasterKilled={raw}\n")).owner_when_master_killed,
            SlaveChangeOwnerType::Killer,
            "{raw:?} keeps the default"
        );
    }
    assert_eq!(
        read("Slaved.OwnerWhenMasterKilled=<default>\n").owner_when_master_killed,
        SlaveChangeOwnerType::Killer,
        "<default> is not a reset token here"
    );
}
