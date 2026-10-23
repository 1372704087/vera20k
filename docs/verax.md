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

## Namespace

`--features verax` compiles `rules::VERAX_ENABLED`. New VeraX-only readers take their keys under
`#[cfg(feature = "verax")]` and cite their native addresses like every other chain. VeraX is one of
four orthogonal capabilities: it combines with `ares`, `phobos` and `npatch`
(`--features ares,phobos,npatch,verax`). The compatibility rule and the pattern for two extensions
touching one native site are in
[NPatch placeholder](npatch.md#cross-extension-compatibility).
