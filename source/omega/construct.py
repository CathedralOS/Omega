#!/usr/bin/env python3
"""Invoke the Omega compiler constructor on its actual closed source set.

This adapter orders exact files and explicit package edges. It performs no
Omega parsing, import rewriting, interpretation or code generation. Build
acquisition/provider admission is outside this separately supplied source set.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
PACKAGES = (
    ('omega_compiler_construction', 'source/omega/application/construction', 0),
    ('omega_compiler', 'source/omega/compilation', 0),
    ('psi', 'source/psi', 0),
    ('omega_language_std', 'source/library/std', 2),
    ('omega_language_core', 'source/library/core', 1),
)
# These are explicit construction inputs, matching the ordinary application and
# library declarations. The invocation does not claim to execute those Builds.
EDGES = ((0, 1, 'omega_compiler'), (0, 2, 'psi'),
         (0, 3, 'omega_language_std'), (1, 2, 'psi'))
ENTRY = 'source/omega/application/construction/main.omg'
ENTRY_NAME = b'entry_start'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def encode(packages, edges, units, entry_unit, entry_name):
    """Shared explicit row contract; source-unit contents are never changed."""
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
    if len(package_rows) > 64 or len(edge_rows) > 256 or len(unit_rows) > 512:
        raise ValueError('constructor row capacity exceeded')
    if len(buffer) > 4194304:
        raise ValueError('constructor byte capacity exceeded')
    words = [len(package_rows), len(edge_rows), len(unit_rows), entry_unit,
             entry_start, entry_end, len(buffer)]
    for rows in (package_rows, edge_rows, unit_rows):
        for row in rows:
            words.extend(row)
    wire = struct.pack('<' + 'I' * len(words), *words) + buffer
    return wire, package_rows, edge_rows, unit_rows


def compiler_input():
    sources = []
    declarations = []
    for package, (_, directory, _) in enumerate(PACKAGES):
        base = ROOT / directory
        if not base.is_dir():
            raise FileNotFoundError(base)
        build = base / 'build.omg'
        if build.is_file():
            declarations.append({'path': build.relative_to(ROOT).as_posix(),
                                 'sha256': digest(build.read_bytes())})
        # A package root is explicit. Ignore only invocation build output and
        # its Build declaration; no filename is translated into a module name.
        for path in sorted(base.rglob('*.omg')):
            relative = path.relative_to(base)
            if 'build' in relative.parts or path == build:
                continue
            data = path.read_bytes()
            sources.append((package, path.relative_to(ROOT).as_posix(), data))
    paths = [path for _, path, _ in sources]
    entry_unit = paths.index(ENTRY)
    wire, packages, edges, units = encode(
        [(name, role) for name, _, role in PACKAGES], EDGES,
        [(package, data) for package, _, data in sources], entry_unit, ENTRY_NAME)
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
                   'start': row[1], 'end': row[2], 'sha256': digest(data)}
                  for index, ((package, path, data), row) in enumerate(zip(sources, units))],
    }
    return wire, manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', type=Path, default=ROOT / 'build/omega-construction/input.bin')
    parser.add_argument('--compiler', type=Path, help='exact executable constructor; omitted to only materialize its input')
    parser.add_argument('--output', type=Path, default=ROOT / 'build/omega-construction/omega.tape')
    args = parser.parse_args()
    wire, manifest = compiler_input()
    args.input.parent.mkdir(parents=True, exist_ok=True)
    args.input.write_bytes(wire)
    manifest_path = args.input.with_suffix('.json')
    manifest_path.write_text(json.dumps(manifest, indent=2) + '\n')
    print(f'compiler closure: {len(manifest["units"])} units, {manifest["source_bytes"]} source bytes; entry {ENTRY}::{ENTRY_NAME.decode()}')
    print(f'input: {args.input} sha256={manifest["input_sha256"]}')
    if args.compiler is None:
        return 0
    compiler = args.compiler.resolve(strict=True)
    compiler_sha = digest(compiler.read_bytes())
    args.output.parent.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    with tempfile.TemporaryFile() as pending:
        result = subprocess.run([str(compiler)], input=wire, stdout=pending,
                                stderr=subprocess.PIPE, check=False)
        pending.seek(0)
        tape = pending.read()
    current, _ = compiler_input()
    unchanged = current == wire and digest(compiler.read_bytes()) == compiler_sha
    accepted = result.returncode == 0 and bool(tape) and unchanged
    receipt = {'compiler': str(compiler), 'compiler_sha256': compiler_sha,
               'input_sha256': manifest['input_sha256'], 'elapsed_seconds': time.monotonic() - started,
               'exit': result.returncode, 'stdout_bytes': len(tape),
               'stdout_sha256': digest(tape), 'stderr': result.stderr.decode(errors='replace'),
               'inputs_unchanged': unchanged, 'published': accepted}
    if accepted:
        with tempfile.NamedTemporaryFile(dir=args.output.parent, delete=False) as output:
            output.write(tape)
            temporary = Path(output.name)
        temporary.replace(args.output)
    args.input.with_suffix('.result.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps(receipt, indent=2))
    return 0 if accepted else 1


if __name__ == '__main__':
    raise SystemExit(main())
