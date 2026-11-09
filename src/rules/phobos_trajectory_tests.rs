//! `Trajectory` + `Trajectory.Straight.*` reads
//! (`Ext/Bullet/Trajectories/PhobosTrajectory.cpp:24-68`,
//! `StraightTrajectory.cpp:66-102`).
use super::{PhobosTrajectoryRules, StraightTrajectoryRules, TrajectoryFlag};
use crate::rules::ini_parser::IniFile;

fn read(text: &str) -> PhobosTrajectoryRules {
    PhobosTrajectoryRules::from_section(IniFile::from_str(text).section_or_empty("Q"))
}

#[test]
fn trajectory_defaults_and_flag_names() {
    let defaults = read("[Q]\n");
    assert_eq!(defaults.flag, None);
    assert_eq!(defaults.speed, 100.0);
    let straight = StraightTrajectoryRules::default();
    assert_eq!(defaults.straight, straight);
    assert_eq!(straight.detonation_distance, 102);
    assert_eq!(straight.target_snap_distance, 128);
    assert_eq!(straight.pass_detonate_delay, 1);
    assert!(straight.mirror_coord, "MirrorCoord constructs yes");
    assert!(straight.through_vehicles);
    assert!(straight.through_building);
    assert_eq!(
        (straight.edge_attenuation, straight.count_attenuation),
        (1.0, 1.0)
    );
    assert_eq!(straight.axis_of_rotation, [0, 0, 1]);

    for (text, flag) in [
        ("Trajectory=Straight", TrajectoryFlag::Straight),
        ("Trajectory=straight", TrajectoryFlag::Straight),
        ("Trajectory=Bombard", TrajectoryFlag::Bombard),
        ("Trajectory=Parabola", TrajectoryFlag::Parabola),
    ] {
        assert_eq!(read(&format!("[Q]\n{text}\n")).flag, Some(flag), "{text}");
    }
    // An unknown or blank name keeps the stock arms; a dormant Straight key
    // set is not read without the flag.
    let unknown = read("[Q]\nTrajectory=Nope\nTrajectory.Straight.PassThrough=yes\n");
    assert_eq!(unknown.flag, None);
    assert!(!unknown.straight.pass_through);
    let bomb = read("[Q]\nTrajectory=Bombard\nTrajectory.Straight.PassThrough=yes\n");
    assert_eq!(bomb.flag, Some(TrajectoryFlag::Bombard));
    assert!(!bomb.straight.pass_through);
}

#[test]
fn straight_keys_read_with_defaults_and_clamps() {
    let rules = read(
        "[Q]\n\
         Trajectory=Straight\n\
         Trajectory.Speed=0\n\
         Trajectory.Straight.DetonationDistance=0.5\n\
         Trajectory.Straight.TargetSnapDistance=2\n\
         Trajectory.Straight.PassThrough=yes\n\
         Trajectory.Straight.PassDetonate=yes\n\
         Trajectory.Straight.PassDetonateWarhead=WH1\n\
         Trajectory.Straight.PassDetonateDamage=-5\n\
         Trajectory.Straight.PassDetonateDelay=3\n\
         Trajectory.Straight.PassDetonateInitialDelay=4\n\
         Trajectory.Straight.PassDetonateLocal=yes\n\
         Trajectory.Straight.LeadTimeCalculate=yes\n\
         Trajectory.Straight.OffsetCoord=5,-6\n\
         Trajectory.Straight.RotateCoord=90.5\n\
         Trajectory.Straight.MirrorCoord=no\n\
         Trajectory.Straight.UseDisperseBurst=yes\n\
         Trajectory.Straight.AxisOfRotation=1,0\n\
         Trajectory.Straight.ProximityImpact=2\n\
         Trajectory.Straight.ProximityWarhead=WH2\n\
         Trajectory.Straight.ProximityDamage=7\n\
         Trajectory.Straight.ProximityRadius=0.25\n\
         Trajectory.Straight.ProximityDirect=yes\n\
         Trajectory.Straight.ProximityMedial=yes\n\
         Trajectory.Straight.ProximityAllies=yes\n\
         Trajectory.Straight.ProximityFlight=yes\n\
         Trajectory.Straight.ThroughVehicles=no\n\
         Trajectory.Straight.ThroughBuilding=no\n\
         Trajectory.Straight.SubjectToGround=yes\n\
         Trajectory.Straight.ConfineAtHeight=192\n\
         Trajectory.Straight.EdgeAttenuation=-2\n\
         Trajectory.Straight.CountAttenuation=0.5\n",
    );
    assert_eq!(rules.flag, Some(TrajectoryFlag::Straight));
    // `Math::max(0.001, x)` after the read.
    assert_eq!(rules.speed, 0.001);
    assert_eq!(
        read("[Q]\nTrajectory=Straight\nTrajectory.Speed=-4\n").speed,
        0.001
    );
    let s = &rules.straight;
    // `Valueable<Leptons>`: cells x 256.
    assert_eq!(s.detonation_distance, 128);
    assert_eq!(s.target_snap_distance, 512);
    assert!(s.pass_through);
    assert!(s.pass_detonate);
    assert_eq!(s.pass_detonate_warhead.as_deref(), Some("WH1"));
    assert_eq!(s.pass_detonate_damage, -5);
    assert_eq!(s.pass_detonate_delay, 3);
    assert_eq!(s.pass_detonate_initial_delay, 4);
    assert!(s.pass_detonate_local);
    assert!(s.lead_time_calculate);
    assert_eq!(s.offset_coord, [5, -6, 0], "the parsed prefix only");
    assert_eq!(s.rotate_coord, 90.5);
    assert!(!s.mirror_coord);
    assert!(s.use_disperse_burst);
    assert_eq!(s.axis_of_rotation, [1, 0, 1]);
    assert_eq!(s.proximity_impact, 2);
    assert_eq!(s.proximity_warhead.as_deref(), Some("WH2"));
    assert_eq!(s.proximity_damage, 7);
    assert_eq!(s.proximity_radius, 64);
    assert!(s.proximity_direct);
    assert!(s.proximity_medial);
    assert!(s.proximity_allies);
    assert!(s.proximity_flight);
    assert!(!s.through_vehicles);
    assert!(!s.through_building);
    assert!(s.subject_to_ground);
    assert_eq!(s.confine_at_height, 192);
    assert_eq!(s.edge_attenuation, 0.0, "clamped to max(0.0, x)");
    assert_eq!(s.count_attenuation, 0.5);

    // `none` clears a warhead name; an unknown name is kept for the factory.
    let cleared = read(
        "[Q]\nTrajectory=Straight\n\
         Trajectory.Straight.PassDetonateWarhead=<none>\n",
    );
    assert_eq!(cleared.straight.pass_detonate_warhead, None);
    let unknown = read(
        "[Q]\nTrajectory=Straight\n\
         Trajectory.Straight.ProximityWarhead=NOTAWH\n",
    );
    assert_eq!(
        unknown.straight.proximity_warhead.as_deref(),
        Some("NOTAWH")
    );
}
