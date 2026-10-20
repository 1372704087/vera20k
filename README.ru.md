<img src="docs/images/new-conscirpt-hero-image.png" alt="Заглавное изображение VERA20K" width="100%">

<p align="center" dir="ltr">
  <a href="README.sv.md" lang="sv">Svenska</a> · <a href="README.zh-CN.md" lang="zh-CN">简体中文</a> · <a href="README.de.md" lang="de">Deutsch</a> · <a href="README.ar.md" lang="ar" dir="rtl">العربية</a> · <strong>Русский</strong> · <a href="README.th.md" lang="th">ไทย</a> · <a href="README.tr.md" lang="tr">Türkçe</a> · <a href="README.md" lang="en">English</a>
  &nbsp;&nbsp;
  <a href="https://github.com/YuriPlanet/vera20k/actions/workflows/macos.yml?query=branch%3Amain" title="Последний запуск тестов библиотеки на macOS (запускаются вручную)"><img src="https://github.com/YuriPlanet/vera20k/actions/workflows/macos.yml/badge.svg?branch=main" alt="Тесты библиотеки на macOS" height="20" align="middle"></a>
  <a href="https://github.com/YuriPlanet/vera20k/actions/workflows/linux.yml?query=branch%3Amain" title="Последний запуск тестов библиотеки на Linux (запускаются вручную)"><img src="https://github.com/YuriPlanet/vera20k/actions/workflows/linux.yml/badge.svg?branch=main" alt="Тесты библиотеки на Linux" height="20" align="middle"></a>
  <a href="https://github.com/YuriPlanet/vera20k/actions/workflows/windows.yml?query=branch%3Amain" title="Последний запуск тестов библиотеки на Windows (запускаются вручную)"><img src="https://github.com/YuriPlanet/vera20k/actions/workflows/windows.yml/badge.svg?branch=main" alt="Тесты библиотеки на Windows" height="20" align="middle"></a>
  <a href="https://discord.gg/kmjRUn5m5F"><img src="https://img.shields.io/badge/Discord-Join-5865F2?style=flat&amp;logo=discord&amp;logoColor=white" alt="Присоединиться к Discord" height="20" align="middle"></a>
  <img src="https://img.shields.io/badge/QQ_группа-1097193563-1EBAFC?style=flat&amp;logo=qq&amp;logoColor=white" alt="QQ-группа" height="20" align="middle">
</p>

# VERA20K

VERA20K — новая реализация оригинального движка `gamemd.exe`, которую развивает сообщество.
Она использует оригинальные файлы игры, поэтому тебе понадобится собственная копия
Red Alert 2: Yuri's Revenge.

VERA20K делают игроки для игроков, и последнее слово в том, куда движется проект, остаётся за
игроками.

<img src="docs/images/vera20k-screenshots.png" alt="VERA20K: настройки схватки и кадр из игры" width="100%">

## Цели проекта

1. Сохранить игровой процесс, графику и атмосферу оригинальной Red Alert 2: Yuri's Revenge.
2. Поддерживать более крупные сражения: до **30 игроков** и **20 000 юнитов** на картах 500×500.
3. Добавлять известные возможности RTS, а также такие, каких ещё не было.
4. Встроенный клиент для мультиплеера

## Текущее состояние

**Активная разработка.**

- **Схватки:** Доступны на Windows, Linux и macOS: карты из оригинальной игры и случайные
  карты, строительство баз, сбор ресурсов, бои, сохранение и загрузка.
- **Кампании:** Миссии можно запускать; поддержка остаётся неполной.
- **ИИ:** Части оригинального ИИ отвечают за строительство баз и атаки по скриптам.
- **Текущая работа:** Точность игрового поведения, графика и производительность.
- **Мультиплеер:** Сетевая игра пока недоступна.
- **Масштаб:** Сражения с 30 игроками и 20 000 юнитами ещё не продемонстрированы.

## Архитектура

Движок состоит из трёх основных частей:

- **Симуляция:** Отвечает за состояние игры и игровую логику.
- **Рендеринг:** Совершенно новый конвейер отрисовки на GPU, использующий оригинальные игровые
  ресурсы и созданный для сохранения оригинального облика.
- **Приложение:** Обрабатывает ввод, меню, звук и интеграцию с платформой, связывая системы между собой.

Такое разделение поддерживает детерминированный игровой процесс.

Стрелки показывают поток данных:

![Приложение передаёт команды симуляции и получает состояние мира; данные отрисовки передаются рендерингу.](docs/images/architecture.ru.png)

## Сборка и запуск

Понадобится:

- **Игра:** Red Alert 2: Yuri's Revenge.
- **Rust и Git:** актуальная стабильная версия [Rust](https://rustup.rs/), установленная через
  rustup, и [Git](https://git-scm.com/).
- **Инструменты сборки:** на Windows — C++ Build Tools из Visual Studio, которые предложит
  установить установщик Rust; на macOS — `xcode-select --install`; в Debian и Ubuntu —
  `sudo apt install build-essential libasound2-dev pkg-config`.
- **Графика:** видеокарта с поддержкой Vulkan, DirectX 12 или Metal.

В VERA20K уже играли на Windows, Linux и macOS.

```sh
git clone https://github.com/YuriPlanet/vera20k.git
cd vera20k
cp config.toml.example config.toml
# Перед запуском отредактируй config.toml и укажи папку с игрой в ra2_dir:
cargo run --release --bin vera20k
```

## Как мы работаем

Большую часть кода пишут ИИ-агенты для программирования. Они изучают оригинальный движок в
Ghidra, затем переносят его поведение на Rust и проверяют его
[инструментами сравнения](tools/native_oracle.md) и игровыми тестами. Агенты следуют
[AGENTS.md](AGENTS.md).

## Настройка

Необязательные возможности (например, из Ares) должны настраиваться через INI-файлы,
с переключателями в клиенте там, где это уместно.

## Как помочь

Мы рады помощи. Можно писать код, рефакторить движок, тестировать игру, предлагать идеи или
играть параллельно с оригиналом и рассказывать, что кажется неправильным. Открой PR, а дальше
мы разберёмся сами. Обсуждение идёт в [Discord](https://discord.gg/kmjRUn5m5F) и в QQ-группе (1097193563).

## Поддержка проекта

<a href="https://paypal.me/vera20k"><img src="docs/images/paypal-support.svg" alt="Поддержать VERA20K через PayPal" height="40"></a>

Все полученные пожертвования
распределяются между участниками проекта согласно
[журналу вкладов TIBTRE](https://github.com/YuriPlanet/tibtre-ledger).

Участники получают выплаты через:

<p><img src="https://img.shields.io/badge/PayPal-003087?style=flat&amp;logo=paypal&amp;logoColor=white" alt="PayPal" height="20"> <img src="https://img.shields.io/badge/China_UnionPay-E21836?style=flat&amp;logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9Ii02IC02IDI2OCAxNzIiPjxwYXRoIGQ9Ik01MC4wNDE2MzEsMC4wMDA2Njc1NjMzMDUgTDExMy44MjAyMTEsMC4wMDA2Njc1NjMzMDUgQzEyMi43MjMxMjcsMC4wMDA2Njc1NjMzMDUgMTI4LjI2MDQzOCw3LjI1NzMxMzQ1IDEyNi4xODM1NTgsMTYuMTg5NzY1NyBMOTYuNDkwMDcxNSwxNDMuNjc1NDE2IEM5NC4zOTQ1MzY1LDE1Mi41NzY3NzcgODUuNDgwNzM5LDE1OS44Mzk2NDEgNzYuNTcxNjA1LDE1OS44Mzk2NDEgTDEyLjc5OTI0MzMsMTU5LjgzOTY0MSBDMy45MDg3NjQsMTU5LjgzOTY0MSAtMS42NDA5ODQ0NCwxNTIuNTc2Nzc3IDAuNDM1ODk2MjA4LDE0My42NzU0MTYgTDMwLjE0MTgxODYsMTYuMTg5NzY1NyBDMzIuMjE4Njk5Myw3LjI1NzMxMzQ1IDQxLjEyNzgzMzMsMC4wMDA2Njc1NjMzMDUgNTAuMDQxNjMxLDAuMDAwNjY3NTYzMzA1IiBmaWxsPSIjZmZmIiBzdHJva2U9IiNFMjE4MzYiIHN0cm9rZS13aWR0aD0iMTAiIHN0cm9rZS1saW5lam9pbj0icm91bmQiLz48cGF0aCBkPSJNMTA4LjUxMzI4MywwLjAwMDY2NzU2MzMwNSBMMTgxLjg1NzAxOCwwLjAwMDY2NzU2MzMwNSBDMTkwLjc1ODM3OCwwLjAwMDY2NzU2MzMwNSAxODYuNzQ0NTI3LDcuMjU3MzEzNDUgMTg0LjY1MDU0NiwxNi4xODk3NjU3IEwxNTQuOTYxNzI0LDE0My42NzU0MTYgQzE1Mi44Nzg2MjUsMTUyLjU3Njc3NyAxNTMuNTI4NDI3LDE1OS44Mzk2NDEgMTQ0LjYwODQxMSwxNTkuODM5NjQxIEw3MS4yNjQ2Nzc1LDE1OS44Mzk2NDEgQzYyLjM0NDY2MTUsMTU5LjgzOTY0MSA1Ni44MjQ0NDk1LDE1Mi41NzY3NzcgNTguOTE5OTg1LDE0My42NzU0MTYgTDg4LjYwNzI1MjUsMTYuMTg5NzY1NyBDOTAuNzE1MjI0LDcuMjU3MzEzNDUgOTkuNjA1NzAzNSwwLjAwMDY2NzU2MzMwNSAxMDguNTEzMjgzLDAuMDAwNjY3NTYzMzA1IiBmaWxsPSIjZmZmIiBzdHJva2U9IiNFMjE4MzYiIHN0cm9rZS13aWR0aD0iMTAiIHN0cm9rZS1saW5lam9pbj0icm91bmQiLz48cGF0aCBkPSJNMTc4Ljk0NzgzLDAuMDAwNjY3NTYzMzA1IEwyNDIuNzI2NDA5LDAuMDAwNjY3NTYzMzA1IEMyNTEuNjQxNzYyLDAuMDAwNjY3NTYzMzA1IDI1Ny4xNzkwNzQsNy4yNTczMTM0NSAyNTUuMDg1MDkzLDE2LjE4OTc2NTcgTDIyNS4zOTYyNzEsMTQzLjY3NTQxNiBDMjIzLjMwMDczNiwxNTIuNTc2Nzc3IDIxNC4zODA3MTksMTU5LjgzOTY0MSAyMDUuNDY2OTIyLDE1OS44Mzk2NDEgTDE0MS43MTc4NzksMTU5LjgzOTY0MSBDMTMyLjc5Nzg2MywxNTkuODM5NjQxIDEyNy4yNjUyMTUsMTUyLjU3Njc3NyAxMjkuMzU0NTMyLDE0My42NzU0MTYgTDE1OS4wNDgwMTcsMTYuMTg5NzY1NyBDMTYxLjEyNDg5OSw3LjI1NzMxMzQ1IDE3MC4wMjc4MTQsMC4wMDA2Njc1NjMzMDUgMTc4Ljk0NzgzLDAuMDAwNjY3NTYzMzA1IiBmaWxsPSIjZmZmIiBzdHJva2U9IiNFMjE4MzYiIHN0cm9rZS13aWR0aD0iMTAiIHN0cm9rZS1saW5lam9pbj0icm91bmQiLz48L3N2Zz4%3D" alt="China UnionPay" height="20"> <img src="https://img.shields.io/badge/Alipay-1677FF?style=flat&amp;logo=alipay&amp;logoColor=white" alt="Alipay" height="20"> <img src="https://img.shields.io/badge/Vipps-FF5B24?style=flat&amp;logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjQzLjUgMjEuNSAyNyAyNyI%2BPHBhdGggZD0iTTU3LjMsNDAuNmMzLjcsMCw1LjgtMS44LDcuOC00LjRjMS4xLTEuNCwyLjUtMS43LDMuNS0wLjlzMS4xLDIuMywwLDMuN2MtMi45LDMuOC02LjYsNi4xLTExLjMsNi4xIGMtNS4xLDAtOS42LTIuOC0xMi43LTcuN2MtMC45LTEuMy0wLjctMi43LDAuMy0zLjRzMi41LTAuNCwzLjQsMUM1MC41LDM4LjMsNTMuNSw0MC42LDU3LjMsNDAuNnogTTY0LjIsMjguM2MwLDEuOC0xLjQsMy0zLDMgcy0zLTEuMi0zLTNzMS40LTMsMy0zQzYyLjgsMjUuMyw2NC4yLDI2LjYsNjQuMiwyOC4zeiIgZmlsbD0iI2ZmZiIvPjwvc3ZnPg%3D%3D" alt="Vipps" height="20"> <img src="https://img.shields.io/badge/SEPA-2D4B9A?style=flat&amp;logo=sepa&amp;logoColor=white" alt="SEPA" height="20"> <img src="https://img.shields.io/badge/Bitcoin-F7931A?style=flat&amp;logo=bitcoin&amp;logoColor=white" alt="Bitcoin" height="20"></p>

## Благодарности и правовая информация

Спасибо OpenRA, XCC Mixer, вики ModEnc, Project Perfect Mod, EA за публикацию исходного кода
Command & Conquer и Red Alert под GPL, World-Altering Editor, Final Alert, YRpp, Ares,
Phobos и многим другим.

Проект распространяется под лицензией [GPLv3](LICENSE-GPL). В этом репозитории нет файлов игры.
Command & Conquer и Red Alert — товарные знаки Electronic Arts Inc., а на скриншотах показана
игровая графика, принадлежащая Electronic Arts. VERA20K не связан с Electronic Arts и
не одобрен компанией.

Перевод [README.md](README.md); поддерживайте его в соответствии с английской версией.
