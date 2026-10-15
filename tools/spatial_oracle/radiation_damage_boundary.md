# Radiation regression after the Verses correction

The old `rad_damage_fires_on_application_delay_boundary_only` assertion expected
294 heavy-unit health. Original execution establishes **295** for the selected
fixture. The test now compares independent native reader bits, application
admission and final health. It does not claim full radiation parity.

```sh
PYTHONPATH=. python tools/spatial_oracle/radiation_damage_boundary.py --check
```

The six rows cross frames 15/16/17 with center infantry armor0 and adjacent
vehicle armor5. They use the existing test's synthetic Desolator-shaped rules,
including its exact eleven-token RadSite Verses string, `.2` factor, delay16,
cap500 and level500/spread2. This is not a retail-layer export. Metadata pins
the original executable SHA-256 and all substitutions.

Original Warhead constructor and selected reader blocks execute, including
`75DDCC..75DE5A` for Verses. The complete Radiation reader `66CF70` executes
against supplied lexical INI indexes. Original `65B4D0` and `65B4F0` set radius,
level and duration; full `65B9C0` spreads into 25 supplied flat cells through
original map lookups and Cell vtable getters. `Foot4DA554` executes the actual
frame modulo, immunity/air/limbo gates, Cell `487CB0` cap/truncate and factor
conversion through `4DA629`, where concrete ReceiveDamage would be called.
The captured ABI is checked: distance0, selected warhead, null source/house,
ignore-defenses false and arg6 true.

The captured base damage independently enters original kernel `489180` and
original Object receiver `5F5390`. Actual Infantry/Unit object and type vtables
are retained. The intervening concrete Infantry/Unit/Techno receiver bodies
and their defender modifiers are outside this comparison. The fixture uses
fresh unranked targets; this does not prove those omitted gates. Site AI,
decay/light scheduling and a whole native elapsed-frame simulation are also
outside scope. Frames are independent fixtures. No RNG calls or death/detach
calls occur; the original level setter stores duration/remaining500.

| Native result | Center | Adjacent heavy unit |
| --- | ---: | ---: |
| Stored cell level | 500 | 299.99999999999994 |
| Truncated damaging level | 500 | 299 |
| Foot base damage | 100 | 59 |
| Object damage / resulting health | 100 / 200 | 5 / 295 |

Frames15/17 skip application. The original percentage reader stores the heavy
10% value as bits `3fb9999999999999`. Its correction in the existing Verses
owner changes the Rust test's former 6-damage result to 5. The old expected
value and its arithmetic-only comment were therefore incorrect. The bridge
HE landing corpus and its hashes are unchanged.

There is a **required separate radiation precision chain**: current Rust spread
uses host arithmetic and stores300 at this adjacent cell, then emits base60;
original PC53/chop spread stores the value above and emits59. Original
RadLevelFactor ReadDouble stores widened binary32 bits `3fc99999a0000000`;
the shared `ini_value::parse_read_double` now reproduces that conversion and
`RadiationRules` uses it. The sidecar's direct-binary64 parser limitation
describes the generator-era Rust implementation, not the current reader.
Both selected paths reach the same final5 damage with corrected heavy Verses,
so this regression checks that bounded outcome without asserting matching
intermediates. Other armors, factors, cell distances, heights and overlapping
sites can expose different integer damage or later death/occupation timing.
That conditional trigger, its frequency across irradiated cells, and its
bridge height/geometry consumers remain in the whole-bridge audit; this small
regression correction does not complete their migration.

The Rust regression runs through the production frame and each Foot's own
radiation visit. Separate retail DESO tests cover field creation relative to
earlier/later Logic visits, pre-movement sampling and fatal receiver ordering.
Those tests do not extend this native corpus's numeric coverage.
