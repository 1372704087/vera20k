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

### Movement, crash, ejection, garrison and lifecycle

- #2934 (NPExt changelog item 4): the `MakeInfantry=` anim end
  (`AnimClass::AI @ 0x00423AC0`, refused-Unlimbo branch `0x00424B23`) frees
  the failed creation instead of leaving it in limbo. Natively a Genetic
  Mutator over an occupied cell allocated a fresh brute on every completed
  frame and never destroyed the refused one — unbounded object growth while
  the cell stayed blocked. The anim still steps its stage back and retries.

### Capture, mind control, lift and retaliation

### UI, presentation, audio and scenario lifetime
