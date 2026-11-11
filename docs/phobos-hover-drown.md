# Hover vehicle drown death (`HoverDrownable`)

Whether a hover Unit that shut down over water dies when it lands. The type's
`Nullable<bool>` overrides the global, and the landing arm picks the damage
packet from it.

## Keys

| Key | Type | Default |
|---|---|---|
| `HoverDrownable=` | `Nullable<bool>` | unset |
| `[General] HoverDrownable=` | `bool` | `yes` |

Read: `UnitTypeExt::LoadFromINI` (`Ext/UnitType/Body.cpp:87`), declaration
`Ext/UnitType/Body.h:72`, constructor `Body.h:133`. The global is read at
`Ext/Rules/Body.cpp:475`, declared `Ext/Rules/Body.h:385`, constructed `yes` at
`Body.h:890`.

VERA reads the type key in `src/rules/phobos_hover_drown.rs`, owner
`ObjectType::phobos_hover_drown`; the global is
`RuleSet::phobos_globals.hover_drownable`.

## Behaviour

Two halves, both on the same flag.

1. `HoverLocomotionClass_Process` hook `0x514C07`
   (`Ext/Techno/Hooks.cpp:997-1006`) drops the techno as a bomb and then raises
   `TechnoExt::HoverShutdown` (`Ext/Techno/Body.h:81`, saved at
   `Ext/Techno/Body.cpp:1200`, constructed `false` at `Body.h:135`):

   ```cpp
   pTechno->DropAsBomb();
   TechnoExt::Fetch(pTechno)->HoverShutdown = true;
   ```

   VERA's owner is `Simulation::hover_spin_tail`
   (`src/sim/movement/hover_process.rs`): the existing water gate calls
   `drop_as_bomb`, and the `phobos` build then marks the entity
   (`GameEntity::mark_hover_shutdown`, `ObjectFallState::hover_shutdown`).

2. `ObjectClass_Update_FallingDown_ToDead` hook `0x5F4021`
   (`Ext/Techno/Hooks.cpp:1014-1043`) reads and clears the flag inside the
   `IsABomb && IsAlive` arm, before it picks the damage:

   ```cpp
   if (pThis->IsABomb && pThis->IsAlive) {
       const bool hoverShutdown = pExt->HoverShutdown;
       pExt->HoverShutdown = false;
       if (hoverShutdown) {
           if (HoverDrownable.Get(Global()->HoverDrownable)) {
               damage = pThis->Health;            // drown
               ReceiveDamage(&damage, 0, C4Warhead, ...);
               pThis->IsABomb = false;
               return;
           }
           // else falls through to the falling-down damage model
       }
       ...
   }
   ```

   VERA's owner is the landing arm of `Simulation::object_ai_fall`
   (`src/sim/movement/parachute_descent.rs`). The forced C4 receiver is the
   existing stock arm; the `phobos` build only chooses its amount:

   - `HoverDrownable` true — the whole current `Health`, the stock packet;
   - `HoverDrownable` false — the not-drownable arm's whole `Strength`
     (`:1074-1082`, `ratio == FallingDownDamage` with its default `1.0`).

   The flag is hashed while set (`src/sim/world/world_hash.rs`) because it
   gates that amount; a cleared flag leaves the idle hash untouched.

## Residuals

- **`FallingDownDamage` / `.Water` / `.AllowEMP` are unported**
  (`Ext/TechnoType/Body.cpp:1140-1142`, globals `Ext/Rules/Body.cpp:227-228`,
  default `1.0` / `false`). The not-drownable arm is that model
  (`Hooks.cpp:1060-1082`): a blocked landing cell takes the whole health, a
  clear one takes `Strength * ratio` (or `Health * |ratio|` for a negative
  ratio). VERA applies the default ratio only, so a rules set that lowers it
  keeps drowning. `Ext/Techno/Body.h`'s `OnParachuted` half of the same block
  shares that trigger.
- **`HoverDrownable=no` still kills at the default rules**, because the
  default ratio is `1.0` and that packets the whole `Strength`. The key is a
  dependency of `FallingDownDamage`; it only prevents death once that ratio is
  below the health/strength ratio. This is native behaviour, not a VERA
  choice — the page records it because the feature is easy to read as
  "`no` survives".
- **The drop itself is modelled as stock.** Native only reaches `DropAsBomb`
  through the Phobos hook `0x514C07`; VERA's hover water gate predates this
  feature and drops in both builds. The mark is the only `phobos` half.

## Tests

- `src/rules/phobos_hover_drown_tests.rs` — the `Nullable`, the global
  fallback, and the `<default>`/blank reset.
- `src/sim/movement/hover_process.rs` —
  `the_hover_shutdown_marks_the_techno_it_drops`: a spin ending over water
  drops and marks; one ending over clear ground does neither.
- `src/sim/movement/parachute_descent.rs` —
  `only_the_bomb_landing_arm_takes_the_hover_shutdown`: the bomb arm reads and
  clears the flag, a canopy landing never reads it.
