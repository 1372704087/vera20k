#!/usr/bin/env python3
"""Compile and test one optional capability of the engine, or all of them.

The capability list lives HERE and nowhere else: this registry is the single
stated home of the optional feature flags, and `tools/tests/
test_capability_tests.py` fails when a Cargo.toml feature is missing from it or
a registered name is not a feature, so adding a fifth flag cannot silently skip
coverage. `.github/workflows/capability-tests.yml` runs `check` on every push.

Usage (from the checkout root, Python >= 3.12):
    python -m tools.capability_tests list
    python -m tools.capability_tests build phobos
    python -m tools.capability_tests test phobos
    python -m tools.capability_tests test phobos -- --test-threads=8
    python -m tools.capability_tests focus phobos
    python -m tools.capability_tests check

`test <cap>` runs the whole library suite with the capability compiled in,
which is the complete run: the capability's own tests are gated inside
broadly-named modules, so no module list can be exhaustive. `focus <cap>` is
the quick local pass over the modules dedicated to the capability; its
selectors are advisory and may lag a new gated module, which `test` still
covers. `check` is the CI sequence: every capability compiled and tested, the
declared combinations compiled, and the shipping `--no-default-features` lib
configuration checked.

The runner is plain `cargo`, not `tools/cargo_run.py`: CI runners have no
cooperating worktrees to serialise against, and a capability matrix must not
wait on the build lock. Set CARGO_TARGET_DIR as usual to share a local cache.
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys
from typing import Sequence

ROOT = Path(__file__).resolve().parents[1]
CORE = "vera20k-core"
CLIENT = "vera20k"


class Capability:
    """One optional capability: its cargo features and how to run it."""

    def __init__(
        self,
        name: str,
        features: Sequence[str],
        summary: str,
        focus: Sequence[str] = (),
        test: bool = True,
    ) -> None:
        self.name = name
        self.features = tuple(features)
        self.summary = summary
        self.focus = tuple(focus)
        self.test = test

    @property
    def flag(self) -> str:
        return ",".join(self.features)


# The single stated home of the capability list (see the module docstring).
# `test=False` means no capability-specific suite exists yet: the capability is
# compile-covered only, as `npatch` is until its first chain lands.
CAPABILITIES: tuple[Capability, ...] = (
    Capability(
        "ares",
        ("ares",),
        "Ares extension behaviour; its tests are gated per module and named "
        "ares_* where they are dedicated.",
        focus=("ares_",),
    ),
    Capability(
        "phobos",
        ("phobos",),
        "Phobos extension behaviour: custom radiation types, superweapon "
        "techno gates and the build description.",
        focus=(
            "phobos_radiation",
            "phobos_techno_requirements",
            "a_bound_radiation_type",
            "build_text",
        ),
    ),
    Capability(
        "npatch",
        ("npatch",),
        "NPatch placeholder; no behaviour is compiled in yet, so compile "
        "coverage is the whole job until the first chain lands.",
        test=False,
    ),
    Capability(
        "debug-tools",
        ("debug-tools",),
        "Diagnostic builds: the browser dashboard, offline trainer and "
        "per-unit event hooks over the dev overlays.",
    ),
    Capability(
        "telemetry-dashboard",
        ("telemetry-dashboard",),
        "The optional read-only browser dashboard on its own, without the "
        "trainer/event hooks debug-tools adds.",
    ),
)

# The capability mixes #2797 declares meaningful: the three extension flags are
# orthogonal, so every pair and the triple are compiled, plus the diagnostic
# build over the triple. Combinations are compile-covered; each capability is
# tested on its own so the matrix stays bounded.
COMBINATIONS: tuple[tuple[str, ...], ...] = (
    ("ares", "phobos"),
    ("ares", "npatch"),
    ("phobos", "npatch"),
    ("ares", "phobos", "npatch"),
    ("debug-tools", "ares", "phobos", "npatch"),
)

BY_NAME = {capability.name: capability for capability in CAPABILITIES}


def capability(name: str) -> Capability:
    try:
        return BY_NAME[name]
    except KeyError:
        raise SystemExit(
            f"unknown capability {name!r}; known: {', '.join(sorted(BY_NAME))}"
        ) from None


def features_of(names: Sequence[str]) -> list[str]:
    features: list[str] = []
    for name in names:
        for feature in capability(name).features:
            if feature not in features:
                features.append(feature)
    return features


def run_cargo(args: Sequence[str]) -> int:
    command = ["cargo", *args]
    print(f"+ {' '.join(command)}", flush=True)
    return subprocess.call(command, cwd=ROOT, env={**os.environ})


def command_build(names: Sequence[str]) -> int:
    features = features_of(names)
    status = run_cargo(["check", "--workspace", "--lib", "--features", ",".join(features)])
    if status == 0:
        print(f"built {'+'.join(names)}")
    return status


def command_test(capability_name: str, extra: Sequence[str]) -> int:
    spec = capability(capability_name)
    if not spec.test:
        print(f"{capability_name} has no capability-specific suite; use `build`")
        return 0
    status = run_cargo(
        [
            "test",
            "--workspace",
            "--lib",
            "--features",
            spec.flag,
            "--",
            *extra,
        ]
    )
    if status == 0:
        print(f"tested {capability_name}")
    return status


def command_focus(capability_name: str, extra: Sequence[str]) -> int:
    spec = capability(capability_name)
    if not spec.focus:
        print(f"{capability_name} has no dedicated module selectors; use `test`")
        return 0
    status = 0
    for package in (CORE, CLIENT):
        current = run_cargo(
            [
                "test",
                "-p",
                package,
                "--lib",
                "--features",
                spec.flag,
                "--",
                *spec.focus,
                *extra,
            ]
        )
        status = status or current
    if status == 0:
        print(f"focused {capability_name} on {', '.join(spec.focus)}")
    return status


def command_check(extra: Sequence[str]) -> int:
    status = 0
    for spec in CAPABILITIES:
        current = command_build((spec.name,))
        status = status or current
        if spec.test:
            current = command_test(spec.name, extra)
            status = status or current
    for combination in COMBINATIONS:
        current = command_build(combination)
        status = status or current
    # The shipping configuration: no default features at all.
    current = run_cargo(["check", "--workspace", "--lib", "--no-default-features"])
    status = status or current
    if status == 0:
        print("all capabilities and combinations built; capability suites tested")
    else:
        print("capability check FAILED")
    return status


def command_list() -> int:
    for spec in CAPABILITIES:
        suite = "tested" if spec.test else "compile-only"
        print(f"{spec.name:22} --features {spec.flag:24} {suite}")
        print(f"  {spec.summary}")
    print("combinations (compile-covered):")
    for combination in COMBINATIONS:
        print(f"  {'+'.join(combination)}")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("list", help="print the capability registry")
    build = commands.add_parser("build", help="compile one capability or a mix")
    build.add_argument("capabilities", nargs="+", metavar="CAPABILITY")
    test = commands.add_parser("test", help="run the library suite with a capability")
    test.add_argument("capability", metavar="CAPABILITY")
    focus = commands.add_parser("focus", help="run a capability's dedicated modules")
    focus.add_argument("capability", metavar="CAPABILITY")
    check = commands.add_parser(
        "check", help="the CI sequence: every capability, combination and the shipping config"
    )
    del check
    args, extra = parser.parse_known_args(argv)
    # Everything after `--` belongs to the test binary, not to this tool.
    extra = [value for value in extra if value != "--"]

    if args.command == "list":
        return command_list()
    if args.command == "build":
        return command_build(args.capabilities)
    if args.command == "test":
        return command_test(args.capability, extra)
    if args.command == "focus":
        return command_focus(args.capability, extra)
    return command_check(extra)


if __name__ == "__main__":
    sys.exit(main())
