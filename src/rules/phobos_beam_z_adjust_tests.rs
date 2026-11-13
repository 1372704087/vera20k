//! `[AudioVisual] LaserZAdjust=` / `EBoltZAdjust=` reads and the nullable
//! fallback each weapon uses.

use super::{BeamZAdjustGlobals, resolved};
use crate::rules::ini_parser::IniFile;

fn z_adjusts(ini: &str) -> BeamZAdjustGlobals {
    BeamZAdjustGlobals::from_ini(&IniFile::from_str(ini))
}

/// `Ext/Rules/Body.cpp:277-278`: both live in `[AudioVisual]` and default to 0
/// when the section or the key is absent.
#[test]
fn the_global_z_adjusts_default_to_zero_and_read_audio_visual() {
    assert_eq!(z_adjusts("[General]\nLaserZAdjust=5\n").laser, 0);
    assert_eq!(
        z_adjusts("[AudioVisual]\nLaserZAdjust=5\nEBoltZAdjust=-3\n"),
        BeamZAdjustGlobals {
            laser: 5,
            ebolt: -3
        }
    );
}

/// `Nullable<int>::Get(global)`: an unset weapon key inherits, a set one wins.
#[test]
fn a_weapon_override_replaces_the_global() {
    assert_eq!(resolved(None, 4), 4);
    assert_eq!(resolved(Some(-7), 4), -7);
    assert_eq!(resolved(Some(0), 4), 0);
}
