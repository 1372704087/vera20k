# Stock radiation overlap arithmetic

`radiation_damage_boundary.py --overlap` extends the existing reader/map/Foot
comparison. Its default six-row corpus is unchanged. The new
`radiation_overlap.json` stores twelve short field histories and three populated
DOG receiver controls; no bulk replay or full retail files are committed.

For a native replay, set `VERA20K_RADIATION_INPUTS` to an extracted retail folder
containing `RULESMD.INI`, optional `LANGRULE.INI`, `MPBattleMD.ini` and `Hills.mmx`.
The existing populated Infantry owner also needs `VERA20K_SHRAPNEL_INPUTS` and
`VERA20K_ANYTOWN_INPUTS` as described in
[the Foot mission comparison](anytown_damage/foot_missions.md). Then run:

```sh
PYTHONPATH=. python -m tools.spatial_oracle.radiation_damage_boundary --overlap --check
PYTHONPATH=. python -m tools.spatial_oracle.radiation_damage_boundary --check
```

The sidecar records the actual executable and replay command. `layers` retains
only selected lexical inputs and source identities. Original readers execute
for RadSite Verses, Radiation fields, RadEruptionWeapon RadLevel and
RadEruptionWarhead CellSpread. The populated DOG control independently executes
its full type reader in the existing Battle/Anytown context; the selected stock
DOG/RadSite values agree with the field's Battle/Hills inputs.

The stock seven-cell separation exposes a real integer boundary. Sites centered
at `(17,24)`, `(31,24)`, `(24,17)` each contribute to `(24,24)`. Native stored sums
are `4064d55555555555`, `4074d55555555555`, `407f3fffffffffff`; the last value is
499.99999999999994. Cell487CB0 returns499 and Foot4DA604 emits99, whereas a
nearest-rounded three-way sum becomes500. The separate full Infantry517FA0 ->
Foot4D7330 -> Techno701900 -> Object5F5390 chain leaves newly constructed stock
DOGs at67,34,1 health for the33,66,99 packets. They remain alive/out of limbo;
all three receiver controls leave the tracked RNG streams unchanged.

## Execution and boundaries

Field construction uses original RadSite65B1E0, SetSpread65B4D0 and
SetRadLevel65B4F0. Activation65B580..65B74C runs the real timer/step/scalar stores;
LightSource construction and drawing are excluded, then original65B9C0 spreads
the field. Radius2688, level500, duration/remaining500 and level-steps5 are native
outputs. The inactive timer's scratch TimeLeft word is deliberately omitted.

Same-center refresh executes65B530..65B570, including actual effective-level
conversion and Decrease65BB50, before activation/spread. `tick_to90` executes the
original65B800..65B858 lifetime/level-timer prefix each frame; it reaches the
first Decay65BD00 and resets the timer. Independent light-timer and final
site-deletion suffixes are excluded.

Creation and iteration order are different: constructor65B27E..65B293 appends
to the RadSite vector; Logic55B5CD..55B5E8 visits that vector backwards before
ordinary objects. Field controls read the actual constructor-populated vector
and use that reverse order. Each stage exposes `update_order`. Reversed creation
histories retain the same sites while checking the resulting field bits. They
are explicit numeric histories, not a captured complete Logic frame.

Each stage's `samples` holds native field bits, Cell damaging level and the
original Foot factor fragment's base damage. `source_inputs` holds native-read
level/spread/timing/factor values. The field row's final Object receiver is the
old neutral300HP control; use `concrete_receivers` for the populated DOG result.
No wrapper invents an expected arithmetic value. The per-site `current_level`
executes the signed wrapping integer getter65B510 used by deployed Guard. The
separate `large_level_merge` control sets positive RadLevel50000 and exercises
65B530's float multiply/divide before adding the second50000. This prevents
reusing the wrapping getter for the merge; the two native bodies differ.
The extra `(22,29)` sample measures the five-by-five diagonal from `(17,24)`.

AbstractCtor410170, called by RadSiteCtor65B1E3, stores UniqueID=-1 at410191
and contains no calls or Scenario counter access. RadSiteCtor then appends its
own registry entry; it does not call AbstractCreateID410230. This establishes
no ID allocation at these constructors only, not a full RadSite identity or
lifecycle audit.

The two zero-step direct-kernel controls explicitly override RadLevelDelay to600
while level/duration remain500. They call Decay/Decrease directly; they do not
claim a stock site lives until that timer. Positive contributions divide by zero
and subtract to zero; outside-radius0/0 yields negative quiet NaN
`fff8000000000000`, which CellSubtract487D00 retains through its unordered
comparison. Cell487CB0 returns0 there. Only raw bits are serialized.

The native comparisons establish these numeric and receiver boundaries. Rust
production tests must separately exercise real DESO Guard/Bullet creation,
Foot application order, persistence and later cleanup. They do not establish
whole RadSite lighting, all layer/default behavior, complete native startup or
all BulletClass behavior.
