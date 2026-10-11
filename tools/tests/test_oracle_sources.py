"""Source-provenance drift, dependent discovery and replay classification."""
import hashlib
from pathlib import Path
import sys
from types import ModuleType, SimpleNamespace
import unittest
from unittest.mock import patch

from tools import oracle_sources as sources


def digest(text: str) -> str:
    return hashlib.sha256(text.encode('utf-8')).hexdigest()


def main_module(name=None, file=None):
    module = ModuleType('__main__')
    module.__spec__ = SimpleNamespace(name=name) if name else None
    if file:
        module.__file__ = file
    return module


class ReplayCommandTests(unittest.TestCase):
    def test_module_run_records_its_arguments_without_the_mode(self):
        command = sources.replay_command(['--gate', '--check', '--output', 'out dir/x.json'],
                                         main_module('tools.spatial_oracle.building_guard_attack'))
        self.assertEqual(command, "python -m tools.spatial_oracle.building_guard_attack "
                                  "--gate --output 'out dir/x.json'")
        self.assertEqual(sources.replay_arguments(command),
                         ['-m', 'tools.spatial_oracle.building_guard_attack', '--gate',
                          '--output', 'out dir/x.json'])

    def test_script_runs_become_module_runs_and_other_mains_are_not_recorded(self):
        script = str(sources.REPO / 'tools' / 'render_depth_oracle.py')
        self.assertEqual(sources.replay_command(['--write'], main_module(file=script)),
                         'python -m tools.render_depth_oracle')
        self.assertEqual(sources.replay_command([], main_module('tools.anim_oracle.__main__')),
                         'python -m tools.anim_oracle')
        self.assertIsNone(sources.replay_command([], main_module('unittest')))
        self.assertIsNone(sources.replay_command([], main_module(file='/elsewhere/tool.py')))

    def test_declared_commands_drop_environment_and_mode(self):
        self.assertEqual(sources.replay_arguments(
            "RA2_DIR='<retail>' X=1 python -m tools.input_oracle.campaign_start --houses --check"),
            ['-m', 'tools.input_oracle.campaign_start', '--houses'])
        self.assertIsNone(sources.replay_arguments('python tools/foo.py --check'))
        self.assertIsNone(sources.replay_arguments('python -m unittest'))


class DriftTests(unittest.TestCase):
    def test_drift_compares_by_source_and_translates_legacy_labels(self):
        source_paths = {'producer': sources.REPO / 'tools' / 'a.py', 'helper': sources.REPO / 'tools' / 'b.py'}
        recorded = {sources.SOURCE_FIELD: {'producer': '1', 'helper': '2', 'tools/gone.py': '3'}}
        current = {sources.SOURCE_FIELD: {'tools/a.py': '1', 'tools/b.py': '9', 'tools/new.py': '4'}}
        self.assertEqual(sources.source_drift(recorded, current, source_paths),
                         dict(changed=['tools/b.py'], added=['tools/new.py'], removed=['tools/gone.py']))
        self.assertEqual(sources.describe_drift(sources.source_drift(current, current)), '')

    def test_refresh_replaces_only_source_provenance_in_recorded_order(self):
        recorded = {'schema_version': 1, sources.SOURCE_FIELD: {'a': '1'}, 'scope': 's',
                    'payload_sha256': 'p'}
        current = {'scope': 's', sources.SOURCE_FIELD: {'tools/a.py': '2'}, 'payload_sha256': 'p',
                   'schema_version': 1, sources.REPLAY_FIELD: 'python -m tools.x'}
        refreshed = sources.refreshed(recorded, current)
        self.assertEqual(list(refreshed), ['schema_version', sources.SOURCE_FIELD, 'scope',
                                           'payload_sha256', sources.REPLAY_FIELD])
        self.assertEqual(refreshed[sources.SOURCE_FIELD], {'tools/a.py': '2'})
        kept = sources.refreshed(dict(recorded, **{sources.REPLAY_FIELD: 'python -m tools.y'}),
                                 {key: value for key, value in current.items()
                                  if key != sources.REPLAY_FIELD})
        self.assertEqual(kept[sources.REPLAY_FIELD], 'python -m tools.y')
        self.assertNotIn(sources.SOURCE_FIELD, sources.refreshed(recorded, {'scope': 's'}))


class DependentTests(unittest.TestCase):
    def setUp(self):
        self.sources = sources.Sources({
            'tools/native_oracle.py': digest('runner v2'),
            'tools/spatial_oracle/map_queries.py': digest('queries v1'),
            'tools/spatial_oracle/unit_entry.py': digest('unit entry'),
            'tools/spatial_oracle/rally.py': digest('rally a'),
            'tools/procedural_drawing_oracle/rally.py': digest('rally b'),
            'src/sim/door.rs': digest('door v2'),
            'tools/spatial_oracle/house_base.py': digest('helper module'),
        }, texts={'tools/spatial_oracle/unit_entry.py': "if __name__=='__main__':\n    main()\n",
                  'tools/spatial_oracle/house_base.py':
                      "OUTPUT = 'unit_entry_counter_mode.json'\ndef helper(): pass\n"})

    def corpus(self, pins, **metadata):
        return sources.inspect('tools/spatial_oracle/unit_entry_counter_mode.meta.json',
                               {sources.SOURCE_FIELD: pins, **metadata}, self.sources)

    def test_pins_resolve_by_path_content_and_name(self):
        corpus = self.corpus({
            'tools/spatial_oracle/map_queries.py': digest('queries v1'),
            'src/sim/door.rs': digest('door v1'),
            'tools/deleted.py': digest('deleted'),
            'shared_runner': digest('runner v2'),
            'tools.native_oracle': digest('runner v1'),
            'rally.py': digest('rally a'),
            'rally': digest('rally old'),
        }, **{sources.REPLAY_FIELD: 'python -m tools.spatial_oracle.unit_entry --counter-mode'})
        states = {pin.key: (pin.path, pin.state) for pin in corpus.pins}
        self.assertEqual(states, {
            'tools/spatial_oracle/map_queries.py': ('tools/spatial_oracle/map_queries.py', 'current'),
            'src/sim/door.rs': ('src/sim/door.rs', 'changed'),
            'tools/deleted.py': ('tools/deleted.py', 'missing'),
            'shared_runner': ('tools/native_oracle.py', 'current'),
            'tools.native_oracle': ('tools/native_oracle.py', 'changed'),
            'rally.py': ('tools/spatial_oracle/rally.py', 'current'),
            # Two files share the stem; the one beside the sidecar is the guess.
            'rally': ('tools/spatial_oracle/rally.py', 'changed'),
        })
        self.assertEqual(corpus.replay, ['-m', 'tools.spatial_oracle.unit_entry', '--counter-mode'])
        self.assertEqual(corpus.replay_origin, 'recorded')
        self.assertEqual(len(corpus.stale()), 4)

    def test_role_labels_name_the_producer(self):
        corpus = self.corpus({'producer': digest('unit entry old')},
                             commands=['python -m tools.spatial_oracle.unit_entry --write',
                                       'python -m tools.spatial_oracle.unit_entry --check'])
        self.assertEqual(corpus.replay_origin, 'declared by the producer')
        self.assertEqual([(pin.path, pin.state) for pin in corpus.pins],
                         [('tools/spatial_oracle/unit_entry.py', 'changed')])

    def test_replay_is_inferred_from_a_matching_producer_file_only(self):
        inferred = sources.inspect('tools/spatial_oracle/unit_entry.meta.json', {}, self.sources)
        self.assertEqual(inferred.replay, ['-m', 'tools.spatial_oracle.unit_entry'])
        unknown = self.corpus({})
        self.assertIsNone(unknown.replay)
        self.assertEqual(unknown.producers, ['tools/spatial_oracle/house_base.py'])
        self.assertIn('house_base.py names unit_entry_counter_mode.json',
                      '\n'.join(sources.describe(unknown, [], 'pins')))
        # A same-named module that cannot run is not a producer.
        self.assertIsNone(sources.inspect('tools/spatial_oracle/house_base.meta.json', {}, self.sources).replay)

    def test_affected_matches_paths_and_earlier_content_of_legacy_labels(self):
        by_path = self.corpus({'src/sim/door.rs': digest('door v1')})
        by_label = self.corpus({'checked_runner': digest('runner v1')})
        unrelated = self.corpus({'tools/spatial_oracle/map_queries.py': digest('queries v1')})
        found = sources.affected([by_path, by_label, unrelated], {'src/sim/door.rs'}, {digest('runner v1')})
        self.assertEqual([(corpus, [pin.key for pin in pins]) for corpus, pins in found],
                         [(by_path, ['src/sim/door.rs']), (by_label, ['checked_runner'])])


class ReplayOutcomeTests(unittest.TestCase):
    def outcome(self, code, sidecar='tools/x.meta.json'):
        corpus = sources.Corpus(sidecar, sources.payload_name(sidecar), [], ['-c', code], 'recorded')
        return sources.replay(corpus, timeout=60)

    def test_outcomes_distinguish_refresh_current_changed_blocked_and_elsewhere(self):
        self.assertEqual(self.outcome("print('REFRESHED source provenance in /r/tools/x.meta.json')")[0],
                         'REFRESHED')
        self.assertEqual(self.outcome("print('PASS /r/tools/x.json: native outputs match')")[0], 'CURRENT')
        changed = self.outcome("raise SystemExit('OracleError: Reference mismatch in tools/x.json: $.a')")
        self.assertEqual(changed[0], 'CHANGED')
        blocked = self.outcome("import os; os.environ['VERA20K_SURELY_UNSET_INPUTS']")
        self.assertEqual(blocked, ('BLOCKED', "KeyError: 'VERA20K_SURELY_UNSET_INPUTS'"))
        self.assertEqual(self.outcome("print('PASS /r/tools/other.json')")[0], 'ELSEWHERE')
        self.assertEqual(sources.replay(sources.Corpus('a.meta.json', 'a.json', [], None, None), 1)[0],
                         'UNKNOWN')


class RepositoryTests(unittest.TestCase):
    def test_checkout_scan_resolves_every_recorded_pin(self):
        """Every pin in the tracked sidecars names a file, by path, content or history."""
        try:
            every = sources.corpora(sources.Sources.tracked())
            shallow = sources.git('rev-parse', '--is-shallow-repository').strip() == 'true'
        except (OSError, sources.subprocess.CalledProcessError) as error:
            self.skipTest(f'Git checkout unavailable: {error}')
        self.assertGreater(len(every), 50)
        if shallow:
            self.skipTest('A shallow clone lacks the history that names older legacy labels')
        unresolved = [(corpus.sidecar, pin.key) for corpus in every for pin in corpus.pins
                      if pin.state == 'unresolved']
        self.assertEqual(unresolved, [])


if __name__ == '__main__':
    unittest.main()
