//! Phobos `SWTypeExt`'s techno gates: `SW.TechLevel`, `SW.AuxTechnos` and
//! `SW.NegTechnos` (`src/Ext/SWType/SWHelpers.cpp::IsAvailable`). Synthetic
//! Rust regressions over the shared availability owner, not native executable
//! comparisons.

use super::*;
use crate::rules::ini_parser::IniFile;
use crate::sim::house_state::HouseState;

const OWNER: &str = "Americans";
const BASE: &str = "[InfantryTypes]\n0=E1\n1=DESO\n[VehicleTypes]\n0=MTNK\n\
[AircraftTypes]\n[BuildingTypes]\n0=GACLAB\n1=NATIVEAUX\n\
[GACLAB]\nStrength=1000\n\
[NATIVEAUX]\nStrength=1000\n\
[E1]\nStrength=100\nSpeed=4\n\
[DESO]\nStrength=100\nSpeed=4\n\
[MTNK]\nStrength=300\nSpeed=6\n\
[SuperWeaponTypes]\n0=GateSpecial\n\
[GateSpecial]\nType=PsychicReveal\nRechargeTime=1\nIsPowered=no\n";
const ART: &str = "[GACLAB]\nFoundation=1x1\n[NATIVEAUX]\nFoundation=1x1\n";
const SW: &str = "GateSpecial";

fn rules_with(extra: &str) -> RuleSet {
    let mut ini = crate::sim::arena_fixture::clear_ground_ini(BASE);
    ini.merge(&IniFile::from_str(extra));
    RuleSet::from_ini_with_fixed_art_for_test(&ini, &IniFile::from_str(ART))
        .expect("rules with the Phobos gates parse")
}

fn world(rules: &RuleSet) -> Simulation {
    let mut sim = Simulation::with_seed(23);
    sim.intern_rule_type_ids(rules);
    sim.resolve_type_handles(rules);
    let owner = sim.interner.intern(OWNER);
    sim.houses
        .register(HouseState::new(owner, 0u8, None, true, 2000, 10));
    construct_house_supers(&mut sim, rules, owner, false);
    sim.session.game_options_mut_for_test().super_weapons = true;
    sim.resolved_terrain = Some(crate::map::resolved_terrain::test_grid(32, 32, |x, y| {
        super::cell_receiver_tests::test_terrain_cell(x, y)
    }));
    sim.set_playfield_bounds_for_test(Some(super::cell_receiver_tests::test_playfield_bounds()));
    crate::sim::arena_fixture::supply_native_map(&mut sim);
    sim
}

fn spawn(sim: &mut Simulation, rules: &RuleSet, name: &str, x: u16) {
    sim.spawn_object_at_height(name, OWNER, x, 10, 0, 0, rules)
        .expect("techno spawns");
}

fn allowed(sim: &Simulation, rules: &RuleSet) -> bool {
    super_weapon_available(sim, rules, sim.interner.get(OWNER).unwrap(), SW)
}

/// The three keys land in the projected availability with their native
/// meaning: a minimum TechLevel, any-of `SW.AuxTechnos`, none-of
/// `SW.NegTechnos`, and unknown techno names skipped.
#[cfg(feature = "phobos")]
#[test]
fn phobos_reads_the_three_superweapon_gates() {
    let rules = rules_with(
        "[GateSpecial]\nType=PsychicReveal\nRechargeTime=1\nIsPowered=no\n\
         SW.TechLevel=5\nSW.AuxTechnos=e1,NOSUCHTYPE\nSW.NegTechnos=DESO\n",
    );
    let availability = rules
        .phobos_super_weapon_metadata(SW)
        .expect("the capability projects every registered definition");
    assert!(availability.allows_tech_level(5));
    assert!(
        !availability.allows_tech_level(4),
        "a plain minimum, not a cap"
    );
    assert!(
        availability.allows_technos(|id| id.eq_ignore_ascii_case("E1")),
        "the known token binds case-insensitively"
    );
    assert!(
        !availability.allows_technos(|_| false),
        "an aux list of one known type needs that type"
    );
    assert!(
        !availability.allows_technos(|id| id.eq_ignore_ascii_case("DESO")),
        "a present neg techno refuses"
    );

    let unknown_only = rules_with(
        "[GateSpecial]\nType=PsychicReveal\nRechargeTime=1\nIsPowered=no\n\
                    SW.AuxTechnos=NOSUCHTYPE,NOSUCHOTHER\n",
    );
    let availability = unknown_only
        .phobos_super_weapon_metadata(SW)
        .expect("registered definition");
    assert!(
        availability.allows_technos(|_| false),
        "unknown names are skipped, leaving an empty gate"
    );
}

/// `SW.AuxTechnos` looks at every techno category, so one owned vehicle
/// satisfies a list whose other entry is infantry.
#[cfg(feature = "phobos")]
#[test]
fn phobos_aux_technos_scan_every_category() {
    let rules = rules_with(
        "[GateSpecial]\nType=PsychicReveal\nRechargeTime=1\nIsPowered=no\n\
         SW.AuxTechnos=E1,MTNK\n",
    );
    let mut sim = world(&rules);
    assert!(!allowed(&sim, &rules), "no listed techno is owned");
    spawn(&mut sim, &rules, "MTNK", 12);
    assert!(allowed(&sim, &rules), "the vehicle satisfies the list");
}

/// `SW.NegTechnos` refuses availability as soon as one listed techno exists,
/// and `SW.TechLevel` gates before any techno test.
#[cfg(feature = "phobos")]
#[test]
fn phobos_neg_technos_and_tech_level_gate_availability() {
    let rules = rules_with(
        "[GateSpecial]\nType=PsychicReveal\nRechargeTime=1\nIsPowered=no\n\
         SW.TechLevel=5\nSW.NegTechnos=DESO\n",
    );
    let mut sim = world(&rules);
    assert!(
        allowed(&sim, &rules),
        "level 10 meets the gate and no DESO exists"
    );
    spawn(&mut sim, &rules, "DESO", 14);
    assert!(!allowed(&sim, &rules), "the owned neg techno refuses");

    let owner = sim.interner.get(OWNER).unwrap();
    sim.houses
        .get_mut(&owner)
        .unwrap()
        .set_tech_level_for_test(4);
    assert!(
        !allowed(&sim, &rules),
        "below SW.TechLevel stays refused even without the neg techno"
    );
}
