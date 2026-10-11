//! The pursuit stage's weapon and range verdict: whether an attacker holding
//! a target can shoot it from where it stands or must keep closing.

use super::*;

/// Resolve the weapon an attacker would use against a `TargetKind` while
/// pursuing it.
///
/// Uses the same weapon-select inputs as the combat tick's Phase 2 weapon
/// selection so pursuit and combat agree on "in range" at the boundary.
///
/// Returns `None` only when the selected slot names no weapon. Legality is
/// not asked: `FootClass::Approach_Target @ 0x004D5690` selects (`0x004D56CA`)
/// and measures with CanFireAt without GetFireError. The fire routine asks
/// GetFireError itself, and `TechnoClass::AI`'s 16-frame check drops an
/// ILLEGAL or CANT target (a building drops it at once).
pub(crate) fn pursuit_selected_weapon<'a>(
    entity: &GameEntity,
    target: &TargetKind,
    entities: &EntityStore,
    rules: &'a RuleSet,
    interner: &StringInterner,
    terrain: Option<&ResolvedTerrainGrid>,
    alliances: Option<&HouseAllianceMap>,
) -> Option<&'a WeaponType> {
    pursuit_selection(
        entity, target, entities, rules, interner, terrain, alliances,
    )
    .map(|selected| selected.weapon)
}

/// [`pursuit_selected_weapon`]'s whole selection, native weapon index
/// included.
pub(crate) fn pursuit_selection<'a>(
    entity: &GameEntity,
    target: &TargetKind,
    entities: &EntityStore,
    rules: &'a RuleSet,
    interner: &StringInterner,
    terrain: Option<&ResolvedTerrainGrid>,
    alliances: Option<&HouseAllianceMap>,
) -> Option<combat_weapon::SelectedWeapon<'a>> {
    let attacker_obj = rules.object(interner.resolve(entity.type_ref()))?;
    select_weapon_against(
        rules,
        entity,
        attacker_obj,
        &combat_weapon::attacker_facts(entity, attacker_obj),
        Some(target),
        entities,
        interner,
        terrain,
        alliances,
    )
    .and_then(|(_, selected)| selected)
}

/// What the pursuit stage should do with an attacker that is holding a target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PursuitRangeVerdict {
    /// The full `InRange` gate passes — halt and let the combat tick fire.
    CanFire,
    /// Refused, and closing the distance is what native's approach search does
    /// about it: too far, or a wall or a cliff on the line.
    CloseIn,
    /// Refused because the attacker stands INSIDE the weapon's `MinimumRange`.
    ///
    /// **VERA-internal, and a deliberate deferral of a native mechanism.**
    /// Native's approach search 0x004D5690 scans candidate coordinates and
    /// takes one where `InRange` holds, so a V3 that has been closed on backs
    /// off to five cells. VERA's pursuit produces exactly one candidate — the
    /// target's own cell — which for a too-close refusal is the WORST cell in
    /// the set. So this verdict holds position instead, which is bit-for-bit
    /// the behaviour this stage had before the walk landed.
    /// - Trigger: `MinimumRange=` weapon whose target has come inside it —
    ///   `V3Launcher` (5), `DredLauncher`/`CruiseLauncher` (8), `MagneticBeam`
    ///   (3), `HowitzerGun` (2), the `MissileLauncher`/`HoverMissile` family (1).
    /// - Player effect: the V3 stops rather than backing off, and the fire gate
    ///   keeps refusing until the target moves away again.
    /// - Frequency: ordinary — a V3 shelling an advancing column meets it every
    ///   time the column closes.
    /// - Downstream risk: none to deterministic state; it is a hold, and the
    ///   fire gate already refused before this stage ran. Cured by porting the
    ///   candidate-coordinate scan in 0x004D5690, which is its own mechanism.
    HoldInsideMinimumRange,
}

/// Whether a pursuing attacker can already shoot its target from where it
/// stands — the predicate that decides "halt and fire" against "keep closing".
///
/// gamemd-derived: `FootClass::Mission_Attack @ 0x004D4DC0` dispatches to the
/// approach search — the "walk to somewhere I can shoot my target from" routine
/// at vtable slot `+0x53C`, labelled `FootClass::Greatest_Threat_Scan @
/// 0x004D5690` in the database — through `CALL [EAX+0x53c]` at `0x004D4E6A`,
/// on the arm where TarCom (`[this+0x2B4]`) is non-null; `InfantryClass`'s
/// override `0x00522340` chains straight into the same body at `0x0052236E`.
/// That body decides with `TechnoClass::InRange @ 0x006F7220`, called at
/// `0x004D622C` and `0x004D6550` with a candidate coordinate as arg1
/// (`LEA EAX,[ESP+0x30]`) and TarCom as arg2 — and `InRange` ends in the
/// line-of-fire walk (`CALL 0x004CC310` at `0x006F7642`).
///
/// So the approach predicate and the fire gate are the SAME test in gamemd.
/// Using the 2-D twin here instead would let pursuit believe it had arrived
/// while `resolve_attacker_fire` refuses the shot, and the unit would stand
/// still under a standing order forever.
pub(crate) fn pursuit_in_range(
    entity: &GameEntity,
    target: &TargetKind,
    weapon: &WeaponType,
    entities: &EntityStore,
    rules: &RuleSet,
    interner: &StringInterner,
    terrain: Option<&ResolvedTerrainGrid>,
    los: &line_of_fire::LineOfFireInputs<'_>,
) -> PursuitRangeVerdict {
    let Some(terrain) = terrain else {
        // No resolved terrain (headless fixtures, pre-map bring-up): the 3-D
        // gate has nothing to measure against, so fall back to the same 2-D
        // twin `resolve_attacker_fire` falls back to in that situation. The
        // two stages still agree, which is the property that matters. That twin
        // has no MinimumRange arm either, so it cannot report the too-close
        // verdict.
        let Some((trx, try_, tsx, tsy)) = resolve_target_coords(target, entities) else {
            return PursuitRangeVerdict::CloseIn;
        };
        let dist_sq = lepton_distance_sq_raw(
            entity.position.rx,
            entity.position.ry,
            entity.position.sub_x,
            entity.position.sub_y,
            trx,
            try_,
            tsx,
            tsy,
        );
        return if is_within_range_leptons(dist_sq, weapon.range) {
            PursuitRangeVerdict::CanFire
        } else {
            PursuitRangeVerdict::CloseIn
        };
    };

    let Some(src) =
        in_range::fire_source_coords(entity, target, weapon, entities, terrain, (rules, interner))
    else {
        // The attacker has no resolvable ground height (off-grid). Report
        // "keep closing" rather than freezing; the fire gate returns without
        // firing on the same condition.
        return PursuitRangeVerdict::CloseIn;
    };
    if in_range::compute_in_range(
        entity, src, target, weapon, rules, interner, entities, terrain, los,
    ) {
        return PursuitRangeVerdict::CanFire;
    }
    if in_range::inside_minimum_range(src, target, weapon, rules, interner, entities, terrain) {
        return PursuitRangeVerdict::HoldInsideMinimumRange;
    }
    PursuitRangeVerdict::CloseIn
}
