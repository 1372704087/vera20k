//! Phobos banner actions 800/801/802 over the live action dispatch.

use super::*;
use crate::rules::ruleset::RuleSet;
use crate::sim::anim_class::meteor_tests::formed_sim;

/// Phobos banner actions 800/801/802 (`Ext/TAction/Body.cpp:813-875`): the
/// action's Text finds the `[BannerTypes]` entry, Param3 ids the banner,
/// Param4/Param5 position it as view percents, Param6 names the variable,
/// 801 marks that variable global; 802 deletes the first banner with the
/// action's +90 value.
#[test]
fn banner_actions_create_replace_and_delete_by_id() {
    let rules = RuleSet::from_ini_with_fixed_art_for_test(
        &crate::rules::ini_parser::IniFile::from_str(
            "[BannerTypes]\n0=INTRO\n\n[INTRO]\nCSF=Name:Title\n",
        ),
        &crate::rules::ini_parser::IniFile::empty(),
    )
    .unwrap();
    let actions = crate::map::actions::parse_actions(&crate::rules::ini_parser::IniFile::from_str(
        "[Actions]\n\
         CUE=1,800,4,INTRO,7,50,40,3\n\
         REP=1,801,4,INTRO,7,60,20,5\n\
         DEL=1,802,0,7,0,0,0,0\n",
    ));
    let resolve = |name| {
        RuntimeDefinitions::default().resolve_action(
            actions[name].entries()[0].clone(),
            None,
            None,
            None,
        )
    };
    let mut sim = formed_sim(31, 2);
    let mut effects = Vec::new();
    let ctx = TriggerActionContext {
        live_index: None,
        incoming_house: None,
        input: TagEventInput::default(),
    };

    assert!(sim.apply_live_trigger_action(
        &resolve("CUE"),
        ctx,
        &mut effects,
        &rules,
        None,
        crate::sim::world::FrameEffects::empty()
    ));
    assert_eq!(
        sim.session.banners(),
        vec![crate::sim::banner::RuntimeBanner {
            id: 7,
            type_name: "INTRO".to_owned(),
            position_percent: [50, 40],
            variable: 3,
            global_variable: false,
        }],
        "action 800 appends the banner from its params"
    );

    assert!(sim.apply_live_trigger_action(
        &resolve("REP"),
        ctx,
        &mut effects,
        &rules,
        None,
        crate::sim::world::FrameEffects::empty()
    ));
    assert_eq!(
        sim.session.banners().len(),
        1,
        "the id replaces, not appends"
    );
    let banner = &sim.session.banners()[0];
    assert_eq!(banner.position_percent, [60, 20]);
    assert_eq!(banner.variable, 5);
    assert!(
        banner.global_variable,
        "action 801 marks the variable global"
    );

    assert!(sim.apply_live_trigger_action(
        &resolve("DEL"),
        ctx,
        &mut effects,
        &rules,
        None,
        crate::sim::world::FrameEffects::empty()
    ));
    assert!(sim.session.banners().is_empty(), "802 erases by id");
}
