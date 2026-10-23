#!/usr/bin/env python3
"""Run one committed test map by scenario name, headless or interactive.

The maps under `tests/maps/` are purpose-built shapes, not retail lookalikes:
each proves one scenario the engine needs, and every one is regenerated
byte-identically by the `tools.scale_map` command its entry records, so the
committed files can never drift from that command. `tools/tests/
test_test_map.py` fails when they do.

Usage (from the checkout root, Python >= 3.12):
    python -m tools.run_test_map list
    python -m tools.run_test_map flat                          # headless march run
    python -m tools.run_test_map match --ticks 300             # headless match run
    python -m tools.run_test_map flat --debug -- --units 50    # fast debug build
    python -m tools.run_test_map flat --interactive            # launch the client

The headless run goes through `sim-bench`, whose scenario and flags pass
through: `--scenario` selects the shape's scenario (each entry carries its
default), everything after `--` reaches sim-bench unchanged, and `--debug`
skips the release build sim-bench's timings are documented for. `--interactive`
launches the client with `RA2_QUICKPLAY` naming the map, which needs a display
and the retail archives, like any ordinary local run.

Neither mode needs a map from the retail install; the rules and archives still
come from it through `RA2_DIR`, as every other run does.
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys
from typing import Sequence

ROOT = Path(__file__).resolve().parents[1]


class TestMap:
    """One committed test map: where it is and how it is run and rebuilt."""

    def __init__(
        self,
        name: str,
        path: str,
        scenario: str,
        summary: str,
        regenerate: Sequence[str],
        extra: Sequence[str] = (),
    ) -> None:
        self.name = name
        self.path = path
        self.scenario = scenario
        self.summary = summary
        self.regenerate = tuple(regenerate)
        self.extra = tuple(extra)

    @property
    def absolute(self) -> Path:
        return (ROOT / self.path).resolve()


# The committed set: a scenario per shape the engine needs to prove. Adding a
# map means adding it here, regenerating it with the recorded command, and
# letting tools/tests/test_test_map.py pin the bytes.
MAPS: tuple[TestMap, ...] = (
    TestMap(
        "flat",
        "tests/maps/flat.map",
        "march",
        "Flat 64x64-cell ground with no overlays: movement, pathing and "
        "scale runs over bare terrain.",
        regenerate=("python", "-m", "tools.scale_map", "tests/maps/flat.map", "--size", "32"),
    ),
    TestMap(
        "match",
        "tests/maps/match.map",
        "match",
        "160x160-cell match shape: four grid starts, an ore field beside each "
        "and tree clumps between them, for harvesting, bases and battles.",
        regenerate=(
            "python",
            "-m",
            "tools.scale_map",
            "tests/maps/match.map",
            "--size",
            "80",
            "--starts",
            "4",
        ),
    ),
)

BY_NAME = {entry.name: entry for entry in MAPS}


def test_map(name: str) -> TestMap:
    try:
        return BY_NAME[name]
    except KeyError:
        raise SystemExit(
            f"unknown test map {name!r}; known: {', '.join(sorted(BY_NAME))}"
        ) from None


def run(command: Sequence[str], env_extra: dict[str, str] | None = None) -> int:
    print(f"+ {' '.join(command)}", flush=True)
    env = {**os.environ, **(env_extra or {})}
    return subprocess.call(command, cwd=ROOT, env=env)


def headless(entry: TestMap, scenario: str, release: bool, extra: Sequence[str]) -> int:
    command = ["cargo", "run", "-p", "vera20k-core", "--bin", "sim-bench"]
    if release:
        command.append("--release")
    command += [
        "--",
        "--map",
        str(entry.absolute),
        "--scenario",
        scenario,
        *entry.extra,
        *extra,
    ]
    return run(command)


def interactive(entry: TestMap, extra: Sequence[str]) -> int:
    command = ["cargo", "run", "--bin", "vera20k", *extra]
    return run(command, {"RA2_QUICKPLAY": str(entry.absolute)})


def command_list() -> int:
    for entry in MAPS:
        print(f"{entry.name:8} {entry.path}  (scenario: {entry.scenario})")
        print(f"  {entry.summary}")
        print(f"  regenerate: {' '.join(entry.regenerate)}")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("list", help="print the committed test maps")
    play = commands.add_parser("play", help="run one test map")
    play.add_argument("map", metavar="MAP")
    play.add_argument("--scenario", help="sim-bench scenario (default: the map's own)")
    play.add_argument(
        "--debug",
        action="store_true",
        help="skip the release build; hashes stay meaningful, timings do not",
    )
    play.add_argument(
        "--interactive",
        action="store_true",
        help="launch the client on the map with RA2_QUICKPLAY",
    )
    args, extra = parser.parse_known_args(argv)
    extra = [value for value in extra if value != "--"]

    if args.command == "list":
        return command_list()
    entry = test_map(args.map)
    if not entry.absolute.is_file():
        raise SystemExit(f"{entry.path} is missing; regenerate: {' '.join(entry.regenerate)}")
    if args.interactive:
        return interactive(entry, extra)
    return headless(entry, args.scenario or entry.scenario, not args.debug, extra)


if __name__ == "__main__":
    sys.exit(main())
