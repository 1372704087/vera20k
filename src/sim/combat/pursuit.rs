//! The weapon an object closing on its target selects: Approach's
//! SelectWeapon (`FootClass::Approach_Target @ 0x004D5690`, `0x004D56CA`).

use super::*;

/// The weapon an attacker selects against `target` while it closes on it,
/// with the same inputs as the fire routine's selection, so the approach and
/// the fire gate agree on "in range" at the boundary; native weapon index
/// included.
///
/// None only when the selected slot names no weapon. Legality is not asked:
/// Approach selects (`0x004D56CA`) and measures with CanFireAt without
/// GetFireError. The fire routine asks GetFireError itself, and
/// `TechnoClass::AI`'s 16-frame check drops an ILLEGAL or CANT target (a
/// building drops it at once).
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
