//! Phobos `Convert.Undeploy` through the production deployment-completion
//! owner and the DoingAI latch reset.

use crate::rules::ini_parser::IniFile;
use crate::rules::ruleset::RuleSet;
use crate::sim::house_state::HouseState;
use crate::sim::world::{FrameEffects, Simulation};

fn rules(a_extra: &str) -> RuleSet {
    RuleSet::from_ini(&crate::sim::arena_fixture::clear_ground_ini(&format!(
        "[InfantryTypes]\n0=A\n1=B\n2=C\n\
         [A]\nStrength=100\nSpeed=4\n{a_extra}\
         [B]\nStrength=100\nSpeed=4\nConvert.Undeploy=C\n\
         [C]\nStrength=100\nSpeed=4\n"
    )))
    .expect("convert undeploy rules")
}

fn world(rules: &RuleSet) -> (Simulation, u64) {
    let mut sim = Simulation::new();
    crate::sim::arena_fixture::flat_ground(&mut sim, rules);
    let owner = sim.intern("Americans");
    sim.houses
        .register(HouseState::new(owner, 0, None, false, 0, 10));
    let id = sim
        .spawn_object("A", "Americans", 10, 10, 0, rules)
        .expect("A spawns");
    (sim, id)
}

fn kind(sim: &Simulation, id: u64) -> String {
    sim.interner
        .resolve(sim.entities().get(id).unwrap().type_ref())
        .to_owned()
}

/// `InfantryClass_DoingAI_DeployConvert_Undeploy @ 0x520B99`
/// (`Ext/Infantry/Hooks.cpp:36-50`) with the `0x520E75` latch reset: one
/// conversion per Undeploy sequence, re-armed outside Deploy/Undeploy.
#[test]
fn an_undeploy_completion_converts_once_per_sequence() {
    let rules = rules("Convert.Undeploy=B\n");
    let (mut sim, id) = world(&rules);
    sim.infantry_deploy_completion(id, 31, &rules, FrameEffects::default())
        .unwrap();
    assert_eq!(kind(&sim, id), "B");
    // The latch keeps the same sequence from converting again; B's own
    // `Convert.Undeploy=C` is what a missing latch would reach.
    sim.infantry_deploy_completion(id, 31, &rules, FrameEffects::default())
        .unwrap();
    assert_eq!(
        kind(&sim, id),
        "B",
        "the latch allows one conversion per sequence"
    );
    // The DoingAI reset clears it while the sequence is neither Deploy nor
    // Undeploy (the completion left the man on Ready).
    let _ = sim.infantry_sequencer(id, &rules, FrameEffects::default());
    sim.infantry_deploy_completion(id, 31, &rules, FrameEffects::default())
        .unwrap();
    assert_eq!(kind(&sim, id), "C", "the reset re-arms the next Undeploy");
}

/// A type without `Convert.Undeploy` keeps its type on completion.
#[test]
fn without_the_key_the_completion_does_not_convert() {
    let rules = rules("");
    let (mut sim, id) = world(&rules);
    sim.infantry_deploy_completion(id, 31, &rules, FrameEffects::default())
        .unwrap();
    assert_eq!(kind(&sim, id), "A");
}
