//! Rust regression tests for script actions that act on every member.

use super::*;
use crate::rules::ini_parser::IniFile;
use crate::rules::team_ai_ini::TeamAiDefinitionSource;
use crate::sim::house_state::HouseState;
use crate::sim::team_script_vm::{TeamScriptAction, TeamScriptDefinition};

const RULES: &str = "\
[InfantryTypes]
0=E1
[E1]
Strength=125
Speed=4
Locomotor={4A582744-9839-11D1-B709-00A024DDAFD1}
";

fn rules() -> RuleSet {
    RuleSet::from_ini_with_fixed_art_for_test(&IniFile::from_str(RULES), &IniFile::from_str(""))
        .expect("team action rules")
}

/// Action 21 (`0x006E9650`): every member scatters from the null coordinate,
/// forced, so each takes a neighbouring cell to walk to, and the action is
/// finished at once.
#[test]
fn scatter_sends_every_member_to_a_neighbouring_cell() {
    let rules = rules();
    let mut sim = Simulation::with_seed(0x6E9650);
    let owner = sim.interner.intern("Russians");
    sim.houses
        .insert(owner, HouseState::new(owner, 1, None, false, 0, 10));
    sim.session.house_order.push(owner);
    let grid = crate::sim::arena_fixture::flat_arena(&mut sim, &rules);
    let starts = [(10, 10), (11, 10), (12, 10)];
    let members: Vec<u64> = starts
        .iter()
        .map(|&(rx, ry)| {
            sim.spawn_object_at_height("E1", "Russians", rx, ry, 0, 0, &rules)
                .expect("E1 spawns")
        })
        .collect();
    let script = sim.interner.intern("SCATTER SCRIPT");
    sim.team_script_vm.register_script(TeamScriptDefinition {
        id: script,
        source: TeamAiDefinitionSource::FixedAimd,
        actions: vec![
            TeamScriptAction {
                action_id: 21,
                argument: 0,
            },
            TeamScriptAction {
                action_id: 49,
                argument: 0,
            },
        ],
    });
    let team = sim
        .team_script_vm
        .create_team(owner, script, members.clone(), 0);

    for _ in 0..30 {
        if sim
            .team_script_vm
            .team(team)
            .is_some_and(|team| team.cursor() >= 1)
        {
            break;
        }
        let commands = sim.take_due_commands();
        sim.advance_tick(&commands, Some(&rules), Some(&grid), None, 33);
    }

    let state = sim.team_script_vm.team(team).expect("the team lives");
    assert!(state.cursor() >= 1, "action 21 finished");
    assert!(state.refusal().is_none());
    for (member, start) in members.iter().zip(starts) {
        match sim
            .substrate
            .entities
            .get(*member)
            .unwrap()
            .navigation
            .nav_com
        {
            Some(NavTargetRef::Cell { rx, ry }) => assert!(
                rx.abs_diff(start.0) <= 1 && ry.abs_diff(start.1) <= 1 && (rx, ry) != start,
                "member {member} heads for ({rx}, {ry}) from {start:?}"
            ),
            other => panic!("member {member} did not scatter: {other:?}"),
        }
    }
}

/// Through the production readers, the one retail script naming action 21
/// (`0CA2192C-G` "Soviet Flak Defense": 58, 21, 11) runs only ported actions,
/// so the one TeamType running it no longer idles on 21.
#[test]
fn the_retail_scatter_script_runs_ported_actions() {
    use crate::rules::retail_ini_fixture::{retail_ini, retail_rules_and_art};
    use crate::rules::team_ai_ini::TeamAiIniRegistry;
    let Some((rules_ini, art_ini)) = retail_rules_and_art() else {
        return;
    };
    let rules = RuleSet::from_ini_with_fixed_art_for_test(&rules_ini, &art_ini).unwrap();
    let aimd = retail_ini("aimd.ini").expect("retail AIMD");
    let registry = TeamAiIniRegistry::from_sources(&aimd, &IniFile::from_str(""), true);
    let mut sim = Simulation::new();
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    sim.install_team_ai_registry(&registry, &rules)
        .expect("retail AIMD installs");
    let vm = &sim.team_script_vm;
    let scripts: Vec<_> = vm
        .scripts
        .values()
        .filter(|script| script.actions.iter().any(|action| action.action_id == 21))
        .collect();
    assert_eq!(scripts.len(), 1);
    let actions: Vec<i32> = scripts[0]
        .actions
        .iter()
        .map(|action| action.action_id)
        .collect();
    assert_eq!(actions, [58, 21, 11]);
    assert!(actions.iter().all(|&action| action_is_ported(action)));
    let running = vm
        .team_types
        .values()
        .filter(|team_type| team_type.script_id == scripts[0].id)
        .count();
    assert_eq!(running, 1);
}
