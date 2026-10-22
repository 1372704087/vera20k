//! Full-selection input proof through production keyboard, mouse and command owners.
use super::*;
use crate::sim::command::{Command, MegaMissionOrder, MegaMissionRecord, StopRecord};
use crate::sim::components::DriveCoord;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(super) struct MassInput {
    selected: Vec<u64>,
    selected_set: BTreeSet<u64>,
    positions: Vec<DriveCoord>,
    replay_cursor: usize,
    order: usize,
    pending: Option<Batch>,
    batches: Vec<Value>,
    observed_by_house: BTreeMap<i8, [u64; 2]>,
    executed_by_house: BTreeMap<i8, [u64; 2]>,
    initialized: bool,
}
struct Batch {
    kind: &'static str,
    issued_frame: u32,
    submitted: usize,
    sent_before: u64,
    accepted: BTreeMap<u64, Option<(i16, i16)>>,
    executed: BTreeMap<u64, Option<(i16, i16)>>,
    quiet: Option<(u32, Vec<DriveCoord>)>,
}

impl MassInput {
    pub(super) fn step(&mut self, state: &mut AppState, frame: u32) -> Result<bool> {
        self.observe_admissions(state, frame)?;
        self.observe_executions(state)?;
        if !self.initialized && frame >= 10 {
            battle::combatants_with_keyboard(state)?;
            self.selected = battle::projected_selection(state);
            self.selected_set = self.selected.iter().copied().collect();
            let expected = local_combatants(state)?;
            ensure!(
                self.selected_set == expected,
                "CombatantSelect omitted local combatants or selected foreign actors"
            );
            ensure!(
                self.selected_set.len() == self.selected.len(),
                "selection contains duplicate actors"
            );
            self.positions = positions(state, &self.selected)?;
            self.initialized = true;
            state
                .match_state
                .match_diagnostics
                .begin_command_observation();
            log::info!(
                "Mass capture selected {} local combatants",
                self.selected.len()
            );
        }
        if !self.initialized {
            return Ok(false);
        }
        if self.selected.is_empty() {
            return Ok(true);
        }
        if let Some(batch) = self.pending.as_mut() {
            if !batch.covers(&self.selected_set) {
                return Ok(false);
            }
            let current = positions(state, &self.selected)?;
            if batch.kind == "Move" {
                let (since, _) = batch.quiet.get_or_insert_with(|| (frame, Vec::new()));
                if frame.saturating_sub(*since) < 20 {
                    return Ok(false);
                }
                let moved = current
                    .iter()
                    .zip(&self.positions)
                    .filter(|(now, before)| now != before)
                    .count();
                ensure!(moved > 0, "full-selection Move caused no gameplay movement");
                self.finish_batch(state, frame, moved, 0)?;
            } else {
                let sim = simulation(state)?;
                let cleared = self
                    .selected
                    .iter()
                    .filter(|id| {
                        sim.entities().get(**id).is_some_and(|entity| {
                            entity.navigation.nav_com().is_none()
                                && entity.navigation.nav_queue().is_empty()
                        })
                    })
                    .count();
                if cleared != self.selected.len() {
                    return Ok(false);
                }
                if batch
                    .quiet
                    .as_ref()
                    .is_none_or(|(_, previous)| previous != &current)
                {
                    batch.quiet = Some((frame, current));
                    return Ok(false);
                }
                if frame.saturating_sub(batch.quiet.as_ref().unwrap().0) < 10 {
                    return Ok(false);
                }
                self.finish_batch(state, frame, 0, cleared)?;
            }
        }
        if self.pending.is_none() && self.order < 4 {
            ensure!(
                battle::projected_selection(state) == self.selected,
                "full selection changed before the next command"
            );
            self.positions = positions(state, &self.selected)?;
            let before = simulation(state)?.pending_local_command_count();
            let sent_before = state
                .match_state
                .live_network
                .as_ref()
                .context("live transport")?
                .sent_commands() as u64;
            let kind = if self.order % 2 == 0 {
                battle::selected_move_with_mouse(state, self.selected[0], self.order / 2)?;
                "Move"
            } else {
                battle::selected_stop_with_keyboard(state)?;
                "Stop"
            };
            let submitted = simulation(state)?
                .pending_local_command_count()
                .checked_sub(before)
                .context("local command FIFO shrank during input")?;
            ensure!(
                submitted == self.selected.len(),
                "{kind} queued {submitted} of {} selected actors",
                self.selected.len()
            );
            log::info!("Mass capture {} submitted {} actors", kind, submitted);
            self.pending = Some(Batch {
                kind,
                issued_frame: frame,
                submitted,
                sent_before,
                accepted: BTreeMap::new(),
                executed: BTreeMap::new(),
                quiet: None,
            });
        }
        Ok(self.order == 4 && self.pending.is_none())
    }

    fn observe_admissions(&mut self, state: &AppState, frame: u32) -> Result<()> {
        let log = state
            .match_state
            .match_diagnostics
            .replay_log
            .as_ref()
            .context("mass capture requires production command recording")?;
        ensure!(
            log.ticks.len() >= self.replay_cursor,
            "mass command timeline replaced"
        );
        for tick in &log.ticks[self.replay_cursor..] {
            for envelope in &tick.commands {
                let Command::SynchronizedRecord { record } = &envelope.payload else {
                    continue;
                };
                let (kind, actor, target) = if let Some(command) = MegaMissionRecord::decode(record)
                {
                    let MegaMissionOrder::Move { target_x, target_y } = command.order else {
                        continue;
                    };
                    ("Move", command.source_id, Some((target_x, target_y)))
                } else if let Some(command) = StopRecord::decode(record) {
                    ("Stop", command.entity_id, None)
                } else {
                    continue;
                };
                self.observed_by_house.entry(record.house_id()).or_default()
                    [usize::from(kind == "Stop")] += 1;
                let Some(batch) = self.pending.as_mut() else {
                    continue;
                };
                if kind != batch.kind || record.frame_stamp() < batch.issued_frame as i32 {
                    continue;
                }
                let Ok(actor) = u64::try_from(actor) else {
                    continue;
                };
                if !self.selected_set.contains(&actor) {
                    continue;
                }
                ensure!(
                    batch.accepted.insert(actor, target).is_none(),
                    "duplicate {kind} admission for selected actor {actor} at frame{frame}"
                );
            }
        }
        self.replay_cursor = log.ticks.len();
        Ok(())
    }

    fn observe_executions(&mut self, state: &mut AppState) -> Result<()> {
        use crate::sim::components::NavTargetRef;
        use crate::sim::mission::{MissionId, MissionType};
        for execution in state
            .match_state
            .match_diagnostics
            .take_command_observations()?
        {
            let record = &execution.record;
            let (kind, target) = if let Some(command) = MegaMissionRecord::decode(record) {
                let MegaMissionOrder::Move { target_x, target_y } = command.order else {
                    continue;
                };
                ("Move", Some((target_x, target_y)))
            } else if StopRecord::decode(record).is_some() {
                ("Stop", None)
            } else {
                continue;
            };
            if execution.applied {
                self.executed_by_house.entry(record.house_id()).or_default()
                    [usize::from(kind == "Stop")] += 1;
            }
            let Some(batch) = self.pending.as_mut() else {
                continue;
            };
            if kind != batch.kind
                || record.frame_stamp() < batch.issued_frame as i32
                || !self.selected_set.contains(&execution.actor)
            {
                continue;
            }
            ensure!(
                execution.applied,
                "{kind} receiver rejected selected actor {}",
                execution.actor
            );
            ensure!(
                execution.frame >= batch.issued_frame,
                "execution preceded issued input"
            );
            match target {
                Some((x, y)) => {
                    let x = u16::try_from(x).context("negative executed Move X")?;
                    let y = u16::try_from(y).context("negative executed Move Y")?;
                    let moving = MissionId::from_known(MissionType::Move).raw();
                    let moving_to_target = execution.destination == Some(NavTargetRef::cell(x, y))
                        && execution
                            .mission
                            .is_some_and(|(current, queued)| current == moving || queued == moving);
                    let already_at_target =
                        execution.cell == Some((x, y)) && execution.destination.is_none();
                    ensure!(
                        moving_to_target || already_at_target,
                        "Move execution did not install or reach its target for actor {}",
                        execution.actor
                    );
                }
                None => ensure!(
                    execution.mission.is_some() && execution.destination.is_none(),
                    "Stop execution retained a destination for actor {}",
                    execution.actor
                ),
            }
            ensure!(
                batch.executed.insert(execution.actor, target).is_none(),
                "duplicate executed {kind} for selected actor {}",
                execution.actor
            );
        }
        Ok(())
    }

    fn finish_batch(
        &mut self,
        state: &AppState,
        frame: u32,
        moved: usize,
        stopped: usize,
    ) -> Result<()> {
        let batch = self.pending.take().context("mass batch")?;
        let accepted: BTreeSet<_> = batch.accepted.keys().copied().collect();
        ensure!(
            accepted == self.selected_set && batch.covers(&self.selected_set),
            "mass command omitted a selected actor execution suffix"
        );
        let sent = state
            .match_state
            .live_network
            .as_ref()
            .context("live transport")?
            .sent_commands() as u64;
        ensure!(
            sent.saturating_sub(batch.sent_before) >= batch.submitted as u64,
            "command admission preceded the full network send"
        );
        let mut target_bytes = Vec::new();
        for (actor, target) in &batch.executed {
            target_bytes.extend_from_slice(&actor.to_le_bytes());
            if let Some((x, y)) = target {
                target_bytes.extend_from_slice(&x.to_le_bytes());
                target_bytes.extend_from_slice(&y.to_le_bytes());
            }
        }
        self.batches.push(json!({"order":self.order,"kind":batch.kind,"issued_frame":batch.issued_frame,
            "observed_frame":frame,"selected_actor_count":self.selected.len(),"submitted_actor_count":batch.submitted,
            "accepted_actor_count":batch.accepted.len(),"executed_actor_count":batch.executed.len(),
            "accepted_ids_digest":ids_digest(accepted.iter().copied()),"executed_ids_digest":ids_digest(batch.executed.keys().copied()),
            "targets_digest":crate::util::sha256::sha256_hex(&target_bytes),"omitted_ids":[],"moved_actor_count":moved,
            "stable_stopped_actor_count":stopped,"sent_frontier_before":batch.sent_before,"sent_frontier_after":sent}));
        log::info!(
            "Mass capture {} admitted all {} actors",
            batch.kind,
            batch.accepted.len()
        );
        self.order += 1;
        Ok(())
    }

    pub(super) fn receipt(&self) -> Value {
        json!({"selected_ids":self.selected,"selected_actor_count":self.selected.len(),
            "selected_ids_digest":ids_digest(self.selected_set.iter().copied()),
            "executed_actor_commands":self.order * self.selected.len(),"batches":self.batches,
            "observed_by_house":self.observed_by_house,"executed_by_house":self.executed_by_house,"complete":self.initialized && (self.selected.is_empty() || self.order == 4)})
    }
}
impl Batch {
    fn covers(&self, expected: &BTreeSet<u64>) -> bool {
        self.accepted.len() == expected.len()
            && self.executed.len() == expected.len()
            && expected
                .iter()
                .all(|id| self.accepted.contains_key(id) && self.executed.contains_key(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queued_prefix_cannot_pass_without_every_actor_executing() {
        let expected = (0..1024).collect::<BTreeSet<u64>>();
        let mut batch = Batch {
            kind: "Move",
            issued_frame: 1,
            submitted: 1024,
            sent_before: 0,
            accepted: expected.iter().map(|&id| (id, Some((10, 10)))).collect(),
            executed: (0..256).map(|id| (id, Some((10, 10)))).collect(),
            quiet: None,
        };
        assert!(
            !batch.covers(&expected),
            "a DoList prefix is not a full selection execution"
        );
        batch.executed = batch.accepted.clone();
        assert!(batch.covers(&expected));
    }
}

fn ids_digest(ids: impl Iterator<Item = u64>) -> String {
    crate::util::sha256::sha256_hex(&ids.flat_map(u64::to_le_bytes).collect::<Vec<_>>())
}
fn simulation(state: &AppState) -> Result<&crate::sim::world::Simulation> {
    Ok(&state
        .match_state
        .sim_runtime
        .as_ref()
        .context("mass simulation")?
        .simulation)
}
fn positions(state: &AppState, ids: &[u64]) -> Result<Vec<DriveCoord>> {
    let sim = simulation(state)?;
    ids.iter()
        .map(|id| {
            let entity = sim.entities().get(*id).context("mass actor disappeared")?;
            ensure!(
                entity.is_alive() && !entity.in_limbo(),
                "mass actor died during input proof"
            );
            Ok(crate::sim::movement::ground_pose::position_world_coord(
                &entity.position,
            ))
        })
        .collect()
}
fn local_combatants(state: &AppState) -> Result<BTreeSet<u64>> {
    let sim = simulation(state)?;
    let rules = state.rules().context("mass rules")?;
    let owner = sim.session().current_house().context("local House")?;
    Ok(sim
        .entities()
        .values_sorted()
        .filter(|entity| {
            entity.owner() == owner
                && entity.is_alive()
                && !entity.in_limbo()
                && entity.category() != crate::map::entities::EntityCategory::Structure
                && sim
                    .interner()
                    .object_type(entity.type_ref(), rules)
                    .is_some_and(|object| object.is_selectable_combatant)
                && crate::app::input::entity_pick::selection_dynamic_prefix(
                    entity,
                    sim.entities(),
                    Some(rules),
                    Some(sim.interner()),
                )
                && sim.object_is_selectable(entity.stable_id(), rules)
        })
        .map(|entity| entity.stable_id())
        .collect())
}
