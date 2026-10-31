use super::{InsigniaTypeCatalog, normalize_insignia_image};
use crate::rules::ini_parser::IniFile;

fn catalog(text: &str) -> InsigniaTypeCatalog {
    let mut catalog = InsigniaTypeCatalog::default();
    catalog.apply_pass(&IniFile::from_str(text));
    catalog
}

#[test]
fn insignia_type_defaults_and_promotable_overrides() {
    let catalog = catalog(
        "[InsigniaTypes]\n0=PLAIN\n1=FULL\n\
         [PLAIN]\n\
         [FULL]\nInsignia=shared\nInsignia.Veteran=vet\n\
         InsigniaFrame=4\nInsigniaFrame.Elite=7\n",
    );
    let plain = catalog.entry("plain").expect("case-insensitive entry");
    assert_eq!(plain.images, [None, None, None]);
    assert_eq!(plain.frames, [-1, -1, -1]);

    let full = catalog.entry("FULL").expect("entry");
    assert_eq!(
        full.images,
        [
            Some("shared.shp".to_owned()),
            Some("vet.shp".to_owned()),
            Some("shared.shp".to_owned())
        ],
        "the common key sets all three ranks, then Veteran overrides one"
    );
    assert_eq!(full.frames, [4, 4, 7]);
}

#[test]
fn insignia_images_apply_the_shp_suffix_case_insensitively() {
    assert_eq!(normalize_insignia_image("RANK"), "RANK.shp");
    assert_eq!(normalize_insignia_image("RANK.SHP"), "RANK.SHP");
    assert_eq!(normalize_insignia_image("Rank.shp"), "Rank.shp");
}

#[test]
fn later_passes_append_entries_and_keep_previous_values() {
    let mut catalog = catalog("[InsigniaTypes]\n0=FIRST\n[FIRST]\nInsignia=a\nInsigniaFrame=1\n");
    catalog.apply_pass(&IniFile::from_str(
        "[InsigniaTypes]\n0=SECOND\n[SECOND]\nInsignia=b\n[FIRST]\n",
    ));
    assert_eq!(catalog.entries().len(), 2);
    let first = catalog.entry("FIRST").unwrap();
    assert_eq!(first.images[0].as_deref(), Some("a.shp"));
    assert_eq!(first.frames, [1, 1, 1], "absent keys keep the earlier pass");
    assert_eq!(
        catalog.entry("SECOND").unwrap().images[0].as_deref(),
        Some("b.shp")
    );
}

use crate::rules::building_select_bracket::BuildingSelectBracketPosition;
use crate::rules::ini_parser::IniSection;

fn section(text: &str) -> IniFile {
    IniFile::from_str(text)
}

#[test]
fn phobos_draw_globals_read_general_and_audio_visual_defaults() {
    let plain = super::PhobosDrawGlobals::from_ini(&IniFile::from_str("[General]\n"));
    assert_eq!(plain, super::PhobosDrawGlobals::default());
    assert!(plain.enemy_insignia);
    assert_eq!(plain.adjust_infantry, [5, 2]);
    assert_eq!(plain.adjust_units, [10, 6]);
    assert_eq!(plain.adjust_buildings, [10, 6]);
    assert_eq!(plain.buildings_anchor, None);
    assert_eq!(
        plain.disguise_blinking_visibility,
        crate::rules::phobos_shields::AffectedHouse::OWNER
    );

    let ini = IniFile::from_str(
        "[General]\nEnemyInsignia=no\nDisguiseBlinkingVisibility=enemies\n\
         [AudioVisual]\nDrawInsignia.OnlyOnSelected=yes\n\
         DrawInsignia.UsePixelSelectionBracketDelta=yes\n\
         DrawInsignia.AdjustPos.Infantry=7,3\n\
         DrawInsignia.AdjustPos.Units=12,8\n\
         DrawInsignia.AdjustPos.Buildings=14,9\n\
         DrawInsignia.AdjustPos.BuildingsAnchor=leftbottom\n",
    );
    let globals = super::PhobosDrawGlobals::from_ini(&ini);
    assert!(!globals.enemy_insignia);
    assert!(globals.only_on_selected);
    assert!(globals.use_pixel_selection_bracket_delta);
    assert_eq!(globals.adjust_infantry, [7, 3]);
    assert_eq!(globals.adjust_units, [12, 8]);
    assert_eq!(globals.adjust_buildings, [14, 9]);
    assert_eq!(
        globals.buildings_anchor,
        Some(BuildingSelectBracketPosition::LeftBottom)
    );
    assert_eq!(
        globals.disguise_blinking_visibility,
        crate::rules::phobos_shields::AffectedHouse::ENEMIES
    );

    let cleared = super::PhobosDrawGlobals::from_ini(&IniFile::from_str(
        "[AudioVisual]\nDrawInsignia.AdjustPos.BuildingsAnchor=<none>\n",
    ));
    assert_eq!(cleared.buildings_anchor, None, "IsBlank clears a nullable");
}

#[test]
fn passengers_by_size_defaults_to_yes_and_reads_the_key() {
    let on = section("[ON]\nPassengers=2\n");
    let on = crate::rules::object_type::ObjectType::from_ini_section(
        "ON",
        on.section("ON").unwrap(),
        crate::rules::object_type::ObjectCategory::Vehicle,
    );
    assert!(on.phobos_passengers_by_size);
    let off = section("[OFF]\nPassengers=2\nPassengers.BySize=no\n");
    let off = crate::rules::object_type::ObjectType::from_ini_section(
        "OFF",
        off.section("OFF").unwrap(),
        crate::rules::object_type::ObjectCategory::Vehicle,
    );
    assert!(!off.phobos_passengers_by_size);
}

#[test]
fn parse_insignia_reads_the_slot_families() {
    let ini = section(
        "[TEST]\nInsignia=rank\nInsignia.Veteran=vet\nInsigniaFrame=2\n\
         InsigniaFrame.Elite=5\nInsigniaFrames=1,2,3\nInsignia.ShowEnemy=yes\n\
         WeaponCount=2\nPassengers=1\n\
         Insignia.Weapon1=gun1\nInsigniaFrame.Weapon1=8\nInsigniaFrames.Weapon1=4,5,6\n\
         Insignia.Weapon2.Rookie=gun2r\n\
         Insignia.Passengers0=p0\nInsigniaFrame.Passengers1=9\n",
    );
    let ty = ini.section("TEST").unwrap();
    let insignia = super::parse_insignia(ty, 2, 1);
    assert_eq!(insignia.base.images[0].as_deref(), Some("rank.shp"));
    assert_eq!(insignia.base.images[1].as_deref(), Some("vet.shp"));
    assert_eq!(insignia.base.images[2].as_deref(), Some("rank.shp"));
    assert_eq!(insignia.base.frames, [2, 2, 5]);
    assert_eq!(insignia.base.frame_lists, [1, 2, 3]);
    assert_eq!(insignia.show_enemy, Some(true));

    assert_eq!(insignia.weapons.len(), 2, "Weapon1..WeaponN");
    assert_eq!(insignia.weapons[0].images[0].as_deref(), Some("gun1.shp"));
    assert_eq!(insignia.weapons[0].frames[0], 8);
    assert_eq!(insignia.weapons[0].frame_lists, [4, 5, 6]);
    assert_eq!(insignia.weapons[1].images[0].as_deref(), Some("gun2r.shp"));
    assert_eq!(insignia.weapons[1].images[1], None);

    assert_eq!(insignia.passengers.len(), 2, "Passengers0..PassengersN");
    assert_eq!(insignia.passengers[0].images[0].as_deref(), Some("p0.shp"));
    assert_eq!(insignia.passengers[1].frames, [9, 9, 9]);
}

#[test]
fn insignia_type_overrides_the_direct_keys_even_when_unknown() {
    let ini = section(
        "[TEST]\nInsigniaType=SOME\nInsignia=direct\nInsigniaFrames=1,2,3\nWeaponCount=1\nInsignia.Weapon1=directgun\nInsigniaType.Weapon1=SOMEGUN\n",
    );
    let ty = ini.section("TEST").unwrap();
    let mut catalog = InsigniaTypeCatalog::default();
    catalog.apply_pass(&IniFile::from_str(
        "[InsigniaTypes]\n0=SOME\n[SOME]\nInsignia=typed\nInsigniaFrame=6\n",
    ));
    let mut insignia = super::parse_insignia(ty, 1, 0);
    assert_eq!(
        insignia.base.images[0].as_deref(),
        Some("direct.shp"),
        "the direct keys stay readable until the type resolution is known"
    );
    insignia.bind_types(&catalog);
    assert_eq!(
        insignia.base.images[0].as_deref(),
        Some("typed.shp"),
        "a resolved InsigniaType overrides the direct keys"
    );
    assert_eq!(insignia.base.frames, [6, 6, 6]);
    assert_eq!(
        insignia.base.frame_lists,
        [-1, -1, -1],
        "InsigniaType holds no frame vector"
    );
    assert_eq!(
        insignia.weapons[0].images[0].as_deref(),
        Some("directgun.shp"),
        "an unknown InsigniaType.Weapon1 falls back to the direct keys"
    );
}

#[test]
fn selection_merges_passengers_then_weapons() {
    let ini = section(
        "[TEST]\nInsignia=base\nInsigniaFrame=1\nInsigniaFrames=1,2,3\n\
         WeaponCount=1\nPassengers=1\n\
         Insignia.Weapon1=gun\nInsigniaFrame.Weapon1=7\n\
         Insignia.Passengers1=carry\nInsigniaFrames.Passengers1=4,4,4\nInsigniaFrame.Passengers1=2\n",
    );
    let ty = ini.section("TEST").unwrap();
    let insignia = super::parse_insignia(ty, 1, 1);
    let base = insignia.select(None, None);
    assert_eq!(base.images[0].as_deref(), Some("base.shp"));
    assert_eq!(base.frames, [1, 1, 1]);
    assert_eq!(base.frame_lists, [1, 2, 3]);

    let carried = insignia.select(None, Some(1));
    assert_eq!(carried.images[0].as_deref(), Some("carry.shp"));
    assert_eq!(carried.frames, [2, 2, 2]);
    assert_eq!(carried.frame_lists, [4, 4, 4]);

    let gunned = insignia.select(Some(0), Some(1));
    assert_eq!(
        gunned.images[0].as_deref(),
        Some("gun.shp"),
        "the weapon variant wins over the passenger variant"
    );
    assert_eq!(gunned.frames, [7, 7, 7]);
    assert_eq!(
        gunned.frame_lists,
        [4, 4, 4],
        "a variant without InsigniaFrames keeps the merged vector"
    );
}

#[test]
fn frame_precedence_matches_draw_insignia() {
    let ini =
        section("[TEST]\nInsigniaFrames=3,4,5\nInsigniaFrame.Veteran=8\nInsigniaFrame.Elite=-1\n");
    let ty = ini.section("TEST").unwrap();
    let insignia = super::parse_insignia(ty, 0, 0).select(None, None);
    assert_eq!(
        insignia.frame(0, true),
        Some(3),
        "custom rookie uses InsigniaFrames.X"
    );
    assert_eq!(
        insignia.frame(0, false),
        Some(3),
        "the vector wins over stock -1"
    );
    assert_eq!(
        insignia.frame(1, true),
        Some(8),
        "the single Veteran frame wins over the vector"
    );
    assert_eq!(insignia.frame(2, true), Some(5), "Elite -1 falls back to Z");

    let bare = super::parse_insignia(
        section("[TEST]\nImage=EMPTY\n").section("TEST").unwrap(),
        0,
        0,
    )
    .select(None, None);
    assert_eq!(bare.frame(0, true), Some(0), "custom default frame 0");
    assert_eq!(bare.frame(0, false), None, "stock rookie draws nothing");
    assert_eq!(bare.frame(1, false), Some(14), "stock Veteran frame 14");
    assert_eq!(bare.frame(2, false), Some(15), "stock Elite frame 15");
}
