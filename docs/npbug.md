# Optional NPBUG bug-fix extension

NPBUG is VERA20K's own opt-in compatibility extension. It collects gameplay
corrections adopted from the NPExt (NPatch Extended) bug-fix changelog: each
chain fixes a defect the original game carries, so enabling NPBUG is a
deliberate, documented deviation from native behavior, not a parity fix.

The extension exists because the reports are compatibility candidates, not
VERA-versus-retail defects — the inventory and its policy live in
[#3024](https://github.com/YuriPlanet/vera20k/issues/3024). Builds without the
feature keep every native path and its pinned goldens.

Enable it explicitly:

```sh
cargo run --release --bin vera20k --features npbug
```

The feature compiles to `rules::NPBUG_ENABLED`. A save or network session
recorded with NPBUG cannot load or join in a build without it, matching the
other capabilities (`RulesCompatibility`).

## Chains

Each implemented fix records its issue, the NPExt changelog item it adopts,
and the native site the defect lives at, next to the Rust that changes it.
The sections follow the behavior chains of #3024. The capability scaffold
adds no behavior yet.

### Rules and repeated INI reads

### Superweapon and AI strategy

### Combat target, order, fire and damage eligibility

- #2985 (NPExt changelog item 77): the T41 land refusal
  (`0x006FC7EB..0x006FC868`) also applies to a terrain object on land — a
  tree is an Object target, so the Cell-only `LandTargeting=1` check never
  reached it and submerged naval units could shell shore trees.

- #2982 (NPExt changelog item 66): a garrison kill's veterancy pays the
  shooter — the rotation slot one step back from the post-advance fire
  index — not the next occupant in line. Native reads the slot after
  `FireAt` advanced it (`0x006FF031..0x006FF085`), so promotion landed on
  the wrong soldier; occupants leaving between shot and kill stay the
  recorded stale-slot residual.

### Weapon, wave and projectile source, path and range

### Building placement, construction, economy and sensors

### Movement, crash, ejection, garrison and lifecycle

### Capture, mind control, lift and retaliation

### UI, presentation, audio and scenario lifetime
