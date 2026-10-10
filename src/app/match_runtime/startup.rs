//! App-owned admission for an accepted fresh startup or committed saved world.
//! Restoring a save retires the outgoing startup's evidence without disabling
//! diagnostic stepping of the validated replacement.
//!
//! VERA-internal admission protocol; gamemd equivalent UNCHECKED. Existing startup
//! observation checks remain in `match_bootstrap::RustL0Observation`.

use crate::match_bootstrap::{
    MatchCorrelationId, PreparedMatchStartup, RustL0Observation, RustL0Receipt,
};
use crate::sim::world::Simulation;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct MatchStartup {
    phase: StartupPhase,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum StartupPhase {
    #[default]
    Noncertifying,
    Loading(MatchCorrelationId),
    Accepted {
        startup: PreparedMatchStartup,
        receipt: RustL0Receipt,
    },
    Restored,
}

impl MatchStartup {
    /// Replacing an attempt retires all prior admission evidence together.
    pub(crate) fn begin(&mut self, correlation: Option<MatchCorrelationId>) {
        self.phase = correlation.map_or(StartupPhase::Noncertifying, StartupPhase::Loading);
    }

    pub(crate) fn clear(&mut self) {
        self.phase = StartupPhase::Noncertifying;
    }

    /// Only the successful saved-world commit calls this, after resource and
    /// snapshot validation. A restored timeline is admitted without claiming
    /// the outgoing world's fresh-start receipt. Rejected preparation never
    /// reaches this transition, so it preserves the current admission.
    pub(crate) fn acknowledge_saved_load(&mut self) {
        self.phase = StartupPhase::Restored;
    }

    /// Publish the startup and its receipt together, only after the existing L0
    /// observation validates. A rejected observation leaves the attempt intact;
    /// the orchestrator chooses its error screen and clears the attempt.
    pub(crate) fn acknowledge(
        &mut self,
        startup: PreparedMatchStartup,
        simulation: Option<&Simulation>,
        screen_is_loading: bool,
    ) -> Result<(), String> {
        let simulation =
            simulation.ok_or_else(|| "accepted map load produced no Simulation".to_string())?;
        let StartupPhase::Loading(active_correlation) = self.phase else {
            return Err("accepted map load lost its active correlation".to_string());
        };
        let receipt = RustL0Observation {
            startup: &startup,
            simulation,
            active_correlation,
            prior_receipt: None,
            screen_is_loading,
        }
        .acknowledge()
        .map_err(|error| error.to_string())?;
        self.phase = StartupPhase::Accepted { startup, receipt };
        Ok(())
    }

    pub(crate) fn accepted(&self) -> Option<(&PreparedMatchStartup, &RustL0Receipt)> {
        match &self.phase {
            StartupPhase::Accepted { startup, receipt } => Some((startup, receipt)),
            _ => None,
        }
    }

    pub(crate) fn startup(&self) -> Option<&PreparedMatchStartup> {
        self.accepted().map(|(startup, _)| startup)
    }

    pub(crate) fn receipt(&self) -> Option<&RustL0Receipt> {
        self.accepted().map(|(_, receipt)| receipt)
    }

    pub(crate) fn admits_ordinary_tick(&self) -> bool {
        // Preserve the previous None/None policy, including during loading.
        // The app frame's InGame screen gate prevents loading from advancing.
        crate::match_bootstrap::accepted_tick_is_admitted(self.startup(), self.receipt())
    }

    pub(crate) fn admits_exact_step(&self) -> bool {
        matches!(self.phase, StartupPhase::Restored)
            || (self.accepted().is_some() && self.admits_ordinary_tick())
    }
}
