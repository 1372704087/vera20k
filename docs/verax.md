# VeraX extension

VeraX is VERA20K's own first-party extension namespace, separate from the Ares, Phobos and
NPatch compatibility capabilities. Build it explicitly:

```sh
cargo run --release --bin vera20k --features verax
```

The `verax` feature currently compiles one command, AutoLoad. The
translucent-fog option and foot-path overlay in "Origins and related
first-party work" are adjacent VERA work, not VeraX features.

## AutoLoad

Ctrl+D issues standard Enter orders for the current selection: selected infantry and vehicles are
balanced by type into the selected carriers that are within 30 cells, have `Passengers=` room and
accept the passenger's `Size=` under `SizeLimit=`. Each type takes turns, the closest hulls fill
first, and a loaded carrier or a vehicle already assigned one role cannot also take the other.
Distance and engine admission are checked for each pair before room is reserved. Orders are ordinary Enter
missions, so a replay, a save or another client sees the same commands; the engine's own
`Receive_Radio` 0x0F admission decides every boarding. Buildings that garage through their own
Garrison/Occupy channels are not part of this chord.

The command is registered as `AutoLoad` in the keyboard page's `vera20k` category, so it can be
rebound or cleared like a retail hotkey. Ctrl+D is installed as the reload default when neither
AutoLoad nor Ctrl+D already has a saved binding, and an explicit `AutoLoad=0` keeps it cleared.
The assignment is written to the profile with the other bindings.

## Origins and related first-party work

VeraX does not bundle third-party binaries or source. Its ideas and the
related presentation work came from:

- **AutoLoad** follows the vehicle-loading idea shown in a video by Bilibili
  user [335956233](https://space.bilibili.com/335956233). VERA implements its
  own allocation, range and command policy; the creator's binary and source are
  not used and are not needed to reproduce the observed behavior.
- **Translucent fog** is VERA's own idea, not copied from an extension. It is a
  client graphics option (`[graphics] translucent_fog=true`, off by default for
  the retail look) that keeps terrain visible through fog while concealing
  unseen enemy objects, enemy projectiles and walls. It is not gated by the
  `verax` feature.
- **Foot path steps** is a diagnostic overlay rather than an extension feature.
  In `dev-overlays` builds, `Ctrl+Shift+O` draws the remaining path cells and
  step labels for selected moving units (`src/app/diagnostics/debug_overlays.rs`).
  Release builds (`--no-default-features`) do not include it. Its presentation
  follows a video by Bilibili user [1995852436](https://space.bilibili.com/1995852436);
  no third-party binary or source is used.

## Namespace

`--features verax` compiles `rules::VERAX_ENABLED`. VeraX-only readers take their keys and
commands under `#[cfg(feature = "verax")]`. AutoLoad's distribution and range are VeraX input
policy; its ordinary Enter orders and boarding admission reuse the native-derived engine owners
(`FootClass::ClickedAction` at `0x004D74E0`, `UnitClass::Receive_Radio` at `0x00737430`).
VeraX is one of four orthogonal capabilities: it combines with `ares`, `phobos` and
`npatch` (`--features ares,phobos,npatch,verax`). The compatibility rule and the pattern for two
extensions touching one native site are in
[NPatch compatibility](npatch.md#cross-extension-compatibility).

简体中文：[扩展支持与原作者](zh-CN/credits.md)。
