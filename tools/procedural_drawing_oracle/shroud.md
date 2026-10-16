# Ordinary tactical shroud ABuffer evidence

`shroud.py/json/meta.json` execute the original reset, SHP access, cell-edge
selection and tactical ABuffer producer against physical stock `SHROUD.SHP`.
The sidecar records executable, emulator, source and payload identities. This
is the ordinary `FogOfWar=no`, empty-AlphaShape-list route; it does not establish
all vision lifecycle state or the wider fog/AlphaShape mechanisms.

```sh
PYTHONDONTWRITEBYTECODE=1 python -m tools.procedural_drawing_oracle.shroud --check
```

Use the executable/environment described in [`native_oracle.md`](../native_oracle.md).
The existing `rally.Rally` owns Surface/ABuffer setup and `stock.mix` owns retail
archive extraction. There is no second rasterizer producing the golden pixels.

## Concrete prerequisite correction

Stock `ra2.mix/conquer.mix/SHROUD.SHP` has SHA256
`9818f7e2685fb1e8ed75ceff568aaf5d36350c747cbd4f30856458bba12e4964`, canvas60x30
and96 frames. The selector uses frames0..46. Each of these47 frames has exactly
the same900-pixel nontransparent diamond; their only other pixels are254(skip).
Frame15's drawn pixels are **2**, not0. Frame0's drawn pixels are127.

Original `47EFE0` writes each source byte directly as a u16 A value except254,
which preserves the destination. Original `4801F0` chooses frame15 when
`6D8700` returns-2. Executed flags `Cell+0x12C=0` produce cache-2/frame15 and
900 A=2 pixels. A generated black diamond therefore changes native rally pass
admission: `4C0750` distinguishes A=0 from A!=0. For ordinary palette blitters,
small nonzero A can still choose a black palette row; it is not interchangeable
with A=0 for procedural consumers.

The prior Rust diamond's scan intervals match frame15's900-pixel mask exactly.
Row0 is empty (the prior comment claiming an added tip was incorrect); rows1..15
expand4,8,...,60 pixels and rows16..29 contract56,...,4. Reuse the loaded stock
frame15 in the existing CPU frame blitter and remove the generated substitute.
That retains one asset and drawing owner and also respects replacement assets.

## Executed coverage and schema

- `leaf_cases`:73 original47EFE0 executions: all47 selected stock frames,
  left/top/right/bottom/offscreen origins, four interior clips and two circular
  storage offsets. `input.point` is the native top-left frame origin;
  `input.clip` defaults to `[0,0,160,120]`. `frame_rect` is returned by original
  69E7E0. `alpha_hex` contains19200 logical row-major A bytes, without padding.
- `selector_cases`: all256 neighbor masks with center flags0x18 through whole
  4801F0/6D8700. `caches` are signed Cell+120/+121 bytes; `frame` is the actual
  47EFE0 argument. Sixteen `flag_controls` cover0x00,0x08,0x10,0x18 with masks
  0x00,0x01,0xAA,0xFF. The two bitmask domains remain distinct from Rust's compact
  presentation bitmap.
- `scenes`: nine whole6D3660 executions. Full redraw reaches6D71E0; dirty-cell
  cases reach480A30/6D1F10. Cases cover clear, unrevealed, a revealed factory
  frontier, an island, integer camera changes, circular storage wrap, flat and
  raised dirty cells, and a separately bounded partial0x10 flag state.
- Every reset runs original4112D0 and asserts all A values become127. SHP rect,
  SHP data, clipping, circular-row access and pixel stores all execute original
  instructions. Only platform Surface storage comes from the existing native
  BSurface fixture; there are no function-result substitutions.
- The factory-frontier scene feeds the resulting original ABuffer to original
  rally6DA9D0/4C0750 and records `rally_passes`. Factory/selection/color fields
  are the existing rally fixture inputs, not a complete Scenario constructor.

Production tests should load the SHA-identified asset through `AssetManager`,
`ShpFile` and `extract_shp_brightness`, then compare the shared CPU frame/rebuild
owner's bytes to this corpus. Synthetic GPU tests alone do not establish these
A inputs. Container hashes are in `stock`; this oracle reads the named physical
archive, not a complete native archive/loose-file override resolver.

## Projection, ordering and clipping

Dirty-cell6D3660 calls GetCenter480A30, forces Z=0 at6D36C0, projects with6D1F10,
subtracts integer camera+0xB0/+0xB4 and centers the frame by(-30,-15). Full scan
6D71E0 executes the original inverse-matrix constructor values and projection.
The same Cell10,20 at native camera[-320,440] is drawn at[-10,10]; the raised
level8 dirty control has identical A bytes to level0. Rust's existing world
coordinate convention adds15 to camera Y, so its matching camera is[-320,455].
Fractional camera/zoom behavior is a separate presentation extension.

The native full scan walks diagonal screen rows; dirty cells use queue order.
Rust walks `ry` then `rx`. For all47 selected stock frames, the common mask has
zero intersections with its lattice neighbors at offsets(30,15),(-30,15),
(0,30),(60,0). Thus the order cannot change this ordinary shroud-only result.
Every full scene also replays the exact original per-cell calls through
4801F0/47EFE0 in Rust row order and with full-plane clips; byte equality is
asserted and recorded as `row_major_full_plane_replay_equal`. This establishes
that native per-cell clipped rectangles and full-plane CPU blits agree for
these cases. It does not establish fog/AlphaShape blend ordering.

Frame0 writes127 instead of being empty. Skipping it is equivalent only after
a full127 reset with the established disjoint stock masks. Incremental native
redraws must still execute the write; this is not a general frame0 skip rule.

## Remaining boundaries

The partial `0x10` state chooses frame0 even though bit0x08 is clear. It is saved
as a raw consumer boundary, not claimed to be an ordinary reachable state in
this chain. Existing `shroud_current_sight` evidence establishes selected
counter/open/pending transitions but explicitly excludes complete edge-cache
outputs. Its shared FogState/vision owners remain the lifecycle authority.

Dynamic AlphaShape `420F40/421350` can further alter A; enabled fog uses47F250
and FOG.SHP under the scenario0x1000 gate. Those writers, incremental dirty
invalidation, runtime scroll production, map-edge dummy state, mod assets and
full reveal traversal are outside this ordinary-input corpus. No full-game or
whole procedural-drawing closure follows from these vectors alone.

## Selected ordinary building brackets

The additive `--building-brackets` mode writes/checks
`building_brackets.json` and its provenance sidecar. It reuses
`HutTypeReader` for original construction and INI caches,
`action_lines.physical_palette` / `PaletteReader` for physical palette startup,
and `building_prism.LaserPixels` → `rally.Rally` for the original packed Surface
kernel. It creates no second VM, INI reader, projection or line rasterizer.

```sh
PYTHONDONTWRITEBYTECODE=1 python -m tools.procedural_drawing_oracle.shroud --building-brackets --check
```

The installed original archives beside the configured `gamemd.exe` supply
`expandmd01.mix/RULESMD.INI`, optional loose `LANGRULE.INI`,
`ra2md.mix/localmd.mix/MPBattleMD.ini`, and `multimd.mix/XMP03T4.MAP`, in that
order. Native HasSection `526810` admits the selected `GAPOWR` sections before
original Image caller `5F92F9..5F9340` runs. Fixed
`ra2md.mix/localmd.mix/ARTMD.INI` supplies Height/OccupyHeight and Foundation to
`4610D8..461112` and `461225..46125D`; it is not a rules layer. Existing lexical
extraction supplies strings, and the original parsers/defaults/stores execute.
Every layer's identity, selected keys and actual reads are in `geometry.readers`.
This is focused field loading, not full Scenario or type-file loading.

The complete original BuildingType constructor `45DD90` yields Height2 and
Foundation0; these physical reads yield Height4, Foundation3. Original
Dimension2 `464AF0` returns `[512,512,416]` with the explicitly supplied native
startup height factor104. Original palette initialization consumes physical
`ra2.mix/cache.mix/ANIM.PAL` and `PALETTE.PAL`. Index15 of global `87F6C4`'s
middle lookup row is `0xFFFF`; the packed color is a native palette result.

Original complete DrawBehind `6F60D0` and DrawExtras `6F5190` prefix execute on
prepared selected GAPOWR state. Extras stops at `6F5E17` before health drawing.
Original GetCoords, pair helper `6F5EF0` and Tactical `6DBB60` produce the saved
world endpoints, screen endpoints and paired Z adjustments. GetHeight
`5F5F40` returns supplied ground-relative0. Location `[2688,5248,0]`, native
camera `[-384,376]` and viewport160×160 keep the full cuboid in the supplied
crop; points are not translated after projection. `geometry.passes` contains
10 `behind` and11 `extras` commands. Deselected produces0+0; the prepared
sinking gate produces10+0, without claiming that ordinary GAPOWR can sink.

Each captured seven-argument Surface call crosses a declared recording
boundary into the existing pixel runtime. Original `4BFD30` then executes its
clipping, three-axis walk, strict unsigned Z admission, A sampling, RGB565
modulation and color stores. Every command has writeZ0. The four
`raster_controls` are:

- `clear`: A127 and Z65535, producing315 ordered writes at311 distinct pixels.
- `mixed_a`: existing Rally's `(0,1,63,127,255)` pattern, producing270 writes at
  266 pixels. A0 skips; A1 writes black; A63 writes `0x7BEF`; A127 writes
  `0xFFFF`; A255 writes `0xEFBD`. The native modulation does not clamp A to127.
- `occluding_z`: A127 with supplied Z32700 in rectangle `[40,40,120,120]` and
  Z65535 elsewhere, producing270 writes at266 pixels. This synthetic depth
  control establishes consumer occlusion, not native building-body occlusion.
- `native_ground_body_z`: A127 with `input.native_z_runs` from actual original
  flat ClearTile/TMP and GAPOWR body/shadow execution. Runs are
  `[x,y,length,u16]`; omitted positions retain `input.z` (65535). All12 lower
  commands fail native Z admission on this flat control, leaving132 roof
  pixels. This establishes ordinary terrain occlusion, without claiming that
  12 visible stubs were missing from the prior presentation.

`stores` preserves original write order and repeated pixels as `[x,y,word]`;
`pixels` gives final changed words in row-major order. Each segment identifies
its pass/index and half-open `store_range`. Every execution checks unchanged
original code, A and Z, pixel guards, and no observed raw/ranged RNG entry.
The Rust producer and shared packed-line consumer consume these native values.

`ground_occlusion` describes the fourth control's upstream native depth. The
existing TMP/PaletteReader runtime owns physical archive, lazy TMP, palette,
lighting and Surface setup. Original BuildingType45DD90/45FE50 consume layered
physical rules and fixed ART, resolving physical `GGPOWR.SHP`; original45E8F0
loads/remaps physical `BUILDNGZ.SHA`. Native GAPOWR BibShape isNULL. The existing
`color_scheme_palette` owner constructs LightGold in that runtime. Native
Building43B740 and idle/frame routines execute with explicit placed-state,
local House, flat/revealed Cell, A127 and camera priors; active Anim slots are
empty. Placement itself and native Scenario/selection acquisition do not run.

The actual `+104` Building admission43CEA0 executes its native455C20 bounds
and virtual `+AC`459EF0 render coordinates, projecting body point`[84,74]`.
This differs from bracket GetCoords center`[84,104]`: render coordinates are
Location minus`[128,128,0]`. Whole43D290/705E00/4AED70 draw physical body frame0
and shadow frame3, consuming BUILDNGZ. Behind and initial Extras precede this
body; a repeated pre-health Extras prefix follows it. Per-command store
counts, original shape arguments and full crop color/Z hashes are retained;
the final Z plane crosses to the packed-line control as compact native row
runs. Complete color planes and screenshots are excluded. This consumer test
uses supplied native depth; it is not a GPU body-renderer parity claim.

The corpus does not execute placement, the whole Display loop, owned active
Anim rendering, native shroud/AlphaShapes, or a whole GPU frame. Native callers
place behind and initial Extras inside each Building parent's slot before
that parent's body, then invoke Extras again in a separate late sweep. The
two command lists alone do not establish that ordering or intervening body
composition. The bounded original ground/body composition supplies one
occlusion control. Complete app output, nearby overlap and whole-frame order
remain validation gates; the synthetic Z rectangle cannot close them.

## Ordinary selected mobile health strip

The independent additive `--unit-health` mode uses one existing
`bridge_render_inputs_palette.PaletteReader` runtime and the shared
`bridge_render_shape.prepare_surface` BSurface/A/Z storage. It executes
unchanged complete original DrawExtras6F5190, DrawHealth6F64A0 and their
ordinary PipScaleNone709A90/70AA60 tail to normal return. Every physical SHP
call4AED70 executes frame access, centering, clipping, picker490B90, rowwalker
and actual pixel leaves; there is no argument-sink substitution in this mode.

```sh
PYTHONDONTWRITEBYTECODE=1 python -m tools.procedural_drawing_oracle.shroud --unit-health --check
```

`unit_health.json` and its sidecar retain the original executable identity and
command. Normal STOCK_DIGITAL production asset lookup independently resolved
PIPS.SHP to `ra2md.mix/conqmd.mix` (21frames,1920bytes,
SHA256`7525b25f80b5b67e9a5c2977b7624c4b3ce7ad9ce332169c4622eddb91c61486`),
PIPBRD.SHP to `ra2.mix/conquer.mix` (272bytes,
SHA256`8da08a64beb8c7916a3ae51fc7239d9474ef3366e73e78e893e24e31c119812e`),
and PALETTE.PAL to `ra2.mix/cache.mix` (768bytes,
SHA256`e85c535002573b5f95f7c361c4bdb95ee43e4914ddc926afda9ace995c54242b`).
Loose/expansion/language overrides were excluded by that production lookup;
the oracle then asserts these same immutable archive-byte identities. It does
not emulate original archive search. The older19frame RA2 PIPS is excluded.
ANIM.PAL is an inherited palette-startup input; health uses only the original
plain PALETTE.PAL Convert87F6C4. Full palette/LUT construction executes.

Whole Rules constructor665650 supplies both Condition thresholds0.5; whole
UnitType7470D0 supplies Strength0, PixelSelectionBracketDelta0 and PipScale0.
Original selected scalar/enum caller/read/store ranges consume exact lexical
RULESMD, optional LANGRULE, MPBattleMD and XMP03T4 layers. The result is
Strength300, delta0, PipScaleNone0, ConditionYellow0.5 and ConditionRed0.25.
These are rules keys, not ART. Physical MTNK leaves SpawnsNumber, Passengers
and Organic unset; their actual constructor values remain the ordinary-tail
inputs. No Python numeric parse supplies these values.

Prepared original Unit table/type/House linkage, rookie state, no group,
selfheal, cargo or managers and supplied HP/selection/hover/sinking are
consumer boundaries. Original5F65A0/6D2140 project actor XYZ through native
camera`[-380,385]` to point`[80,80]`; the supplied overlap actor projects to
`[84,80]`. This is not a native Unit lifecycle, occupancy, damage, Display loop,
MTNK VXL body or shroud-generation execution. The two nearby points prove
component draw order; legal dense-unit placement is not claimed.

The `raster_controls` share explicit input size/background/A/Z/clip and actors.
Each `draws` row has actual asset/frame/point/flags, native leaf/vtable and
half-open `store_range` into ordered `[x,y,u16]` stores. `pixels` contains final
changed RGB565 words. The existing producer and GPU consumers use these native
values; no second health/count/raster implementation supplies goldens.

Selected HP300/150/75/1 yields17/8/4/1 filled pips, using16/17/18 at actual
native threshold comparisons.151/76 distinguish the just-above arms. Selected
PIPBRD0 uses flagsE00, leaf493DF0/vtable7E57E0 and consumes144A samples without
Z. PIPS uses flags600, leaf4912B0/vtable7E5A20, bypassing both A and Z. Hover
omits the border; absent/sinking controls emit no shapes. Original ordinary
709A90 and70AA60 execute, with no additional shape calls for these priors.

A127, partial63, stock unrevealedA2 and explicitA0 controls preserve the fill
colors while changing the border. Existing original shroud evidence establishes
A2 from physical frame15; this mode supplies that downstream A plane rather
than rerunning its writer. Z0 over the entire strip has identical color output
to Z65535: both shapes bypass Z and leave it unchanged. One left-edge clipped
control exercises original shape clipping; one ordered two-parent overlap
retains each parent's border→fills before the next parent's border→fills.

Original code/vtables, prepared actor bytes, A/Z and buffer guards stay
unchanged; no raw/ranged RNG entry is observed. Commands are frame-local.
Selection/hover acquisition, fog/AlphaShapes, active selfheal flashes,
rank/status/control-group text, other PipScales/classes and whole DrawExtras
closure remain outside this ordinary selected rookie MTNK evidence. Production
GPU readback and normal game captures remain separate validation.

## Ordinary Building health and actual body composition

The additive `--building-health` mode extends the same health generator and
recorder as `--unit-health`. It borrows the existing bracket ground/body
initializer's already initialized `tmp_clear_render`/`PaletteReader`; it creates
no second VM, palette, SHP decoder, INI parser, rasterizer or body mask.

```sh
PYTHONDONTWRITEBYTECODE=1 python -m tools.procedural_drawing_oracle.shroud --building-health --check
```

`building_health.json` has two explicit comparison boundaries:

- `raster_controls` share the unit-health input/draw/pixel schema. Inputs use raw
  Object Location XYZ; `actor_results.coords` and `.point` come from original
  Building GetCoords447AC0 and Tactical6D2140. Positive selected rows enter the
  complete health function6F64A0 with third argument0. Hover-only, absent and
  sinking negatives enter complete Extras6F5190 instead, preserving its caller
  gates without adding selected bracket pixels to a health-only comparison.
  The exact native entry is recorded per actor. Health, selection, hover and
  sinking are supplied priors, not lifecycle proof.
- `composition_controls` execute complete Behind, complete first Extras, actual
  admitted GAPOWR body, and complete late Extras, with a second control adding
  one earlier unselected body and its empty Extras. Native4AED70 commands and
  entries carry phase/actor identities. `before_body_pixels`,
  `before_late_pixels` and `pixels` sample the locations touched by actual health
  stores; `body_overwritten_health_pixels` records any original first/body/final
  word differences; both controls record an empty difference list. They
  demonstrate no positive visible order difference for these placements.
  These are bounded consumer samples, not full color planes.
  Full native color/Z crop hashes and the native body-admission point/clip
  identify the intervening body interval. The two health phases remain explicit
  even where their repeated opaque pixels are idempotent.

The selected physical stack is RULESMD, optional LANGRULE, Battle MPBattleMD,
then XMP03T4. Fixed ARTMD supplies Building properties. Whole native
Rules665650/BuildingType45DD90 and BuildingType45FE50 execute; constructor and
per-layer fields include Strength, Height/Foundation, Pip/PipScale,
PipsDrawForAll, CanBeOccupied and SpawnsNumber. The shared original
ConditionRed/Yellow reader blocks consume each layer's AudioVisual section.
Stock GAPOWR resolves Strength750, Height4/Foundation3, dimensions512/512/416,
delta0, PipScale0, PipsDrawForAllfalse and CanBeOccupiedfalse. The ordinary
709A90 tail executes; its lack of extra SHP draws is bounded to these priors.

Physical PIPS.SHP21 frames, PIPBRD.SHP and PALETTE.PAL use the chain3 normal
STOCK_DIGITAL winners. Body dependencies GGPOWR.SHP6 frames, UNITTEM.PAL,
BUILDNGZ.SHA and fixed ARTMD were also resolved through the production asset
owner with no loose override; the corpus pins their actual archive bytes.
Original native palette/LightGold constructors run in the same VM. Original
Cell47BBF0/TMP480350 establish flat ground, and
43CEA0→455C20/459EF0→43D290→705E00→4AED70 establishes actual body admission,
render coordinates, SHP/shadow and BUILDNGZ depth. GAPOWR's native BibShape is
NULL. Sparse Cell enrollment, traversal order, placed byte6E4, House/light/frame,
startup height factor104 and camera/viewport are declared inputs.

Component controls cover HP750/375/187/1, immediate-above thresholds, A127/63/2/0,
old Z0, actual viewport-edge clipping and ordered overlapping strips. Building
health always draws15 physical PIPS frames, including frame0 for empty pips;
flags600 select the plain leaf and bypass A/Z. The recorder closes each shape
at its real caller return, so following bracket A/Z accesses do not contaminate
health/body policy counts. Original code and component A/Z remain unchanged;
no drawing RNG entry is observed. These samples do not establish other Building
health/status routes, selection/damage lifecycle, whole Display/Map loading,
a GPU body renderer or a whole native frame. Existing production replay and
runtime capture remain separate integration gates.

The Building component corpus retains `store_count` and per-draw `color_stores`
instead of ordered color-store arrays; no consumer needs a second bulk write
trace. Historical unit/bracket payloads retain their original schema and bytes.
