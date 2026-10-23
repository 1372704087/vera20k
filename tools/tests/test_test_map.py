"""Pin the committed test maps to the commands that regenerate them.

Each entry in tools/run_test_map.py records its `tools.scale_map` invocation; this
runs that invocation and requires byte-identical output, so a hand-edited map
fails instead of silently diverging from the shape its entry documents.
"""

import unittest
from pathlib import Path

from tools.scale_map import build_map
from tools.run_test_map import MAPS

ROOT = Path(__file__).resolve().parents[2]


def regenerate(entry) -> bytes:
    """Run the entry's recorded regeneration command in-process."""
    command = list(entry.regenerate)
    size, starts = 250, None
    index = 0
    while index < len(command):
        if command[index] == "--size":
            size = int(command[index + 1])
            index += 2
        elif command[index] == "--starts":
            starts = int(command[index + 1])
            index += 2
        else:
            index += 1
    return build_map(size, starts).encode("ascii")


class TestMapRegistryTests(unittest.TestCase):
    def test_every_registered_map_matches_its_regeneration_command(self):
        for entry in MAPS:
            with self.subTest(map=entry.name):
                path = ROOT / entry.path
                self.assertTrue(path.is_file(), f"{entry.path} is missing")
                committed = path.read_bytes()
                committed.decode("ascii")  # scale_map writes ASCII
                self.assertEqual(
                    committed,
                    regenerate(entry),
                    f"{entry.path} does not match its recorded command "
                    f"({' '.join(entry.regenerate)})",
                )

    def test_map_names_are_unique(self):
        names = [entry.name for entry in MAPS]
        self.assertEqual(len(names), len(set(names)))

    def test_every_map_declares_a_scenario_and_a_regeneration(self):
        for entry in MAPS:
            self.assertTrue(entry.scenario, entry.name)
            self.assertIn("tools.scale_map", " ".join(entry.regenerate), entry.name)
