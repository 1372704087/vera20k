//! Phobos `AutoTargetOwnPosition=` through `Retaliate_And_Scan`
//! (`Ext/Techno/Hooks.TargetEvaluation.cpp:8-23`, hook `0x7098B9`).

use super::{ScanHost, retaliate_and_scan};
use crate::sim::combat::fire_error::FireError;
use crate::sim::combat::{ScanMission, TargetKind};

/// The branch's own inputs, and every callee the replaced block would have
/// reached.
struct Host {
    own_position: Option<bool>,
    target: Option<TargetKind>,
    passive: bool,
    greatest_threat_calls: u32,
    distribute_calls: u32,
}

impl Host {
    fn new(own_position: Option<bool>) -> Self {
        Self {
            own_position,
            // A passively acquired target is what the replaced block would
            // drop on a bad `FireError`; the own-position branch must keep it
            // only if its own `assign_target` wrote one.
            target: Some(TargetKind::Entity(9)),
            passive: true,
            greatest_threat_calls: 0,
            distribute_calls: 0,
        }
    }
}

impl ScanHost for Host {
    type Weapon = ();

    fn frame(&self) -> i32 {
        0
    }
    fn set_last_scan_frame(&mut self, _frame: i32) {}
    fn committed_area_guard(&self) -> bool {
        false
    }
    fn random_ranged(&mut self, _min: i32, _max: i32) -> i32 {
        0
    }
    fn targeting_delay(&self, _area_guard: bool) -> i32 {
        0
    }
    fn arm_targeting_timer(&mut self, _start: i32, _duration: i32) {}
    fn target(&self) -> Option<TargetKind> {
        self.target
    }
    fn passively_acquired(&self) -> bool {
        self.passive
    }
    fn select_weapon(&mut self, _target: Option<TargetKind>) -> i32 {
        0
    }
    fn fire_error(&mut self, _target: Option<TargetKind>, _weapon: i32) -> FireError {
        // The value that would drop a passive target.
        FireError::Illegal
    }
    fn has_spawn_manager(&self) -> bool {
        false
    }
    fn clear_spawn_targets(&mut self) {}
    fn assign_target(&mut self, target: Option<TargetKind>) {
        self.target = target;
        self.passive = false;
    }
    fn greatest_threat(&mut self, _mask: ScanMission) -> Option<TargetKind> {
        self.greatest_threat_calls += 1;
        Some(TargetKind::Entity(7))
    }
    fn distributed_fire(&mut self) -> bool {
        false
    }
    fn distribute_fire(&mut self) {
        self.distribute_calls += 1;
    }
    fn weapon(&mut self, _index: i32) -> Option<()> {
        None
    }
    fn inaccurate(&self, _weapon: ()) -> bool {
        false
    }
    fn is_techno(&self, _target: TargetKind) -> bool {
        false
    }
    fn estimated_damage(&mut self, _target: TargetKind, _weapon: ()) -> i32 {
        0
    }
    fn debit(&mut self, _target: TargetKind, _amount: i32) {}
    fn auto_target_own_position(&self) -> Option<bool> {
        self.own_position
    }
    fn own_position_target(&self, self_target: bool) -> TargetKind {
        if self_target {
            TargetKind::Entity(1)
        } else {
            TargetKind::Cell(5, 5)
        }
    }
}

/// `Some(false)` installs the object's own cell, `Some(true)` the object
/// itself, and both skip `Greatest_Threat` and `DistributeFire` entirely.
#[test]
fn the_own_position_branch_installs_its_target_and_skips_the_scan() {
    for (self_target, expected) in [
        (false, TargetKind::Cell(5, 5)),
        (true, TargetKind::Entity(1)),
    ] {
        let mut host = Host::new(Some(self_target));
        assert!(retaliate_and_scan(&mut host, ScanMission::Guard));
        assert_eq!(host.target, Some(expected));
        assert_eq!(host.greatest_threat_calls, 0, "no Greatest_Threat");
        assert_eq!(host.distribute_calls, 0, "no DistributeFire");
    }
}

/// `None` keeps the stock body: the held passive target is dropped on the
/// `Illegal` the host answers, then `Greatest_Threat` supplies the pick.
#[test]
fn an_unset_key_keeps_the_stock_scan() {
    let mut host = Host::new(None);
    assert!(retaliate_and_scan(&mut host, ScanMission::Guard));
    assert_eq!(host.target, Some(TargetKind::Entity(7)));
    assert_eq!(host.greatest_threat_calls, 1);
}
