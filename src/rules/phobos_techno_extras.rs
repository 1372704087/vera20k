//! Phobos `TechnoTypeExt` keys that no other type owner reads, one group per
//! mechanism family.
//!
//! Read order follows `TechnoTypeExt::LoadFromINI`
//! (`Ext/TechnoType/Body.cpp`):
//!
//! ```ini
//! RadarJamHouses=            ; Nullable<AffectedHouse>, unset   :780
//! RadarJamDelay=             ; Nullable<int>, unset             :781
//! RadarJamAffect=            ; ValueableVector<BuildingType*>, empty :782
//! RadarJamIgnore=            ; ValueableVector<BuildingType*>, empty :783
//! VoiceCreated=              ; NullableIdx<VocClass>, unset     :853
//! UseDisguiseMovementSpeed=  ; Nullable<bool>, unset            :877
//! AutoFire= / AutoTargetOwnPosition=           ; Valueable<bool>, no :898-899
//! AutoFire.TargetSelf= / AutoTargetOwnPosition.Self= ; Valueable<bool>, no :904-905
//! NoQueueUpToEnter=          ; Nullable<bool>, unset           :1045
//! NoQueueUpToEnter.BoardDistance= ; Nullable<int>, unset       :1046
//! VeteranReload=             ; Nullable<double>, unset         :1062
//! VeteranEmptyReload=        ; Nullable<double>, unset         :1063
//! Spawns.Queue=              ; ValueableVector<AircraftType*>, empty :1118
//! DropCrate=                 ; Nullable<Powerup>, unset        :1446
//! ```
//!
//! Constructor values: `Ext/TechnoType/Body.h:482-485`, `:544`, `:561-562`,
//! `:602`, `:709-710`, `:725-726`, `:768`, `:851`.
//!
//! The `AutoFire`/`AutoFire.TargetSelf` spellings are read **before** their
//! `AutoTargetOwnPosition` replacements at the same lines, so both names work
//! and the newer one wins when a section carries both; Phobos logs a
//! deprecation warning for the old pair.
//!
//! Every `[General]`/`[AudioVisual]` fallback lives on
//! [`crate::rules::phobos_globals::PhobosGlobals`].
//!
//! Two keys cannot be resolved at rules-parse time:
//!
//! - `VoiceCreated=` is a `NullableIdx<VocClass>` natively, so Phobos converts
//!   the name to a Vox index while reading (`Utilities/TemplateDef.h:1878-1895`
//!   calls `Lookuper::FindIndex`). VERA has no Vox registry here, so the name
//!   is kept and resolved at use — the same deferred-lookup DRIFT that
//!   `[CrateRules] HealCrateSound=` already records.
//! - `DropCrate=` is a `Nullable<Powerup>`, and `detail::read<Powerup>`
//!   (`Utilities/TemplateDef.h:648-676`) is an exact case-insensitive scan of
//!   the fixed `[Powerups]` name table. [`crate::rules::powerups::POWERUP_NAMES`]
//!   is that same table in the same order, so the slot index is resolved here
//!   and an unknown name leaves the key unset, exactly as natively.
//!
//! `RadarJamHouses=` is a genuine `Nullable<AffectedHouse>`, and `none` is one
//! of its values (`AffectedHouse::None`, "jam nobody"), not a clearing token —
//! only blank and `<default>` reset it (`Utilities/TemplateDef.h:1833-1849`).
//!
//! ## Dependency rules
//! - Part of rules/ — no dependency on sim/, render/, ui/, etc.

use crate::rules::ini_parser::IniSection;
use crate::rules::phobos_shields::AffectedHouse;

/// The twelve `TechnoTypeExt` keys owned here.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PhobosTechnoExtras {
    /// `RadarJamHouses=` (`:780`), `Nullable`: which Houses a
    /// `RadarJamRadius=` jammer may jam, measured from the jammer's owner.
    /// `None` reads `[General] RadarJamHouses` (default `Enemies`).
    pub radar_jam_houses: Option<AffectedHouse>,
    /// `RadarJamDelay=` (`:781`), `Nullable`: frames between scans. `None`
    /// reads `[General] RadarJamDelay` (default 30).
    pub radar_jam_delay: Option<i32>,
    /// `RadarJamAffect=` (`:782`): the only building types the jammer may jam;
    /// empty admits every Radar/SpySat type.
    pub radar_jam_affect: Vec<String>,
    /// `RadarJamIgnore=` (`:783`): building types the jammer never jams, even
    /// when `RadarJamAffect=` lists them.
    pub radar_jam_ignore: Vec<String>,
    /// `VoiceCreated=` (`:853`), `NullableIdx`: the voice an object plays when
    /// a factory produces it (`Ext/House/Hooks.UnitFromFactory.cpp:7-24`).
    pub voice_created: Option<String>,
    /// `UseDisguiseMovementSpeed=` (`:877`), `Nullable`: a disguised object
    /// moves at its disguise's speed (`Ext/Techno/Hooks.cpp:815`). `None`
    /// reads `[General] UseDisguiseMovementSpeed`.
    pub use_disguise_movement_speed: Option<bool>,
    /// `NoQueueUpToEnter=` (`:1045`), `Nullable`: an infantryman boards within
    /// `NoQueueUpToEnter.BoardDistance=` without joining the queue. `None`
    /// reads `[General] NoQueueUpToEnter`.
    pub no_queue_up_to_enter: Option<bool>,
    /// `NoQueueUpToEnter.BoardDistance=` (`:1046`), `Nullable` leptons. `None`
    /// reads `[General] NoQueueUpToEnter.BoardDistance` (default 384).
    pub no_queue_up_to_enter_board_distance: Option<i32>,
    /// `NoQueueUpToUnload=` (`:1047`), `Nullable`: a transport with this key
    /// unloads its whole cargo within one frame (`0x73DC1E`); a building
    /// returns 0 from its Unload mission (`0x44DCB1`). `None` reads
    /// `[General] NoQueueUpToUnload`.
    pub no_queue_up_to_unload: Option<bool>,
    /// `VeteranReload=` (`:1062`), `Nullable`: the reload multiplier once the
    /// object is veteran or better (`Ext/Techno/Hooks.Firing.cpp:1186-1187`).
    pub veteran_reload: Option<f64>,
    /// `VeteranEmptyReload=` (`:1063`), `Nullable`: the same multiplier when
    /// the clip is empty, which outranks `VeteranReload=`.
    pub veteran_empty_reload: Option<f64>,
    /// `Spawns.Queue=` (`:1118`): the order a spawner releases its
    /// `Spawns=` pool; empty keeps the type order.
    pub spawns_queue: Vec<String>,
    /// `DropCrate=` (`:1446`), `Nullable`: the `[Powerups]` slot this object
    /// leaves where it dies (`Ext/Techno/Body.Internal.cpp:275-294`).
    pub drop_crate: Option<usize>,
}

impl PhobosTechnoExtras {
    /// Reads every key in `LoadFromINI` order.
    pub fn from_section(section: &IniSection) -> Self {
        let type_list = |key: &str| {
            section
                .read_list(key, 0x800)
                .map(|tokens| tokens.into_iter().map(str::to_string).collect())
                .unwrap_or_default()
        };
        Self {
            radar_jam_houses: section
                .read_name("RadarJamHouses", 2048)
                .and_then(AffectedHouse::parse),
            radar_jam_delay: section.read_phobos_nullable_int("RadarJamDelay", None),
            radar_jam_affect: type_list("RadarJamAffect"),
            radar_jam_ignore: type_list("RadarJamIgnore"),
            voice_created: section
                .read_phobos_nullable_with("VoiceCreated", None, |raw| Some(raw.to_owned())),
            use_disguise_movement_speed: section
                .read_phobos_nullable_bool("UseDisguiseMovementSpeed", None),
            no_queue_up_to_enter: section.read_phobos_nullable_bool("NoQueueUpToEnter", None),
            no_queue_up_to_enter_board_distance: section
                .read_phobos_nullable_int("NoQueueUpToEnter.BoardDistance", None),
            no_queue_up_to_unload: section.read_phobos_nullable_bool("NoQueueUpToUnload", None),
            veteran_reload: section.read_phobos_nullable_double("VeteranReload", None),
            veteran_empty_reload: section.read_phobos_nullable_double("VeteranEmptyReload", None),
            spawns_queue: type_list("Spawns.Queue"),
            // `detail::read<Powerup>` (`Utilities/TemplateDef.h:648-676`): an
            // exact, case-insensitive scan of the fixed name table; an unknown
            // name is a parse failure, which leaves the nullable unset.
            drop_crate: section.read_name("DropCrate", 2048).and_then(|raw| {
                crate::rules::powerups::POWERUP_NAMES
                    .iter()
                    .position(|name| name.eq_ignore_ascii_case(raw))
            }),
        }
    }

    /// `NoQueueUpToEnter.Get([General] NoQueueUpToEnter)`
    /// (`Ext/Techno/Hooks.Transport.cpp:332`, `Ext/TechnoType/Body.cpp:1045`).
    pub fn no_queue_up_to_enter(&self, global: bool) -> bool {
        self.no_queue_up_to_enter.unwrap_or(global)
    }

    /// `NoQueueUpToEnter_BoardDistance.Get([General]
    /// NoQueueUpToEnter.BoardDistance)` (`Ext/Techno/Hooks.Transport.cpp:223`,
    /// `Ext/TechnoType/Body.cpp:1046`).
    pub fn no_queue_board_distance(&self, global: i32) -> i32 {
        self.no_queue_up_to_enter_board_distance.unwrap_or(global)
    }

    /// `NoQueueUpToUnload.Get([General] NoQueueUpToUnload)`
    /// (`Ext/Techno/Hooks.Transport.cpp:436`, `Ext/TechnoType/Body.cpp:1047`).
    pub fn no_queue_up_to_unload(&self, global: bool) -> bool {
        self.no_queue_up_to_unload.unwrap_or(global)
    }
}

#[cfg(test)]
#[path = "phobos_techno_extras_tests.rs"]
mod tests;
