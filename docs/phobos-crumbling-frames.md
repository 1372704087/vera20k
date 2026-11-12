# Terrain crumbling frames

Build with `--features phobos` to let a terrain object play a crumbling stage
after a lethal hit instead of vanishing in the same call. Stock gamemd removes
a destroyed tree immediately; with `HasCrumblingFrames=yes` the object stays
live at zero health, draws a crumbling animation, and is removed only when the
stage reaches half the SHP's frames.

```ini
[SOMETREE]                          ; TerrainType, rulesmd.ini
HasCrumblingFrames=no               ; boolean, default no
CrumblingSound=                     ; sound name, played when crumbling starts
DestroyAnim=                        ; anim list, one random pick at removal
DestroySound=                       ; sound name, played at removal
HasDamagedFrames=no                 ; boolean, the SHP holds a damaged half
AnimationLength=                    ; integer, unset = Frames / (2*(HasDamagedFrames+1))
```

`HasCrumblingFrames`, `CrumblingSound`, `DestroySound`, `HasDamagedFrames` and
`AnimationLength` were already read; this chain adds the `DestroyAnim` list and
the whole consumer lifecycle.

## Behavior

A lethal hit on a `HasCrumblingFrames` type (`TerrainClass_TakeDamage_NowDead_Add`,
`0x71BB2C`, `Ext/TerrainType/Hooks.cpp:286-307`) skips `PlayDestroyEffects` and
the same-call UnInit: it joins the object to Logic, plays `CrumblingSound` and
restarts its Stage at rate 2 / value 0 (`Start_To_Crumble @ 0x71C8E0`). The
object keeps health 0 and stays drawn.

Each Logic visit while crumbling (`TerrainClass_AI_Crumbling`, `0x71C812`,
`Hooks.cpp:65-95`) steps the Stage and folds
`currentStage = Animation.Value + (IsAnimated ? AnimationLength*(HasDamagedFrames+1) : HasDamagedFrames)`.
When `currentStage + 1 == GetImage()->Frames / 2` it plays
`PlayDestroyEffects` (`DestroySound` at the coords plus one random registered
`DestroyAnim`, `Ext/TerrainType/Body.cpp:15-21`) and removes the object. The
`HasDamagedFrames && Health > 0` arm instead clears `IsCrumbling` (and leaves
Logic when the type is neither animated nor flammable), resuming the ordinary
draw.

The draw (`TerrainClass_Draw_PickFrame`, `0x71C1FE`, `Hooks.cpp:96-126`) selects
`1 + Animation.Value` for a non-animated crumbling type and
`AnimationLength*(HasDamagedFrames+1) + 1 + Animation.Value` for an animated one;
the stock stage / `health < 2` arms are unchanged when the flag is off.

Retail leaves `HasCrumblingFrames` unset, so stock removal is bit-for-bit
unchanged.

## Where VERA20K owns it

- `rules/terrain_object_type.rs` - the `DestroyAnim` list (bound through the
  anim asset catalog) and the shared `crumbling_animation_length` fold.
- `sim/terrain_object.rs` - `TerrainObjectState::is_crumbling` and the store's
  `begin_crumbling` / `end_crumbling`; `receive_damage` returns a `Crumbling`
  result instead of finalizing when the flag is set.
- `sim/combat/world_receiver.rs` - the lethal transaction hands a `Crumbling`
  result to `start_terrain_crumbling` after DetachAll and the nested C4.
- `sim/terrain_spawn.rs` - `start_terrain_crumbling` (Logic join + Stage restart
  + `CrumblingSound`), `tick_terrain_crumbling` (the `Frames/2` fold, removal and
  `PlayDestroyEffects`) and the Stage `update` half shared with the spawner.
- `app/presentation/instances/overlays.rs` - `terrain_crumbling_frame` selects
  the crumbling draw frame; the simulation owns the Stage, not the renderer.

## Tests
`sim::terrain_spawn::tests` drives the production receiver and AI:
`a_crumbling_type_keeps_the_object_and_joins_logic_on_a_lethal_hit` (survival +
Logic + `CrumblingSound`, with the stock control finalizing in the same call),
`the_crumbling_ai_removes_at_half_frames_with_destroy_effects` (removal at
`Frames/2` with `DestroySound` and one random `DestroyAnim`) and
`an_animated_crumbling_type_uses_the_animation_length_override`.
`app::presentation::instances::terrain_render_tests::terrain_crumbling_frame_selects_the_crumbling_half`
pins both draw folds and the unchanged stock selector.

## Residuals

- The two non-lethal crumbling triggers are not ported: `TerrainClass_FireOut_Crumbling`
  (`0x71C6EE`, `Hooks.cpp:239-255`) starts crumbling when a flammable object
  fires out, and `TerrainClass_TakeDamage_RefreshDamageFrame` (`0x71B98B`,
  `Hooks.cpp:271-284`) marks `IsCrumbling` on a non-animated `HasDamagedFrames`
  type crossing `ConditionYellow` downward. VERA starts crumbling only on the
  lethal arm; a mod that relies on fire-out or the yellow-crossing redraw will
  not crumble there. Trigger: those two paths with `HasCrumblingFrames=yes`.
- `Mark(Change)` / `Disappear(true)` and the dirty-rect registration in
  `TerrainTypeExt::Remove` are presentation bookkeeping; VERA redraws every
  frame and retires through the common non-entity owner, so no separate dirty
  area is registered.
- An animated crumbling type with no `AnimationLength` derives
  `Frames/(2*(HasDamagedFrames+1))`, which makes `currentStage + 1 == Frames/2`
  unreachable at value 0; native never removes it either. Author
  `AnimationLength` to terminate an animated crumble.
