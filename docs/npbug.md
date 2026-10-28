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

### Weapon, wave and projectile source, path and range

- #2944 (NPExt changelog item 14): a wave remembers the slot whose FireAt
  emitted it. Native re-reads GetFLH(0) (`0x00762AF0`) for the geometry and
  GetWeapon(0) for the sonic cell damage, so a non-primary emitter anchored
  its visuals and paid the primary's AmbientDamage/warhead. Under npbug both
  follow the emitting weapon.

### Building placement, construction, economy and sensors

### Movement, crash, ejection, garrison and lifecycle

### Capture, mind control, lift and retaliation

### UI, presentation, audio and scenario lifetime
