# RA1 multi-turret / multi-barrel (`BarrelOverTurret`, `BarrelOffset`, `Extra*`, `BurstPerTurret`)

Phobos draws a Unit with more than one turret or barrel and cycles the burst
across them. VERA parses the block and records the consumers below as
residuals: nothing reads the offsets yet.

## Keys

| Key | Type | Default |
|---|---|---|
| `BarrelOverTurret=` | `Nullable<bool>` | unset |
| `BarrelOffset=` | `int` | `0` |
| `ExtraBarrelCount=` | `int` | `0` |
| `ExtraBarrelOffset<n>=` | `int` | `0`, `n < ExtraBarrelCount` |
| `ExtraTurretCount=` | `int` | `0` |
| `ExtraTurretOffset<n>=` | `CoordStruct` | `0,0,0`, `n < ExtraTurretCount` |
| `BurstPerTurret=` | `int` | `0`, read only when `ExtraTurretCount > 0` |

Read: `UnitTypeExt::LoadFromINI` (`Ext/UnitType/Body.cpp:100-134`),
declarations `Ext/UnitType/Body.h:76-82`, constructor defaults `Body.h:135-141`.
Two clamps are load-bearing: a non-positive `ExtraBarrelCount=` /
`ExtraTurretCount=` clamps to zero and reads no offsets (`:116`, `:134`), and
`BurstPerTurret=` is read only inside the positive `ExtraTurretCount=` arm
(`:121-131`).

VERA reads the block in `src/rules/phobos_turret_barrel.rs`, owner
`ObjectType::phobos_turret_barrel`, through
`IniSection::read_phobos_int_triple` for the `CoordStruct` offsets.

## Behaviour

None of it is wired. Native's three consumers:

- **Draw matrix** — `UnitClass_DrawVoxelBody`'s matrix hook
  (`Ext/TechnoType/Hooks.MatrixOp.cpp:38-914`) replaces the single-body matrix
  whenever `TurretOffset != 0,0,0 || ExtraTurretCount > 0 || ExtraBarrelCount
  > 0` (`:30-44`). It emits one body, one matrix per barrel
  (`BarrelOffset` for the stock barrel, `ExtraBarrelOffset<n>` for the rest,
  `:174`, `:205-206`, `:885`) and one per turret
  (`ExtraTurretOffset<n>` through `UnitTypeExt::ApplyTurretOffsetUnit`,
  `Ext/UnitType/Body.cpp:5-14`); `BarrelOverTurret` chooses the turret matrix
  the barrel rides.
- **Recoil** — `UnitExt::InitializeRecoilData` (`Ext/Unit/Body.cpp:382-421`)
  keeps `ExtraTurretRecoil[ExtraTurretCount]` and
  `ExtraBarrelRecoil[(ExtraBarrelCount+1)*(ExtraTurretCount+1)-1]`, each a
  `RecoilData` cloned from the type's `TurretAnimData` / `BarrelAnimData`.
  `UnitExt::RecordRecoilData` (`:423-460`) picks the slot from the burst
  index:

  ```cpp
  turretIndex = BurstPerTurret
      ? ((CurrentBurstIndex / BurstPerTurret) % (ExtraTurretCount + 1)) : 0;
  barrelIndex = (ExtraTurretCount || ExtraBarrelCount)
      ? (CurrentBurstIndex % ((ExtraBarrelCount + 1) * (ExtraTurretCount + 1))) : 0;
  ```

  and vanilla's own recoil update is skipped (`DEFINE_JUMP 0x6FA4D1 ->
  0x6FA4FB`).
- **Firing origin** — `TechnoClass::GetFLH`'s hook
  (`Ext/Techno/Hooks.Firing.cpp:1076-1084`) converts the same burst index into
  the turret the muzzle comes from. Note its modulo is `BurstPerTurret + 1`,
  not `ExtraTurretCount + 1`; the two readers disagree upstream and VERA
  copies whichever the consumer needs.

## Residuals

- **No consumer.** `barrel_offset`, `extra_barrel_offsets`,
  `extra_turret_offsets`, `barrel_over_turret` and `burst_per_turret` are
  parsed and unread. Wiring the draw needs a multi-instance Unit body: VERA's
  voxel body is one `(model, BodyDraw)` pair per entity (`drawn_model_id` /
  `body_draw` in `src/app/presentation/instances/units.rs`) with a single
  turret split (`draws_turret_parts`), so the `Hooks.MatrixOp.cpp` routine has
  no owner to extend.
- **No per-slot recoil state.** `GameEntity` retains one `TurretRecoil` /
  `BarrelRecoil` pair. The feature needs the two extra vectors, their
  `TravelPerFrame`/`TravelSoFar`/`State`/`TravelFramesLeft` fields initialized
  from the type's anim data, their own snapshot and `world_hash` words, and a
  retained burst index — the same trigger that keeps
  `Ext/Techno/Hooks.Firing.cpp`'s origin hook unported.
- **The keys are read from the wrong section.** Native reads them from the
  **art** INI section (`INI_Art`, `pThis->ImageFile`) like every other
  `UnitTypeExt` art key; VERA's `ObjectType::from_ini_section` reads the rule
  section, because `ArtEntry` (`src/rules/art_data.rs`) does not carry them
  yet. The correct owner is `RuleSet::project_art_data`
  (`src/rules/ruleset.rs:11705`), which already resolves `art_key =
  obj.image` else `obj.id` and merges `QueueingCell`/`DockingOffset`; that
  merge is building-only today. Trigger: a type that puts
  `ExtraTurretCount=` in art.ini, or one whose `Image=` renames its art
  section — neither reads here.

## Tests

- `src/rules/phobos_turret_barrel_tests.rs` — the constructor defaults, both
  count clamps, the indexed loops in index order, the `CoordStruct` triples,
  the gated `BurstPerTurret=`, and the `BarrelOverTurret=` reset.
