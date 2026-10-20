//! `InfantryClass::Movement_Actions`' unreachable-NavCom recovery
//! (`0x00520FAF..0x00521144`) on retail rules, through the production
//! Movement_Actions entry.

use crate::rules::ruleset::RuleSet;
use crate::sim::components::NavTargetRef;
use crate::sim::mission::state::MissionTestFixture;
use crate::sim::mission::{MissionDispatchTimer, MissionId, MissionType};
use crate::sim::world::FrameEffects;

/// Beyond the Water column, out of an infantryman's zone.
const ACROSS: NavTargetRef = NavTargetRef::Cell { rx: 20, ry: 10 };
/// On his side of it.
const NEAR: NavTargetRef = NavTargetRef::Cell { rx: 12, ry: 10 };

#[derive(Clone, Copy, Debug, PartialEq)]
enum Case {
    Plain,
    Tethered,
    OffPlayfield,
    Entering,
}

/// One Movement_Actions visit of a `type_id` man standing at (10,10) on a
/// retail arena that a Water column at x 16 splits, holding `nav` as his
/// NavCom: the NavCom afterwards.
fn visit(type_id: &str, nav: NavTargetRef, case: Case) -> Option<Option<NavTargetRef>> {
    let rules: RuleSet = crate::rules::retail_ini_fixture::retail_unlayered_rules_with_art()?;
    let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 7);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    crate::sim::arena_fixture::flood(&mut sim, &rules, 32, (0..32).map(|y| (16, y)));
    let id = sim
        .spawn_object(type_id, "Americans", 10, 10, 0, &rules)
        .unwrap();
    let partner = sim
        .spawn_object("E1", "Americans", 8, 8, 0, &rules)
        .unwrap();
    let man = sim.substrate.entities.get_mut(id).unwrap();
    assert!(
        man.in_playfield(),
        "{type_id} {case:?}: spawned in the playfield"
    );
    man.navigation.set_nav_com_for_test(Some(nav));
    match case {
        Case::Plain => {}
        Case::Tethered => man.tether.set_for_test(Some(partner)),
        Case::OffPlayfield => man.set_in_playfield(false),
        Case::Entering => man.mission.apply_test_fixture(MissionTestFixture {
            current: MissionId::from_known(MissionType::Enter),
            suspended: MissionId::NONE,
            queued: MissionId::NONE,
            movement_bypass_latch: 0,
            handler_state: 0,
            mission_start_frame: 0,
            ai_counter: 0,
            dispatch_timer: MissionDispatchTimer::at_frame(0),
        }),
    }
    assert_ne!(
        man.mission.effective().known(),
        Some(MissionType::Move),
        "the stopped Move prefix stays out of these rows"
    );
    sim.infantry_movement_actions(id, &rules, None, FrameEffects::default());
    Some(sim.substrate.entities.get(id).unwrap().navigation.nav_com())
}

/// A standing rifleman drops a NavCom across the water and keeps one on his
/// side; a tethered man, one outside the playfield, one entering and an
/// `Infiltrate=` spy keep theirs.
#[test]
fn an_infantryman_drops_a_nav_com_he_cannot_reach() {
    let Some(dropped) = visit("E1", ACROSS, Case::Plain) else {
        return;
    };
    assert_eq!(dropped, None, "unreachable");
    assert_eq!(visit("E1", NEAR, Case::Plain).unwrap(), Some(NEAR));
    for case in [Case::Tethered, Case::OffPlayfield, Case::Entering] {
        assert_eq!(visit("E1", ACROSS, case).unwrap(), Some(ACROSS), "{case:?}");
    }
    let rules = crate::rules::retail_ini_fixture::retail_unlayered_rules_with_art().unwrap();
    assert!(rules.object("SPY").unwrap().infiltrate);
    assert_eq!(
        visit("SPY", ACROSS, Case::Plain).unwrap(),
        Some(ACROSS),
        "spy"
    );
}
