# VeraX extension

VeraX is VERA20K's own first-party extension namespace, separate from the Ares, Phobos and NPatch
compatibility capabilities. Build it explicitly:

```sh
cargo run --release --bin vera20k --features verax
```

## View distance

With the cursor over the battlefield, the mouse wheel selects one of four view distances; the game
window keeps its size and only the camera's world-per-pixel scale changes. The tier's logical
resolution is the 1280x800 reference frame multiplied by its scale:

| Tier | Name | Scale | Resolution |
| ---: | --- | ---: | --- |
| 0 | Small view | 0.8 | 1024x640 |
| 1 | Default | 1.0 | 1280x800 |
| 2 | Large view | 1.25 | 1600x1000 |
| 3 | Huge view | 1.5 | 1920x1200 |

Wheel up selects the next smaller view distance (zoom in), wheel down the next larger one; the
cycle clamps at both ends. The tactical centre stays planted while the zoom eases to the new tier.
Over the sidebar column and the bottom command strip the wheel keeps gamemd's native
`SidebarUp`/`SidebarDown` build-strip scrolling.

The tiers drive VERA's existing camera zoom (`app/input/camera.rs`), which is client-only and never
part of the simulation. The saved tactical view retains the current zoom. The Options `Zoom` frame
pass is independent of this setting and still scales the whole frame, UI included.

## Semi-transparent fog of war

VeraX renders the tactical A-buffer (SHROUD.SHP and FOG.SHP) at half strength, so the map stays
faintly visible instead of the native near-black. The engine keeps the original A bytes and lifts
the selector halfway toward the neutral value 127 at sample time, once in the CPU surface sampler
(`ShroudBuffer::sample_world`) and once in the shared GPU `tactical_a_at`; CPU lines and world
shaders therefore agree pixel for pixel.

Enemy objects are not revealed by the lift: while the local viewer does not currently see their
anchor cell, enemy units, buildings, building-attached animation parts, wall overlays with their
shadows, native remembered building pictures, weapon projectiles and their trails leave the draw
plan entirely. Own and allied objects always draw. Superweapon effect visuals (ion blast, lightning,
nuke lighting, chrono/psychic effects) and the dedicated beam/effect owners stay visible. Map
scenery (terrain, ore, trees, crates, bridges) stays faintly visible, which is the point of the
translucent fog.

Only rendering changes: visibility, shroud knowledge, targeting and every other simulation rule
still read the native fog state. Build without `verax` for the original opaque shroud and native
draw admission.

## Namespace

`--features verax` compiles `rules::VERAX_ENABLED`. New VeraX-only readers take their keys under
`#[cfg(feature = "verax")]` and cite their native addresses like every other chain. VeraX is one of
four orthogonal capabilities: it combines with `ares`, `phobos` and `npatch`
(`--features ares,phobos,npatch,verax`). The compatibility rule and the pattern for two extensions
touching one native site are in
[NPatch placeholder](npatch.md#cross-extension-compatibility).
