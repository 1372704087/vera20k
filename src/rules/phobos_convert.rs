//! Phobos `Convert.Undeploy` TechnoType key: an infantryman that finishes its
//! Undeploy sequence converts to another type once.
//!
//! Reader `Ext/TechnoType/Body.cpp:1014` (`Valueable<TechnoTypeClass*>`),
//! declaration `Ext/TechnoType/Body.h:249`, constructor null `:681`.
//! Consumers: `InfantryClass_DoingAI_DeployConvert_Undeploy @ 0x520B99`
//! (`Ext/Infantry/Hooks.cpp:36-50`) with the `HasUndeployConverted` latch and
//! its reset at `0x520E75` (`:52-66`); the deploy-side twin reads
//! `Convert.Deploy` and is a separate chain.
//!
//! ## Dependency rules
//! - Part of rules/ — no dependency on sim/, render/, ui/, etc.

use crate::rules::ini_parser::IniSection;

/// The `Convert.*` self-conversion triggers this chain reads.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct PhobosConvert {
    /// `Convert.Undeploy=`: the type an infantryman converts to when its
    /// Undeploy sequence completes; absent or `none` disables.
    pub undeploy: Option<String>,
}

impl PhobosConvert {
    pub fn from_section(section: &IniSection) -> Self {
        Self {
            undeploy: section
                .read_type_name("Convert.Undeploy", 0x800)
                .filter(|name| !name.eq_ignore_ascii_case("none"))
                .map(str::to_owned),
        }
    }
}
