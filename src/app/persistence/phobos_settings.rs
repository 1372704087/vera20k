//! Phobos client settings, read from the imported player profile.
//!
//! Phobos reads `RA2MD.INI [Phobos]` directly each launch. VERA imports that
//! file once into `VERA.INI` and keeps it the writable profile, so the same
//! section arrives here from `profile_file`. Only settings that gate a
//! compiled extension live here; retail options keep `options_profile`.

use std::path::Path;

use crate::rules::ini_parser::IniFile;

/// The `[Phobos]` client switches this build reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PhobosClientSettings {
    /// `[Phobos] ShowPlacementPreview=`, source default `yes` (b36 key; the
    /// old `ShowBuildingPlacementPreview` is not read).
    pub(crate) show_placement_preview: bool,
    /// `[Phobos] DigitalDisplay.Enable=`, default `no`.
    pub(crate) digital_display_enabled: bool,
    /// `[Phobos] SuperWeaponSidebar.RequiredSignificance=`, default `0`.
    pub(crate) superweapon_sidebar_required_significance: i32,
    /// `[Phobos] ShowFlashOnSelecting=`, default `no`: the client switch for
    /// `[AudioVisual] SelectionFlashDuration`.
    pub(crate) show_flash_on_selecting: bool,
    /// `[Phobos] ShowPowerPlantEnhancerRange=`, default `no`: the client switch
    /// for the placement-time enhancer rings; the rules switch must also be on.
    pub(crate) show_power_plant_enhancer_range: bool,
    /// `[Phobos] ShowPowerDelta=`, default `yes` (`Phobos.INI.cpp:108`): the
    /// client half of the power-delta sidebar indicator. The `uimd.ini` switch
    /// must also be on.
    pub(crate) show_power_delta: bool,
    /// `[Phobos] ShowHarvesterCounter=`, default `yes` (`Phobos.INI.cpp:109`):
    /// the client half of the harvester counter; the `uimd.ini` switch must be
    /// on too.
    pub(crate) show_harvester_counter: bool,
    /// `[Phobos] ShowWeedsCounter=`, default `yes` (`Phobos.INI.cpp:110` reads
    /// with `true`; the `false` static initializer at `:71` is overwritten):
    /// the client half of the weeds counter.
    pub(crate) show_weeds_counter: bool,
    /// `[Phobos] PriorityDeployFiltering=`, default `yes`
    /// (`Phobos.INI.cpp:46` static, read `:94`): drops a
    /// `LowDeployPriority=yes` techno from a multi-object deploy when the
    /// selection also holds one that is not low priority.
    pub(crate) priority_deploy_filtering: bool,
    /// `[Phobos] TypeSelectUseIFVMode=`, default `yes`
    /// (`Phobos.INI.cpp:95`): a type-selection click enters IFV mode instead of
    /// a plain move/select.
    pub(crate) type_select_use_ifv_mode: bool,
    /// `[Phobos] MessageDisplayInCenter=`, default `no`
    /// (`Phobos.INI.cpp:98`): the center-message layout switch.
    pub(crate) message_display_in_center: bool,
    /// `[Phobos] SaveGameOnScenarioStart=`, default `yes`
    /// (`Phobos.INI.cpp:106`): a campaign/mission start writes a save before
    /// play begins.
    pub(crate) save_game_on_scenario_start: bool,
    /// `[Phobos] HideLaserTrailEffects=`, default `no`
    /// (`Phobos.INI.cpp:73` static, read `:112`): suppresses laser-trail
    /// drawing; the read default overrides the static `true`.
    pub(crate) hide_laser_trail_effects: bool,
    /// `[Phobos] HideLightFlashEffects=`, default `no`
    /// (`Phobos.INI.cpp:72` static, read `:111`): skips the combat light
    /// flash (`DoFlash @ 0x48A62E`, `Misc/Hooks.LightEffects.cpp:51-56`).
    pub(crate) hide_light_flash_effects: bool,
    /// `[Phobos] HideShakeEffects=`, default `no`
    /// (`Phobos.INI.cpp:74` static, read `:113`): skips the screen-shake
    /// writes (`Ext/Bullet/Hooks.DetonateLogics.cpp:33`).
    pub(crate) hide_shake_effects: bool,
    /// `[Phobos] ShowGameTime=`, default `no` (`Phobos.INI.cpp:116`): draws the
    /// elapsed-game-time clock.
    pub(crate) show_game_time: bool,
    /// `[Phobos] EnableSelectBox=`, default `no` (`Phobos.INI.cpp:104`): the
    /// client switch for the `[SelectBoxTypes]` custom selection box.
    pub(crate) enable_select_box: bool,
    /// `[Phobos] CampaignDefaultGameSpeed=`, engine speed index
    /// (`6 - ReadInteger(4)` at `Phobos.INI.cpp:120`): 2 when the computed
    /// index leaves 0..=6 (`:121-124`), i.e. the stock `GS4` forced when speed
    /// control is off.
    pub(crate) campaign_default_game_speed: i32,
}

impl Default for PhobosClientSettings {
    /// The source defaults, also used when there is no readable profile and by
    /// sealed captures.
    fn default() -> Self {
        Self {
            show_placement_preview: true,
            digital_display_enabled: false,
            superweapon_sidebar_required_significance: 0,
            show_flash_on_selecting: false,
            show_power_plant_enhancer_range: false,
            show_power_delta: true,
            show_harvester_counter: true,
            show_weeds_counter: true,
            priority_deploy_filtering: true,
            type_select_use_ifv_mode: true,
            message_display_in_center: false,
            save_game_on_scenario_start: true,
            hide_laser_trail_effects: false,
            hide_light_flash_effects: false,
            hide_shake_effects: false,
            show_game_time: false,
            enable_select_box: false,
            campaign_default_game_speed: 2,
        }
    }
}

/// Read the switches from the profile in `settings_dir`.
pub(crate) fn read(settings_dir: &Path) -> PhobosClientSettings {
    let defaults = PhobosClientSettings::default();
    let path = super::profile_file::path(settings_dir);
    let Ok(bytes) = std::fs::read(&path) else {
        return defaults;
    };
    let Ok(ini) = IniFile::from_bytes(&bytes) else {
        log::warn!("Could not parse {} for Phobos settings", path.display());
        return defaults;
    };
    let Some(section) = ini.section("Phobos") else {
        return defaults;
    };
    PhobosClientSettings {
        show_placement_preview: section
            .read_bool("ShowPlacementPreview", defaults.show_placement_preview),
        digital_display_enabled: section
            .read_bool("DigitalDisplay.Enable", defaults.digital_display_enabled),
        superweapon_sidebar_required_significance: section.read_int(
            "SuperWeaponSidebar.RequiredSignificance",
            defaults.superweapon_sidebar_required_significance,
        ),
        show_flash_on_selecting: section
            .read_bool("ShowFlashOnSelecting", defaults.show_flash_on_selecting),
        show_power_plant_enhancer_range: section.read_bool(
            "ShowPowerPlantEnhancerRange",
            defaults.show_power_plant_enhancer_range,
        ),
        show_power_delta: section.read_bool("ShowPowerDelta", defaults.show_power_delta),
        show_harvester_counter: section
            .read_bool("ShowHarvesterCounter", defaults.show_harvester_counter),
        show_weeds_counter: section.read_bool("ShowWeedsCounter", defaults.show_weeds_counter),
        priority_deploy_filtering: section.read_bool(
            "PriorityDeployFiltering",
            defaults.priority_deploy_filtering,
        ),
        type_select_use_ifv_mode: section
            .read_bool("TypeSelectUseIFVMode", defaults.type_select_use_ifv_mode),
        message_display_in_center: section
            .read_bool("MessageDisplayInCenter", defaults.message_display_in_center),
        save_game_on_scenario_start: section.read_bool(
            "SaveGameOnScenarioStart",
            defaults.save_game_on_scenario_start,
        ),
        hide_laser_trail_effects: section
            .read_bool("HideLaserTrailEffects", defaults.hide_laser_trail_effects),
        hide_light_flash_effects: section
            .read_bool("HideLightFlashEffects", defaults.hide_light_flash_effects),
        hide_shake_effects: section.read_bool("HideShakeEffects", defaults.hide_shake_effects),
        show_game_time: section.read_bool("ShowGameTime", defaults.show_game_time),
        enable_select_box: section.read_bool("EnableSelectBox", defaults.enable_select_box),
        // `Phobos.INI.cpp:120-124`: index `6 - key`, then the same 0..=6 guard
        // that forces the stock `GS4` speed 2 when the read leaves the range.
        campaign_default_game_speed: {
            let speed = 6 - section.read_int("CampaignDefaultGameSpeed", 4);
            if (0..=6).contains(&speed) { speed } else { 2 }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static NEXT_PROFILE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn profile_with(section: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "vera-phobos-settings-{}-{}",
            std::process::id(),
            NEXT_PROFILE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("VERA.INI"), section).unwrap();
        dir
    }

    #[test]
    fn missing_profile_or_section_keeps_the_source_defaults() {
        let missing = std::env::temp_dir().join("vera-phobos-settings-missing");
        assert_eq!(read(&missing), PhobosClientSettings::default());
        let dir = profile_with("[Options]\nGameSpeed=1\n");
        assert_eq!(read(&dir), PhobosClientSettings::default());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn imported_switches_are_read() {
        let dir = profile_with(
            "[Phobos]\nShowPlacementPreview=no\nDigitalDisplay.Enable=yes\n\
             SuperWeaponSidebar.RequiredSignificance=2\nShowFlashOnSelecting=yes\n\
             ShowPowerPlantEnhancerRange=yes\nShowPowerDelta=no\nShowHarvesterCounter=no\n\
             ShowWeedsCounter=no\nPriorityDeployFiltering=no\nTypeSelectUseIFVMode=no\n\
             MessageDisplayInCenter=yes\nSaveGameOnScenarioStart=no\n\
             HideLaserTrailEffects=yes\nShowGameTime=yes\nEnableSelectBox=yes\n\
             CampaignDefaultGameSpeed=4\n\
             HideLightFlashEffects=yes\nHideShakeEffects=yes\n",
        );
        assert_eq!(
            read(&dir),
            PhobosClientSettings {
                show_placement_preview: false,
                digital_display_enabled: true,
                superweapon_sidebar_required_significance: 2,
                show_flash_on_selecting: true,
                show_power_plant_enhancer_range: true,
                show_power_delta: false,
                show_harvester_counter: false,
                show_weeds_counter: false,
                priority_deploy_filtering: false,
                type_select_use_ifv_mode: false,
                message_display_in_center: true,
                save_game_on_scenario_start: false,
                hide_laser_trail_effects: true,
                hide_light_flash_effects: true,
                hide_shake_effects: true,
                show_game_time: true,
                enable_select_box: true,
                campaign_default_game_speed: 2,
            }
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// `Phobos.INI.cpp:120-124`: the stored value is `6 - key`; a key that
    /// leaves the 0..=6 range (here 7 gives -1) falls back to speed 2.
    #[test]
    fn campaign_default_game_speed_inverts_the_key_and_clamps() {
        let dir = profile_with("[Phobos]\nCampaignDefaultGameSpeed=0\n");
        assert_eq!(read(&dir).campaign_default_game_speed, 6);
        std::fs::remove_dir_all(dir).unwrap();
        let dir = profile_with("[Phobos]\nCampaignDefaultGameSpeed=7\n");
        assert_eq!(read(&dir).campaign_default_game_speed, 2);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
