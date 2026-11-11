# Auto-target non-threatening buildings

Enemy buildings that pose no threat are normally outside a human player's
auto-target scan. Three keys can admit them:

- `[General] AutoTarget.NoThreatBuildings=` (default `no`, human houses)
- `[General] AutoTargetAI.NoThreatBuildings=` (default `yes`, computer houses)
- `[WeaponType] AttackNoThreatBuildings=` (`Nullable<bool>`, per weapon)

Read by `Ext/Rules/Body.cpp:517-518` and `Ext/WeaponType/Body.cpp:206`.

```ini
[General]
AutoTarget.NoThreatBuildings=yes

[Weapon]
AttackNoThreatBuildings=yes
```

## Native evidence

`Ext/Techno/Hooks.cpp:1848-1865` at `0x6F85CF`, inside
`TechnoClass::CanAutoTargetObject`'s unarmed/non-threatening building reject:

```cpp
bool canAttack = pThis->Owner->IsControlledByHuman()
    ? RulesExt::Global()->AutoTarget_NoThreatBuildings
    : RulesExt::Global()->AutoTargetAI_NoThreatBuildings;

if (const auto pWeaponExt = CanAutoTargetTemp::WeaponExt)
    canAttack = pWeaponExt->AttackNoThreatBuildings.Get(canAttack);

if (canAttack)
    return CanAttack; // 0x6F8604, past the reject

R->EAX(pTarget->GetTurretWeapon());
return Continue;     // 0x6F85D9, the weapon/ThreatPosed test
```

`CanAutoTargetTemp::WeaponExt` is the scanner's selected weapon, set by the
`0x6F7E30` hook from `CanAutoTargetObject`'s weapon argument. The reject
itself tests the candidate's armed state and live `ThreatPosed`; the override
jumps straight past both.

## Where VERA20K owns it

- `rules::phobos_globals::PhobosGlobals::auto_target_no_threat_buildings` /
  `auto_target_ai_no_threat_buildings` (`[General]`).
- `rules::weapon_type::WeaponType::phobos_attack_no_threat_buildings`.
- `sim::combat::greatest_threat::attack_no_threat_buildings`, called inside the
  G24 reject (`sim/combat/greatest_threat.rs`): the selected weapon's override,
  else the human/AI global, and true skips the whole reject as the hook's
  `0x6F8604` jump does.

## Residuals

- The G24 reject already runs only for a human scanner (the native computer
  skip at `0x006F85AB..0x006F85B8` stays), so in retail the human global is the
  one that can change an outcome; the AI global is read for the same hook
  shape. Both default values match the Phobos constructors.
