//! The twelve
//! [`crate::rules::phobos_techno_extras`].

use crate::rules::ini_parser::IniFile;
use crate::rules::phobos_shields::AffectedHouse;
use crate::rules::phobos_techno_extras::PhobosTechnoExtras;

fn read(keys: &str) -> PhobosTechnoExtras {
    PhobosTechnoExtras::from_section(
        IniFile::from_str(&format!("[Q]\n{keys}")).section_or_empty("Q"),
    )
}

/// `Body.h:482-485`, `:544`, `:602`, `:709-710`, `:725-726`, `:768`, `:851`:
/// seven unset nullables and five empty vectors.
#[test]
fn the_twelve_keys_construct_unset() {
    let defaults = PhobosTechnoExtras::default();
    assert_eq!(read(""), defaults);
    assert_eq!(defaults.radar_jam_houses, None);
    assert_eq!(defaults.radar_jam_delay, None);
    assert!(defaults.radar_jam_affect.is_empty());
    assert!(defaults.radar_jam_ignore.is_empty());
    assert_eq!(defaults.voice_created, None);
    assert_eq!(defaults.use_disguise_movement_speed, None);
    assert_eq!(defaults.no_queue_up_to_enter, None);
    assert_eq!(defaults.no_queue_up_to_enter_board_distance, None);
    assert_eq!(defaults.veteran_reload, None);
    assert_eq!(defaults.veteran_empty_reload, None);
    assert!(defaults.spawns_queue.is_empty());
    assert_eq!(defaults.drop_crate, None);
}

#[test]
fn every_key_reads_on_its_own() {
    let set = read(
        "RadarJamHouses=enemies\nRadarJamDelay=12\nRadarJamAffect=BARRACKS\nRadarJamIgnore=GAPILE\n\
         VoiceCreated=UnitCreated\nUseDisguiseMovementSpeed=yes\n\
         NoQueueUpToEnter=yes\nNoQueueUpToEnter.BoardDistance=512\n\
         VeteranReload=0.75\nVeteranEmptyReload=1.5\nSpawns.Queue=HORNET,ASW\n\
         DropCrate=Firepower\n",
    );
    assert_eq!(set.radar_jam_houses, Some(AffectedHouse::ENEMIES));
    assert_eq!(set.radar_jam_delay, Some(12));
    assert_eq!(set.radar_jam_affect, ["BARRACKS"]);
    assert_eq!(set.radar_jam_ignore, ["GAPILE"]);
    assert_eq!(set.voice_created.as_deref(), Some("UnitCreated"));
    assert_eq!(set.use_disguise_movement_speed, Some(true));
    assert_eq!(set.no_queue_up_to_enter, Some(true));
    assert_eq!(set.no_queue_up_to_enter_board_distance, Some(512));
    assert_eq!(set.veteran_reload, Some(0.75));
    assert_eq!(set.veteran_empty_reload, Some(1.5));
    assert_eq!(set.spawns_queue, ["HORNET", "ASW"]);
    // `POWERUP_NAMES[11] == "Firepower"`.
    assert_eq!(set.drop_crate, Some(11));
}

/// `Utilities/TemplateDef.h:1833-1849`: `Nullable::Read` resets on `<default>`
/// and on a blank read, and only those.
#[test]
fn a_default_or_blank_read_clears_the_nullables() {
    let cleared = read(
        "RadarJamDelay=<default>\nUseDisguiseMovementSpeed=\nVeteranReload=<default>\n\
         NoQueueUpToEnter.BoardDistance=\n",
    );
    assert_eq!(cleared.radar_jam_delay, None);
    assert_eq!(cleared.use_disguise_movement_speed, None);
    assert_eq!(cleared.veteran_reload, None);
    assert_eq!(cleared.no_queue_up_to_enter_board_distance, None);
}

/// `AffectedHouse` parses `none` as a value, so `RadarJamHouses=none` means
/// "jam nobody" and must not fall back to the `[General]` default. Only a
/// blank read or an unknown token leaves the key unset.
#[test]
fn radar_jam_houses_parses_none_as_a_value() {
    assert_eq!(
        read("RadarJamHouses=none\n").radar_jam_houses,
        Some(AffectedHouse::NONE)
    );
    assert_eq!(read("RadarJamHouses=\n").radar_jam_houses, None);
    assert_eq!(
        read("RadarJamHouses=enemies,allies\n")
            .radar_jam_houses
            .map(AffectedHouse::bits),
        Some(AffectedHouse::ENEMIES.bits() | AffectedHouse::ALLIES.bits())
    );
    assert_eq!(read("RadarJamHouses=bogus\n").radar_jam_houses, None);
}

/// `detail::read<Powerup>` (`Utilities/TemplateDef.h:648-676`) is an exact
/// scan of the fixed `[Powerups]` table: an unknown name is a parse failure,
/// and the match is case-insensitive.
#[test]
fn drop_crate_resolves_a_powerup_slot_and_rejects_unknown_names() {
    assert_eq!(read("DropCrate=money\n").drop_crate, Some(0));
    assert_eq!(read("DropCrate=HealBase\n").drop_crate, Some(2));
    assert_eq!(read("DropCrate=Pod\n").drop_crate, Some(18));
    assert_eq!(read("DropCrate=Bogus\n").drop_crate, None);
}

/// `ValueableVector::Read` is one `ReadString`, then a comma `strtok`; the
/// empty tokens the leading/trailing/double commas produce never reach the
/// vector.
#[test]
fn the_two_type_lists_split_on_commas_only() {
    let lists = read("RadarJamAffect=,AAA, BBB ,,CCC,\nRadarJamIgnore=DDD\n");
    assert_eq!(lists.radar_jam_affect, ["AAA", " BBB ", "CCC"]);
    assert_eq!(lists.radar_jam_ignore, ["DDD"]);
}
