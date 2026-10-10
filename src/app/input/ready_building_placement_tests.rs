//! Retail Battle rules -> paid House factories -> retained sidebar -> gadget
//! press -> SelectClass decision -> the shared pending-placement input owner.
//!
//! Clear terrain and the initial healthy base are supplied fixture inputs. The
//! ordinary frame spine pays and completes the builds; this test does not pin
//! native production cadence, whole-map loading, a window/GPU, or physical audio.
//! The pending guard is HouseClass::Manual_Place4FB852..4FB859: any pending
//! building rejects a new entry, including another type's completed building.

use super::MatchInputState;
use crate::app::input::gadget_input::ID_CAMEO_BASE;
use crate::app::input::hotkeys::HotkeyBindings;
use crate::app::input::sidebar_eva::{self, CameoClick, CameoOrder, CameoPress};
use crate::app::sidebar_projection::{SidebarProjectionState, cameo_candidates};
use crate::app::types::TargetingMode;
use crate::rules::overlay_types::OverlayTypeRegistry;
use crate::rules::ruleset::RuleSet;
use crate::sim::command::{Command, CommandEnvelope};
use crate::sim::house_state::HouseState;
use crate::sim::intern::InternedId;
use crate::sim::production::{self, BuildQueueState, ProductionCategory};
use crate::sim::world::{LifecycleOutput, SimFrameOutput, SimSoundEvent, Simulation, TickLane};
use crate::ui::gadget::focus::FocusState;
use crate::ui::gadget::list::{GadgetList, GadgetSpec};
use crate::ui::gadget::tick::{GadgetInput, TickOutput, tick};
use crate::ui::gadget::{
    GadgetRect, KEY_LMB_DOWN, KEY_LMB_UP, KEY_RMB_DOWN, KEY_RMB_UP, ListId, RESULT_BUTTON,
    RESULT_RIGHT,
};
use crate::ui::sidebar::gadget_flash::SidebarGadgetState;
use crate::ui::sidebar::{
    ArmedSidebarEntry, SidebarAction, SidebarChromeLayoutSpec, SidebarTab,
    build_sidebar_view_with_spec, hit_test_item,
};

struct Fixture {
    sim: Simulation,
    rules: RuleSet,
    overlays: OverlayTypeRegistry,
    owner: InternedId,
    input: MatchInputState,
    projection: SidebarProjectionState,
}

fn paid_ready_fixture() -> Option<Fixture> {
    let retail = crate::rules::retail_ini_fixture::retail_battle_rules_for_map("XMP03T4.MAP")?;
    let overlays = OverlayTypeRegistry::from_ini(&retail.processed_rules, Some(&retail.fixed_art));
    let rules = retail.rules;
    let mut sim = Simulation::with_seed(0x4FB8_4000);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    let owner = sim.intern("Americans");
    let mut house = HouseState::new(owner, 0, Some(owner), true, 50_000, 10);
    house.project_country_mults(&rules, &sim.interner);
    sim.houses.insert(owner, house);
    sim.session.house_order.push(owner);
    sim.session.current_house = Some(owner);
    crate::sim::arena_fixture::flat_ground(&mut sim, &rules);
    for (name, rx) in [("GACNST", 4), ("GAPOWR", 11), ("GAPILE", 18)] {
        sim.spawn_object(name, "Americans", rx, 4, 0, &rules)
            .unwrap_or_else(|| panic!("healthy retail placement prerequisite {name}"));
    }

    let starting_credits = production::credits_for_owner(&sim, "Americans");
    let total_cost: i32 = ["GAPOWR", "GAPILL"]
        .iter()
        .map(|name| sim.cost_of(owner, rules.object(name).unwrap(), &rules))
        .sum();
    assert!(production::enqueue_by_type(
        &mut sim,
        &rules,
        "Americans",
        "GAPOWR"
    ));
    assert!(production::enqueue_by_type(
        &mut sim,
        &rules,
        "Americans",
        "GAPILL"
    ));
    // A fixture timeout, not an expected native completion frame. Both strips
    // run their real timers and charge the same House economy during these frames.
    for _ in 0..4096 {
        sim.advance_app_frame(
            &[],
            Some(&rules),
            Some(&overlays),
            67,
            TickLane::Ordinary,
            None,
        )
        .expect("ordinary paid production frame");
        if production::ready_buildings_for_owner(&sim, &rules, "Americans").len() == 2 {
            break;
        }
    }
    let queue = production::queue_view_for_owner(&sim, &rules, "Americans");
    assert_eq!(queue.len(), 2, "one real object in each structure strip");
    assert!(queue.iter().all(|item| item.state == BuildQueueState::Done));
    assert_eq!(
        production::credits_for_owner(&sim, "Americans"),
        starting_credits - total_cost,
        "the completed objects must have been paid through the House economy"
    );
    assert_eq!(sim.houses[&owner].economy.spent_credits(), total_cost);

    Some(Fixture {
        sim,
        rules,
        overlays,
        owner,
        input: MatchInputState::new(HotkeyBindings::default()),
        projection: SidebarProjectionState::default(),
    })
}

fn press_cameo(
    fixture: &mut Fixture,
    name: &str,
    tab: SidebarTab,
    press: CameoPress,
) -> CameoClick {
    let queue = production::queue_view_for_owner(&fixture.sim, &fixture.rules, "Americans");
    let ready = production::ready_buildings_for_owner(&fixture.sim, &fixture.rules, "Americans");
    let options =
        production::all_build_options_for_owner(&fixture.sim, &fixture.rules, "Americans");
    let candidates = cameo_candidates(
        &fixture.sim,
        &fixture.rules,
        None,
        fixture.owner,
        &options,
        &ready,
        &[],
    );
    let layout_spec = SidebarChromeLayoutSpec::stock();
    let visible_rows = crate::ui::sidebar::compute_layout_with_spec(layout_spec, 800.0, 1200.0, 0)
        .side2_tile_count;
    fixture
        .projection
        .reconcile_cameos(fixture.owner, &candidates, visible_rows);
    fixture
        .projection
        .reconcile_factory_links(&fixture.sim, fixture.owner);
    let armed = fixture
        .input
        .targeting_mode
        .as_ref()
        .and_then(TargetingMode::as_building_placement)
        .map(|name| ArmedSidebarEntry::BuildingPlacement(name.to_owned()));
    let (power_produced, power_drained) =
        production::power_balance_for_owner(&fixture.sim, &fixture.rules, "Americans");
    let factory_statuses = fixture
        .projection
        .factory_statuses(&fixture.sim, fixture.owner);
    let view = build_sidebar_view_with_spec(
        layout_spec,
        800.0,
        1200.0,
        tab,
        production::credits_for_owner(&fixture.sim, "Americans"),
        power_produced,
        power_drained,
        None,
        &queue,
        &factory_statuses,
        &options,
        &ready,
        armed.as_ref(),
        &[],
        Some(&fixture.sim.interner),
        &[],
        &SidebarGadgetState::new(),
        None,
        None,
        None,
        None,
        [None; 2],
        fixture.projection.cameo_strips(),
    );
    let slot = view
        .items
        .iter()
        .position(|item| item.type_id == name)
        .unwrap_or_else(|| panic!("retail {name} on retained {tab:?} strip"));
    let item = &view.items[slot];
    if name == "GAPOWR"
        && fixture
            .sim
            .production
            .factories
            .view(fixture.owner, ProductionCategory::Building)
            .and_then(|factory| factory.complete_object())
            .is_some_and(|object| fixture.sim.interner.resolve(object.type_id) == "GAPILE")
    {
        assert!(
            item.is_ready,
            "StripDraw6A9822 reads the same rearmed Factory as input"
        );
        assert!(!item.is_on_hold);
        assert_eq!(
            item.queued_count, 0,
            "the separate House/type badge has no old Power product"
        );
    }
    if ready
        .iter()
        .any(|ready| fixture.sim.resolve(ready.type_id) == name)
    {
        assert!(item.is_ready, "the actual completed factory draws Ready");
    }
    let mut list = GadgetList::new(ListId(1));
    let handles: Vec<_> = view
        .items
        .iter()
        .enumerate()
        .map(|(slot, item)| {
            let rect = item.rect;
            list.add_tail(GadgetSpec::cameo(
                GadgetRect::new(
                    rect.x.round() as i32,
                    rect.y.round() as i32,
                    rect.w.round() as i32,
                    rect.h.round() as i32,
                ),
                ID_CAMEO_BASE + slot as u16,
            ))
        })
        .collect();
    let x = (item.rect.x + item.rect.w / 2.0).round() as i32;
    let y = (item.rect.y + item.rect.h / 2.0).round() as i32;
    let mut focus = FocusState::new();
    let mut output = TickOutput::default();
    let (is_right, shift) = match press {
        CameoPress::Left => (false, false),
        CameoPress::Right { shift } => (true, shift),
    };
    let result = tick(
        &mut list,
        &mut focus,
        &GadgetInput {
            queued_key: if is_right { KEY_RMB_DOWN } else { KEY_LMB_DOWN },
            event_x: x,
            event_y: y,
            mouse_x: x,
            mouse_y: y,
            left_held: !is_right,
            right_held: is_right,
            shift,
            ..Default::default()
        },
        &mut output,
    );
    let right_flag = if is_right { RESULT_RIGHT } else { 0 };
    assert_eq!(
        result,
        RESULT_BUTTON | right_flag | (ID_CAMEO_BASE + slot as u16)
    );
    assert_eq!(output.consumed_by, Some(handles[slot]));
    let fired_slot = ((result & !(RESULT_BUTTON | RESULT_RIGHT)) - ID_CAMEO_BASE) as usize;
    let action = hit_test_item(&view.items[fired_slot], (result & RESULT_RIGHT) != 0, shift);
    let SidebarAction::CameoPress {
        type_id,
        right,
        shift,
        at_build_limit,
    } = action
    else {
        panic!("build gadget must reach the ordinary cameo decision");
    };
    assert_eq!(type_id, name);
    let dispatched_press = if right {
        CameoPress::Right { shift }
    } else {
        CameoPress::Left
    };
    assert_eq!(dispatched_press, press);
    let category = production::category_for_object(fixture.rules.object(&type_id).unwrap());
    let click = sidebar_eva::cameo_click(
        &queue,
        category,
        fixture.sim.interner.get(&type_id),
        fixture
            .projection
            .cameo_factory(
                &fixture.sim,
                fixture.owner,
                fixture.sim.interner.get(&type_id).unwrap(),
            )
            .map(|factory| factory.cameo_state()),
        dispatched_press,
        at_build_limit,
    );
    if click.clear_building_placement {
        fixture.input.cancel_building_placement();
    }
    // Cameos fire on DOWN; the real UP edge must not dispatch another entry.
    let release = tick(
        &mut list,
        &mut focus,
        &GadgetInput {
            queued_key: if is_right { KEY_RMB_UP } else { KEY_LMB_UP },
            event_x: x,
            event_y: y,
            mouse_x: x,
            mouse_y: y,
            shift,
            ..Default::default()
        },
        &mut output,
    );
    assert_eq!(release & RESULT_BUTTON, 0);
    click
}

fn press_ready_cameo(fixture: &mut Fixture, name: &str, tab: SidebarTab) -> bool {
    let click = press_cameo(fixture, name, tab, CameoPress::Left);
    assert!(
        click.sound,
        "ready presses retain the ordinary GUI sound request"
    );
    assert_eq!(click.eva, None);
    assert_eq!(click.order, Some(CameoOrder::Place));
    assert!(!click.clear_building_placement);
    let type_id = fixture
        .projection
        .cameo_factory(
            &fixture.sim,
            fixture.owner,
            fixture.sim.interner.get(name).unwrap(),
        )
        .unwrap()
        .complete_object()
        .unwrap()
        .type_id;
    let product = production::ready_building_entity_for_owner(
        &fixture.sim,
        &fixture.rules,
        fixture.owner,
        type_id,
    )
    .expect("completed product comes from the authoritative House factory");
    production::object_find_factory(&fixture.sim, &fixture.rules, product, false, false)
        .expect("the actual held object finds its healthy retail builder");
    fixture.input.begin_building_placement(
        fixture.sim.interner.resolve(type_id),
        product,
        fixture.owner,
    )
}

fn arm_gapowr_with_preview(fixture: &mut Fixture) -> production::BuildingPlacementPreview {
    assert!(press_ready_cameo(fixture, "GAPOWR", SidebarTab::Building));
    let preview = production::placement_preview_for_owner_with_overlays(
        &fixture.sim,
        &fixture.rules,
        "Americans",
        "GAPOWR",
        8,
        4,
        Some(&fixture.overlays),
    )
    .expect("armed ready factory supplies its real placement preview");
    assert!(preview.valid, "clear adjacent healthy-yard placement site");
    fixture.input.building_placement_preview = Some(preview.clone());
    preview
}

fn ready_identities(fixture: &Fixture) -> [(InternedId, u64); 2] {
    ["GAPOWR", "GAPILL"].map(|name| {
        let type_id = fixture.sim.interner.get(name).unwrap();
        let product = production::ready_building_entity_for_owner(
            &fixture.sim,
            &fixture.rules,
            fixture.owner,
            type_id,
        )
        .expect("the paid factory must still hold its completed object");
        (type_id, product)
    })
}

fn ready_product(fixture: &Fixture, name: &str) -> u64 {
    production::ready_building_entity_for_owner(
        &fixture.sim,
        &fixture.rules,
        fixture.owner,
        fixture.sim.interner.get(name).unwrap(),
    )
    .expect("completed product identity remains owned by its House factory")
}

fn submit_ready_building(
    fixture: &mut Fixture,
    name: &str,
    tab: SidebarTab,
    cell: (u16, u16),
) -> CommandEnvelope {
    assert!(press_ready_cameo(fixture, name, tab));
    submit_current_building(fixture, cell)
}

fn submit_current_building(fixture: &mut Fixture, cell: (u16, u16)) -> CommandEnvelope {
    let name = fixture
        .input
        .targeting_mode
        .as_ref()
        .and_then(TargetingMode::as_building_placement)
        .expect("the current actual product type supplies PLACE")
        .to_string();
    let preview = production::placement_preview_for_owner_with_overlays(
        &fixture.sim,
        &fixture.rules,
        "Americans",
        &name,
        cell.0,
        cell.1,
        Some(&fixture.overlays),
    )
    .expect("ready object's real preview");
    assert!(preview.valid, "site is admitted before any later blocker");
    fixture.input.building_placement_preview = Some(preview);
    assert_eq!(
        fixture.input.building_placement_issuer(),
        Some(fixture.owner)
    );
    let command = CommandEnvelope::new(
        fixture.input.building_placement_issuer().unwrap(),
        fixture.sim.session.tick + 1,
        Command::PlaceReadyBuilding {
            type_id: fixture.sim.interner.get(&name).unwrap(),
            rx: cell.0,
            ry: cell.1,
        },
    );
    fixture.input.submit_building_placement();
    assert!(fixture.input.targeting_mode.is_none());
    assert!(fixture.input.building_placement_preview.is_none());
    assert!(fixture.input.building_placement_issuer().is_none());
    command
}

fn cancel_envelope(fixture: &Fixture, name: &str) -> CommandEnvelope {
    CommandEnvelope::new(
        fixture.owner,
        fixture.sim.session.tick + 1,
        Command::CancelProductionByType {
            type_id: fixture.sim.interner.get(name).unwrap(),
            all: false,
        },
    )
}

fn advance_commands(fixture: &mut Fixture, commands: &[CommandEnvelope]) -> SimFrameOutput {
    fixture.sim.queue_commands(commands.iter().cloned());
    let due = fixture.sim.take_due_commands();
    fixture
        .sim
        .advance_app_frame(
            &due,
            Some(&fixture.rules),
            Some(&fixture.overlays),
            67,
            TickLane::Ordinary,
            None,
        )
        .expect("ordinary authoritative command frame")
}

fn placement_facts(frame: &SimFrameOutput) -> Vec<LifecycleOutput> {
    frame
        .lifecycle_outputs
        .iter()
        .copied()
        .filter(|output| {
            matches!(
                output,
                LifecycleOutput::BuildingPlacementRetry { .. }
                    | LifecycleOutput::BuildingPlacementSucceeded { .. }
                    | LifecycleOutput::BuildingProductionAbandoned { .. }
            )
        })
        .collect()
}

fn consume_frame_outputs(
    fixture: &mut Fixture,
    frame: &SimFrameOutput,
    local_house: Option<InternedId>,
) -> usize {
    let mut retries = 0;
    for output in &frame.lifecycle_outputs {
        if let LifecycleOutput::ProductionFactoryLinked {
            owner,
            type_id,
            category,
            construction_order,
        } = *output
            && local_house == Some(owner)
        {
            fixture
                .projection
                .link_factory(owner, type_id, category, construction_order);
        }
        retries += usize::from(
            fixture
                .input
                .apply_building_placement_output(output, local_house),
        );
    }
    retries
}

fn install_late_stationary_blocker(fixture: &mut Fixture, cell: (u16, u16)) -> u64 {
    // Original43B740/440580 can reveal this independent stationary object
    // between admitted input and Event execution. Supply that lifecycle
    // boundary through the shared constructor/Unlimbo owner, not a whole
    // trigger cadence or a fabricated failing PLACE return.
    fixture
        .sim
        .spawn_object("GAPILL", "Americans", cell.0, cell.1, 0, &fixture.rules)
        .expect("independent stationary GAPILL blocks the admitted site")
}

fn native_after(history_id: &str, command: &str, occurrence: usize) -> &'static serde_json::Value {
    static CORPUS: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
    let corpus = CORPUS.get_or_init(|| {
        serde_json::from_str(crate::test_fixture::text(
            "tools/sidebar_oracle/cameo_order.json",
        ))
        .expect("executed ready-building corpus")
    });
    let history = corpus["ready_building_histories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"].as_str() == Some(history_id))
        .unwrap_or_else(|| panic!("native ready-building history {history_id}"));
    &history["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|step| step["command"].as_str() == Some(command))
        .nth(occurrence)
        .unwrap_or_else(|| panic!("native {history_id} step {command}"))["after"]
}

fn native_placement_after(
    history_id: &str,
    command: &str,
    occurrence: usize,
) -> &'static serde_json::Value {
    &native_after(history_id, command, occurrence)["placement"]
}

// The executable corpus pins these control-flow fields. Fixture coordinates,
// numeric stable IDs, and camera-derived preview pixels are independent
// inputs rather than assertions of whole native scene parity.
fn assert_current_matches_native(fixture: &Fixture, expected: &serde_json::Value) {
    assert_eq!(
        fixture
            .input
            .targeting_mode
            .as_ref()
            .and_then(TargetingMode::as_building_placement),
        expected["current_type"].as_str(),
    );
    let product_present = match fixture.input.targeting_mode.as_ref() {
        Some(TargetingMode::BuildingPlacement(target)) => target.product_id.is_some(),
        _ => false,
    };
    assert_eq!(product_present, !expected["current_product"].is_null());
    assert_eq!(
        fixture
            .input
            .targeting_mode
            .as_ref()
            .and_then(TargetingMode::as_building_placement)
            .is_some(),
        expected["active_foundation"].as_str() != Some("0x0"),
    );
}

fn assert_saved_matches_native(fixture: &Fixture, expected: &serde_json::Value) {
    assert_eq!(
        fixture
            .input
            .saved_building_placement
            .as_ref()
            .map(|saved| saved.target.type_id()),
        expected["saved_type"].as_str(),
    );
    assert_eq!(
        fixture
            .input
            .saved_building_placement
            .as_ref()
            .is_some_and(|saved| saved.target.product_id.is_some()),
        !expected["saved_product"].is_null(),
    );
    assert_eq!(
        fixture.input.saved_building_placement.is_some(),
        expected["saved_foundation"].as_str() != Some("0x0"),
    );
}

fn assert_same_preview(
    after: &production::BuildingPlacementPreview,
    before: &production::BuildingPlacementPreview,
) {
    assert_eq!(
        (
            after.rx,
            after.ry,
            after.width,
            after.height,
            after.valid,
            &after.reason,
            &after.cell_valid,
            &after.wall_autofill_cells,
        ),
        (
            before.rx,
            before.ry,
            before.width,
            before.height,
            before.valid,
            &before.reason,
            &before.cell_valid,
            &before.wall_autofill_cells,
        ),
        "every placement preview field is retained"
    );
}

fn assert_pending_repeat(name: &str, tab: SidebarTab) {
    let Some(mut fixture) = paid_ready_fixture() else {
        return;
    };
    let queue_before: Vec<_> =
        production::queue_view_for_owner(&fixture.sim, &fixture.rules, "Americans")
            .into_iter()
            .map(|item| (item.type_id, item.queue_category, item.state, item.progress))
            .collect();
    let held_before: Vec<_> = [ProductionCategory::Building, ProductionCategory::Defense]
        .map(|category| {
            let factory = fixture
                .sim
                .production
                .factories
                .view(fixture.owner, category)
                .unwrap();
            let object = factory.complete_object().unwrap();
            (object.type_id, object.entity_id.unwrap())
        })
        .into_iter()
        .collect();
    let credits_before = production::credits_for_owner(&fixture.sim, "Americans");
    let spent_before = fixture.sim.houses[&fixture.owner].economy.spent_credits();
    let hash_before = fixture.sim.state_hash();
    let rng_before = (
        fixture.sim.main_rng.logical_state(),
        fixture.sim.scenario_rng.logical_state(),
        fixture.sim.mapgen_rng.logical_state(),
    );
    let preview = arm_gapowr_with_preview(&mut fixture);
    let pending_before = fixture.input.targeting_mode.clone();
    let entered = press_ready_cameo(&mut fixture, name, tab);

    let queue_after: Vec<_> =
        production::queue_view_for_owner(&fixture.sim, &fixture.rules, "Americans")
            .into_iter()
            .map(|item| (item.type_id, item.queue_category, item.state, item.progress))
            .collect();
    assert_eq!(
        queue_after, queue_before,
        "ready clicks cannot edit either factory queue"
    );
    for (type_id, stable_id) in held_before {
        let category = production::category_for_object(
            fixture.rules.object(fixture.sim.resolve(type_id)).unwrap(),
        );
        let object = fixture
            .sim
            .production
            .factories
            .view(fixture.owner, category)
            .unwrap()
            .complete_object()
            .unwrap();
        assert_eq!(object.type_id, type_id);
        assert_eq!(object.entity_id, Some(stable_id));
        assert!(
            fixture
                .sim
                .entities()
                .get(stable_id)
                .unwrap()
                .lifecycle
                .in_limbo
        );
    }
    assert_eq!(
        production::credits_for_owner(&fixture.sim, "Americans"),
        credits_before
    );
    assert_eq!(
        fixture.sim.houses[&fixture.owner].economy.spent_credits(),
        spent_before
    );
    assert_eq!(fixture.sim.state_hash(), hash_before);
    assert_eq!(
        (
            fixture.sim.main_rng.logical_state(),
            fixture.sim.scenario_rng.logical_state(),
            fixture.sim.mapgen_rng.logical_state(),
        ),
        rng_before,
        "input admission has no simulated RNG side effects"
    );
    assert!(
        !entered,
        "Manual_Place refuses every entry while another placement is pending"
    );
    assert_eq!(
        fixture
            .input
            .targeting_mode
            .as_ref()
            .and_then(TargetingMode::as_building_placement),
        Some("GAPOWR"),
        "the first completed building remains pending"
    );
    assert_eq!(
        fixture.input.targeting_mode, pending_before,
        "a refused entry also retains the actual product and House issuer"
    );
    let after = fixture
        .input
        .building_placement_preview
        .as_ref()
        .expect("pending preview is retained");
    assert_same_preview(after, &preview);
}

#[test]
fn same_ready_cameo_retains_pending_building_and_preview() {
    assert_pending_repeat("GAPOWR", SidebarTab::Building);
}

#[test]
fn other_ready_cameo_retains_pending_building_and_preview() {
    assert_pending_repeat("GAPILL", SidebarTab::Defense);
}

#[test]
fn own_ready_cameo_right_clears_other_category_pending_before_abandon() {
    for shift in [false, true] {
        let Some(mut fixture) = paid_ready_fixture() else {
            return;
        };
        arm_gapowr_with_preview(&mut fixture);
        let identities_before = ready_identities(&fixture);
        let credits_before = production::credits_for_owner(&fixture.sim, "Americans");
        let spent_before = fixture.sim.houses[&fixture.owner].economy.spent_credits();
        let hash_before = fixture.sim.state_hash();
        let click = press_cameo(
            &mut fixture,
            "GAPILL",
            SidebarTab::Defense,
            CameoPress::Right { shift },
        );

        assert!(click.clear_building_placement);
        assert!(click.sound);
        assert_eq!(click.eva, Some(sidebar_eva::EVA_CANCELED));
        assert_eq!(click.order, Some(CameoOrder::Abandon { all: shift }));
        assert!(fixture.input.targeting_mode.is_none());
        assert!(fixture.input.building_placement_preview.is_none());
        assert_eq!(ready_identities(&fixture), identities_before);
        assert_eq!(
            production::credits_for_owner(&fixture.sim, "Americans"),
            credits_before
        );
        assert_eq!(
            fixture.sim.houses[&fixture.owner].economy.spent_credits(),
            spent_before
        );
        assert_eq!(
            fixture.sim.state_hash(),
            hash_before,
            "the local prefix clears input before the later Abandon receiver"
        );
        let queue = production::queue_view_for_owner(&fixture.sim, &fixture.rules, "Americans");
        assert_eq!(queue.len(), 2);
        assert!(queue.iter().all(|item| item.state == BuildQueueState::Done));
    }
}

#[test]
fn queued_only_cameo_right_retains_pending_and_preview_before_abandon() {
    for shift in [false, true] {
        let Some(mut fixture) = paid_ready_fixture() else {
            return;
        };
        // Infantry genuinely queues in native. Keep the existing completed
        // building strips separate from this real barracks queue; no building
        // queue fixture relies on VERA's recorded BeginProduction residual.
        let options =
            production::all_build_options_for_owner(&fixture.sim, &fixture.rules, "Americans");
        for name in ["E1", "ENGINEER"] {
            let type_id = fixture.sim.interner.get(name).unwrap();
            assert!(
                options
                    .iter()
                    .any(|option| option.type_id == type_id && option.visible_in_sidebar()),
                "the retail barracks fixture must expose {name}"
            );
            assert!(production::enqueue_by_type(
                &mut fixture.sim,
                &fixture.rules,
                "Americans",
                name,
            ));
        }
        let engineer = fixture.sim.interner.get("ENGINEER").unwrap();
        let queue = production::queue_view_for_owner(&fixture.sim, &fixture.rules, "Americans");
        assert!(queue.iter().any(|item| {
            item.type_id == engineer
                && item.queue_category == ProductionCategory::Infantry
                && item.state == BuildQueueState::Queued
        }));
        let preview = arm_gapowr_with_preview(&mut fixture);
        let pending_before = fixture.input.targeting_mode.clone();
        let identities_before = ready_identities(&fixture);
        let hash_before = fixture.sim.state_hash();
        let credits_before = production::credits_for_owner(&fixture.sim, "Americans");
        let click = press_cameo(
            &mut fixture,
            "ENGINEER",
            SidebarTab::Infantry,
            CameoPress::Right { shift },
        );

        assert!(!click.clear_building_placement);
        assert!(click.sound);
        assert_eq!(click.eva, None);
        assert_eq!(click.order, Some(CameoOrder::Abandon { all: shift }));
        assert_eq!(
            fixture
                .input
                .targeting_mode
                .as_ref()
                .and_then(TargetingMode::as_building_placement),
            Some("GAPOWR")
        );
        assert_eq!(fixture.input.targeting_mode, pending_before);
        assert_same_preview(
            fixture.input.building_placement_preview.as_ref().unwrap(),
            &preview,
        );
        assert_eq!(ready_identities(&fixture), identities_before);
        assert_eq!(
            production::credits_for_owner(&fixture.sim, "Americans"),
            credits_before
        );
        assert_eq!(fixture.sim.state_hash(), hash_before);
        let after = production::queue_view_for_owner(&fixture.sim, &fixture.rules, "Americans");
        assert_eq!(
            after
                .iter()
                .map(|item| (item.type_id, item.queue_category, item.state, item.progress))
                .collect::<Vec<_>>(),
            queue
                .iter()
                .map(|item| (item.type_id, item.queue_category, item.state, item.progress))
                .collect::<Vec<_>>(),
            "the gadget prefix does not execute the requested queued-copy Abandon"
        );
    }
}

/// Executed HousePlace4FB35F/4FB395 control: an independent stationary
/// GAPILL appears after admission. The failure restores the saved product
/// and House issuer, then consumes that one backup.
#[test]
fn late_stationary_blocker_restores_saved_product_and_requests_preview_refresh() {
    let Some(mut fixture) = paid_ready_fixture() else {
        return;
    };
    let product = ready_product(&fixture, "GAPOWR");
    let command = submit_ready_building(&mut fixture, "GAPOWR", SidebarTab::Building, (8, 4));
    assert!(fixture.input.saved_building_placement.is_some());
    let blocker = install_late_stationary_blocker(&mut fixture, (8, 4));
    let credits = production::credits_for_owner(&fixture.sim, "Americans");
    let frame = advance_commands(&mut fixture, &[command]);

    assert_eq!(frame.tick.executed_commands, 1);
    assert!(!frame.tick.spawned_entities);
    assert_eq!(
        placement_facts(&frame),
        vec![LifecycleOutput::BuildingPlacementRetry {
            owner: fixture.owner
        }]
    );
    assert_eq!(
        frame
            .sound_events
            .iter()
            .filter(|event| matches!(event, SimSoundEvent::CannotDeployHere { owner } if *owner == fixture.owner))
            .count(),
        1
    );
    let owner = fixture.owner;
    assert_eq!(consume_frame_outputs(&mut fixture, &frame, Some(owner)), 1);
    let Some(TargetingMode::BuildingPlacement(target)) = fixture.input.targeting_mode.as_ref()
    else {
        panic!("failed Unlimbo restores the exact saved placement");
    };
    assert_eq!(target.type_id(), "GAPOWR");
    assert_eq!(target.product_id, Some(product));
    assert_eq!(target.owner, fixture.owner);
    assert_eq!(
        fixture.input.building_placement_issuer(),
        Some(fixture.owner)
    );
    assert!(fixture.input.saved_building_placement.is_none());
    assert_eq!(ready_product(&fixture, "GAPOWR"), product);
    let native = native_placement_after(
        "admitted_then_blocked_retry_then_abandon",
        "local_event_corridor",
        0,
    );
    assert_current_matches_native(&fixture, native);
    assert_saved_matches_native(&fixture, native);
    assert!(
        fixture
            .sim
            .entities()
            .get(product)
            .unwrap()
            .lifecycle
            .in_limbo
    );
    assert!(
        !fixture
            .sim
            .entities()
            .get(blocker)
            .unwrap()
            .lifecycle
            .in_limbo
    );
    assert_eq!(
        production::credits_for_owner(&fixture.sim, "Americans"),
        credits
    );
    // The consumer requests a fresh actual-cursor preview. This headless
    // fixture does not equate its supplied (8,4) preview with native tactical
    // cursor projection or execute the AppState camera/GPU wrapper.
}

/// Executed HousePlace4FB328..4FB33F and4FB395 controls in both insertion
/// orders: the first success clears the saved second click; the later actual
/// Unlimbo failure therefore restores NULL while retaining that held object.
#[test]
fn same_frame_success_then_failure_clears_backup_in_both_building_orders() {
    for orders in [
        [
            ("GAPOWR", SidebarTab::Building),
            ("GAPILL", SidebarTab::Defense),
        ],
        [
            ("GAPILL", SidebarTab::Defense),
            ("GAPOWR", SidebarTab::Building),
        ],
    ] {
        let Some(mut fixture) = paid_ready_fixture() else {
            return;
        };
        let first_id = ready_product(&fixture, orders[0].0);
        let second_id = ready_product(&fixture, orders[1].0);
        let first = submit_ready_building(&mut fixture, orders[0].0, orders[0].1, (8, 4));
        let second = submit_ready_building(&mut fixture, orders[1].0, orders[1].1, (8, 4));
        let credits = production::credits_for_owner(&fixture.sim, "Americans");
        let frame = advance_commands(&mut fixture, &[first, second]);

        assert_eq!(frame.tick.executed_commands, 2);
        assert!(frame.tick.spawned_entities);
        assert_eq!(
            placement_facts(&frame),
            vec![
                LifecycleOutput::BuildingPlacementSucceeded {
                    owner: fixture.owner
                },
                LifecycleOutput::BuildingPlacementRetry {
                    owner: fixture.owner
                },
            ]
        );
        let owner = fixture.owner;
        assert_eq!(consume_frame_outputs(&mut fixture, &frame, Some(owner)), 0);
        assert!(fixture.input.targeting_mode.is_none());
        assert!(fixture.input.building_placement_preview.is_none());
        assert!(fixture.input.saved_building_placement.is_none());
        let history = if orders[0].0 == "GAPILL" {
            "queued_same_cell_GAPILL_then_GAPOWR"
        } else {
            "queued_same_cell_GAPOWR_then_GAPILL"
        };
        let native = native_placement_after(history, "local_event_corridor", 0);
        assert_current_matches_native(&fixture, native);
        assert_saved_matches_native(&fixture, native);
        let placed = fixture.sim.entities().get(first_id).unwrap();
        assert!(!placed.lifecycle.in_limbo);
        assert_eq!((placed.position.rx, placed.position.ry), (8, 4));
        assert_eq!(ready_product(&fixture, orders[1].0), second_id);
        assert!(
            fixture
                .sim
                .entities()
                .get(second_id)
                .unwrap()
                .lifecycle
                .in_limbo
        );
        let ready =
            production::ready_buildings_for_owner(&fixture.sim, &fixture.rules, "Americans");
        assert_eq!(ready.len(), 1);
        assert_eq!(fixture.sim.resolve(ready[0].type_id), orders[1].0);
        assert_eq!(
            production::credits_for_owner(&fixture.sim, "Americans"),
            credits
        );
    }
}

#[test]
fn retry_then_other_category_abandon_clears_before_product_expiry_in_the_same_batch() {
    let Some(mut fixture) = paid_ready_fixture() else {
        return;
    };
    let abandoned_product = ready_product(&fixture, "GAPILL");
    let retained_product = ready_product(&fixture, "GAPOWR");
    let place = submit_ready_building(&mut fixture, "GAPOWR", SidebarTab::Building, (8, 4));
    install_late_stationary_blocker(&mut fixture, (8, 4));
    let abandon = cancel_envelope(&fixture, "GAPILL");
    let frame = advance_commands(&mut fixture, &[place, abandon]);

    assert_eq!(frame.tick.executed_commands, 2);
    assert!(!frame.tick.spawned_entities);
    assert_eq!(
        placement_facts(&frame),
        vec![
            LifecycleOutput::BuildingPlacementRetry {
                owner: fixture.owner
            },
            LifecycleOutput::BuildingProductionAbandoned {
                owner: fixture.owner
            },
        ]
    );
    let clear = frame
        .lifecycle_outputs
        .iter()
        .position(|output| {
            *output
                == LifecycleOutput::BuildingProductionAbandoned {
                    owner: fixture.owner,
                }
        })
        .unwrap();
    let expiry = frame
        .lifecycle_outputs
        .iter()
        .position(|output| {
            *output
                == LifecycleOutput::ObjectPointerExpired {
                    stable_id: abandoned_product,
                }
        })
        .unwrap();
    assert!(
        clear < expiry,
        "Abandon's local clear precedes the held product destructor"
    );
    let owner = fixture.owner;
    assert_eq!(consume_frame_outputs(&mut fixture, &frame, Some(owner)), 1);
    assert!(fixture.input.targeting_mode.is_none());
    assert!(fixture.input.building_placement_preview.is_none());
    assert!(fixture.input.saved_building_placement.is_none());
    assert!(fixture.sim.entities().get(abandoned_product).is_none());
    assert_eq!(ready_product(&fixture, "GAPOWR"), retained_product);
    let native = native_placement_after("queued_failure_then_abandon", "local_event_corridor", 0);
    assert_current_matches_native(&fixture, native);
    assert_saved_matches_native(&fixture, native);
    assert_eq!(
        production::ready_buildings_for_owner(&fixture.sim, &fixture.rules, "Americans").len(),
        1
    );
}

#[test]
fn abandon_then_place_rejects_early_without_retry_or_cannot_deploy() {
    let Some(mut fixture) = paid_ready_fixture() else {
        return;
    };
    let product = ready_product(&fixture, "GAPOWR");
    let place = submit_ready_building(&mut fixture, "GAPOWR", SidebarTab::Building, (8, 4));
    let abandon = cancel_envelope(&fixture, "GAPOWR");
    let frame = advance_commands(&mut fixture, &[abandon, place]);

    assert_eq!(frame.tick.executed_commands, 2);
    assert!(!frame.tick.spawned_entities);
    assert_eq!(
        placement_facts(&frame),
        vec![LifecycleOutput::BuildingProductionAbandoned {
            owner: fixture.owner
        }]
    );
    assert!(!frame.sound_events.iter().any(|event| {
        matches!(
            event,
            SimSoundEvent::CannotDeployHere { .. } | SimSoundEvent::BuildingPlaced { .. }
        )
    }));
    let owner = fixture.owner;
    assert_eq!(consume_frame_outputs(&mut fixture, &frame, Some(owner)), 0);
    assert!(fixture.input.targeting_mode.is_none());
    assert!(fixture.input.saved_building_placement.is_none());
    assert!(fixture.sim.entities().get(product).is_none());
    assert_eq!(
        production::ready_buildings_for_owner(&fixture.sim, &fixture.rules, "Americans").len(),
        1
    );
}

#[test]
fn remote_success_and_retry_facts_do_not_consume_the_local_backup() {
    let Some(mut fixture) = paid_ready_fixture() else {
        return;
    };
    let first = submit_ready_building(&mut fixture, "GAPOWR", SidebarTab::Building, (8, 4));
    let second_product = ready_product(&fixture, "GAPILL");
    let second = submit_ready_building(&mut fixture, "GAPILL", SidebarTab::Defense, (8, 4));
    let frame = advance_commands(&mut fixture, &[first, second]);
    // Supplied nonlocal House identity for the output consumer; this does
    // not claim a second House's production or native player initialization.
    let remote = fixture.sim.interner.intern("Russians");
    for local_house in [None, Some(remote)] {
        assert_eq!(consume_frame_outputs(&mut fixture, &frame, local_house), 0);
        assert!(fixture.input.targeting_mode.is_none());
        let saved = fixture.input.saved_building_placement.as_ref().unwrap();
        assert_eq!(saved.target.type_id(), "GAPILL");
        assert_eq!(saved.target.product_id, Some(second_product));
        assert_eq!(saved.target.owner, fixture.owner);
    }
    let owner = fixture.owner;
    assert_eq!(consume_frame_outputs(&mut fixture, &frame, Some(owner)), 0);
    assert!(fixture.input.targeting_mode.is_none());
    assert!(fixture.input.saved_building_placement.is_none());
}

#[test]
fn remote_other_category_abandon_does_not_clear_the_current_building() {
    let Some(mut fixture) = paid_ready_fixture() else {
        return;
    };
    let preview = arm_gapowr_with_preview(&mut fixture);
    let pending = fixture.input.targeting_mode.clone();
    let abandon = cancel_envelope(&fixture, "GAPILL");
    let frame = advance_commands(&mut fixture, &[abandon]);
    let remote = fixture.sim.interner.intern("Russians");
    assert_eq!(consume_frame_outputs(&mut fixture, &frame, Some(remote)), 0);
    assert_eq!(fixture.input.targeting_mode, pending);
    assert_same_preview(
        fixture.input.building_placement_preview.as_ref().unwrap(),
        &preview,
    );
    let owner = fixture.owner;
    assert_eq!(consume_frame_outputs(&mut fixture, &frame, Some(owner)), 0);
    assert!(fixture.input.targeting_mode.is_none());
    assert!(fixture.input.building_placement_preview.is_none());
}

/// DispatchPointerExpired7258D0's product-only control keeps type/foundation;
/// BandBoxLeftUp4AB9B0 subsequently has no House issuer to enqueue. This test
/// reaches the shared expiry/output/getter boundaries, not the AppState click.
#[test]
fn current_product_expiry_retains_type_and_preview_but_has_no_issuer() {
    let Some(mut fixture) = paid_ready_fixture() else {
        return;
    };
    let preview = arm_gapowr_with_preview(&mut fixture);
    let product = ready_product(&fixture, "GAPOWR");
    let defense_product = ready_product(&fixture, "GAPILL");
    fixture
        .sim
        .detach_all_pointer_expired(product, &fixture.rules, Some(&fixture.overlays));
    let frame = advance_commands(&mut fixture, &[]);
    assert!(
        frame
            .lifecycle_outputs
            .contains(&LifecycleOutput::ObjectPointerExpired { stable_id: product })
    );
    let owner = fixture.owner;
    assert_eq!(consume_frame_outputs(&mut fixture, &frame, Some(owner)), 0);
    assert_eq!(
        fixture
            .input
            .targeting_mode
            .as_ref()
            .and_then(TargetingMode::as_building_placement),
        Some("GAPOWR")
    );
    assert!(fixture.input.building_placement_issuer().is_none());
    assert_same_preview(
        fixture.input.building_placement_preview.as_ref().unwrap(),
        &preview,
    );
    let native = native_placement_after("current_product_expiry", "dispatch_pointer_expired", 0);
    assert_current_matches_native(&fixture, native);
    assert_saved_matches_native(&fixture, native);
    let factory = fixture
        .sim
        .production
        .factories
        .view(fixture.owner, ProductionCategory::Building)
        .expect("product expiry alone retains its factory before a building-state recheck");
    assert_eq!(factory.progress, 54);
    assert!(factory.object.is_none());
    assert!(factory.suspended);
    assert_eq!(ready_product(&fixture, "GAPILL"), defense_product);
    assert!(
        production::ready_building_entity_for_owner(
            &fixture.sim,
            &fixture.rules,
            fixture.owner,
            fixture.sim.interner.get("GAPOWR").unwrap(),
        )
        .is_none()
    );
    assert!(
        !production::ready_buildings_for_owner(&fixture.sim, &fixture.rules, "Americans")
            .iter()
            .any(|ready| fixture.sim.resolve(ready.type_id) == "GAPOWR")
    );
    assert!(
        !fixture.input.cancel_building_placement(),
        "TacticalRight4AAD34 tests current product, not the retained type"
    );
    assert_same_preview(
        fixture.input.building_placement_preview.as_ref().unwrap(),
        &preview,
    );
    assert!(!press_ready_cameo(
        &mut fixture,
        "GAPILL",
        SidebarTab::Defense
    ));
}

#[test]
fn null_product_cameo_right_retains_orphan_until_house_abandon() {
    let Some(mut fixture) = paid_ready_fixture() else {
        return;
    };
    let preview = arm_gapowr_with_preview(&mut fixture);
    let product = ready_product(&fixture, "GAPOWR");
    let defense_product = ready_product(&fixture, "GAPILL");
    let credits = production::credits_for_owner(&fixture.sim, "Americans");
    fixture
        .sim
        .detach_all_pointer_expired(product, &fixture.rules, Some(&fixture.overlays));
    let frame = advance_commands(&mut fixture, &[]);
    let owner = fixture.owner;
    consume_frame_outputs(&mut fixture, &frame, Some(owner));
    let click = press_cameo(
        &mut fixture,
        "GAPOWR",
        SidebarTab::Building,
        CameoPress::Right { shift: false },
    );
    assert!(click.clear_building_placement);
    assert!(click.sound);
    assert_eq!(click.eva, Some(sidebar_eva::EVA_CANCELED));
    assert_eq!(click.order, Some(CameoOrder::Abandon { all: false }));
    assert_current_matches_native(
        &fixture,
        native_placement_after("current_product_expiry_right_receiver", "right_cameo", 0),
    );
    assert_eq!(
        fixture
            .input
            .targeting_mode
            .as_ref()
            .and_then(TargetingMode::as_building_placement),
        Some("GAPOWR"),
        "Select6AADCF's current-product guard does not admit the orphan"
    );
    assert_same_preview(
        fixture.input.building_placement_preview.as_ref().unwrap(),
        &preview,
    );
    let abandon = cancel_envelope(&fixture, "GAPOWR");
    let frame = advance_commands(&mut fixture, &[abandon]);
    assert_eq!(
        placement_facts(&frame),
        vec![LifecycleOutput::BuildingProductionAbandoned { owner }]
    );
    consume_frame_outputs(&mut fixture, &frame, Some(owner));
    assert!(fixture.input.targeting_mode.is_none());
    assert!(fixture.input.building_placement_preview.is_none());
    assert_current_matches_native(
        &fixture,
        native_placement_after(
            "current_product_expiry_right_receiver",
            "local_event_corridor",
            0,
        ),
    );
    assert!(
        fixture
            .sim
            .production
            .factories
            .view(owner, ProductionCategory::Building)
            .is_none()
    );
    assert_eq!(ready_product(&fixture, "GAPILL"), defense_product);
    assert_eq!(
        production::credits_for_owner(&fixture.sim, "Americans"),
        credits
    );
    assert!(press_ready_cameo(
        &mut fixture,
        "GAPILL",
        SidebarTab::Defense
    ));
    let click = press_cameo(
        &mut fixture,
        "GAPOWR",
        SidebarTab::Building,
        CameoPress::Right { shift: false },
    );
    assert_eq!(
        click,
        CameoClick::default(),
        "deleted Factory links are invalidated"
    );
    assert_current_matches_native(
        &fixture,
        native_placement_after(
            "null_product_direct_abandon_then_no_link_right",
            "right_cameo",
            0,
        ),
    );
}

#[test]
fn null_product_house_abandon_accepts_nonzero_buildcat_without_a_held_type() {
    for name in ["GAPOWR", "GAPILE"] {
        let Some(mut fixture) = paid_ready_fixture() else {
            return;
        };
        arm_gapowr_with_preview(&mut fixture);
        let product = ready_product(&fixture, "GAPOWR");
        let credits = production::credits_for_owner(&fixture.sim, "Americans");
        fixture
            .sim
            .detach_all_pointer_expired(product, &fixture.rules, Some(&fixture.overlays));
        let frame = advance_commands(&mut fixture, &[]);
        let owner = fixture.owner;
        consume_frame_outputs(&mut fixture, &frame, Some(owner));
        assert!(fixture.rules.object("GACNST").unwrap().build_cat.is_none());
        let abandon = cancel_envelope(&fixture, "GACNST");
        let frame = advance_commands(&mut fixture, &[abandon]);
        assert!(
            placement_facts(&frame).is_empty(),
            "retail BuildCat0 retains the empty Factory"
        );
        consume_frame_outputs(&mut fixture, &frame, Some(owner));
        assert_current_matches_native(
            &fixture,
            native_placement_after("null_product_requested_category_guards", "house_abandon", 0),
        );
        assert!(
            fixture
                .sim
                .production
                .factories
                .view(owner, ProductionCategory::Building)
                .is_some()
        );
        assert!(fixture.rules.object(name).unwrap().build_cat.is_some());
        let abandon = cancel_envelope(&fixture, name);
        let frame = advance_commands(&mut fixture, &[abandon]);
        assert_eq!(
            placement_facts(&frame),
            vec![LifecycleOutput::BuildingProductionAbandoned { owner }],
            "HouseAbandon4FABF6 skips the index match for a NULL held object"
        );
        consume_frame_outputs(&mut fixture, &frame, Some(owner));
        assert!(fixture.input.targeting_mode.is_none());
        assert_current_matches_native(
            &fixture,
            native_placement_after("null_product_requested_category_guards", "house_abandon", 1),
        );
        assert!(
            fixture
                .sim
                .production
                .factories
                .view(owner, ProductionCategory::Building)
                .is_none()
        );
        assert_eq!(
            production::credits_for_owner(&fixture.sim, "Americans"),
            credits
        );
    }
}

#[test]
fn defense_null_product_house_abandon_clears_only_its_live_factory() {
    let Some(mut fixture) = paid_ready_fixture() else {
        return;
    };
    assert!(press_ready_cameo(
        &mut fixture,
        "GAPILL",
        SidebarTab::Defense
    ));
    let product = ready_product(&fixture, "GAPILL");
    let power = ready_product(&fixture, "GAPOWR");
    let credits = production::credits_for_owner(&fixture.sim, "Americans");
    fixture
        .sim
        .detach_all_pointer_expired(product, &fixture.rules, Some(&fixture.overlays));
    let frame = advance_commands(&mut fixture, &[]);
    let owner = fixture.owner;
    consume_frame_outputs(&mut fixture, &frame, Some(owner));
    let abandon = cancel_envelope(&fixture, "GAPILL");
    let frame = advance_commands(&mut fixture, &[abandon]);
    assert_eq!(
        placement_facts(&frame),
        vec![LifecycleOutput::BuildingProductionAbandoned { owner }]
    );
    consume_frame_outputs(&mut fixture, &frame, Some(owner));
    assert_current_matches_native(
        &fixture,
        native_placement_after("defense_null_product_direct_abandon", "house_abandon", 0),
    );
    assert!(
        fixture
            .sim
            .production
            .factories
            .view(owner, ProductionCategory::Defense)
            .is_none()
    );
    assert_eq!(ready_product(&fixture, "GAPOWR"), power);
    assert_eq!(
        production::credits_for_owner(&fixture.sim, "Americans"),
        credits
    );
}

#[test]
fn same_factory_rearm_retains_the_old_cameos_factory_link_and_construction_order() {
    let Some(mut fixture) = paid_ready_fixture() else {
        return;
    };
    let product = ready_product(&fixture, "GAPOWR");
    let factory_order = fixture
        .sim
        .production
        .factories
        .iter_insertion_ordered()
        .iter()
        .map(|factory| (factory.category, factory.insertion_seq))
        .collect::<Vec<_>>();
    // A real admission is saved; its returned envelope is deliberately not
    // delivered before the supplied product-expiry notification.
    let _pending = submit_ready_building(&mut fixture, "GAPOWR", SidebarTab::Building, (8, 4));
    fixture
        .sim
        .detach_all_pointer_expired(product, &fixture.rules, Some(&fixture.overlays));
    let frame = advance_commands(&mut fixture, &[]);
    let owner = fixture.owner;
    consume_frame_outputs(&mut fixture, &frame, Some(owner));
    assert!(press_ready_cameo(
        &mut fixture,
        "GAPILL",
        SidebarTab::Defense
    ));
    assert!(production::enqueue_by_type(
        &mut fixture.sim,
        &fixture.rules,
        "Americans",
        "GAPILE"
    ));
    let type_id = fixture.sim.interner.get("GAPILE").unwrap();
    for _ in 0..4096 {
        if production::ready_building_entity_for_owner(&fixture.sim, &fixture.rules, owner, type_id)
            .is_some()
        {
            break;
        }
        let frame = advance_commands(&mut fixture, &[]);
        consume_frame_outputs(&mut fixture, &frame, Some(owner));
    }
    let product =
        production::ready_building_entity_for_owner(&fixture.sim, &fixture.rules, owner, type_id)
            .expect("the rearmed Factory pays and completes its new retail build");
    let native_rearm = native_after(
        "expired_product_same_category_rearm_retained_old_right",
        "produce_GAPILE",
        0,
    );
    assert_eq!(
        native_rearm["factory_registry"],
        native_after(
            "expired_product_same_category_rearm_retained_old_right",
            "dispatch_pointer_expired",
            0,
        )["factory_registry"],
        "executed native Factory identities and order survive rearm"
    );
    let native_order = native_rearm["factory_registry"]
        .as_array()
        .unwrap()
        .iter()
        .map(|factory| match factory["id"].as_str().unwrap() {
            "GAPOWR_factory" => ProductionCategory::Building,
            "GAPILL_factory" => ProductionCategory::Defense,
            id => panic!("unexpected native Factory identity {id}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        fixture
            .sim
            .production
            .factories
            .iter_insertion_ordered()
            .iter()
            .map(|factory| factory.category)
            .collect::<Vec<_>>(),
        native_order,
    );
    assert_eq!(
        fixture
            .sim
            .production
            .factories
            .iter_insertion_ordered()
            .iter()
            .map(|factory| (factory.category, factory.insertion_seq))
            .collect::<Vec<_>>(),
        factory_order,
        "native Begin reuses the Factory rather than appending a new one"
    );
    let credits = production::credits_for_owner(&fixture.sim, "Americans");
    let click = press_cameo(
        &mut fixture,
        "GAPOWR",
        SidebarTab::Building,
        CameoPress::Right { shift: false },
    );
    assert!(click.clear_building_placement);
    assert!(click.sound);
    assert_eq!(click.order, Some(CameoOrder::Abandon { all: false }));
    assert!(fixture.input.targeting_mode.is_none());
    assert_current_matches_native(
        &fixture,
        native_placement_after(
            "expired_product_same_category_rearm_retained_old_right",
            "right_cameo",
            0,
        ),
    );
    let abandon = cancel_envelope(&fixture, "GAPOWR");
    let frame = advance_commands(&mut fixture, &[abandon]);
    assert!(
        placement_facts(&frame).is_empty(),
        "the receiver refuses the mismatched held GAPILE index"
    );
    assert_eq!(
        production::ready_building_entity_for_owner(&fixture.sim, &fixture.rules, owner, type_id),
        Some(product)
    );
    assert_eq!(
        production::credits_for_owner(&fixture.sim, "Americans"),
        credits
    );
    assert!(press_ready_cameo(
        &mut fixture,
        "GAPOWR",
        SidebarTab::Building
    ));
    assert_current_matches_native(
        &fixture,
        native_placement_after(
            "expired_product_same_category_rearm_retained_old_left",
            "left_cameo",
            0,
        ),
    );
    assert_eq!(fixture.input.building_placement_issuer(), Some(owner));
    assert_eq!(
        fixture
            .input
            .targeting_mode
            .as_ref()
            .and_then(TargetingMode::as_building_placement),
        Some("GAPILE")
    );
    let command = submit_current_building(&mut fixture, (8, 4));
    assert_saved_matches_native(
        &fixture,
        native_placement_after(
            "expired_product_same_category_rearm_retained_old_left",
            "tactical_left_up",
            0,
        ),
    );
    let frame = advance_commands(&mut fixture, &[command]);
    assert_eq!(
        placement_facts(&frame),
        vec![LifecycleOutput::BuildingPlacementSucceeded { owner }]
    );
    consume_frame_outputs(&mut fixture, &frame, Some(owner));
    let expected = native_placement_after(
        "expired_product_same_category_rearm_retained_old_left",
        "local_event_corridor",
        0,
    );
    assert_current_matches_native(&fixture, expected);
    assert_saved_matches_native(&fixture, expected);
    assert!(
        fixture
            .sim
            .production
            .factories
            .view(owner, ProductionCategory::Building)
            .is_none()
    );
    assert_eq!(
        production::credits_for_owner(&fixture.sim, "Americans"),
        credits
    );
}

/// TacticalPointerExpired6DA57D consumes the saved product transaction while
/// the independent active placement survives. A later failure restores NULL.
#[test]
fn saved_product_expiry_clears_backup_without_clearing_another_current_product() {
    let Some(mut fixture) = paid_ready_fixture() else {
        return;
    };
    let product = ready_product(&fixture, "GAPOWR");
    let submitted = submit_ready_building(&mut fixture, "GAPOWR", SidebarTab::Building, (8, 4));
    assert!(press_ready_cameo(
        &mut fixture,
        "GAPILL",
        SidebarTab::Defense
    ));
    let pending = fixture.input.targeting_mode.clone();
    fixture
        .sim
        .detach_all_pointer_expired(product, &fixture.rules, Some(&fixture.overlays));
    let frame = advance_commands(&mut fixture, &[submitted]);
    let owner = fixture.owner;
    assert_eq!(consume_frame_outputs(&mut fixture, &frame, Some(owner)), 0);
    assert_eq!(fixture.input.targeting_mode, pending);
    assert!(fixture.input.saved_building_placement.is_none());
    assert_saved_matches_native(
        &fixture,
        native_placement_after("saved_product_expiry", "dispatch_pointer_expired", 0),
    );
    assert!(
        production::ready_building_entity_for_owner(
            &fixture.sim,
            &fixture.rules,
            fixture.owner,
            fixture.sim.interner.get("GAPOWR").unwrap(),
        )
        .is_none()
    );
    assert!(
        !production::ready_buildings_for_owner(&fixture.sim, &fixture.rules, "Americans")
            .iter()
            .any(|ready| fixture.sim.resolve(ready.type_id) == "GAPOWR")
    );
    assert!(
        placement_facts(&frame).is_empty(),
        "the expired saved object's queued PLACE is an early guard"
    );

    install_late_stationary_blocker(&mut fixture, (8, 4));
    let failed = CommandEnvelope::new(
        fixture.owner,
        fixture.sim.session.tick + 1,
        Command::PlaceReadyBuilding {
            type_id: fixture.sim.interner.get("GAPILL").unwrap(),
            rx: 8,
            ry: 4,
        },
    );
    let frame = advance_commands(&mut fixture, &[failed]);
    assert_eq!(
        placement_facts(&frame),
        vec![LifecycleOutput::BuildingPlacementRetry { owner }]
    );
    assert_eq!(consume_frame_outputs(&mut fixture, &frame, Some(owner)), 0);
    assert!(fixture.input.targeting_mode.is_none());
    assert!(fixture.input.saved_building_placement.is_none());
}

#[test]
fn fresh_targeting_reset_drops_current_and_saved_without_consuming_factories() {
    let Some(mut fixture) = paid_ready_fixture() else {
        return;
    };
    let identities = ready_identities(&fixture);
    submit_ready_building(&mut fixture, "GAPOWR", SidebarTab::Building, (8, 4));
    assert!(press_ready_cameo(
        &mut fixture,
        "GAPILL",
        SidebarTab::Defense
    ));
    assert!(fixture.input.saved_building_placement.is_some());
    let hash = fixture.sim.state_hash();
    fixture.input.reset_targeting_for_new_world();

    assert!(fixture.input.targeting_mode.is_none());
    assert!(fixture.input.building_placement_preview.is_none());
    assert!(fixture.input.saved_building_placement.is_none());
    assert!(fixture.input.building_placement_issuer().is_none());
    assert_eq!(ready_identities(&fixture), identities);
    assert_eq!(fixture.sim.state_hash(), hash);
    assert!(press_ready_cameo(
        &mut fixture,
        "GAPOWR",
        SidebarTab::Building
    ));
    assert_eq!(
        fixture.input.building_placement_issuer(),
        Some(fixture.owner)
    );
    // Exercises the input owner's reset; fresh-world production integration
    // is validated separately at the scenario-installation boundary.
    // It does not execute held saved-load/scenario replacement or claim that
    // native RuleType pointers live across a world replacement.
}

/// HousePlace4FB520 routes zero/negative cell sentinels to the actual yard's
/// human BuildingExit443C60 failure. It never enters building Unlimbo, and
/// the human failure does not refund or destroy the completed object.
#[test]
fn sentinel_placement_keeps_human_factories_and_current_without_retry_or_eva() {
    let Some(mut fixture) = paid_ready_fixture() else {
        return;
    };
    let preview = arm_gapowr_with_preview(&mut fixture);
    let pending = fixture.input.targeting_mode.clone();
    let identities = ready_identities(&fixture);
    let product = ready_product(&fixture, "GAPOWR");
    production::object_find_factory(&fixture.sim, &fixture.rules, product, false, true)
        .expect("the sentinel's alternate ObjectFindFactory(0,1) finds the actual healthy yard");
    let credits = production::credits_for_owner(&fixture.sim, "Americans");
    let spent = fixture.sim.houses[&fixture.owner].economy.spent_credits();
    for (occurrence, cell) in [(0, 0), (u16::MAX, u16::MAX)].into_iter().enumerate() {
        let command = CommandEnvelope::new(
            fixture.owner,
            fixture.sim.session.tick + 1,
            Command::PlaceReadyBuilding {
                type_id: fixture.sim.interner.get("GAPOWR").unwrap(),
                rx: cell.0,
                ry: cell.1,
            },
        );
        let frame = advance_commands(&mut fixture, &[command]);
        assert_eq!(frame.tick.executed_commands, 1);
        assert!(!frame.tick.spawned_entities);
        assert!(placement_facts(&frame).is_empty());
        assert!(!frame.sound_events.iter().any(|event| {
            matches!(
                event,
                SimSoundEvent::CannotDeployHere { .. } | SimSoundEvent::BuildingPlaced { .. }
            )
        }));
        let owner = fixture.owner;
        assert_eq!(consume_frame_outputs(&mut fixture, &frame, Some(owner)), 0);
        assert_eq!(fixture.input.targeting_mode, pending);
        assert_same_preview(
            fixture.input.building_placement_preview.as_ref().unwrap(),
            &preview,
        );
        assert_eq!(ready_identities(&fixture), identities);
        assert_eq!(
            production::credits_for_owner(&fixture.sim, "Americans"),
            credits
        );
        assert_eq!(
            fixture.sim.houses[&fixture.owner].economy.spent_credits(),
            spent
        );
        let native = native_placement_after(
            "direct_empty_and_negative_cell_guards",
            "direct_receiver",
            occurrence,
        );
        assert_current_matches_native(&fixture, native);
        assert_saved_matches_native(&fixture, native);
    }
}
