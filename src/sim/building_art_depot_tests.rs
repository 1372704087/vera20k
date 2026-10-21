//! UpdateAnimation's depot arm (`0x00450A86..0x00450B34`) on retail GADEPT.
use super::*;
use crate::sim::mission::{MissionId, MissionType};

/// A depot off its Repair mission that still holds SpecialAnimTwo (slot 11,
/// GADEPT_B, the lowered arm) raises it on its next Building AI:
/// SpecialAnimThree into slot 12 (GADEPT_C, or GADEPT_CD at or below
/// ConditionYellow), and slot 11 is cleared. A depot on Repair keeps it.
#[test]
fn a_depot_off_its_repair_mission_raises_its_arm() {
    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    // Asset binding gives each anim its frame count; without one the store
    // refuses the type. The damaged depot also lights its damage fires.
    let mut rules = retail.rules.clone();
    let fires: Vec<String> = (rules.general.damage_fire_types.iter())
        .map(|anim| anim.name.clone())
        .collect();
    for name in ["GADEPT_B", "GADEPT_C", "GADEPT_CD"]
        .into_iter()
        .chain(fires.iter().map(String::as_str))
    {
        rules.bind_anim_frame_count_for_test(name, 20);
    }
    let rules = &rules;
    let mut sim = crate::sim::arena_fixture::rival_arena(rules, 0x450A86);
    sim.intern_rule_type_ids(rules);
    sim.resolve_type_handles(rules);
    let mut lowered = |x| {
        let id = sim
            .spawn_object("GADEPT", "Americans", x, 4, 0, rules)
            .expect("depot");
        sim.clear_building_anim_slot(id, 12);
        assert!(
            sim.set_building_anim_slot(id, 11, false, false, 0, rules)
                .is_some()
        );
        id
    };
    let healthy = lowered(2);
    let damaged = lowered(8);
    let servicing = lowered(14);
    let strength = rules.object("GADEPT").unwrap().strength;
    sim.substrate
        .entities
        .get_mut(damaged)
        .unwrap()
        .health
        .set(strength / 4);
    let now = sim.session.binary_frame();
    let _ = sim.substrate.mission_assign_exact(
        servicing,
        MissionId::from_known(MissionType::Repair),
        now,
    );
    let slots = |sim: &Simulation, id: u64| {
        let anim = |slot: usize| {
            sim.entities().get(id).unwrap().building_art.anim_slots()[slot].map(|anim| {
                sim.interner()
                    .resolve(sim.substrate.anims.get(anim).unwrap().type_id())
                    .to_owned()
            })
        };
        (anim(11), anim(12))
    };
    let lowered_arm = (Some("GADEPT_B".to_owned()), None);
    // The Repair mission's own exits also restore the arm, so its row asks
    // the UpdateAnimation owner directly.
    sim.update_depot_raise_anim(servicing, rules);
    assert_eq!(slots(&sim, servicing), lowered_arm);

    sim.advance_tick(&[], Some(rules), None, None, 67);
    assert_eq!(slots(&sim, healthy), (None, Some("GADEPT_C".to_owned())));
    assert_eq!(slots(&sim, damaged), (None, Some("GADEPT_CD".to_owned())));
}
