use crate::sim::combat::TargetKind;
use crate::sim::components::NavTargetRef;
use crate::sim::world::Simulation;

/// `0x0043B150` on retail rules: an enemy tank's intent shows to the local
/// player whose powered Psychic Sensor (`NAPSIS`, `PsychicDetectionRadius=15`)
/// stands within 15 cells of its Target or, with none in range, of its
/// NavCom; never to an ally, and never without a local player.
#[test]
fn a_psychic_sensor_senses_an_enemy_target_or_destination_in_range() {
    let Some(rules) = crate::rules::retail_ini_fixture::retail_unlayered_rules_with_art() else {
        return;
    };
    assert_eq!(rules.object("NAPSIS").unwrap().psychic_detection_radius, 15);
    let (mut sim, tank, americans) = super::psychic_sensor_watch_fixture(&rules);
    let set = |sim: &mut Simulation, target: Option<TargetKind>, nav: Option<NavTargetRef>| {
        let entity = sim.substrate.entities.get_mut(tank).unwrap();
        entity.attack_target.set_for_test(target);
        entity.navigation.set_nav_com_for_test(nav);
    };
    let near = Some(TargetKind::Cell(18, 18));
    let far = Some(TargetKind::Cell(30, 30));
    assert!(sim.techno_intent_psychically_detected(tank, &rules));
    sim.session.current_house = None;
    assert!(
        !sim.techno_intent_psychically_detected(tank, &rules),
        "no local player"
    );
    sim.session.current_house = Some(americans);
    set(&mut sim, far, None);
    assert!(!sim.techno_intent_psychically_detected(tank, &rules));
    set(&mut sim, far, Some(NavTargetRef::cell(19, 16)));
    assert!(
        sim.techno_intent_psychically_detected(tank, &rules),
        "a Foot's NavCom in range"
    );
    set(&mut sim, None, Some(NavTargetRef::cell(30, 30)));
    assert!(!sim.techno_intent_psychically_detected(tank, &rules));

    set(&mut sim, near, None);
    sim.house_alliances
        .entry("AMERICANS".into())
        .or_default()
        .insert("RUSSIANS".into());
    assert!(
        !sim.techno_intent_psychically_detected(tank, &rules),
        "an ally's intent"
    );
}
