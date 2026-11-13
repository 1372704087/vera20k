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
`AnimationLength` were already read; this page covers the `DestroyAnim` list
(`ValueableVector`, one registered entry picked per removal) and the consumer
lifecycle.

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

- `rules/terrain_object_type.rs` - the `DestroyAnim` list
  (`ValueableVector<AnimTypeClass*>`, one registered entry picked per removal,
  `Ext/Anim/Body.cpp:407-412`) and the per-pass `TerrainTypeExt` read state;
  the effect asset catalog binds every listed anim.
- `sim/terrain_object.rs` - `TerrainObjectState::is_crumbling` and the store's
  `set_crumbling`; `TerrainObjectStore::start_crumble_animation` seeds or
  rewinds the Stage at rate 2.
- `sim/world/lifecycle.rs` - `terrain_start_crumble` (IsCrumbling, Stage
  restart, Logic join and `CrumblingSound`) and `terrain_crumble_finish`.
- `sim/combat/world_receiver.rs` - the lethal transaction reads
  `HasCrumblingFrames=` after DetachAll and the nested C4; a crumble skips
  `PlayDestroyEffects` and the same-call UnInit, every other type plays
  `terrain_play_destroy_effects` first.
- `sim/terrain_spawn.rs` - `tick_terrain_crumble` (the `Frames/2` fold, destroy
  effects and removal) and `terrain_play_destroy_effects`
  (`DestroySound` plus one random `DestroyAnim`).
- `app/presentation/instances/overlays.rs` - `terrain_draw_frame` selects
  `1 + Animation.Value` for a static crumbling type and
  `AnimationLength*(HasDamagedFrames+1) + 1 + Animation.Value` for an animated
  one; the simulation owns the Stage, not the renderer.

## Tests

`sim::terrain_spawn::tests::phobos_crumbling_tree_keeps_alive_then_destroys_and_removes`
drives the AI: survival with Logic and `CrumblingSound`, then removal on the
visit whose Stage step reaches `Frames/2 - 1` with `DestroySound` and one
random `DestroyAnim`.
`a_lethal_area_hit_routes_crumbling_and_plain_trees_through_the_receiver`
drives the production area receiver: a crumbling tree stays live and leaves
the transaction's finalizing set, a plain tree finalizes in the call, and
(under Ares) an on-fire tree skips Start_To_Crumble.
`app::presentation::instances::terrain_render_tests::phobos_terrain_draw_frame_follows_type_keys_and_crumbling`
pins both draw folds and the unchanged stock selector.

The receiver-path test, the on-fire skip and the step-before-fold order were
each mutated red and restored.

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
- The draw's damaged test reads the stock `[General] ConditionYellow=`; the
  Phobos `[AudioVisual] ConditionYellow.Terrain=` override
  (`Ext/Rules/Body.cpp`, used at `0x71C1FE`) is not read yet.
- The anim asset catalog binds every authored `DestroyAnim` token; a token
  that resolves to no AnimType is dropped at the random pick (the native
  `ValueableVector` parser drops it earlier), so the draw range and RNG
  stream match but an unresolved name can produce a load warning.
- An animated crumbling type with no `AnimationLength` derives
  `Frames/(2*(HasDamagedFrames+1))`, which makes `currentStage + 1 == Frames/2`
  unreachable at value 0; native never removes it either. Author
  `AnimationLength` to terminate an animated crumble.
