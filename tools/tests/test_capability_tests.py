"""Pin the capability registry to Cargo.toml so a new flag cannot skip coverage.

The registry in tools/capability_tests.py is the single stated home of the
optional capability list: a feature that no build compiles is exactly the gap
issue #2798 describes, so adding one to the manifest without registering it
must fail here. The capability workflow runs this module before it compiles
anything; the wider Python suite (`python-tools.yml`) is manual.
"""

import tomllib
import unittest
from pathlib import Path

from tools.capability_tests import BY_NAME, CAPABILITIES, COMBINATIONS, features_of

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "Cargo.toml"


def manifest_features() -> dict[str, list[str]]:
    with MANIFEST.open("rb") as handle:
        return tomllib.load(handle)["features"]


class CapabilityRegistryTests(unittest.TestCase):
    def setUp(self):
        self.features = manifest_features()

    def test_every_optional_feature_is_registered(self):
        covered = set(self.features["default"])
        for spec in CAPABILITIES:
            covered.update(spec.features)
        uncovered = set(self.features) - {"default"} - covered
        self.assertEqual(
            uncovered,
            set(),
            "Cargo.toml features no build compiles; register them in "
            "tools/capability_tests.py (or add them to the default set)",
        )

    def test_every_registered_feature_exists_in_the_manifest(self):
        registered = {feature for spec in CAPABILITIES for feature in spec.features}
        unknown = registered - set(self.features)
        self.assertEqual(unknown, set(), "registered features that are not manifest features")

    def test_capability_names_are_unique_and_combination_names_known(self):
        names = [spec.name for spec in CAPABILITIES]
        self.assertEqual(len(names), len(set(names)), "duplicate capability names")
        for combination in COMBINATIONS:
            for name in combination:
                self.assertIn(name, BY_NAME, f"combination names unknown capability {name!r}")

    def test_combinations_enable_distinct_features(self):
        for combination in COMBINATIONS:
            features = features_of(combination)
            self.assertEqual(
                len(features),
                len(set(features)),
                f"combination {'+'.join(combination)} enables a feature twice",
            )

    def test_combination_features_exist_in_the_manifest(self):
        for combination in COMBINATIONS:
            for feature in features_of(combination):
                self.assertIn(feature, self.features, f"combination feature {feature!r}")
