//! `WorkingSound=`/`NotWorkingSound=` (`BuildingTypeClass::ReadINI @
//! 0x0046088F`/`0x004608DD`) as the production process owner binds them from
//! the extracted retail INIs. Their reader is the shared registered-reference
//! one `turret_rotate_sound_tests` covers.

use std::sync::Arc;

use super::NativeRulesProcessOwner;
use super::NativeScenarioRulesPrefix;
use crate::rules::ini_parser::IniFile;
use crate::rules::sound_ini::SoundRegistry;

/// Seven defences author both power cues; the oil derrick and the secret lab
/// author only the working loop.
#[test]
fn retail_working_sound_authors() {
    let (Some(rules_ini), Some(art_ini), Some(sound_ini)) = (
        crate::rules::retail_ini_fixture::retail_ini("rulesmd.ini"),
        crate::rules::retail_ini_fixture::retail_ini("artmd.ini"),
        crate::rules::retail_ini_fixture::retail_ini("soundmd.ini"),
    ) else {
        return;
    };
    let mut owner = NativeRulesProcessOwner::from_cold_start_sources(
        rules_ini,
        None,
        art_ini,
        Arc::new(SoundRegistry::from_ini(&sound_ini)),
    )
    .unwrap();
    let rules = owner
        .load_scenario(
            NativeScenarioRulesPrefix::NonCampaign(None),
            &IniFile::empty(),
        )
        .unwrap()
        .into_parts()
        .0;
    let mut authors = rules
        .all_objects()
        .filter(|object| object.working_sound.is_some() || object.not_working_sound.is_some())
        .map(|object| {
            (
                object.id.as_str(),
                object.working_sound.as_deref(),
                object.not_working_sound.as_deref(),
            )
        })
        .collect::<Vec<_>>();
    authors.sort_unstable();
    let power = (Some("PowerOn"), Some("PowerOff"));
    let derrick = (Some("OilDerrickLoop"), None);
    assert_eq!(
        authors,
        [
            ("ATESLA", power.0, power.1),
            ("CAEAST02", power.0, power.1),
            ("CAOILD", derrick.0, derrick.1),
            ("CASLAB", derrick.0, derrick.1),
            ("GTGCAN", power.0, power.1),
            ("NAFLAK", power.0, power.1),
            ("NASAM", power.0, power.1),
            ("TESLA", power.0, power.1),
            ("YAGGUN", power.0, power.1),
        ]
    );
}
