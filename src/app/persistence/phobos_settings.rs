//! Phobos client settings, read from the imported player profile.
//!
//! Phobos reads `RA2MD.INI [Phobos]` directly each launch. VERA imports that
//! file once into `VERA.INI` and keeps it the writable profile, so the same
//! section arrives here from `profile_file`. Only settings that gate a
//! compiled extension live here; retail options keep `options_profile`.

use std::path::Path;

use crate::rules::ini_parser::IniFile;

/// `[Phobos] DigitalDisplay.Enable=`, default `no`.
pub(crate) fn digital_display_enabled(settings_dir: &Path) -> bool {
    let path = super::profile_file::path(settings_dir);
    let Ok(bytes) = std::fs::read(&path) else {
        return false;
    };
    let Ok(ini) = IniFile::from_bytes(&bytes) else {
        log::warn!("Could not parse {} for Phobos settings", path.display());
        return false;
    };
    ini.section("Phobos")
        .is_some_and(|section| section.read_bool("DigitalDisplay.Enable", false))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_with(section: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "vera-phobos-settings-{}-{}",
            std::process::id(),
            section.len()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("VERA.INI"), section).unwrap();
        dir
    }

    #[test]
    fn digital_display_is_off_without_the_section_or_key() {
        let missing = std::env::temp_dir().join("vera-phobos-settings-missing");
        assert!(!digital_display_enabled(&missing));
        let dir = profile_with("[Options]\nGameSpeed=1\n");
        assert!(!digital_display_enabled(&dir));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn digital_display_reads_the_imported_phobos_switch() {
        let dir = profile_with("[Phobos]\nDigitalDisplay.Enable=yes\n");
        assert!(digital_display_enabled(&dir));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
