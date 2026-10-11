"""Source-provenance drift, dependent discovery and replay classification."""
from contextlib import redirect_stdout
import hashlib
import io
from pathlib import Path
import subprocess
import tempfile
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
    def test_module_run_records_its_arguments_without_mode_or_output(self):
        module = main_module('tools.spatial_oracle.building_guard_attack')
        for argv in (['--gate', '--check', '--output', 'out dir/x.json'],
                     ['--output=/tmp/x.json', '--gate', '--write']):
            command = sources.replay_command(argv, module)
            self.assertEqual(command, 'python -m tools.spatial_oracle.building_guard_attack --gate')
        command = sources.replay_command(['--label', 'a b', '--refresh-sources'], module)
        self.assertEqual(sources.replay_arguments(command),
                         ['-m', 'tools.spatial_oracle.building_guard_attack', '--label', 'a b'])

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

    def test_newly_recorded_provenance_has_one_order(self):
        recorded = {'scope': 's', 'payload_sha256': 'p'}
        for current in ({sources.REPLAY_FIELD: 'r', sources.SOURCE_FIELD: {}, **recorded},
                        {sources.SOURCE_FIELD: {}, **recorded, sources.REPLAY_FIELD: 'r'}):
            self.assertEqual(list(sources.refreshed(recorded, current)),
                             ['scope', 'payload_sha256', sources.SOURCE_FIELD, sources.REPLAY_FIELD])


class FakeHistory:
    def __init__(self, digests):
        self.found = digests

    def digests(self, sidecar):
        return self.found


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

    def corpus(self, pins, history=None, **metadata):
        return sources.inspect('tools/spatial_oracle/unit_entry_counter_mode.meta.json',
                               {sources.SOURCE_FIELD: pins, **metadata}, self.sources, history)

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
        states = {pin.key: (pin.path, pin.state, pin.found_by) for pin in corpus.pins}
        self.assertEqual(states, {
            'tools/spatial_oracle/map_queries.py': ('tools/spatial_oracle/map_queries.py', 'current', 'path'),
            'src/sim/door.rs': ('src/sim/door.rs', 'changed', 'path'),
            'tools/deleted.py': ('tools/deleted.py', 'missing', 'path'),
            'shared_runner': ('tools/native_oracle.py', 'current', 'content'),
            'tools.native_oracle': ('tools/native_oracle.py', 'changed', 'name'),
            'rally.py': ('tools/spatial_oracle/rally.py', 'current', 'content'),
            # Two files share the stem; the one beside the sidecar is the guess.
            'rally': ('tools/spatial_oracle/rally.py', 'changed', 'name'),
        })
        self.assertEqual(corpus.replay, ['-m', 'tools.spatial_oracle.unit_entry', '--counter-mode'])
        self.assertEqual(corpus.replay_origin, 'recorded')
        self.assertEqual(len(corpus.stale()), 4)
        self.assertEqual(corpus.pinned_fields(), [])

    def test_history_names_a_label_whose_content_is_gone(self):
        corpus = self.corpus({'shared_runner': digest('runner v1')},
                             FakeHistory({digest('runner v1'): 'tools/native_oracle.py'}))
        self.assertEqual([(pin.path, pin.state, pin.found_by) for pin in corpus.pins],
                         [('tools/native_oracle.py', 'changed', 'history')])

    def test_pins_in_other_fields_are_found_and_cannot_be_refreshed(self):
        corpus = self.corpus(
            {'tools/spatial_oracle/map_queries.py': digest('queries v1')},
            FakeHistory({digest('runner v1'): 'tools/native_oracle.py',
                         digest('payload'): 'tools/spatial_oracle/rally.py'}),
            source_pins={'tools/spatial_oracle/map_queries.py': digest('queries v0'), 'note': 'x'},
            helper_sources={'rally.py': digest('rally a')},
            harness_sha256=digest('unit entry'),
            runner_sha256=digest('runner v1'),
            # Not named like a source identity: never looked up in history.
            payload_sha256=digest('payload'),
            native_sha256='f' * 64,
            **{sources.REPLAY_FIELD: 'python -m tools.spatial_oracle.unit_entry --counter-mode'})
        other = {(pin.field, pin.path, pin.state, pin.found_by) for pin in corpus.pins
                 if pin.field != sources.SOURCE_FIELD}
        self.assertEqual(other, {
            ('source_pins', 'tools/spatial_oracle/map_queries.py', 'changed', 'path'),
            ('helper_sources', 'tools/spatial_oracle/rally.py', 'current', 'path'),
            ('harness_sha256', 'tools/spatial_oracle/unit_entry.py', 'current', 'content'),
            ('runner_sha256', 'tools/native_oracle.py', 'changed', 'history'),
        })
        self.assertEqual(corpus.pinned_fields(), ['runner_sha256', 'source_pins'])
        self.assertIn('  runner_sha256, source_pins also pin sources; a refresh cannot record those',
                      sources.describe(corpus, corpus.stale(), 'pins'))
        pinned = ('PINNED', 'runner_sha256, source_pins pin changed sources; '
                            '--refresh-sources cannot record those')
        self.assertEqual(sources.replay(corpus, timeout=1), pinned)
        corpus.replay = None
        self.assertEqual(sources.replay(corpus, timeout=1), pinned)
        found = sources.affected([corpus], set(), {digest('runner v1')})
        self.assertEqual([(pin.field, pin.path) for pin in found[0][1]],
                         [('runner_sha256', 'tools/native_oracle.py')])

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
        unknown = self.corpus({'tools/spatial_oracle/map_queries.py': digest('queries v1')})
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
    def outcome(self, code, timeout=60):
        corpus = sources.Corpus('tools/x.meta.json', 'tools/x.json', [], ['-c', code], 'recorded')
        return sources.replay(corpus, timeout=timeout)

    def test_successful_runs_report_this_corpus(self):
        self.assertEqual(self.outcome("print('REFRESHED source provenance in /r/tools/x.meta.json')")[0],
                         'REFRESHED')
        self.assertEqual(self.outcome("print('PASS /r/tools/x.json: native outputs match')")[0], 'CURRENT')
        # Another corpus refreshed by the same command does not make this one refreshed.
        self.assertEqual(self.outcome("print('REFRESHED source provenance in /r/tools/y.meta.json')\n"
                                      "print('PASS /r/tools/x.json: native outputs match')")[0], 'CURRENT')
        self.assertEqual(self.outcome("print('PASS /r/tools/other.json')")[0], 'ELSEWHERE')

    def test_only_a_comparison_mismatch_is_changed(self):
        changed = self.outcome("class OracleMismatch(Exception): pass\n"
                               "raise OracleMismatch('Reference mismatch in tools/x.json: $.a')")
        self.assertEqual(changed, ('CHANGED', 'OracleMismatch: Reference mismatch in tools/x.json: $.a'))
        missing = self.outcome("class OracleError(Exception): pass\n"
                               "raise OracleError('Full raw archive or provenance missing: tools/x.raw.json.gz')")
        self.assertEqual(missing[0], 'FAILED')
        self.assertEqual(self.outcome("import os; os.environ['VERA20K_SURELY_UNSET_INPUTS']"),
                         ('FAILED', "KeyError: 'VERA20K_SURELY_UNSET_INPUTS'"))

    def test_output_after_the_traceback_does_not_hide_the_error(self):
        self.assertEqual(self.outcome("import atexit, sys\n"
                                      "atexit.register(lambda: print('shutdown note', file=sys.stderr))\n"
                                      "raise KeyError('VERA20K_X')"),
                         ('FAILED', "KeyError: 'VERA20K_X'"))

    def test_unsupported_timeout_and_unknown(self):
        self.assertEqual(self.outcome("import argparse; argparse.ArgumentParser().parse_args()"),
                         ('UNSUPPORTED', 'the producer does not take --refresh-sources'))
        self.assertEqual(self.outcome("import time; time.sleep(30)", timeout=0.5)[0], 'TIMEOUT')
        self.assertEqual(sources.replay(sources.Corpus('a.meta.json', 'a.json', [], None, None), 1)[0],
                         'UNKNOWN')


class MainTests(unittest.TestCase):
    def run_main(self, *outcomes, arguments=('stale', '--replay')):
        pin = sources.Pin('tools/a.py', 'old', 'tools/a.py', 'new')
        every = [sources.Corpus(f'tools/c{index}.meta.json', f'tools/c{index}.json', [pin],
                                ['-m', 'tools.c'], 'recorded') for index in range(len(outcomes))]
        with patch.object(sources.Sources, 'tracked', return_value=sources.Sources({})), \
             patch.object(sources, 'corpora', return_value=every), \
             patch.object(sources, 'replay', side_effect=[(outcome, 'detail') for outcome in outcomes]), \
             redirect_stdout(io.StringIO()) as output:
            return sources.main(list(arguments)), output.getvalue()

    def test_exit_status_separates_changed_from_unconfirmed(self):
        self.assertEqual(self.run_main('REFRESHED', 'CURRENT')[0], 0)
        self.assertEqual(self.run_main('CHANGED', 'TIMEOUT')[0], 1)
        self.assertEqual(self.run_main('ELSEWHERE')[0], 1)
        for unconfirmed in sources.UNCONFIRMED:
            self.assertEqual(self.run_main('REFRESHED', unconfirmed)[0], 3)
        code, output = self.run_main('FAILED', 'REFRESHED')
        self.assertTrue(output.endswith('1 refreshed, 1 failed\n'))
        self.assertEqual(self.run_main('FAILED', arguments=('stale',))[0], 0)


class GitTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        (self.root / 'tools').mkdir()
        self.git('init', '-q')

    def git(self, *arguments):
        subprocess.run(['git', '-c', 'user.name=t', '-c', 'user.email=t@t', '-c', 'commit.gpgsign=false',
                        '-c', 'core.hooksPath=/dev/null', *arguments],
                       cwd=self.root, check=True, capture_output=True)

    def commit(self, **files):
        for name, text in files.items():
            (self.root / name).write_text(text)
        self.git('add', '-A')
        self.git('commit', '-q', '-m', 'c')

    def test_history_names_the_file_a_sidecar_was_written_with(self):
        self.commit(**{'tools/helper.py': 'v1', 'tools/x.meta.json': '{}'})
        self.commit(**{'tools/helper.py': 'v2'})
        self.assertEqual(sources.History(self.root).digests('tools/x.meta.json').get(digest('v1')),
                         'tools/helper.py')
        self.assertEqual(sources.History(self.root).digests('tools/never.meta.json'), {})

    def test_a_rename_lists_both_paths_and_the_old_content(self):
        self.commit(**{'tools/old.py': 'body'})
        base = subprocess.run(['git', 'rev-parse', 'HEAD'], cwd=self.root, check=True,
                              capture_output=True, text=True).stdout.strip()
        self.git('mv', 'tools/old.py', 'tools/new.py')
        self.commit()
        self.assertEqual(sources.changed_since(base, self.root),
                         ({'tools/old.py', 'tools/new.py'}, {digest('body')}))


class RepositoryTests(unittest.TestCase):
    def test_checkout_scan_proves_every_recorded_pin(self):
        """Every pin names a file by its path, or by content now or when its sidecar was written."""
        try:
            every = sources.corpora(sources.Sources.tracked())
            shallow = sources.git('rev-parse', '--is-shallow-repository').strip() == 'true'
        except (OSError, subprocess.CalledProcessError) as error:
            self.skipTest(f'Git checkout unavailable: {error}')
        self.assertGreater(len(every), 50)
        if shallow:
            self.skipTest('A shallow clone lacks the history that names older legacy labels')
        unproven = [(corpus.sidecar, pin.field, pin.key, pin.found_by) for corpus in every
                    for pin in corpus.pins if pin.found_by not in ('path', 'content', 'history')]
        self.assertEqual(unproven, [])


if __name__ == '__main__':
    unittest.main()
