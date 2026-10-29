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

### Building placement, construction, economy and sensors

- #2972 (NPExt changelog item 12): a building sensor array's add and remove
  radii are the unified `SensorsSight=` value. Native added over
  `SensorsSight=` but removed over the signed `CloakRadiusInCells=` byte,
  leaving permanent fringe counts and blind zones around psychic sensors;
  the dormancy of the cloak-reveal deposit arm stays native.

### Movement, crash, ejection, garrison and lifecycle

### Capture, mind control, lift and retaliation

### UI, presentation, audio and scenario lifetime
