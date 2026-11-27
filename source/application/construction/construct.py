#!/usr/bin/env python3
"""Invoke the Omega compiler constructor on its actual closed source set.

This adapter orders exact files and explicit package edges. It performs no
Omega parsing, import rewriting, interpretation or code generation. Build
acquisition/provider admission is outside this separately supplied source set.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[3]
PACKAGES = (
    ('omega_compiler_construction', 'source/application/construction', 0),
    ('omega_compiler', 'source/subsystems/compiler/driver', 0),
    ('psi', 'source/subsystems/compiler/source_to_psi', 0),
    ('system', 'source/library/system', 2),
    ('omega_language_core', 'source/library/core', 1),
    ('compiler_alpha', 'source/subsystems/compiler/alpha', 0),
    ('wire', 'source/foundation/wire', 0),
)
# These are explicit construction inputs, matching the ordinary application and
# library declarations. The invocation does not claim to execute those Builds.
EDGES = ((0, 1, 'omega_compiler'), (0, 2, 'psi'),
         (0, 3, 'system'), (0, 6, 'wire'),
         (1, 2, 'psi'), (1, 5, 'compiler_alpha'), (5, 2, 'psi'))
ENTRY = 'source/application/construction/main.omg'
ENTRY_NAME = b'entry_start'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def validate_build_sources(packages, sources, entries):
    if len(sources) > 512 or len(entries) != len(packages):
        raise ValueError('expected one Build entry selection per package and at most 512 Build units')
    keys = set()
    core = [i for i, (_, role) in enumerate(packages) if role == 1]
    for supplier, path, _ in sources:
        if not 0 <= supplier < len(packages):
            raise ValueError('Build source has no owning package')
        if any(part in ('', '.', '..') for part in path.split('/')) or '\\' in path or '\x00' in path:
            raise ValueError('invalid package-relative Build source path')
        if (supplier, path) in keys:
            raise ValueError('duplicate package-relative Build source')
        keys.add((supplier, path))
    for index, (requester, source) in enumerate(entries):
        if requester != index or not 0 <= source < len(sources):
            raise ValueError('missing, duplicate or invalid package Build entry selection')
        supplier, path, _ = sources[source]
        if (supplier, path) == (requester, 'build.omg'):
            continue
        if core != [supplier] or path != 'defaults/build.omg' or (requester, 'build.omg') in keys:
            raise ValueError('Build entry is neither its package root nor the selected core default')


def encode(packages, edges, units, entry_unit, entry_name, *, paths,
           build_sources=None, build_entries=None):
    """Return the wire and the three original Product metadata tables."""
    return _encode(packages, edges, units, entry_unit, entry_name, paths,
                   build_sources, build_entries)[:4]


def _encode(packages, edges, units, entry_unit, entry_name, paths, build_sources, build_entries):
    if (build_sources is None) != (build_entries is None):
        raise ValueError('Build files and entry selections must be supplied together')
    if build_sources is not None:
        validate_build_sources(packages, build_sources, build_entries)
    buffer = bytearray()

    def retain(data):
        start = len(buffer)
        buffer.extend(data)
        return start, len(buffer)

    package_rows = [(*retain(name.encode('ascii')), role) for name, role in packages]
    edge_rows = [(requester, target, *retain(alias.encode('ascii')))
                 for requester, target, alias in edges]
    entry_start, entry_end = retain(entry_name)
    unit_rows = [(package, *retain(data)) for package, data in units]
    # Keep source coordinates independent of the lengths of the filenames.
    unit_rows = [(*row, *retain(path.encode('utf-8')))
                 for row, path in zip(unit_rows, paths, strict=True)]
    build_rows = []
    if build_sources is not None:
        build_rows = [(supplier, *retain(data), *retain(path.encode('utf-8')))
                      for supplier, path, data in build_sources]
    if len(package_rows) > 64 or len(edge_rows) > 256 or len(unit_rows) > 512:
        raise ValueError('constructor row capacity exceeded')
    if len(buffer) > 8 * 1024 * 1024:
        raise ValueError('constructor byte capacity exceeded')
    version = 1 if build_sources is None else 2
    words = [0, version, len(package_rows), len(edge_rows), len(unit_rows), entry_unit,
             entry_start, entry_end, len(buffer)]
    if build_sources is not None:
        words.append(len(build_rows))
    for rows in (package_rows, edge_rows, unit_rows, build_entries or (), build_rows):
        for row in rows:
            words.extend(row)
    wire = struct.pack('<' + 'I' * len(words), *words) + buffer
    return wire, package_rows, edge_rows, unit_rows, build_rows


def compiler_source_paths(base):
    paths = []
    for directory, children, files in os.walk(base):
        parent = Path(directory)
        excluded = {'.git'}
        if parent == base:
            excluded.add('build')
        children[:] = [name for name in children if name not in excluded]
        paths.extend(parent / name for name in files if name.endswith('.omg'))
    return sorted(paths)


def compiler_input(with_build_sources=False):
    sources = []
    declarations = []
    build_sources = []
    for package, (_, directory, role) in enumerate(PACKAGES):
        base = ROOT / directory
        if not base.is_dir():
            raise FileNotFoundError(base)
        build_inputs = {base / 'build.omg'}
        if role == 1:
            # The Rust driver admits the core default Build in build scope.
            build_inputs.add(base / 'defaults/build.omg')
        for path in compiler_source_paths(base):
            relative = path.relative_to(base)
            data = path.read_bytes()
            if path in build_inputs:
                build_sources.append((package, relative.as_posix(), data))
                declarations.append({'path': path.relative_to(ROOT).as_posix(),
                                     'sha256': digest(data)})
                continue
            sources.append((package, path.relative_to(ROOT).as_posix(), data))
    paths = [path for _, path, _ in sources]
    entry_unit = paths.index(ENTRY)
    build_entries = []
    if with_build_sources:
        roots = {(supplier, path): index for index, (supplier, path, _) in enumerate(build_sources)}
        default = next((roots.get((i, 'defaults/build.omg')) for i, (_, _, role) in enumerate(PACKAGES)
                        if role == 1), None)
        for requester in range(len(PACKAGES)):
            source = roots.get((requester, 'build.omg'), default)
            if source is None:
                raise ValueError('package has neither its own Build nor a captured core default')
            build_entries.append((requester, source))
    wire, packages, edges, units, build_rows = _encode(
        [(name, role) for name, _, role in PACKAGES], EDGES,
        [(package, data) for package, _, data in sources], entry_unit, ENTRY_NAME,
        [Path(path).relative_to(PACKAGES[package][1]).as_posix()
         for package, path, _ in sources],
        build_sources if with_build_sources else None, build_entries if with_build_sources else None)
    manifest = {
        'contract': 'closed-source-set construction; not OCREQ admission',
        'target': 'Alpha', 'entry_unit': entry_unit, 'entry_name': ENTRY_NAME.decode(),
        'input_sha256': digest(wire), 'input_bytes': len(wire),
        'source_bytes': sum(len(data) for _, _, data in sources),
        'packages': [{'name': name, 'root': root, 'role': role, 'row': list(row)}
                     for (name, root, role), row in zip(PACKAGES, packages)],
        'edges': [list(row) for row in edges],
        'build_declarations_not_executed': declarations,
        'units': [{'id': index, 'package': package, 'path': path,
                   'start': row[1], 'end': row[2], 'path_start': row[3], 'path_end': row[4],
                   'sha256': digest(data)}
                  for index, ((package, path, data), row) in enumerate(zip(sources, units))],
    }
    if with_build_sources:
        manifest['closed_set_version'] = 2
        manifest['build_entries'] = build_entries
        manifest['build_units'] = [
            {'id': index, 'supplier': supplier, 'path': path, 'sha256': digest(data),
             'source_start': row[1], 'source_end': row[2], 'path_start': row[3], 'path_end': row[4]}
            for index, ((supplier, path, data), row) in enumerate(zip(build_sources, build_rows, strict=True))]
    return wire, manifest


def epsilon_tape(observation):
    """Extract stdout only when the Epsilon construction entry exited zero."""
    if len(observation) < 5 or observation[0] != 0:
        return b'', {'epsilon_observation_prefix': observation[:40].hex()}
    code = int.from_bytes(observation[1:5], 'little', signed=True)
    outcome = {'epsilon_exit': code}
    if code == 0:
        return observation[5:], outcome
    if code in (1, 2, 3) and len(observation) == 9:
        outcome['source_offset'] = int.from_bytes(observation[5:], 'little')
    return b'', outcome


def stamp_constructor(tape, executable):
    # The bootstrap helper verifies the seed for this host before stamping.
    subprocess.run(['sh', '-eu', '-c', '''
        OMEGA_REPO_ROOT=$1
        . "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
        . "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"
        stamp_seed "$2" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$3"
    ''', 'stamp-constructor', str(ROOT), str(tape), str(executable)], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', type=Path, default=ROOT / 'build/omega-construction/input.bin')
    parser.add_argument('--with-build-sources', action='store_true',
                        help='include package-owned Build sources and entry selections in a closed-set v2 input')
    producers = parser.add_mutually_exclusive_group()
    producers.add_argument('--compiler', type=Path, help='executable Omega constructor that writes raw tape')
    producers.add_argument('--epsilon-compiler', type=Path,
                           help='compiled Epsilon constructor that writes a framed guest outcome')
    producers.add_argument('--compiler-tape', type=Path, help='Omega constructor tape to run on the Alpha seed')
    parser.add_argument('--output', type=Path, default=ROOT / 'build/omega-construction/omega.tape')
    args = parser.parse_args()
    producer = args.compiler or args.epsilon_compiler or args.compiler_tape
    if producer is not None:
        paths = [args.input, args.input.with_suffix('.json'), args.input.with_suffix('.result.json'),
                 producer, args.output]
        if len({path.resolve() for path in paths}) != len(paths):
            parser.error('the constructor, input, manifests and output need distinct paths')
    wire, manifest = compiler_input(with_build_sources=args.with_build_sources)
    args.input.parent.mkdir(parents=True, exist_ok=True)
    args.input.write_bytes(wire)
    manifest_path = args.input.with_suffix('.json')
    manifest_path.write_text(json.dumps(manifest, indent=2) + '\n')
    print(f'compiler closure: {len(manifest["units"])} units, {manifest["source_bytes"]} source bytes; entry {ENTRY}::{ENTRY_NAME.decode()}')
    print(f'input: {args.input} sha256={manifest["input_sha256"]}')
    if producer is None:
        return 0
    compiler = producer.resolve(strict=True)
    compiler_sha = digest(compiler.read_bytes())
    args.output.parent.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    with tempfile.TemporaryDirectory() as temp:
        runner = compiler
        if args.compiler_tape:
            runner = Path(temp) / 'constructor'
            stamp_constructor(compiler, runner)
        runner_sha = digest(runner.read_bytes()) if args.compiler_tape else compiler_sha
        result = subprocess.run([str(runner)], input=wire, capture_output=True, check=False)
    if args.epsilon_compiler:
        producer_kind = 'epsilon-executable'
        tape, outcome = epsilon_tape(result.stdout)
    else:
        producer_kind = 'alpha-tape' if args.compiler_tape else 'executable'
        tape, outcome = result.stdout, {}
    current, _ = compiler_input(with_build_sources=args.with_build_sources)
    unchanged = current == wire and digest(compiler.read_bytes()) == compiler_sha
    accepted = result.returncode == 0 and bool(tape) and unchanged
    receipt = {'compiler': str(compiler), 'compiler_kind': producer_kind, 'compiler_sha256': compiler_sha,
               'input_sha256': manifest['input_sha256'], 'elapsed_seconds': time.monotonic() - started,
               'runner_sha256': runner_sha, 'exit': result.returncode, 'stdout_bytes': len(result.stdout),
               'stdout_sha256': digest(result.stdout), 'stderr': result.stderr.decode(errors='replace'),
               'inputs_unchanged': unchanged, 'published': accepted, **outcome}
    if accepted:
        receipt.update(tape_bytes=len(tape), tape_sha256=digest(tape))
        with tempfile.NamedTemporaryFile(dir=args.output.parent, delete=False) as output:
            output.write(tape)
            temporary = Path(output.name)
        temporary.replace(args.output)
    args.input.with_suffix('.result.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps(receipt, indent=2))
    return 0 if accepted else 1


if __name__ == '__main__':
    raise SystemExit(main())
