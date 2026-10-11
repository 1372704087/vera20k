"""Source provenance of native comparison sidecars: identity, drift and dependents.

`native_oracle.finish_vectors` records the normalized UTF-8/LF SHA-256 of every
repository source a producer declares (`source_normalized_lf_sha256`) and the
command that ran it (`replay_command`). Together they say which sources last
reproduced the payload and how to replay it. Neither can change native evidence,
so a later edit to a shared helper is drift, not a mismatch:

- `--check` compares the payload and every other metadata field exactly and
  reports source drift separately;
- `--refresh-sources` records the current sources only after a replay
  reproduced the payload, archive and all other metadata exactly;
- this CLI lists the corpora a source edit affects or whose recorded sources
  have drifted, says how to replay each and can run those replays:

    python -m tools.oracle_sources affected [--since REF] [PATH ...] [--replay]
    python -m tools.oracle_sources stale [--replay]

Older producers also hash sources into metadata fields of their own, such as
`harness_sha256`, `sources` or `source_pins`. Those pins are listed too, but a
refresh cannot record them, and a producer that compares its whole sidecar
still fails `--check` when one of them changes.

Listing needs only Git; replays run with this interpreter, which needs Unicorn.
"""

from __future__ import annotations

import argparse
from collections import defaultdict
from dataclasses import dataclass, field
import hashlib
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys
import time

REPO = Path(__file__).resolve().parents[1]
SOURCE_FIELD = 'source_normalized_lf_sha256'
REPLAY_FIELD = 'replay_command'
SOURCE_PROVENANCE = (SOURCE_FIELD, REPLAY_FIELD)
MODE_FLAGS = frozenset(('--check', '--write', '--refresh-sources'))
# finish_vectors' staging target. A replay regenerates the corpus where it lives.
OUTPUT_FLAG = '--output'
# Producer labels for the producer's own file, from before pins were keyed by path.
ROLE_LABELS = frozenset(('producer', 'generator', 'oracle', 'caller', 'query', '__main__'))
MAIN_GUARD = re.compile(r'''__name__\s*==\s*['"]__main__['"]''')
DIGEST = re.compile(r'[0-9a-f]{64}')
CODE_SUFFIXES = ('.py', '.rs')
# A bare digest in another field pins a source only if it equals a source's
# content. Only fields named like these are looked up in history, which keeps
# payload, input and executable digests out of that slower search.
SOURCE_DIGEST_NAME = re.compile(r'(harness|runner|source|owner|projection|probe|fixture).*sha256$')
CONFIRMED = ('REFRESHED', 'CURRENT')
# The replay ran and its evidence differs, or the command compared another corpus.
REJECTED = ('CHANGED', 'ELSEWHERE')
UNCONFIRMED = ('PINNED', 'UNSUPPORTED', 'FAILED', 'TIMEOUT', 'UNKNOWN')


def source_sha256(path: Path) -> str:
    """UTF-8 text with universal newlines, so CRLF and LF checkouts agree."""
    return hashlib.sha256(Path(path).read_text(encoding='utf-8').encode('utf-8')).hexdigest()


def text_sha256(raw: bytes) -> str:
    text = raw.decode('utf-8').replace('\r\n', '\n').replace('\r', '\n')
    return hashlib.sha256(text.encode('utf-8')).hexdigest()


def source_key(label: str, path: Path) -> str:
    """Key repository sources by path, so a sidecar names what it depends on."""
    resolved = Path(path).resolve()
    return resolved.relative_to(REPO).as_posix() if resolved.is_relative_to(REPO) else label


def source_identity(source_paths) -> dict:
    return {source_key(label, path): source_sha256(path)
            for label, path in (source_paths or {}).items()}


def evidence(metadata: dict) -> dict:
    """The metadata a replay must reproduce exactly: all but source provenance."""
    return {key: value for key, value in metadata.items() if key not in SOURCE_PROVENANCE}


def source_drift(recorded: dict, current: dict, source_paths=None) -> dict:
    """Compare recorded and current pins by source path, not by label.

    Sidecars written before pins were keyed by path use producer labels; the
    producer's current labels translate them, so a relabel is not drift.
    """
    keys = {label: source_key(label, path) for label, path in (source_paths or {}).items()}
    old = {keys.get(name, name): digest for name, digest in recorded.get(SOURCE_FIELD, {}).items()}
    new = current.get(SOURCE_FIELD, {})
    return dict(changed=sorted(name for name in old.keys() & new.keys() if old[name] != new[name]),
                added=sorted(new.keys() - old.keys()), removed=sorted(old.keys() - new.keys()))


def describe_drift(drift: dict) -> str:
    parts = []
    for name, label in (('changed', 'changed'), ('added', 'newly declared'),
                        ('removed', 'no longer declared')):
        paths = drift[name]
        if paths:
            more = f' and {len(paths) - 5} more' if len(paths) > 5 else ''
            parts.append(f'{label}: {", ".join(paths[:5])}{more}')
    return '; '.join(parts)


def refreshed(recorded: dict, current: dict) -> dict:
    """The recorded sidecar with only its source provenance replaced.

    Evidence fields keep their recorded order, and newly recorded provenance
    follows them in SOURCE_PROVENANCE order. A run whose command cannot be
    derived keeps the recorded replay command.
    """
    result = {}
    for key, value in recorded.items():
        if key == SOURCE_FIELD:
            if key in current:
                result[key] = current[key]
        else:
            result[key] = current.get(key, value) if key == REPLAY_FIELD else value
    for key in SOURCE_PROVENANCE:
        if key in current and key not in result:
            result[key] = current[key]
    return result


def replay_command(argv=None, main=None) -> str | None:
    """The `python -m tools...` invocation running now, without its mode or output."""
    main = sys.modules.get('__main__') if main is None else main
    spec = getattr(main, '__spec__', None)
    module = spec.name.removesuffix('.__main__') if spec is not None and spec.name else None
    if module is None and getattr(main, '__file__', None):
        path = Path(main.__file__).resolve()
        if path.suffix == '.py' and path.is_relative_to(REPO):
            module = '.'.join(path.relative_to(REPO).with_suffix('').parts)
    if not module or not module.startswith('tools.'):
        return None
    arguments, output = [], False
    for value in sys.argv[1:] if argv is None else argv:
        if output:
            output = False
        elif value == OUTPUT_FLAG:
            output = True
        elif value not in MODE_FLAGS and not value.startswith(OUTPUT_FLAG + '='):
            arguments.append(value)
    return shlex.join(['python', '-m', module, *arguments])


def replay_arguments(command: str) -> list[str] | None:
    """`-m module args...` from a recorded command, without environment or mode."""
    try:
        words = shlex.split(command)
    except ValueError:
        return None
    if '-m' not in words[:-1]:
        return None
    index = words.index('-m')
    module = words[index + 1]
    if not module.startswith('tools.'):
        return None
    return ['-m', module, *(word for word in words[index + 2:] if word not in MODE_FLAGS)]


def git(*arguments: str, root: Path = REPO) -> str:
    return subprocess.run(['git', *arguments], cwd=root, check=True, capture_output=True,
                          text=True, encoding='utf-8').stdout


def tracked(*patterns: str) -> list[str]:
    return [name for name in git('ls-files', '-z', '--', *patterns).split('\0') if name]


class Sources:
    """Current identities of tracked tool and Rust sources, by path and content."""

    def __init__(self, digests: dict[str, str], texts: dict[str, str] | None = None):
        self.digests = digests
        self.texts = texts
        self.read = {}
        self.by_digest = defaultdict(list)
        self.by_stem = defaultdict(list)
        for name, digest in digests.items():
            self.by_digest[digest].append(name)
            if name.endswith('.py'):
                self.by_stem[Path(name).stem].append(name)

    def text(self, name: str) -> str:
        if self.texts is not None:
            return self.texts.get(name, '')
        if name not in self.read:
            self.read[name] = (REPO / name).read_text(encoding='utf-8')
        return self.read[name]

    @classmethod
    def tracked(cls) -> Sources:
        digests = {}
        for name in tracked('tools/*.py', 'src/*.rs'):
            try:
                digests[name] = source_sha256(REPO / name)
            except (OSError, UnicodeDecodeError):
                continue
        return cls(digests)

    def named(self, label: str, near: str, producer: str | None) -> str | None:
        """Resolve a legacy producer label to the file it most plausibly named."""
        if label in ROLE_LABELS:
            return producer
        if label.startswith('tools.'):
            base = label.replace('.', '/')
            return next((name for name in (base + '.py', base + '/__init__.py')
                         if name in self.digests), None)
        matches = self.by_stem.get(label.removesuffix('.py'), [])
        local = [name for name in matches if Path(name).parent.as_posix() == near]
        return matches[0] if len(matches) == 1 else local[0] if len(local) == 1 else None


@dataclass
class Pin:
    key: str
    recorded: str
    path: str | None = None
    current: str | None = None
    # The metadata field holding the pin; any but SOURCE_FIELD is compared as evidence.
    field: str = SOURCE_FIELD
    # How the path was found: 'path', 'content', 'history' (content at the
    # sidecar's commit or earlier in the named file) or 'name' (a guess).
    found_by: str | None = None

    @property
    def state(self) -> str:
        if self.path is None:
            return 'unresolved'
        if self.current is None:
            return 'missing'
        return 'current' if self.current == self.recorded else 'changed'


@dataclass
class Corpus:
    sidecar: str
    payload: str
    pins: list[Pin]
    replay: list[str] | None
    replay_origin: str | None
    producers: list[str] = field(default_factory=list)

    def stale(self) -> list[Pin]:
        return [pin for pin in self.pins if pin.state != 'current']

    def pinned_fields(self) -> list[str]:
        """Fields other than source provenance that pin a source the checkout changed."""
        return sorted({pin.field for pin in self.stale() if pin.field != SOURCE_FIELD})


class History:
    """Tool sources as of the commit that last wrote each sidecar.

    A pin whose content no longer exists is resolved by the file that had that
    content when its sidecar was committed. A squash merge drops the branch that
    produced a sidecar, so that commit can already hold a newer producer; the
    file the pin's label names then proves it by any earlier committed content.
    """

    def __init__(self, root: Path = REPO):
        self.root = root
        self.last = {}
        self.commits = {}
        self.paths = {}

    def contents(self, objects: list[str]) -> list[bytes | None]:
        """`git cat-file --batch` bodies in request order; None for a missing object."""
        batch = subprocess.run(['git', 'cat-file', '--batch'], cwd=self.root, check=True,
                               capture_output=True,
                               input=''.join(f'{name}\n' for name in objects).encode()).stdout
        bodies, offset = [], 0
        for _ in objects:
            header_end = batch.index(b'\n', offset)
            header = batch[offset:header_end]
            offset = header_end + 1
            if header.endswith(b' missing'):
                bodies.append(None)
                continue
            size = int(header.split()[2])
            bodies.append(batch[offset:offset + size])
            offset += size + 1
        return bodies

    def digests(self, sidecar: str) -> dict[str, str]:
        if sidecar not in self.last:
            self.last[sidecar] = git('log', '-1', '--format=%H', '--', sidecar, root=self.root).strip()
        commit = self.last[sidecar]
        if not commit:
            return {}
        if commit not in self.commits:
            listing = git('ls-tree', '-r', '-z', commit, '--', 'tools', root=self.root).split('\0')
            blobs = [(entry.split('\t', 1)[1], entry.split()[2]) for entry in listing
                     if entry.endswith('.py') and ' blob ' in entry]
            found = {}
            for (path, _), body in zip(blobs, self.contents([blob for _, blob in blobs])):
                try:
                    found.setdefault(text_sha256(body), path)
                except UnicodeDecodeError:
                    continue
            self.commits[commit] = found
        return self.commits[commit]

    def versions(self, path: str) -> set[str]:
        """Every committed content of one source file."""
        if path not in self.paths:
            commits = git('log', '--format=%H', '--', path, root=self.root).split()
            found = set()
            for body in self.contents([f'{commit}:{path}' for commit in commits]):
                if body is None:
                    # The commit that deleted the file.
                    continue
                try:
                    found.add(text_sha256(body))
                except UnicodeDecodeError:
                    continue
            self.paths[path] = found
        return self.paths[path]


def payload_name(sidecar: str) -> str:
    return sidecar.removesuffix('.meta.json') + '.json'


def declared_replay(sidecar: str, metadata: dict, sources: Sources) -> tuple[list[str] | None, str | None]:
    if command := metadata.get(REPLAY_FIELD):
        return replay_arguments(command), 'recorded'
    for key in ('command', 'reproduce_command', 'commands'):
        value = metadata.get(key)
        for command in value if isinstance(value, list) else [value] if isinstance(value, str) else []:
            if arguments := replay_arguments(command):
                return arguments, 'declared by the producer'
    producer = sidecar.removesuffix('.meta.json') + '.py'
    if producer in sources.digests and MAIN_GUARD.search(sources.text(producer)):
        return ['-m', producer.removesuffix('.py').replace('/', '.')], 'inferred from the file name'
    return None, None


def producer_file(replay: list[str] | None, sources: Sources) -> str | None:
    if not replay:
        return None
    base = replay[1].replace('.', '/')
    return next((name for name in (base + '.py', base + '/__main__.py', base + '/__init__.py')
                 if name in sources.digests), None)


def inspect(sidecar: str, metadata: dict, sources: Sources, history: History | None = None) -> Corpus:
    replay, origin = declared_replay(sidecar, metadata, sources)
    producer = producer_file(replay, sources)
    near = Path(sidecar).parent.as_posix()

    def earlier(digest, named=None):
        if not history:
            return None
        if path := history.digests(sidecar).get(digest):
            return path
        return named if named and digest in history.versions(named) else None

    def pin(name, digest, where=SOURCE_FIELD):
        found = Pin(name, digest, field=where)
        local = f'{near}/{name}'
        if '/' in name:
            found.path, found.found_by = name, 'path'
        elif where != SOURCE_FIELD and local in sources.digests:
            found.path, found.found_by = local, 'path'
        else:
            # Matching content proves the file; a name is only a plausible guess.
            named = sources.named(name, near, producer)
            if matches := sources.by_digest.get(digest):
                found.path, found.found_by = named if named in matches else matches[0], 'content'
            elif path := earlier(digest, named):
                found.path, found.found_by = path, 'history'
            elif named:
                found.path, found.found_by = named, 'name'
        if found.path is not None:
            found.current = sources.digests.get(found.path)
            if found.current is None and (REPO / found.path).is_file():
                found.current = source_sha256(REPO / found.path)
        return found

    pins = [pin(key, digest) for key, digest in metadata.get(SOURCE_FIELD, {}).items()]
    for where, value in metadata.items():
        if where in SOURCE_PROVENANCE:
            continue
        if isinstance(value, dict):
            pins += [pin(name, digest, where) for name, digest in value.items()
                     if name.endswith(CODE_SUFFIXES) and isinstance(digest, str)
                     and DIGEST.fullmatch(digest)]
        elif isinstance(value, str) and DIGEST.fullmatch(value):
            path, found_by = next(iter(sources.by_digest.get(value, ())), None), 'content'
            if path is None and SOURCE_DIGEST_NAME.search(where):
                path, found_by = earlier(value), 'history'
            if path is not None:
                pins.append(Pin(where, value, path, sources.digests.get(path), where, found_by))
    payload = payload_name(sidecar)
    # Without a command, name the tools that mention the payload file.
    producers = [] if replay or not pins else [name for name in sorted(sources.digests)
                                               if name.endswith('.py')
                                               and Path(payload).name in sources.text(name)]
    return Corpus(sidecar, payload, pins, replay, origin, producers)


def corpora(sources: Sources) -> list[Corpus]:
    """Every tracked sidecar that pins a source (a raw archive's main one)."""
    result = []
    history = History()
    for sidecar in tracked('*.meta.json'):
        if sidecar.endswith('.raw.meta.json'):
            continue
        try:
            metadata = json.loads((REPO / sidecar).read_text(encoding='utf-8'))
        except (OSError, ValueError):
            continue
        if isinstance(metadata, dict) and (corpus := inspect(sidecar, metadata, sources, history)).pins:
            result.append(corpus)
    return result


def old_digests(reference: str, paths: set[str], root: Path = REPO) -> set[str]:
    """Digests of PATHS at REF, which legacy pins recorded before the edit."""
    old = set()
    for name in paths:
        try:
            old.add(text_sha256(subprocess.run(['git', 'show', f'{reference}:{name}'], cwd=root,
                                               check=True, capture_output=True).stdout))
        except (subprocess.CalledProcessError, UnicodeDecodeError):
            continue
    return old


def changed_since(reference: str, root: Path = REPO) -> tuple[set[str], set[str]]:
    """Changed tool/Rust paths since REF (working tree included) and their old digests.

    A rename lists both paths: pins name the old one.
    """
    paths = {name for name in git('diff', '--name-only', '--no-renames', '-z', reference, '--',
                                  root=root).split('\0')
             if name.startswith(('tools/', 'src/')) and name.endswith(CODE_SUFFIXES)}
    return paths, old_digests(reference, paths, root)


def affected(corpora_: list[Corpus], paths: set[str], digests: set[str]) -> list[tuple[Corpus, list[Pin]]]:
    """Corpora with a pin on one of PATHS, or a legacy pin on their earlier content."""
    result = []
    for corpus in corpora_:
        hits = [pin for pin in corpus.pins if pin.path in paths or pin.recorded in digests]
        if hits:
            result.append((corpus, hits))
    return result


def describe(corpus: Corpus, pins: list[Pin], heading: str) -> list[str]:
    lines = [corpus.sidecar]
    groups = defaultdict(list)
    for pin in pins:
        name = pin.path or f'{pin.key} (unresolved label)'
        other = pin.field != SOURCE_FIELD
        groups[other, pin.state].append(f'{pin.field}: {name}' if other else name)
    for other in (False, True):
        for state in ('changed', 'missing', 'unresolved', 'current'):
            if names := sorted(groups[other, state]):
                more = f' and {len(names) - 6} more' if len(names) > 6 else ''
                kind = f'{heading} in other fields' if other else heading
                lines.append(f'  {kind} {state} ({len(names)}): {", ".join(names[:6])}{more}')
    if fields := corpus.pinned_fields():
        lines.append(f'  {", ".join(fields)} also pin sources; a refresh cannot record those')
    if corpus.replay:
        lines.append(f'  replay: {shlex.join(["python", *corpus.replay, "--refresh-sources"])}'
                     f'  [{corpus.replay_origin}]')
    elif corpus.producers:
        lines.append(f'  replay: unknown; {", ".join(corpus.producers)} names {Path(corpus.payload).name}')
    else:
        lines.append(f'  replay: unknown; no tool names {Path(corpus.payload).name}')
    return lines


def failure(stderr: str) -> str:
    """The exception a failed producer raised, else its last error line.

    Output after a traceback (a shutdown warning, say) does not hide it.
    """
    lines = [line for line in stderr.splitlines() if line.strip()]
    starts = [index for index, line in enumerate(lines)
              if line.startswith('Traceback (most recent call last):')]
    if starts:
        return next((line for line in lines[starts[-1] + 1:] if not line.startswith(' ')), lines[-1])
    return lines[-1] if lines else ''


def replay(corpus: Corpus, timeout: float | None) -> tuple[str, str]:
    """Run one recorded replay with --refresh-sources; classify the outcome."""
    if fields := corpus.pinned_fields():
        return 'PINNED', f'{", ".join(fields)} pin changed sources; --refresh-sources cannot record those'
    if not corpus.replay:
        return 'UNKNOWN', 'no replay command recorded or inferable'
    command = [sys.executable, *corpus.replay, '--refresh-sources']
    started = time.monotonic()
    try:
        done = subprocess.run(command, cwd=REPO, capture_output=True, text=True,
                              encoding='utf-8', errors='replace', timeout=timeout)
    except subprocess.TimeoutExpired:
        return 'TIMEOUT', f'no result after {timeout:.0f} s'
    elapsed = f'{time.monotonic() - started:.1f} s'
    if done.returncode:
        reason = failure(done.stderr).replace('\\', '/')
        if reason.split(':', 1)[0].endswith('OracleMismatch'):
            return 'CHANGED', reason
        if 'unrecognized arguments' in reason and '--refresh-sources' in reason:
            return 'UNSUPPORTED', 'the producer does not take --refresh-sources'
        return 'FAILED', reason or f'exit status {done.returncode}'
    # Producers print their payload or sidecar path, absolute or relative.
    lines = [line for line in done.stdout.replace('\\', '/').splitlines()
             if corpus.payload in line or corpus.sidecar in line]
    if not lines:
        return 'ELSEWHERE', f'the command did not compare this corpus ({elapsed})'
    return ('REFRESHED' if any(line.startswith('REFRESHED') for line in lines) else 'CURRENT'), elapsed


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__.split('\n\n')[0],
        epilog='With --replay, exits 1 if a replay changed or compared another corpus, '
               '3 if any corpus could not be confirmed, else 0.')
    commands = parser.add_subparsers(dest='command', required=True)
    found = commands.add_parser('affected', help='corpora whose recorded sources include PATHS')
    found.add_argument('paths', nargs='*', help='repository paths; default: changes since --since')
    found.add_argument('--since', help='Git revision; default: merge base with origin/main')
    commands.add_parser('stale', help='corpora whose recorded sources differ from the checkout')
    for command in commands.choices.values():
        command.add_argument('--replay', action='store_true',
                             help='run each known replay with --refresh-sources, one at a time')
        command.add_argument('--timeout', type=float, help='seconds allowed for each replay')
    args = parser.parse_args(argv)

    sources = Sources.tracked()
    every = corpora(sources)
    if args.command == 'stale':
        selected = [(corpus, corpus.stale()) for corpus in every if corpus.stale()]
        heading = 'pins'
    else:
        if args.paths:
            paths = {(Path.cwd() / path).resolve().relative_to(REPO).as_posix() for path in args.paths}
            digests = old_digests('HEAD', paths)
        else:
            reference = args.since or git('merge-base', 'HEAD', 'origin/main').strip()
            paths, digests = changed_since(reference)
        selected = affected(every, paths, digests)
        heading = 'affected pins'
    for corpus, pins in selected:
        print('\n'.join(describe(corpus, pins, heading)))
    known = sum(1 for corpus, _ in selected if corpus.replay)
    pinned = sum(1 for corpus, _ in selected if corpus.pinned_fields())
    print(f'{len(selected)} of {len(every)} source-pinned corpora; {known} with a known replay command; '
          f'{pinned} pin changed sources in other fields.')
    if not args.replay:
        return 0
    outcomes = defaultdict(int)
    for corpus, _ in selected:
        print(f'replaying {corpus.sidecar} ...', flush=True)
        outcome, detail = replay(corpus, args.timeout)
        outcomes[outcome] += 1
        print(f'{outcome:11} {corpus.sidecar}: {detail}', flush=True)
    print(', '.join(f'{outcomes[outcome]} {outcome.lower()}'
                    for outcome in CONFIRMED + REJECTED + UNCONFIRMED if outcomes[outcome]))
    if any(outcomes[outcome] for outcome in REJECTED):
        return 1
    return 3 if any(outcomes[outcome] for outcome in UNCONFIRMED) else 0


if __name__ == '__main__':
    sys.exit(main())
