//! Phobos client settings, read from the imported player profile.
//!
//! Phobos reads `RA2MD.INI [Phobos]` directly each launch. VERA imports that
//! file once into `VERA.INI` and keeps it the writable profile, so the same
//! section arrives here from `profile_file`.
//!
//! This chain owns the placement-preview switch. Other optional Phobos
//! client switches keep their own owners.

use std::path::Path;

use crate::rules::ini_parser::IniFile;

/// `[Phobos] ShowPlacementPreview=`, source default `yes` (b36 key; the old
/// `ShowBuildingPlacementPreview` is not read).
pub(crate) fn show_placement_preview(settings_dir: &Path) -> bool {
    let path = super::profile_file::path(settings_dir);
    let Ok(bytes) = std::fs::read(&path) else {
        return true;
    };
    let Ok(ini) = IniFile::from_bytes(&bytes) else {
        log::warn!("Could not parse {} for Phobos settings", path.display());
        return true;
    };
    ini.section("Phobos").map_or(true, |section| {
        section.read_bool("ShowPlacementPreview", true)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    static NEXT_PROFILE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn profile_with(section: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "vera-placement-settings-{}-{}",
            std::process::id(),
            NEXT_PROFILE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("VERA.INI"), section).unwrap();
        dir
    }

    #[test]
    fn missing_profile_keeps_the_source_default() {
        let missing = std::env::temp_dir().join("vera-placement-settings-missing");
        assert!(show_placement_preview(&missing));
        let dir = profile_with("[Options]\nGameSpeed=1\n");
        assert!(show_placement_preview(&dir));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn imported_switch_is_read() {
        let dir = profile_with("[Phobos]\nShowPlacementPreview=no\n");
        assert!(!show_placement_preview(&dir));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
