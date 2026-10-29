//! Phobos airstrike colour keys.
//!
//! `TechnoTypeExt::LaserTargetColor` (nullable `[ColorAdd]` index) selects the
//! word that tints a building this type's Boris airstrike marks
//! (`src/Ext/Techno/Body.Internal.cpp:178`), while
//! `TechnoTypeExt::AirstrikeLineColor` (nullable RGB) colours the beam and the
//! target dot the strike draws (`src/Ext/Techno/Hooks.Airstrike.cpp:308`).
//! Each unset type value takes the matching `[AudioVisual]` global:
//! `LaserTargetColor` is the stock Rules field, and `AirstrikeLineColor`
//! defaults to `255,0,0` (`src/Ext/Rules/Body.h:704`).
//!
//! ## Dependency rules
//! - Part of rules/ — no dependencies on app/, render/, ui/, sim/ or audio/.

/// The `[ColorAdd]` index a strike by `techno` uses for its target's tint.
pub fn laser_target_color(techno: Option<i32>, global: i32) -> i32 {
    techno.unwrap_or(global)
}

/// The RGB a strike by `techno` draws its beam and target dot with.
pub fn airstrike_line_color(techno: Option<[u8; 3]>, global: [u8; 3]) -> [u8; 3] {
    techno.unwrap_or(global)
}

#[cfg(test)]
mod tests {
    use crate::rules::ini_parser::IniFile;
    use crate::rules::ruleset::RuleSet;

    /// The whole chain through the production readers: `[AudioVisual]`, the
    /// TechnoType section, and the resolved values the draw/tint consumers use.
    #[test]
    fn ruleset_resolves_each_colour_from_type_then_global() {
        for (global_line, techno_line, global_laser, techno_laser, line, laser) in [
            (None, None, None, None, [255, 0, 0], 0),
            (Some("0,64,0"), None, Some("4"), None, [0, 64, 0], 4),
            (
                Some("0,64,0"),
                Some("0,0,255"),
                Some("4"),
                Some("7"),
                [0, 0, 255],
                7,
            ),
            // `<default>` clears the type value and falls back to the global.
            (
                Some("0,64,0"),
                Some("<default>"),
                Some("4"),
                Some("<default>"),
                [0, 64, 0],
                4,
            ),
        ] {
            let mut text = String::new();
            text.push_str("[AudioVisual]\n");
            if let Some(value) = global_line {
                text.push_str(&format!("AirstrikeLineColor={value}\n"));
            }
            if let Some(value) = global_laser {
                text.push_str(&format!("LaserTargetColor={value}\n"));
            }
            text.push_str("[InfantryTypes]\n0=TEST\n[TEST]\n");
            if let Some(value) = techno_line {
                text.push_str(&format!("AirstrikeLineColor={value}\n"));
            }
            if let Some(value) = techno_laser {
                text.push_str(&format!("LaserTargetColor={value}\n"));
            }
            let rules = RuleSet::from_ini(&IniFile::from_str(&text)).expect("rules read");
            let ty = rules.object("TEST").expect("TEST type");
            assert_eq!(rules.airstrike_line_color_for(ty), line, "{text}");
            assert_eq!(rules.laser_target_color_for(ty), laser, "{text}");
        }
    }

    /// The Phobos `Parser<BYTE,3>` all-or-nothing rule: a channel outside
    /// `0..=255` rejects the whole color, leaving the previous value.
    #[test]
    fn malformed_type_colors_keep_the_global_value() {
        let mut text = String::from(
            "[AudioVisual]\nAirstrikeLineColor=10,20,30\n\
             [InfantryTypes]\n0=TEST\n[TEST]\nAirstrikeLineColor=1,2\n",
        );
        let rules = RuleSet::from_ini(&IniFile::from_str(&text)).expect("rules read");
        let ty = rules.object("TEST").expect("TEST type");
        assert_eq!(rules.airstrike_line_color_for(ty), [10, 20, 30]);
        text.push_str("AirstrikeLineColor=1,2,300\n");
        let rules = RuleSet::from_ini(&IniFile::from_str(&text)).expect("rules read");
        let ty = rules.object("TEST").expect("TEST type");
        assert_eq!(rules.airstrike_line_color_for(ty), [10, 20, 30]);
    }
}
