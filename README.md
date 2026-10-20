<img src="docs/images/new-conscirpt-hero-image.png" alt="VERA20K hero image" width="100%">

<p align="center" dir="ltr">
  <a href="README.sv.md" lang="sv">Svenska</a> · <a href="README.zh-CN.md" lang="zh-CN">简体中文</a> · <a href="README.de.md" lang="de">Deutsch</a> · <a href="README.ar.md" lang="ar" dir="rtl">العربية</a> · <a href="README.ru.md" lang="ru">Русский</a> · <a href="README.th.md" lang="th">ไทย</a> · <a href="README.tr.md" lang="tr">Türkçe</a> · <strong>English</strong>
  &nbsp;&nbsp;
  <a href="https://github.com/YuriPlanet/vera20k/actions/workflows/macos.yml?query=branch%3Amain" title="Latest macOS library test run (run manually)"><img src="https://github.com/YuriPlanet/vera20k/actions/workflows/macos.yml/badge.svg?branch=main" alt="macOS library tests" height="20" align="middle"></a>
  <a href="https://github.com/YuriPlanet/vera20k/actions/workflows/linux.yml?query=branch%3Amain" title="Latest Linux library test run (run manually)"><img src="https://github.com/YuriPlanet/vera20k/actions/workflows/linux.yml/badge.svg?branch=main" alt="Linux library tests" height="20" align="middle"></a>
  <a href="https://github.com/YuriPlanet/vera20k/actions/workflows/windows.yml?query=branch%3Amain" title="Latest Windows library test run (run manually)"><img src="https://github.com/YuriPlanet/vera20k/actions/workflows/windows.yml/badge.svg?branch=main" alt="Windows library tests" height="20" align="middle"></a>
  <a href="https://discord.gg/kmjRUn5m5F"><img src="https://img.shields.io/badge/Discord-Join-5865F2?style=flat&amp;logo=discord&amp;logoColor=white" alt="Join Discord" height="20" align="middle"></a>
  <img src="https://img.shields.io/badge/QQ_group-1097193563-1EBAFC?style=flat&amp;logo=qq&amp;logoColor=white" alt="QQ group" height="20" align="middle">
</p>

<!-- Keep all translated READMEs in sync when changing the text below. -->

# VERA20K

VERA20K is a community-led rewrite of the original engine, `gamemd.exe`. It uses the original
game files, so you'll need your own copy of Red Alert 2: Yuri's Revenge.

VERA20K is made by gamers, for gamers, and gamers have the final say in where it goes.

<img src="docs/images/vera20k-screenshots.png" alt="VERA20K skirmish setup screen and in-game view" width="100%">

## Project goals

1. Keep the gameplay, visuals and atmosphere of the original Red Alert 2: Yuri's Revenge.
2. Support bigger battles: up to **30 players** and **20,000 units** on 500×500 maps.
3. Incorporate known RTS features and some never seen before.
4. Integrated multiplayer client

## Current status

**In active development.**

- **Skirmish:** Playable on Windows, Linux and macOS, with original and random maps,
  base building, harvesting, combat and save/load.
- **Campaigns:** Missions can launch; support remains incomplete.
- **AI:** Parts of the original AI handle base building and scripted attacks.
- **Ongoing work:** Gameplay accuracy, visuals and performance.
- **Multiplayer:** Network play is not available yet.
- **Scale:** Battles with 30 players and 20,000 units have not yet been demonstrated.

## Architecture

The engine has three main parts:

- **Simulation:** Owns gameplay state and logic.
- **Rendering:** An absolutely new GPU rendering pipeline using the original game assets, built
  to preserve the original look.
- **Application:** Handles input, menus, audio, and platform integration, connecting the systems.

This separation supports deterministic gameplay.

Arrows show data flow:

![Application sends commands to simulation, which returns world state; application sends draw data to rendering.](docs/images/architecture.png)

## Build and run

You need:

- **The game:** Red Alert 2: Yuri's Revenge.
- **Rust and Git:** current stable [Rust](https://rustup.rs/), installed with rustup, and
  [Git](https://git-scm.com/).
- **Build tools:** on Windows, the Visual Studio C++ build tools, which the Rust installer offers
  to install; on macOS, `xcode-select --install`; on Debian and Ubuntu,
  `sudo apt install build-essential libasound2-dev pkg-config`.
- **Graphics:** a GPU with Vulkan, DirectX 12 or Metal.

VERA20K has been played on Windows, Linux and macOS.

```sh
git clone https://github.com/YuriPlanet/vera20k.git
cd vera20k
cp config.toml.example config.toml
# Edit config.toml and set ra2_dir to your game folder before running:
cargo run --release --bin vera20k
```

## How we work

Most of the code is written by AI coding agents. They use Ghidra to study the original
engine, then port its behavior to Rust and check it with [comparison tools](tools/native_oracle.md)
and playtesting. Agents follow [AGENTS.md](AGENTS.md).

## Customization

Optional features (e.g. Ares) should be configurable through INI files, with client toggles
where appropriate.

To launch single-player YR skirmishes or campaigns through a dedicated CnCNet
client, see the [offline client setup](integrations/cncnet/README.md). This needs
a VERA build with the `-SPAWN` startup adapter.

## Contributing

Help is welcome. You can write code, refactor the engine, test the game, share ideas, or play
it next to the original and tell us what feels wrong. Open a PR and we'll take it from there.
Discussion is on [Discord](https://discord.gg/kmjRUn5m5F) and in the QQ group (1097193563).

## Support

<a href="https://paypal.me/vera20k"><img src="docs/images/paypal-support.svg" alt="Support VERA20K via PayPal" height="40"></a>

All donations received are distributed
to contributors according to the [TIBTRE contribution ledger](https://github.com/YuriPlanet/tibtre-ledger).

Payouts to contributors:

<p><img src="https://img.shields.io/badge/PayPal-003087?style=flat&amp;logo=paypal&amp;logoColor=white" alt="PayPal" height="20"> <img src="https://img.shields.io/badge/China_UnionPay-E21836?style=flat&amp;logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9Ii02IC02IDI2OCAxNzIiPjxwYXRoIGQ9Ik01MC4wNDE2MzEsMC4wMDA2Njc1NjMzMDUgTDExMy44MjAyMTEsMC4wMDA2Njc1NjMzMDUgQzEyMi43MjMxMjcsMC4wMDA2Njc1NjMzMDUgMTI4LjI2MDQzOCw3LjI1NzMxMzQ1IDEyNi4xODM1NTgsMTYuMTg5NzY1NyBMOTYuNDkwMDcxNSwxNDMuNjc1NDE2IEM5NC4zOTQ1MzY1LDE1Mi41NzY3NzcgODUuNDgwNzM5LDE1OS44Mzk2NDEgNzYuNTcxNjA1LDE1OS44Mzk2NDEgTDEyLjc5OTI0MzMsMTU5LjgzOTY0MSBDMy45MDg3NjQsMTU5LjgzOTY0MSAtMS42NDA5ODQ0NCwxNTIuNTc2Nzc3IDAuNDM1ODk2MjA4LDE0My42NzU0MTYgTDMwLjE0MTgxODYsMTYuMTg5NzY1NyBDMzIuMjE4Njk5Myw3LjI1NzMxMzQ1IDQxLjEyNzgzMzMsMC4wMDA2Njc1NjMzMDUgNTAuMDQxNjMxLDAuMDAwNjY3NTYzMzA1IiBmaWxsPSIjZmZmIiBzdHJva2U9IiNFMjE4MzYiIHN0cm9rZS13aWR0aD0iMTAiIHN0cm9rZS1saW5lam9pbj0icm91bmQiLz48cGF0aCBkPSJNMTA4LjUxMzI4MywwLjAwMDY2NzU2MzMwNSBMMTgxLjg1NzAxOCwwLjAwMDY2NzU2MzMwNSBDMTkwLjc1ODM3OCwwLjAwMDY2NzU2MzMwNSAxODYuNzQ0NTI3LDcuMjU3MzEzNDUgMTg0LjY1MDU0NiwxNi4xODk3NjU3IEwxNTQuOTYxNzI0LDE0My42NzU0MTYgQzE1Mi44Nzg2MjUsMTUyLjU3Njc3NyAxNTMuNTI4NDI3LDE1OS44Mzk2NDEgMTQ0LjYwODQxMSwxNTkuODM5NjQxIEw3MS4yNjQ2Nzc1LDE1OS44Mzk2NDEgQzYyLjM0NDY2MTUsMTU5LjgzOTY0MSA1Ni44MjQ0NDk1LDE1Mi41NzY3NzcgNTguOTE5OTg1LDE0My42NzU0MTYgTDg4LjYwNzI1MjUsMTYuMTg5NzY1NyBDOTAuNzE1MjI0LDcuMjU3MzEzNDUgOTkuNjA1NzAzNSwwLjAwMDY2NzU2MzMwNSAxMDguNTEzMjgzLDAuMDAwNjY3NTYzMzA1IiBmaWxsPSIjZmZmIiBzdHJva2U9IiNFMjE4MzYiIHN0cm9rZS13aWR0aD0iMTAiIHN0cm9rZS1saW5lam9pbj0icm91bmQiLz48cGF0aCBkPSJNMTc4Ljk0NzgzLDAuMDAwNjY3NTYzMzA1IEwyNDIuNzI2NDA5LDAuMDAwNjY3NTYzMzA1IEMyNTEuNjQxNzYyLDAuMDAwNjY3NTYzMzA1IDI1Ny4xNzkwNzQsNy4yNTczMTM0NSAyNTUuMDg1MDkzLDE2LjE4OTc2NTcgTDIyNS4zOTYyNzEsMTQzLjY3NTQxNiBDMjIzLjMwMDczNiwxNTIuNTc2Nzc3IDIxNC4zODA3MTksMTU5LjgzOTY0MSAyMDUuNDY2OTIyLDE1OS44Mzk2NDEgTDE0MS43MTc4NzksMTU5LjgzOTY0MSBDMTMyLjc5Nzg2MywxNTkuODM5NjQxIDEyNy4yNjUyMTUsMTUyLjU3Njc3NyAxMjkuMzU0NTMyLDE0My42NzU0MTYgTDE1OS4wNDgwMTcsMTYuMTg5NzY1NyBDMTYxLjEyNDg5OSw3LjI1NzMxMzQ1IDE3MC4wMjc4MTQsMC4wMDA2Njc1NjMzMDUgMTc4Ljk0NzgzLDAuMDAwNjY3NTYzMzA1IiBmaWxsPSIjZmZmIiBzdHJva2U9IiNFMjE4MzYiIHN0cm9rZS13aWR0aD0iMTAiIHN0cm9rZS1saW5lam9pbj0icm91bmQiLz48L3N2Zz4%3D" alt="China UnionPay" height="20"> <img src="https://img.shields.io/badge/Alipay-1677FF?style=flat&amp;logo=alipay&amp;logoColor=white" alt="Alipay" height="20"> <img src="https://img.shields.io/badge/Vipps-FF5B24?style=flat&amp;logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjQzLjUgMjEuNSAyNyAyNyI%2BPHBhdGggZD0iTTU3LjMsNDAuNmMzLjcsMCw1LjgtMS44LDcuOC00LjRjMS4xLTEuNCwyLjUtMS43LDMuNS0wLjlzMS4xLDIuMywwLDMuN2MtMi45LDMuOC02LjYsNi4xLTExLjMsNi4xIGMtNS4xLDAtOS42LTIuOC0xMi43LTcuN2MtMC45LTEuMy0wLjctMi43LDAuMy0zLjRzMi41LTAuNCwzLjQsMUM1MC41LDM4LjMsNTMuNSw0MC42LDU3LjMsNDAuNnogTTY0LjIsMjguM2MwLDEuOC0xLjQsMy0zLDMgcy0zLTEuMi0zLTNzMS40LTMsMy0zQzYyLjgsMjUuMyw2NC4yLDI2LjYsNjQuMiwyOC4zeiIgZmlsbD0iI2ZmZiIvPjwvc3ZnPg%3D%3D" alt="Vipps" height="20"> <img src="https://img.shields.io/badge/SEPA-2D4B9A?style=flat&amp;logo=sepa&amp;logoColor=white" alt="SEPA" height="20"> <img src="https://img.shields.io/badge/Bitcoin-F7931A?style=flat&amp;logo=bitcoin&amp;logoColor=white" alt="Bitcoin" height="20"></p>

## Credits and legal

Thanks to OpenRA, XCC Mixer, the ModEnc wiki, Project Perfect Mod, EA's GPL source release of
Command & Conquer and Red Alert, World-Altering Editor, Final Alert, YRpp, Ares, Phobos and
many others.

Licensed under the [GPLv3](LICENSE-GPL). This repository contains no game files. Command &
Conquer and Red Alert are trademarks of Electronic Arts Inc., and the screenshots show game art
owned by Electronic Arts. VERA20K is not affiliated with or endorsed by Electronic Arts.
