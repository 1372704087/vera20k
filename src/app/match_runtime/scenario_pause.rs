//! System-clock620 for the shared ScenarioPause684060/684180 owner.
//! Simulation's ScenarioSession owns62C/630 and the input-lock transitions.
//! This leaf supplies no gameplay clock, RNG or competing pause counter.
use crate::app::AppState;
use crate::sim::timer::CdTimer;
use crate::sim::trigger_runtime::TriggerEffect;

#[derive(Debug, Default)]
pub(crate) struct ScenarioPauseClock {
    timer: CdTimer,
}

impl ScenarioPauseClock {
    /// PostRead683571 records the current bucket, even for duration0;
    /// preserve signed elapsed behavior across the low32 millisecond wrap.
    fn after_load(wall_ms: u64) -> Self {
        Self {
            timer: CdTimer::started(bucket(wall_ms), 0),
        }
    }

    fn rearm(&mut self, duration: i32, wall_ms: u64) {
        if duration != 0 {
            self.timer.start(bucket(wall_ms), duration.wrapping_mul(60));
        }
    }

    fn elapsed(&self, wall_ms: u64) -> bool {
        self.timer.remaining(bucket(wall_ms)) <= 0
    }
}

const fn bucket(wall_ms: u64) -> i32 {
    ((wall_ms as u32) >> 4) as i32
}

pub(crate) fn enter(state: &mut AppState, duration: i32, outermost: bool, wall_ms: u64) {
    state
        .match_state
        .match_presentation
        .scenario_pause
        .rearm(duration, wall_ms);
    if !outermost {
        return;
    }
    if matches!(
        state.match_state.session.mode(),
        super::session::SessionMode::Campaign | super::session::SessionMode::Skirmish
    ) {
        state.match_state.scenario_elapsed_clock.pause(wall_ms);
    }
    if let Some(sfx) = state.audio.sfx_player.as_mut() {
        sfx.set_paused(true, wall_ms);
    }
}

pub(crate) fn resume(state: &mut AppState, wall_ms: u64) {
    let paused = state.match_state.presentation_paused();
    if !paused {
        state.match_state.scenario_elapsed_clock.resume(wall_ms);
    }
    if let Some(sfx) = state.audio.sfx_player.as_mut() {
        sfx.set_paused(paused, wall_ms);
    }
}

/// Main_Tick55D7C2..55D82E: only modes0/5 inspect620. Restored timers
/// begin at zero (PostRead683571..68358E); a saved timed hold releases on
/// the first live poll, while untimed holds remain with their existing owner.
pub(crate) fn poll(state: &mut AppState, wall_ms: u64) -> Vec<TriggerEffect> {
    if matches!(
        state.match_state.session.mode(),
        super::session::SessionMode::Campaign | super::session::SessionMode::Skirmish
    ) && state
        .match_state
        .match_presentation
        .scenario_pause
        .elapsed(wall_ms)
        && let Some(rt) = state.match_state.sim_runtime.as_mut()
        && rt.simulation.session.scenario_pause_timed()
    {
        return rt.simulation.expire_scenario_pause();
    }
    Vec::new()
}

/// Retire only this world's physical clock. Saved62C/630 remain simulation
/// state; a failed load cannot release the outgoing world's authoritative hold.
pub(crate) fn load_clock(state: &mut AppState, wall_ms: u64) {
    state.match_state.match_presentation.scenario_pause = ScenarioPauseClock::after_load(wall_ms);
}

pub(crate) fn reset_clock(state: &mut AppState) {
    state.match_state.match_presentation.scenario_pause = ScenarioPauseClock::default();
}

#[cfg(test)]
#[path = "scenario_pause_tests.rs"]
mod tests;
