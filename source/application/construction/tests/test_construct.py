"""Exercise construction request framing and outcome publication."""
import contextlib
import io
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import construct


class BuildSourceTests(unittest.TestCase):
    packages = [('app', 0), ('core', 1), ('library', 0)]
    sources = [(0, 'build.omg', b'// app\r\nmachine build(builder: @mutable Build) {}'),
               (1, 'defaults/build.omg', b'machine build(builder: @mutable Build) {}')]
    entries = [(0, 0), (1, 1), (2, 1)]

    def frame(self, packages=None, sources=None, entries=None):
        return construct.encode(
            self.packages if packages is None else packages, [], [(0, b'machine entry() {}')],
            0, b'entry', paths=['main.omg'],
            build_sources=self.sources if sources is None else sources,
            build_entries=self.entries if entries is None else entries)[0]

    def test_scoped_frame_accepts_eight_mib_and_rejects_one_more_byte(self):
        paths = [f'unit_{i}.omg' for i in range(64)]
        units = [(0, b'')] * len(paths)

        def encode():
            return construct.encode(self.packages, [], units, 0, b'entry', paths=paths,
                                    build_sources=self.sources, build_entries=self.entries)[0]

        metadata_size = struct.unpack_from('<I', encode(), 32)[0]
        unit_size = 128 * 1024
        full_unit = bytes(unit_size)
        units[:-1] = [(0, full_unit)] * (len(units) - 1)
        last_size = 8 * 1024 * 1024 - metadata_size - (len(units) - 1) * unit_size
        units[-1] = (0, bytes(last_size))
        self.assertEqual(struct.unpack_from('<I', encode(), 32)[0], 8 * 1024 * 1024)

        units[-1] = (0, bytes(last_size + 1))
        with self.assertRaisesRegex(ValueError, 'constructor byte capacity exceeded'):
            encode()

    def test_v2_exact_build_bytes_and_scopes(self):
        wire = self.frame()
        (marker, version, packages, edges, units, entry, name_start, name_end,
         length, builds) = struct.unpack_from('<10I', wire)
        self.assertEqual((marker, version, packages, edges, units, entry, builds), (0, 2, 3, 0, 1, 0, 2))
        arena = wire[-length:]
        self.assertEqual(arena[name_start:name_end], b'entry')
        selections = 4 * (10 + 3 * packages + 4 * edges + 5 * units)
        files = selections + 8 * packages
        self.assertEqual(list(struct.iter_unpack('<2I', wire[selections:files])), self.entries)
        rows = list(struct.iter_unpack('<5I', wire[files:-length]))
        self.assertEqual([(owner, arena[p:q].decode(), arena[a:b]) for owner, a, b, p, q in rows],
                         self.sources)

    def test_v2_requires_owned_entries_and_unique_files(self):
        cases = {
            'missing selection': dict(entries=self.entries[:-1]),
            'duplicate requester': dict(entries=[(0, 0), (0, 1), (2, 1)]),
            'missing file': dict(entries=[(0, 0), (1, 3), (2, 1)]),
            'other package root': dict(entries=[(0, 0), (1, 1), (2, 0)]),
            'shadowed default': dict(entries=[(0, 1), (1, 1), (2, 1)]),
            'duplicate source': dict(sources=self.sources + [self.sources[0]]),
            'ordinary fallback supplier': dict(sources=[self.sources[0], (2, *self.sources[1][1:])]),
            'ambiguous core': dict(packages=[('app', 0), ('core', 1), ('other', 1)]),
            'parent path': dict(sources=self.sources + [(0, '../extra.omg', b'')]),
            'empty component': dict(sources=self.sources + [(0, 'dir//extra.omg', b'')]),
            'unknown supplier': dict(sources=self.sources + [(3, 'extra.omg', b'')]),
        }
        for name, changes in cases.items():
            with self.subTest(name=name), self.assertRaises(ValueError):
                self.frame(**changes)

    def test_compiler_input_keeps_nested_build_sources(self):
        with tempfile.TemporaryDirectory() as temp, patch.object(construct, 'ROOT', Path(temp)):
            for _, package, _ in construct.PACKAGES:
                root = Path(temp) / package
                root.mkdir(parents=True)
                (root / 'build.omg').write_text('machine build(builder: @mutable Build) {}')
                (root / 'main.omg').write_text('machine entry_start() {}')
            psi = Path(temp) / 'source/subsystems/compiler/source_to_psi'
            before = [construct.compiler_input(scoped)[0] for scoped in (False, True)]
            for directory in ('tests/fixtures/example', 'build', '.git', 'tools/.git'):
                excluded = psi / directory
                excluded.mkdir(parents=True)
                (excluded / 'build.omg').write_text('machine build(builder: @mutable Build) {}')
                (excluded / 'main.omg').write_text('machine main() -> u8 { 42 }')
            nested = psi / 'tools/build/reader.omg'
            nested.parent.mkdir(parents=True)
            contents = b'machine read() -> u8 { 7 }'
            nested.write_bytes(contents)
            for scoped in (False, True):
                with self.subTest(scoped=scoped):
                    packet, manifest = construct.compiler_input(scoped)
                    path = nested.relative_to(construct.ROOT).as_posix()
                    unit, = (row for row in manifest['units'] if row['path'] == path)
                    arena = packet[-struct.unpack_from('<I', packet, 32)[0]:]
                    self.assertEqual(arena[unit['start']:unit['end']], contents)
                    self.assertEqual(arena[unit['path_start']:unit['path_end']], b'tools/build/reader.omg')
            nested.unlink()
            self.assertEqual(before, [construct.compiler_input(scoped)[0] for scoped in (False, True)])

    def test_complete_capture_preserves_product_coordinates(self):
        legacy, before = construct.compiler_input()
        scoped, after = construct.compiler_input(with_build_sources=True)
        self.assertEqual(struct.unpack_from('<II', legacy), (0, 1))
        self.assertEqual(struct.unpack_from('<II', scoped), (0, 2))
        self.assertEqual((before['units'], before['entry_unit']), (after['units'], after['entry_unit']))
        arena = scoped[-struct.unpack_from('<I', scoped, 32)[0]:]
        for row in after['build_units']:
            owner = construct.PACKAGES[row['supplier']][1]
            path = arena[row['path_start']:row['path_end']].decode()
            data = arena[row['source_start']:row['source_end']]
            self.assertEqual(path, row['path'])
            self.assertEqual(data, (construct.ROOT / owner / path).read_bytes())
            self.assertEqual(construct.digest(data), row['sha256'])
        for requester, selected in after['build_entries']:
            row = after['build_units'][selected]
            root = construct.ROOT / construct.PACKAGES[requester][1] / 'build.omg'
            if root.is_file():
                self.assertEqual((row['supplier'], row['path']), (requester, 'build.omg'))
            else:
                self.assertEqual((construct.PACKAGES[row['supplier']][2], row['path']),
                                 (1, 'defaults/build.omg'))


class ConstructionOutputTests(unittest.TestCase):
    def test_raw_and_framed_publication(self):
        payload = b'\0\0\0\0\0arbitrary tape bytes'
        success = bytes(5) + payload
        refusal = struct.pack('<BiI', 0, 2, 73)
        cases = [
            ('--compiler', payload, 0, payload),
            ('--epsilon-compiler', success, 0, payload),
            ('--epsilon-compiler', success, 1, None),
            ('--epsilon-compiler', bytes(5), 0, None),
            ('--epsilon-compiler', bytes(4), 0, None),
            ('--epsilon-compiler', refusal, 0, None),
            ('--epsilon-compiler', b'\x01\x02prefix', 0, None),
            ('--epsilon-compiler', b'\x02\x12\x04\0\0\0', 0, None),
            ('--epsilon-compiler', b'\xffEEOUT\x01\0' + bytes(32), 0, None),
        ]
        with tempfile.TemporaryDirectory() as temp:
            work = Path(temp)
            output, request = work / 'result.tape', work / 'request.bin'
            for option, stdout, exit_code, expected in cases:
                with self.subTest(option=option, stdout=stdout, exit_code=exit_code):
                    # Python reads the request from stdin and returns this case's bytes and status.
                    wire = (f'import sys\nsys.stdout.buffer.write({stdout!r})\n'
                            f'raise SystemExit({exit_code})\n').encode('ascii')
                    manifest = {'units': [], 'source_bytes': 0, 'input_sha256': construct.digest(wire)}
                    output.write_bytes(b'previous tape')
                    args = ['construct.py', option, sys.executable, '--input', str(request), '--output', str(output)]
                    with (patch.object(construct, 'compiler_input', return_value=(wire, manifest)),
                          patch.object(sys, 'argv', args), contextlib.redirect_stdout(io.StringIO())):
                        self.assertEqual(construct.main(), int(expected is None))
                    receipt = json.loads(request.with_suffix('.result.json').read_text())
                    self.assertEqual(request.read_bytes(), wire)
                    self.assertEqual(receipt['exit'], exit_code)
                    self.assertEqual(receipt['published'], expected is not None)
                    self.assertEqual(receipt['stdout_sha256'], construct.digest(stdout))
                    self.assertEqual(output.read_bytes(), expected if expected is not None else b'previous tape')
                    if expected is not None:
                        self.assertEqual(receipt['tape_sha256'], construct.digest(expected))
                    if stdout == refusal:
                        self.assertEqual((receipt['epsilon_exit'], receipt['source_offset']), (2, 73))


if __name__ == '__main__':
    unittest.main()
