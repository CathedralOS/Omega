"""Verify standalone construction sources against package roots in a fixed Git tree."""
import argparse
import re
import struct
import subprocess
from pathlib import Path


def canonical_path(raw):
    parts = raw.split(b'/')
    return bool(parts) and all(part not in (b'', b'.', b'..')
                               and re.fullmatch(rb'[A-Za-z0-9_.-]+', part) for part in parts)


def verify(snapshot, revision, roots, wire):
    """Return the external root table only after checking every supplied source byte."""
    if not re.fullmatch('[0-9a-f]{40}|[0-9a-f]{64}', revision):
        raise ValueError('select an exact immutable Git commit')
    roots = [root.encode('ascii') for root in roots]
    if len(roots) != len(set(roots)) or not all(map(canonical_path, roots)):
        raise ValueError('package roots must be distinct canonical relative paths')

    if any(other.startswith(root + b'/') for root in roots for other in roots):
        raise ValueError('overlapping package roots require a separate package inventory')

    def git(*args):
        return subprocess.check_output(['git', '--no-replace-objects', '-C', str(snapshot), *args])

    if git('cat-file', '-t', revision).strip() != b'commit':
        raise ValueError('deployment revision must name a commit')
    if len(wire) < 40:
        raise ValueError('truncated source request')
    zero, version, packages, edges, units, _, _, _, size, builds = struct.unpack_from('<10I', wire)
    if (zero, version, packages) != (0, 2, len(roots)) or not (0 < packages <= 64 and edges <= 256
                                                               and 0 < units <= 512 and 0 < builds <= 512):
        raise ValueError('unsupported deployment header')
    unit_table = 40 + 12 * packages + 16 * edges
    build_table = unit_table + 20 * units + 8 * packages
    arena_start = build_table + 20 * builds
    if arena_start + size != len(wire):
        raise ValueError('deployment frame length differs from its header')
    arena = memoryview(wire)[arena_start:]
    supplied = [{} for _ in roots]
    for start, count in ((unit_table, units), (build_table, builds)):
        for row in range(count):
            owner, lo, hi, path_lo, path_hi = struct.unpack_from('<5I', wire, start + 20 * row)
            if owner >= packages or not (lo <= hi <= size and path_lo < path_hi <= size):
                raise ValueError('invalid source row')
            path = bytes(arena[path_lo:path_hi])
            if not canonical_path(path) or path in supplied[owner]:
                raise ValueError('duplicate or invalid package source path')
            supplied[owner][path] = arena[lo:hi]

    for root, expected in zip(roots, supplied):
        inventory = set()
        for record in git('ls-tree', '-rz', revision, '--', root).split(b'\0'):
            if not record:
                continue
            metadata, path = record.split(b'\t', 1)
            mode, kind, oid = metadata.split()
            if not path.startswith(root + b'/'):
                raise ValueError('invalid package root')
            relative = path[len(root) + 1:]
            if (mode, kind) not in ((b'100644', b'blob'), (b'100755', b'blob')):
                raise ValueError(f'unsupported package entry mode: {path!r}')
            if not relative.endswith(b'.omg'):
                continue
            if relative not in expected or git('cat-file', 'blob', oid) != expected[relative]:
                raise ValueError(f'package source differs at {path!r}')
            inventory.add(relative)
        if not inventory or inventory != expected.keys():
            raise ValueError(f'package source inventory differs at {root!r}')
    return b''.join(root + b'\0' for root in roots)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('snapshot', type=Path)
    parser.add_argument('revision')
    parser.add_argument('request', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('roots', nargs='+', help='package roots in the externally selected deployment order')
    parser.add_argument('--embedding', type=Path, help='also write the exact Epsilon constructor for this verified deployment')
    args = parser.parse_args()
    wire = args.request.read_bytes()
    roots = verify(args.snapshot, args.revision, args.roots, wire)
    args.output.write_bytes(roots)
    if args.embedding:
        from process_domain import embedding, pack
        from tempfile import TemporaryDirectory
        with TemporaryDirectory() as temporary:
            library = pack('bootstrap/omega/compiler/omega_compiler.epsilon.sources', Path(temporary) / 'library.epsilon')
            args.embedding.write_bytes(embedding(library, wire, roots))


if __name__ == '__main__':
    main()
