//! Phobos `SpawnFromEdge=` (`Ext/AircraftType/Body.cpp:24`, declared
//! `Ext/AircraftType/Body.h:15` as `Nullable<EdgeType>`) and the `[General]
//! AircraftSpawnFromEdge=` fallback (`Ext/Rules/Body.cpp:193`,
//! `Ext/Rules/Body.h:99`, constructed `Owner` at `Body.h:636`).
//!
//! `AircraftExt::PickEdgeCellForPlane` (`Ext/Aircraft/Body.cpp:85-126`) reads
//! the pair for every aircraft edge pick: `Owner` keeps the edge the caller
//! passed, `Closest` scatters the destination cell and picks on `Edge::None`,
//! and `Random` spends one `Scenario RandomRanged(North, West)` draw to choose
//! the edge. Callers: `HouseClass::SendSpyPlanes` (`0x65EB2D`),
//! `SendAirstrike` (`0x65E881`), `SendParadrop` (`0x65E6DB`) and
//! `AircraftClass::Mission_Retreat` (`0x415A7A`), hooked at
//! `Ext/House/Hooks.cpp:352-450`.
//!
//! ## Dependency rules
//! - Part of rules/ — no dependency on sim/, render/, ui/, etc.

use crate::rules::ini_parser::IniSection;

/// Phobos `EdgeType` (`Utilities/Enum.h:446-451`: `Owner=0`, `Closest=1`,
/// `Random=2`).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum PhobosEdgeType {
    #[default]
    Owner,
    Closest,
    Random,
}

impl PhobosEdgeType {
    /// `EnumParser<EdgeType>`'s name table; an unknown spelling keeps the
    /// previous value.
    pub fn parse(raw: &str) -> Option<Self> {
        let name = raw.trim();
        if name.eq_ignore_ascii_case("owner") {
            Some(Self::Owner)
        } else if name.eq_ignore_ascii_case("closest") {
            Some(Self::Closest)
        } else if name.eq_ignore_ascii_case("random") {
            Some(Self::Random)
        } else {
            None
        }
    }
}

/// An AircraftType's own `SpawnFromEdge=`, unset when the key is absent or
/// rejected, so the `[General]` value applies.
pub fn read_spawn_from_edge(section: &IniSection) -> Option<PhobosEdgeType> {
    section.read_phobos_nullable_with("SpawnFromEdge", None, PhobosEdgeType::parse)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::ini_parser::IniFile;

    #[test]
    fn the_spawn_from_edge_key_parses_the_three_names() {
        assert_eq!(PhobosEdgeType::parse("owner"), Some(PhobosEdgeType::Owner));
        assert_eq!(
            PhobosEdgeType::parse("Closest"),
            Some(PhobosEdgeType::Closest)
        );
        assert_eq!(
            PhobosEdgeType::parse("random"),
            Some(PhobosEdgeType::Random)
        );
        assert_eq!(PhobosEdgeType::parse("nearest"), None);
    }

    #[test]
    fn an_aircraft_reads_its_own_spawn_from_edge_and_an_unset_one_stays_none() {
        let ini = IniFile::from_str("[PLANE]\nSpawnFromEdge=random\n[OTHER]\nStart=0\n");
        assert_eq!(
            read_spawn_from_edge(ini.section("PLANE").unwrap()),
            Some(PhobosEdgeType::Random)
        );
        assert_eq!(read_spawn_from_edge(ini.section("OTHER").unwrap()), None);
        let bad = IniFile::from_str("[PLANE]\nSpawnFromEdge=nearest\n");
        assert_eq!(read_spawn_from_edge(bad.section("PLANE").unwrap()), None);
    }
}
