//! The campaign carry-over record at `0x008A38E0`: what one campaign mission
//! hands to the next Start_Scenario, and what a restart or defeat retry
//! hands back to the same mission.
//!
//! Native owners (instruction-read):
//! - `0x004C6140` clears it at session preparation (`0x0052DF0E`): the 50
//!   globals false, money +0x34 and timer +0x38 zero, the player's
//!   difficulty row +0x3C = 1, the stage word +0x40 zero.
//! - `0x004C6180` saves it before a won mission's Start_Scenario: the 50
//!   Scenario globals (`0x004C6187..0x004C61A1`), the player's money and
//!   remaining mission timer, the player House's difficulty row House+0x184
//!   (`0x004C61E0..0x004C61EC`) and Scenario+0x35A0.
//! - `0x004C6210` writes it back after Start_Scenario has read the map, for
//!   the won mission (`0x00685D7A`), the in-battle restart (`0x00686506`) and
//!   the defeat retry (`0x0068639A`): each global through SetGlobal
//!   `0x00689670` in index order, then the money, then
//!   `HouseClass::SetDifficulty` `0x004F6EC0` on the player House with +0x3C
//!   (`0x004C628B..0x004C6295`), then the timer and Scenario+0x35A0.
//! - `ContentSave67D31B` / `ContentLoad67E804` call `4C6340` / `4C6320`
//!   to write/read all 0x44 bytes. ScenarioSession retains this incoming
//!   record separately from the current mission's changing global flags.
//!
//! Residuals, inert on retail data: the money (every retail map has
//! `CarryOverMoney=0.000000`), the mission timer (`TimerInherit=no`) and
//! Scenario+0x35A0 (read only by `SkipMapSelect=yes`, never set on retail
//! maps) are not carried.

use crate::rules::ruleset::RuleSet;
use crate::sim::house_state::HouseDifficulty;
use crate::sim::trigger_runtime::SCENARIO_GLOBAL_COUNT;
use crate::sim::world::Simulation;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CampaignCarryOver {
    #[serde(with = "saved_globals")]
    globals: [bool; SCENARIO_GLOBAL_COUNT],
    /// Record+0x3C, the player House's difficulty row.
    player_difficulty: HouseDifficulty,
}

mod saved_globals {
    use super::SCENARIO_GLOBAL_COUNT;
    use serde::de::{SeqAccess, Visitor};
    use serde::ser::SerializeTuple;

    pub fn serialize<S: serde::Serializer>(
        values: &[bool; SCENARIO_GLOBAL_COUNT],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut tuple = serializer.serialize_tuple(SCENARIO_GLOBAL_COUNT)?;
        for value in values {
            tuple.serialize_element(value)?;
        }
        tuple.end()
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<[bool; SCENARIO_GLOBAL_COUNT], D::Error> {
        struct Globals;
        impl<'de> Visitor<'de> for Globals {
            type Value = [bool; SCENARIO_GLOBAL_COUNT];

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("50 campaign globals")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut values = [false; SCENARIO_GLOBAL_COUNT];
                for (index, value) in values.iter_mut().enumerate() {
                    *value = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(index, &self))?;
                }
                Ok(values)
            }
        }
        deserializer.deserialize_tuple(SCENARIO_GLOBAL_COUNT, Globals)
    }
}

impl CampaignCarryOver {
    /// The record `0x004C6140` leaves: every global false and row 1.
    pub const CLEARED: Self = Self {
        globals: [false; SCENARIO_GLOBAL_COUNT],
        player_difficulty: HouseDifficulty::Normal,
    };

    pub const fn globals(&self) -> &[bool; SCENARIO_GLOBAL_COUNT] {
        &self.globals
    }

    pub const fn player_difficulty(&self) -> HouseDifficulty {
        self.player_difficulty
    }
}

impl Simulation {
    /// `0x004C6180`: the record a won mission saves for the next one. A
    /// session without a player House keeps the cleared row.
    pub fn save_campaign_carry_over(&self) -> CampaignCarryOver {
        let player_difficulty = self
            .session
            .current_house()
            .and_then(|player| self.houses.get(&player))
            .map_or(CampaignCarryOver::CLEARED.player_difficulty, |house| {
                house.difficulty()
            });
        CampaignCarryOver {
            globals: self.global_variables(),
            player_difficulty,
        }
    }

    /// `0x004C6210`: write the record back once Start_Scenario has read the
    /// map. SetDifficulty runs on the player House only; computer Houses keep
    /// the rows the scenario read gave them.
    pub fn restore_campaign_carry_over(&mut self, record: &CampaignCarryOver, rules: &RuleSet) {
        for (index, &set) in record.globals.iter().enumerate() {
            Self::set_trigger_variable(
                &self.session,
                &mut self.scenario_rng,
                &mut self.trigger_runtime,
                true,
                index as i32,
                set,
            );
        }
        let Some(player) = self.session.current_house() else {
            return;
        };
        let Some(array_index) = self.houses.index_of(player) else {
            return;
        };
        let frame = self.session.binary_frame() as i32;
        let country = self
            .houses
            .get(&player)
            .and_then(|house| house.country())
            .map(|country| self.interner.resolve(country).to_owned());
        let biases = country.map_or_else(Default::default, |name| {
            rules.country_difficulty_biases(&name)
        });
        if let Some(house) = self.houses.get_mut(&player) {
            house.set_difficulty(
                record.player_difficulty,
                &rules.general,
                biases,
                self.session.game_mode_nonzero(),
                array_index as i32,
                frame,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Restarting the first mission restores the cleared record, whose row 1
    /// replaces the menu row on the player House alone.
    #[test]
    fn restore_sets_the_player_row_and_globals_only() {
        let rules =
            RuleSet::from_ini(&crate::rules::ini_parser::IniFile::from_str("")).expect("rules");
        let mut sim = Simulation::new();
        let player = sim.interner.intern("Player");
        let enemy = sim.interner.intern("Enemy");
        for (name, row) in [
            (player, HouseDifficulty::Easy),
            (enemy, HouseDifficulty::Hard),
        ] {
            let house = sim.houses.get_or_register_for_test(name, || {
                crate::sim::house_state::HouseState::new(name, 0, None, false, 0, 10)
            });
            house.set_difficulty(row, &rules.general, Default::default(), false, 0, 0);
        }
        sim.session.set_current_house_for_test(Some(player));
        let mut globals = [false; SCENARIO_GLOBAL_COUNT];
        globals[7] = true;
        let record = CampaignCarryOver {
            globals,
            player_difficulty: HouseDifficulty::Hard,
        };

        sim.restore_campaign_carry_over(&CampaignCarryOver::CLEARED, &rules);
        assert_eq!(
            sim.houses.get(&player).unwrap().difficulty(),
            HouseDifficulty::Normal
        );
        assert_eq!(
            sim.houses.get(&enemy).unwrap().difficulty(),
            HouseDifficulty::Hard
        );
        assert_eq!(sim.global_variables(), [false; SCENARIO_GLOBAL_COUNT]);

        sim.restore_campaign_carry_over(&record, &rules);
        assert_eq!(sim.save_campaign_carry_over(), record);
    }

    #[test]
    fn snapshot_preserves_incoming_carry_over_separately_from_current_globals() {
        let mut globals = [false; SCENARIO_GLOBAL_COUNT];
        globals[7] = true;
        globals[49] = true;
        let incoming = CampaignCarryOver {
            globals,
            player_difficulty: HouseDifficulty::Hard,
        };
        let mut sim = Simulation::new();
        sim.session.initialize_campaign_for_test(3, 5, incoming);
        let rules =
            RuleSet::from_ini(&crate::rules::ini_parser::IniFile::from_str("")).expect("rules");
        sim.restore_campaign_carry_over(&incoming, &rules);
        Simulation::set_trigger_variable(
            &sim.session,
            &mut sim.scenario_rng,
            &mut sim.trigger_runtime,
            true,
            7,
            false,
        );
        let bytes = crate::sim::snapshot::GameSnapshot::save(&sim, 0, 0, "", 0);
        let loaded = crate::sim::snapshot::GameSnapshot::load(&bytes)
            .unwrap()
            .sim;
        assert_eq!(loaded.session.campaign_index(), Some(3));
        assert_eq!(loaded.session.campaign_mission_counter(), Some(5));
        assert_eq!(loaded.session.campaign_restart_carry_over(), Some(incoming));
        assert!(!loaded.global_variables()[7]);
        assert!(loaded.global_variables()[49]);
        // Native ScenarioLoad resets the embedded RNG; whole-world hashes
        // across that boundary need not match. The retained gameplay record
        // and the current globals above are separate persistence assertions.
    }
}
