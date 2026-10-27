//! Opt-in wall-time measurements beside the existing live capture. Commands,
//! simulation time, and network policy remain owned by their production paths.
use super::*;
use crate::app::match_diagnostics::{
    ActorPresentation, CommandExecutionObservation, ObservationTime,
};
use crate::sim::command::{MegaMissionOrder, MegaMissionRecord, StopRecord};

const TARGET_PAIRS: usize = 30;
const WARMUP_MS: u64 = 5_000;
const WARMUP_LIMIT_MS: u64 = 10_000;
const ACTION_TIMEOUT_MS: u64 = 5_000;
const STOP_QUIET_MS: u64 = 100;

struct Action {
    kind: &'static str,
    actor: u64,
    target: Option<(u16, u16)>,
    issue: ObservationTime,
    issue_frame: u32,
    baseline: ([f32; 2], u16),
    execution: Option<usize>,
    response: Option<ActorPresentation>,
    quiet: Option<ActorPresentation>,
    error: Option<&'static str>,
    completion_logged: bool,
}

struct Execution {
    observed: CommandExecutionObservation,
    kind: &'static str,
    target: Option<(u16, u16)>,
}

#[derive(Default)]
pub(super) struct LatencyCapture {
    started: Option<ObservationTime>,
    start_frame: u32,
    warmup: Option<Value>,
    actor: Option<u64>,
    origin: Option<(u16, u16)>,
    movement_observed: bool,
    navigation_clear_at_end: bool,
    actions: Vec<Action>,
    executions: Vec<Execution>,
    samples: Vec<Value>,
    last_sample: Option<ObservationTime>,
    last_present: Option<ObservationTime>,
    maximum_render_interval_us: u64,
    next_issue: Option<crate::util::time::Instant>,
    errors: Vec<String>,
    peer_rtt: Option<Vec<(u8, u32)>>,
    finished: bool,
}

impl LatencyCapture {
    pub(super) fn observe_setup(&mut self, state: &AppState) {
        if let Some(rtt) = crate::app::lan_service::online_peer_round_trips(state) {
            self.peer_rtt = Some(rtt);
        }
    }

    fn timestamp(&self, at: ObservationTime, frame: u32) -> Value {
        json!({"mono_us":at.instant.saturating_duration_since(self.started.unwrap().instant).as_micros() as u64,
            "unix_us":at.unix_us,"frame":frame})
    }

    fn observe_execution(&mut self, observed: CommandExecutionObservation) {
        let (kind, target) = if let Some(command) = MegaMissionRecord::decode(&observed.record) {
            let MegaMissionOrder::Move { target_x, target_y } = command.order else {
                return;
            };
            ("Move", Some((target_x as u16, target_y as u16)))
        } else if StopRecord::decode(&observed.record).is_some() {
            ("Stop", None)
        } else {
            return;
        };
        if self.executions.len() >= 4096 {
            if self.errors.is_empty() {
                self.errors
                    .push("command observation capacity exceeded".into());
            }
            return;
        }
        let index = self.executions.len();
        if let Some(action) = self.actions.iter_mut().find(|action| {
            action.execution.is_none()
                && action.kind == kind
                && Some(action.actor) == observed.actor
                && action.target == target
                && observed.observed_at.instant >= action.issue.instant
        }) {
            if !observed.applied {
                action.error = Some("receiver rejected command");
            }
            action.execution = Some(index);
        }
        self.executions.push(Execution {
            observed,
            kind,
            target,
        });
    }

    fn observe_presented(&mut self, presented: ActorPresentation) {
        let Some(action) = self.actions.last_mut() else {
            return;
        };
        let Some(index) = action.execution else {
            return;
        };
        let execution = &self.executions[index].observed;
        if action.response.is_some() || presented.at.instant < execution.observed_at.instant {
            return;
        }
        if action.kind == "Move" {
            if (presented.screen, presented.body) != action.baseline {
                action.response = Some(presented);
            }
        } else if execution.destination.is_none() {
            match action.quiet.as_ref() {
                Some(first) if first.screen == presented.screen && first.body == presented.body => {
                    if presented.frame.saturating_sub(first.frame) >= 10
                        && presented
                            .at
                            .instant
                            .saturating_duration_since(first.at.instant)
                            .as_millis()
                            >= u128::from(STOP_QUIET_MS)
                    {
                        action.response = Some(first.clone());
                    }
                }
                _ => action.quiet = Some(presented),
            }
        }
    }

    /// Called after each real render submission; a failed capacity gate remains
    /// evidence, rather than suppressing the commands needed to diagnose it.
    pub(super) fn step(&mut self, state: &mut AppState, limit: u32) -> Result<bool> {
        // Nonblocking dispatch of completion callbacks; delayed callbacks only
        // overestimate latency, unlike CPU submission timestamps.
        let _ = state.renderer.gpu.device.poll(wgpu::PollType::Poll);
        let now = ObservationTime::now();
        let sim = &state
            .match_state
            .sim_runtime
            .as_ref()
            .context("latency simulation")?
            .simulation;
        let frame = sim.session().binary_frame();
        if self.started.is_none() {
            self.started = Some(now);
            self.start_frame = frame;
            state
                .match_state
                .match_diagnostics
                .begin_command_observation();
        }
        let elapsed = now
            .instant
            .saturating_duration_since(self.started.unwrap().instant)
            .as_millis() as u64;
        for observation in state
            .match_state
            .match_diagnostics
            .take_command_observations()?
        {
            self.observe_execution(observation);
        }
        if let Some(presented) = state.match_state.match_diagnostics.actor_presentation() {
            self.observe_presented(presented);
        }
        let sim = &state
            .match_state
            .sim_runtime
            .as_ref()
            .context("latency simulation")?
            .simulation;
        if let Some(entity) = self.actor.and_then(|actor| sim.entities().get(actor)) {
            self.movement_observed |= self
                .origin
                .is_some_and(|origin| origin != (entity.position.rx(), entity.position.ry()));
            self.navigation_clear_at_end =
                entity.navigation.nav_com().is_none() && entity.navigation.nav_queue().is_empty();
        }
        let live = state
            .match_state
            .live_network
            .as_ref()
            .context("latency transport")?;
        let fps = live.fixed_frames_per_second();
        let frontier = live.committed_frontier();
        let recovery = live.recovery_status(
            crate::app::match_runtime::frame_pacer::wall_clock_ms() as u32,
            frame,
        );
        let interval = self.last_present.map(|last| {
            now.instant
                .saturating_duration_since(last.instant)
                .as_micros() as u64
        });
        self.maximum_render_interval_us =
            self.maximum_render_interval_us.max(interval.unwrap_or(0));
        self.last_present = Some(now);
        if self.last_sample.is_none_or(|last| {
            now.instant
                .saturating_duration_since(last.instant)
                .as_millis()
                >= 250
        }) {
            let mut sample = self.timestamp(now, frame);
            sample["fps"] = json!(fps);
            sample["committed_frontier"] = json!(frontier);
            sample["frontier_gap"] = json!(frontier.saturating_sub(frame));
            sample["recovery"] = json!(format!("{recovery:?}"));
            sample["render_interval_us"] = json!(interval);
            sample["maximum_render_interval_us"] = json!(self.maximum_render_interval_us);
            sample["pending_local_commands"] = json!(sim.pending_local_command_count());
            sample["pending_synchronized_commands"] =
                json!(sim.synchronized_network_command_count());
            if self.samples.len() < 4096 {
                self.samples.push(sample);
            }
            self.last_sample = Some(now);
            self.maximum_render_interval_us = 0;
        }
        if self.warmup.is_none() && elapsed >= WARMUP_MS {
            let expected = fps.map(|fps| elapsed as f64 * f64::from(fps) / 1000.0);
            let advanced = frame.saturating_sub(self.start_frame);
            let keeping_up = expected.is_some_and(|expected| f64::from(advanced) >= expected * 0.9)
                && frontier.saturating_sub(frame) <= 2
                && recovery.is_none();
            if keeping_up || elapsed >= WARMUP_LIMIT_MS {
                self.warmup = Some(json!({"minimum_ms":WARMUP_MS,"maximum_ms":WARMUP_LIMIT_MS,
                    "elapsed_ms":elapsed,"passed":keeping_up,"frames_advanced":advanced,
                    "expected_frames":expected,"frontier_gap":frontier.saturating_sub(frame),
                    "recovery":format!("{recovery:?}")}));
            }
        }
        if let Some(action) = self.actions.last_mut() {
            if !action_gpu_complete(action)
                && action.error.is_none()
                && now
                    .instant
                    .saturating_duration_since(action.issue.instant)
                    .as_millis()
                    >= u128::from(ACTION_TIMEOUT_MS)
            {
                action.error = Some("no submitted response within 5000ms");
                log::info!(
                    "Latency action timeout: kind={} actor={} issue_unix_us={} dispatch_unix_us={:?}",
                    action.kind,
                    action.actor,
                    action.issue.unix_us,
                    action
                        .execution
                        .map(|index| self.executions[index].observed.observed_at.unix_us)
                );
            }
        }
        if let Some(action) = self.actions.last_mut()
            && !action.completion_logged
            && action_gpu_complete(action)
        {
            let response = action.response.as_ref().unwrap();
            let gpu = response.gpu_completed_at().unwrap();
            let dispatch = self.executions[action.execution.unwrap()]
                .observed
                .observed_at;
            log::info!(
                "Latency action complete: kind={} actor={} issue_unix_us={} dispatch_unix_us={} submitted_unix_us={} gpu_unix_us={} dispatch_ms={:.3} submitted_ms={:.3} gpu_ms={:.3}",
                action.kind,
                action.actor,
                action.issue.unix_us,
                dispatch.unix_us,
                response.at.unix_us,
                gpu.unix_us,
                dispatch
                    .instant
                    .saturating_duration_since(action.issue.instant)
                    .as_secs_f64()
                    * 1000.0,
                response
                    .at
                    .instant
                    .saturating_duration_since(action.issue.instant)
                    .as_secs_f64()
                    * 1000.0,
                gpu.instant
                    .saturating_duration_since(action.issue.instant)
                    .as_secs_f64()
                    * 1000.0
            );
            action.completion_logged = true;
        }
        // The requested prefix is a bound, even when capacity or rendering is
        // insufficient. Persist all partial observations and use normal exit.
        if frame >= limit {
            self.finished = true;
            state
                .match_state
                .match_diagnostics
                .end_command_observation();
            state
                .match_state
                .match_diagnostics
                .end_presentation_observation();
            return Ok(true);
        }
        if self.warmup.is_none() || !self.errors.is_empty() {
            return Ok(false);
        }
        if let Some(action) = self.actions.last() {
            if !action_gpu_complete(action) && action.error.is_none() {
                return Ok(false);
            }
            if self.next_issue.is_none() {
                self.next_issue = Some(
                    now.instant
                        + std::time::Duration::from_millis(if action.kind == "Move" {
                            350
                        } else {
                            250
                        }),
                );
            }
        }
        if self.actions.len() >= TARGET_PAIRS * 2
            || self
                .next_issue
                .is_some_and(|deadline| now.instant < deadline)
        {
            return Ok(false);
        }
        let kind = if self.actions.len() % 2 == 0 {
            "Move"
        } else {
            "Stop"
        };
        let result = self.issue(state, kind, frame);
        if let Err(error) = result {
            self.errors.push(format!("{error:#}"));
        }
        self.next_issue = None;
        Ok(false)
    }

    fn issue(&mut self, state: &mut AppState, kind: &'static str, frame: u32) -> Result<()> {
        let (actor, target, at) = if kind == "Move" {
            let (actor, from, target, at) =
                battle::latency_move_with_mouse(state, self.actor, self.actions.len() / 2)?;
            if self.actor.is_none() {
                self.actor = Some(actor);
                self.origin = Some(from);
                state
                    .match_state
                    .match_diagnostics
                    .observe_actor_presentation(actor);
            }
            (actor, Some(target), at)
        } else {
            let actor = self.actor.context("latency actor")?;
            (
                actor,
                None,
                battle::latency_stop_with_keyboard(state, actor)?,
            )
        };
        let sim = &state
            .match_state
            .sim_runtime
            .as_ref()
            .context("latency simulation")?
            .simulation;
        let entity = sim.entities().get(actor).context("latency actor")?;
        let pose = crate::app::presentation::interpolation::pose(state, entity, frame);
        log::info!(
            "Latency action issued: kind={kind} actor={actor} issue_unix_us={} frame={frame}",
            at.unix_us
        );
        self.actions.push(Action {
            kind,
            actor,
            target,
            issue: at,
            issue_frame: frame,
            baseline: (pose.screen, pose.body),
            execution: None,
            response: None,
            quiet: None,
            error: None,
            completion_logged: false,
        });
        Ok(())
    }

    fn execution_json(&self, execution: &Execution) -> Value {
        let observed = &execution.observed;
        let mut canonical = observed.record.clone();
        canonical.clear_processed();
        let record_hex: String = canonical
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let mut dispatch = self.timestamp(observed.observed_at, observed.frame);
        dispatch["applied"] = json!(observed.applied);
        json!({"execution_key":{"house":observed.record.house_id(),"opcode":observed.record.opcode(),
            "scheduled_frame":observed.record.frame_stamp(),"actor":observed.actor,"record_hex":record_hex},
            "kind":execution.kind,"target":execution.target,"dispatch":dispatch,
            "receiver":{"destination":observed.destination,"cell":observed.cell,"mission":observed.mission}})
    }

    pub(super) fn receipt(&self) -> Value {
        let actions: Vec<_> = self.actions.iter().enumerate().map(|(id, action)| {
            let execution = action.execution.map(|index| self.execution_json(&self.executions[index]));
            let latency = |at: ObservationTime| at.instant.saturating_duration_since(action.issue.instant).as_secs_f64() * 1000.0;
            let dispatch_ms = action.execution.map(|index| latency(self.executions[index].observed.observed_at));
            let response_ms = action.response.as_ref().map(|response| latency(response.at));
            let submitted = action.response.as_ref().map(|response| {
                let mut value = self.timestamp(response.at, response.frame);
                value["screen"] = json!(response.screen); value["body"] = json!(response.body); value
            });
            let gpu = action.response.as_ref().and_then(|response| response.gpu_completed_at().map(|at| (at,response.frame)));
            let gpu_ms = gpu.map(|(at,_)|latency(at));
            json!({"id":id,"kind":action.kind,"actor":action.actor,"target":action.target,
                "issue":self.timestamp(action.issue,action.issue_frame),
                "execution_key":execution.as_ref().map(|value|&value["execution_key"]),
                "dispatch":execution.as_ref().map(|value|&value["dispatch"]),
                "submitted_response":submitted,"gpu_response":gpu.map(|(at,frame)|self.timestamp(at,frame)),"error":action.error,
                "latency_ms":{"dispatch":dispatch_ms,"submitted_response":response_ms,"gpu_response":gpu_ms},
                "exceeds_500ms":{"dispatch":dispatch_ms.map(|ms|ms>500.0),"submitted_response":response_ms.map(|ms|ms>500.0),"gpu_response":gpu_ms.map(|ms|ms>500.0)}})
        }).collect();
        let completed_pairs = self
            .actions
            .chunks_exact(2)
            .filter(|pair| {
                pair.iter()
                    .all(|a| action_gpu_complete(a) && a.error.is_none())
            })
            .count();
        let complete = completed_pairs == TARGET_PAIRS;
        json!({"schema":"vera20k.online-input-latency.v1","target_pairs":TARGET_PAIRS,
            "completed_pairs":completed_pairs,"status":if complete {"complete"} else if self.finished {"insufficient_capacity"} else {"sampling"},
            "limitation":"submitted admitted Unit draw-plan geometry; GPU callback is a completion upper bound, not monitor scanout; remote observations are dispatch only",
            "clock":{"unix_unit":"microseconds","mono_unit":"microseconds since this round observer began","remote_requires_calibration":true},
            "warmup":self.warmup,"actions":actions,"executions":self.executions.iter().map(|e|self.execution_json(e)).collect::<Vec<_>>(),
            "keep_up":self.samples,"peer_round_trip_ms":self.peer_rtt,"peer_round_trip_scope":"one-shot startup peer RTT; 16ms buckets; separate from command latency",
            "validation":{"movement_observed":self.movement_observed,"navigation_clear_at_end":self.navigation_clear_at_end,
                "last_stop_submitted":self.actions.last().is_some_and(|a|a.kind=="Stop" && a.response.is_some())},
            "errors":self.errors})
    }
}

fn action_gpu_complete(action: &Action) -> bool {
    action
        .response
        .as_ref()
        .is_some_and(|response| response.gpu_completed_at().is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::command::{CommandRecord, IDLE_OPCODE};

    fn at(base: ObservationTime, ms: u64) -> ObservationTime {
        ObservationTime {
            instant: base.instant + std::time::Duration::from_millis(ms),
            unix_us: base.unix_us + ms * 1000,
        }
    }
    fn stop(base: ObservationTime, ms: u64, actor: u64, frame: u32) -> CommandExecutionObservation {
        let mut record = CommandRecord::encode(IDLE_OPCODE, 0, frame as i32, &[]).unwrap();
        StopRecord {
            house_id: 0,
            frame: frame as i32,
            entity_id: actor as i32,
        }
        .write_into(&mut record);
        CommandExecutionObservation {
            observed_at: at(base, ms),
            frame,
            record,
            applied: true,
            actor: Some(actor),
            timing: None,
            destination: None,
            cell: Some((10, 10)),
            mission: Some((0, 0)),
        }
    }
    fn action(issue: ObservationTime) -> Action {
        Action {
            kind: "Stop",
            actor: 7,
            target: None,
            issue,
            issue_frame: 10,
            baseline: ([0., 0.], 0),
            execution: None,
            response: None,
            quiet: None,
            error: None,
            completion_logged: false,
        }
    }

    #[test]
    fn repeated_stops_keep_independent_canonical_executions_and_remote_records() {
        let start = ObservationTime::now();
        let mut capture = LatencyCapture {
            started: Some(start),
            ..Default::default()
        };
        capture.actions.push(action(start));
        capture.observe_execution(stop(start, 100, 99, 12));
        capture.observe_execution(stop(start, 200, 7, 13));
        capture.actions.push(action(at(start, 300)));
        capture.observe_execution(stop(start, 450, 7, 19));
        let receipt = capture.receipt();
        assert_eq!(receipt["executions"].as_array().unwrap().len(), 3);
        assert_eq!(
            receipt["actions"][0]["execution_key"]["scheduled_frame"],
            13
        );
        assert_eq!(
            receipt["actions"][1]["execution_key"]["scheduled_frame"],
            19
        );
        assert_eq!(receipt["actions"][1]["latency_ms"]["dispatch"], 150.0);
        assert_ne!(
            receipt["actions"][0]["execution_key"]["record_hex"],
            receipt["actions"][1]["execution_key"]["record_hex"]
        );
    }

    #[test]
    fn stop_response_uses_first_stationary_submission_after_dispatch() {
        let start = ObservationTime::now();
        let mut capture = LatencyCapture {
            started: Some(start),
            ..Default::default()
        };
        capture.actions.push(action(start));
        capture.observe_execution(stop(start, 200, 7, 13));
        let presentation = |ms, frame, x| ActorPresentation {
            at: at(start, ms),
            frame,
            screen: [x, 0.],
            body: 0,
            gpu_done: std::sync::Arc::new(std::sync::Mutex::new(Some(at(start, ms + 10)))),
        };
        capture.observe_presented(presentation(150, 12, 1.));
        assert!(capture.actions[0].quiet.is_none());
        capture.observe_presented(presentation(250, 14, 1.));
        let first_stationary = presentation(300, 15, 2.);
        let completion = first_stationary.gpu_done.clone();
        *completion.lock().unwrap() = None;
        capture.observe_presented(first_stationary);
        capture.observe_presented(presentation(420, 18, 2.));
        assert!(
            capture.actions[0].response.is_none(),
            "a three-frame pause is insufficient"
        );
        capture.observe_presented(presentation(650, 25, 2.));
        assert!(!action_gpu_complete(&capture.actions[0]));
        *completion.lock().unwrap() = Some(at(start, 310));
        assert!(action_gpu_complete(&capture.actions[0]));
        assert_eq!(
            capture.receipt()["actions"][0]["gpu_response"]["mono_us"],
            310_000
        );
        assert_eq!(
            capture.receipt()["actions"][0]["submitted_response"]["mono_us"],
            300_000
        );
        assert_eq!(
            capture.receipt()["actions"][0]["latency_ms"]["submitted_response"],
            300.0
        );
    }
}
