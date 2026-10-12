"""Replay twelve bounded Anytown witnesses and check their portable manifest."""
import argparse
import importlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

from tools import oracle_sources
from tools.native_oracle import NATIVE_SHA256, OracleError, _canonical, first_difference
from tools.spatial_oracle.shrapnel_repair import packet_io
from tools.spatial_oracle.shrapnel_repair.validate_packet import sha

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
PACKAGE = 'tools.spatial_oracle.anytown_damage'
RUNNERS = ('next_family_native', 'anytown_geometry', 'anytown_resident',
           'anytown_admission', 'anytown_occupants', 'mtnk_attack', 'navigation', 'hut', 'hut_cursor',
           'mission', 'mission_counter', 'mission_interleaving')
PROJECTIONS = ('hut_test_vectors', 'mission_test_vectors')
PROMOTIONS = ('promotion.json', 'navigation_promotion.json', 'hut_promotion.json',
              'hut_cursor_promotion.json', 'mission_promotion.json')
FROZEN_RECEIPTS = ('navigation_receipt.json', 'hut_receipt.json', 'hut_cursor_receipt.json',
                   'mission_receipt.json')


def inputs(root_key='roots'):
    # Default return stays identical for the original six witnesses and hut.
    # Whole-map navigation additionally needs the separate physical MIX roots.
    roots = json.loads((HERE / 'retail_manifest.json').read_bytes())[root_key]
    actual = {}
    for variable, declared in roots.items():
        location = os.environ.get(variable, declared.get('default_directory'))
        if not location:
            raise OracleError(f'Set {variable} for the frozen {root_key} inputs')
        directory = Path(location)
        actual[variable] = {}
        for name, expected in declared['files'].items():
            path = directory / name
            if expected.get('absent'):
                if path.exists():
                    raise OracleError(f'Frozen input requires absent {variable}/{name}')
                actual[variable][name] = {'absent': True}
                continue
            if not path.is_file():
                raise OracleError(f'Extract {name} into {variable}')
            found = {'bytes': path.stat().st_size, 'sha256': sha(path)}
            if not all(found[k] == expected[k] for k in found):
                raise OracleError(f'Retail input mismatch: {name}')
            actual[variable][name] = found
    return actual


def repository_sources():
    """Inventory loaded repository Python sources for all packet producers."""
    sources = {}
    for module in list(sys.modules.values()):
        file = getattr(module, '__file__', None)
        if file:
            path = Path(file).resolve()
            if path.suffix == '.py' and path.is_relative_to(ROOT / 'tools'):
                sources[path.relative_to(ROOT).as_posix()] = sha(path)
    return dict(sorted(sources.items()))


def source_provenance(harness):
    return dict(harness_sha256=sha(Path(harness)), sources=repository_sources())


def check_frozen_receipts():
    """Keep immutable evidence exact; report historical source/metadata drift.

    receipt.json owns current source and metadata identities. Earlier receipts
    preserve their original commands, times and hashes, including retired sources.
    """
    differences = {}
    for name in FROZEN_RECEIPTS:
        receipt = json.loads((HERE / name).read_bytes())
        changed = {}
        for path, expected in receipt['owned'].items():
            current = sha(HERE / path) if (HERE / path).is_file() else None
            if current == expected:
                continue
            if not (path.endswith('.py') or path.endswith('.meta.json')):
                raise OracleError(f'{name}: artifact changed: {path}')
            changed[path] = dict(historical_sha256=expected, current_sha256=current)
        for path, expected in receipt['imported_sources'].items():
            current = sha(ROOT / path) if (ROOT / path).is_file() else None
            if current != expected:
                changed[path] = dict(historical_sha256=expected, current_sha256=current)
        if changed:
            differences[name] = changed
    return differences


def pin_history(previous):
    """A refresh cannot silently replace the evidence it is preserving."""
    for name in FROZEN_RECEIPTS:
        expected = previous.get('owned', {}).get(name)
        if expected is None or sha(HERE / name) != expected:
            raise OracleError(f'Historical receipt changed: {name}')


def check_promotions():
    for source in PROMOTIONS:
        promotion = json.loads((HERE / source).read_bytes())['results']
        for name, expected in promotion.items():
            payload = _canonical(packet_io.read_result(HERE / (name + '.gz')))
            if packet_io.digest(payload) != expected['published_payload_sha256']:
                raise OracleError(f'Promoted payload changed: {name}')


def manifest(retail, archives):
    from tools.spatial_oracle.shrapnel_repair import map_facts
    import capstone
    import unicorn
    for runner in RUNNERS + PROJECTIONS:
        importlib.import_module(f'{PACKAGE}.{runner}')
    owned = {p.relative_to(HERE).as_posix(): sha(p) for p in sorted(HERE.iterdir())
             if p.is_file() and p.name != 'receipt.json' and p.suffix != '.log'}
    dependencies = {name: digest for name, digest in repository_sources().items()
                    if not (ROOT / name).is_relative_to(HERE)}
    return dict(schema=2, native_sha256=NATIVE_SHA256,
                historical_identity_changes=check_frozen_receipts(),
                runtime=dict(python_requires='>=3.10', unicorn=unicorn.__version__,
                             capstone=capstone.__version__, lzo_version=map_facts.lib.lzo_version()),
                runners=[f'{PACKAGE}.{runner}' for runner in RUNNERS],
                projections=[f'{PACKAGE}.{runner}' for runner in PROJECTIONS], owned=owned,
                dependencies=dict(sorted(dependencies.items())), external_inputs=retail,
                archive_inputs=archives,
                validation='--refresh-receipt stages twelve native witnesses and two Rust projections, requires unchanged payloads and evidence metadata, then publishes only source-identity sidecars and this receipt. '
                           'Ten compressed witnesses guard their original unprojected payload '
                           'and lexical-input projection. The v7 counter comparison guards its '
                           'frozen original projection; v8 interleaving checks original execution '
                           'against recorded production inputs. All modes guard historical artifacts and pin their receipt bytes. '
                           'Historical source/metadata drift is recorded without rewriting those original commands or claims. Current active sources are pinned separately; two Rust projections are checked. '
                           'Manifest-only is read-only, skips native execution and does not repeat the saved '
                           'navigation production comparison.',
                scope='Native family identity, damage/repair geometry, selected Recalc, admission, '
                      'MTNK occupants/Detach, host-scheduled FireAt/flight/impact, full physical '
                      'Anytown navigation builders/updates, bounded concrete hut sweeps and '
                      'physical concrete repair-cursor queries, original live MTNK command/Logic '
                      'continuation, counter ordering and selected v8 first-shot RNG interleaving. '
                      'Saved navigation production comparison covers loaded/first-damaged/collapsed '
                      'and repaired states. Mission evidence excludes other-world scheduling '
                      'except recorded v8 Terrain Next prefixes; no ordinary movement/head, '
                      'full hut lifecycle or whole-bridge parity claim.')


# This packet's own source-identity fields, plus the shared publisher's replay
# command (finish_vectors writes the projections and the two counter witnesses).
IDENTITY_FIELDS = frozenset(('sources', 'harness_sha256', 'helper_sources',
                             'projection_source_sha256', oracle_sources.REPLAY_FIELD))


def output_path(runner):
    suffix = '.json' if runner in PROJECTIONS + ('mission_counter', 'mission_interleaving') else '.json.gz'
    return HERE / (runner + suffix)


def sidecar_path(payload):
    if payload.suffix == '.gz':
        payload = payload.with_suffix('')
    return payload.with_suffix('.meta.json')


def check_candidate(committed, candidate):
    """Only source identity may change; native payload/evidence remain frozen."""
    def read(path):
        return packet_io.read_result(path) if path.suffix == '.gz' else json.loads(path.read_bytes())
    if difference := first_difference(read(committed), read(candidate)):
        raise OracleError(f'Candidate payload changed: {committed.name}: {difference}')
    target = sidecar_path(committed)
    previous = json.loads(target.read_bytes())
    candidate_meta = sidecar_path(candidate)
    actual = json.loads(candidate_meta.read_bytes())
    before_evidence = {key: value for key, value in previous.items() if key not in IDENTITY_FIELDS}
    after_evidence = {key: value for key, value in actual.items() if key not in IDENTITY_FIELDS}
    if difference := first_difference(before_evidence, after_evidence):
        raise OracleError(f'Candidate evidence metadata changed: {target.name}: {difference}')
    return target, candidate_meta.read_bytes()


def without_sidecars(snapshot, names):
    """Ignore only staged metadata identities when checking publication races."""
    result = json.loads(json.dumps(snapshot))
    for name in names:
        result.get('owned', {}).pop(name, None)
    history = result.get('historical_identity_changes', {})
    for receipt, changes in list(history.items()):
        for name in names:
            changes.pop(name, None)
        if not changes:
            del history[receipt]
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--refresh-receipt', action='store_true',
                      help='replay into candidates, then refresh source-identity sidecars and receipt')
    mode.add_argument('--check', action='store_true')
    parser.add_argument('--manifest-only', action='store_true',
                        help='read-only source/input/artifact check; no native replay')
    args = parser.parse_args(argv)
    if args.refresh_receipt and args.manifest_only:
        parser.error('--refresh-receipt requires native replay; remove --manifest-only')
    if sys.flags.optimize:
        raise OracleError('Packet witnesses still require assertions; run Python without -O')
    if sys.version_info < (3, 10):
        raise OracleError('Python 3.10 or newer is required')
    path = HERE / 'receipt.json'
    previous_bytes = path.read_bytes()
    previous = json.loads(previous_bytes)
    pin_history(previous)
    check_frozen_receipts()
    check_promotions()
    before = manifest(inputs(), inputs('archive_roots'))
    # A parent launched with -E can ignore PYTHONOPTIMIZE while its children
    # would inherit it. Legacy witness assertions must remain enabled.
    env = dict(os.environ, PYTHONDONTWRITEBYTECODE='1', PYTHONPATH=str(ROOT),
               PYTHONOPTIMIZE='0')
    runners = (() if args.manifest_only else RUNNERS) + PROJECTIONS
    staged = {}
    with tempfile.TemporaryDirectory(prefix='anytown-native-candidates-') as temporary:
        for runner in runners:
            command = [sys.executable, '-m', f'{PACKAGE}.{runner}']
            if args.refresh_receipt:
                target = output_path(runner)
                candidate = Path(temporary) / target.name
                command += ['--write', '--output', str(candidate)]
            else:
                command += ['--check']
            print(f'{runner}: {"stage checked candidate" if args.refresh_receipt else "--check"}', flush=True)
            subprocess.run(command, cwd=ROOT, env=env, check=True)
            if args.refresh_receipt:
                sidecar, raw = check_candidate(target, candidate)
                staged[sidecar] = raw
        pin_history(previous)
        check_promotions()
        actual = manifest(inputs(), inputs('archive_roots'))
        if difference := first_difference(before, actual):
            raise OracleError(f'Inputs/source/artifacts changed during replay: {difference}')
        if path.read_bytes() != previous_bytes:
            raise OracleError('Current receipt changed during replay')
        if args.refresh_receipt:
            retained = {target: target.read_bytes() for target in staged}
            try:
                for target, raw in staged.items():
                    target.write_bytes(raw)
                actual = manifest(inputs(), inputs('archive_roots'))
                names = {target.name for target in staged}
                if difference := first_difference(without_sidecars(before, names), without_sidecars(actual, names)):
                    raise OracleError(f'Inputs/source/artifacts changed during publication: {difference}')
                for target, raw in staged.items():
                    if target.read_bytes() != raw:
                        raise OracleError(f'Sidecar changed during publication: {target.name}')
                if path.read_bytes() != previous_bytes:
                    raise OracleError('Current receipt changed during publication')
                # Publish the aggregate last. Interrupted sidecar writes leave the
                # previous receipt in place, so ordinary checks fail closed.
                with tempfile.NamedTemporaryFile(dir=HERE, prefix='.receipt-', delete=False) as stream:
                    pending = Path(stream.name)
                    stream.write((json.dumps(actual, indent=2, sort_keys=True) + '\n').encode())
                try:
                    pending.replace(path)
                finally:
                    pending.unlink(missing_ok=True)
            except BaseException:
                for target, raw in retained.items():
                    # Restore only bytes this publication still owns. A detected
                    # concurrent edit/deletion belongs to the other session.
                    if target.is_file() and target.read_bytes() == staged[target]:
                        target.write_bytes(raw)
                raise
            print('REFRESHED source-identity sidecars and receipt after native replay; goldens unchanged. Run an independent --check.', flush=True)
        else:
            if difference := first_difference(previous, actual):
                raise OracleError(f'Source/input/artifact manifest changed: {difference}')
            label = 'manifest-only' if args.manifest_only else 'twelve native outputs and manifest'
            print(f'PASS {label}; no files written.', flush=True)


if __name__ == '__main__':
    main()
