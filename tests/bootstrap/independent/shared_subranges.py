#!/usr/bin/env python3
"""Exercise shared-byte reborrows through a supplied source-built constructor."""
import argparse
from pathlib import Path
import runpy
import struct
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'tests/bootstrap'))
from compiled import execute, stamp
encode = runpy.run_path(str(ROOT / 'source/application/construction/construct.py'))['encode']

FIRST = '''data Buffer { bytes: [u8; 3]; }
machine first(part: @[u8]) -> u8 {
    transition part.length > 0 { true -> byte(part), _ -> 0 }
    machine byte(part: @[u8]) -> u8 requires part.length > 0 { part[0] }
}\n'''
MAIN = '''machine main() -> u8 {
    let mut buffer: Buffer; buffer.bytes[0] = 41; buffer.bytes[1] = 42; buffer.bytes[2] = 43;
    cut(@buffer.bytes[..], LO, HI)
}\n'''


def program(range_, requires='lo <= hi && hi <= source.length', lo=1, hi=2, terminal=False):
    body = 'first(@source[' + range_ + '])'
    if terminal:
        body = 'transition { _ -> local(@source[' + range_ + ']) } machine local(part: @[u8]) -> u8 { first(part) }'
    contract = (' requires ' + requires) if requires else ''
    return FIRST + 'machine cut(source: @[u8], lo: u64, hi: u64) -> u8' + contract + ' { ' + body + ' }\n' + MAIN.replace('LO', str(lo)).replace('HI', str(hi))


CASES = (
    ('dynamic', program('lo..hi'), 42),
    ('terminal', program('lo..hi', terminal=True), 42),
    ('bare', program('lo..hi').replace('first(@source[', 'first(source['), 42),
    ('bare-terminal', program('lo..hi', terminal=True).replace('local(@source[', 'local(source['), 42),
    ('argument-staging', program('lo..hi').replace('first(@source[lo..hi])', 'keep(@source[lo..hi], @source[..])') +
        'machine keep(part: @[u8], other: @[u8]) -> u8 { first(part) }', 42),
    ('repeated-tail', FIRST + """machine cut(source: @[u8], lo: u64, hi: u64) -> u8 {
        transition { _ -> walk(source) }
        machine walk(part: @[u8]) -> u8 {
            transition part.length > 1 { true -> step(part), _ -> finish(part) }
        }
        machine step(part: @[u8]) -> u8 requires part.length > 1 { transition { _ -> walk(@part[1..]) } }
        machine finish(part: @[u8]) -> u8 { first(part) }
    }
""" + MAIN.replace('LO', '1').replace('HI', '2'), 43),
    ('empty-at-end', program('lo..lo', 'lo <= source.length', lo=3), 0),
    ('open-end', program('lo..', 'lo <= source.length'), 42),
    ('open-start', program('..hi', 'hi <= source.length'), 41),
    ('whole', program('..', ''), 41),
    ('metadata-empty', program('source.length..source.length', ''), 0),
    ('explicit-length', program('lo..source.length', 'lo <= source.length'), 42),
    ('literal', program('1..2', 'source.length >= 2'), 42),
)
REFUSALS = (
    ('retired-length-endpoint', program('lo..source.len', 'lo <= source.length'), 2),
    ('missing-order', program('lo..hi', 'hi <= source.length'), 2),
    ('missing-extent', program('lo..hi', 'lo <= hi'), 2),
    ('reversed-order', program('lo..hi', 'hi <= lo && hi <= source.length', lo=2, hi=1), 2),
    ('beyond-end', program('lo..hi', lo=0, hi=4), 2),
    ('empty-not-readable', program('lo..lo', 'lo <= source.length', lo=3).replace(FIRST,
        'data Buffer { bytes: [u8; 3]; } machine first(part: @[u8]) -> u8 requires part.length > 0 { part[0] }'), 2),
    ('inclusive-unsupported', program('lo..=hi'), 2),
    ('mutable-from-shared', program('lo..hi').replace('first(@source[', 'first(@mut source['), 2),
    ('effectful-endpoint', program('lo..endpoint()', 'lo <= source.length') + 'machine endpoint() -> u64 { 2 }', 2),
    ('wrong-slice', FIRST + '''machine cut(source: @[u8], other: @[u8], lo: u64, hi: u64) -> u8
        requires lo <= hi && hi <= other.length { first(@source[lo..hi]) }
        machine main() -> u8 { let buffer: Buffer; cut(@buffer.bytes[..], @buffer.bytes[..], 1, 2) }''', 2),
    ('bool-endpoint', FIRST + '''machine cut(source: @[u8], hi: bool) -> u8 { first(@source[0..hi]) }
        machine main() -> u8 { let buffer: Buffer; cut(@buffer.bytes[..], true) }''', 1),
    ('fresh-local-tail', FIRST + '''machine cut(source: @[u8]) -> u8 {
        let local: Buffer; transition { _ -> local_read(@local.bytes[..]) }
        machine local_read(part: @[u8]) -> u8 { first(part) }
        } machine main() -> u8 { let buffer: Buffer; cut(@buffer.bytes[..]) }''', 2),
)


def product_helpers():
    source = (ROOT / 'source/application/construction/build_sources.omg').read_text()
    declarations = source[source.index('machine same_build_path('):source.index('machine has_build_file(')]
    for name, path, valid in (('valid-path', 'alpha/build.omg', True),
                              ('traversal', 'alpha/../build.omg', False),
                              ('empty-component', 'alpha//build.omg', False)):
        arms = 'true -> 42, _ -> 7' if valid else 'true -> 7, _ -> 42'
        entry = 'machine main() -> u8 crashes Trap { transition build_path_valid("' + path + '") { ' + arms + ' } }\n'
        yield 'product-' + name, declarations + entry, 42


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('constructor', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--seconds', type=int)
    parser.add_argument('--product-helpers', action='store_true', help='also require the three unchanged product helper programs to execute42')
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    positives = (*CASES, *(product_helpers() if args.product_helpers else ()))
    for name, source, expected in (*((n, s, ('run', v)) for n, s, v in positives),
                                   *((n, s, ('refuse', v)) for n, s, v in REFUSALS)):
        packet = encode([('app', 0)], [], [(0, source.encode('ascii'))], 0, b'main', paths=['main.omg'])[0]
        (out / (name + '.bin')).write_bytes(packet)
        process = subprocess.run([args.constructor.resolve()], input=packet, capture_output=True, timeout=args.seconds)
        data = process.stdout
        (out / (name + '.eeout')).write_bytes(data)
        assert process.returncode == 0 and process.stderr == b'', (name, process.returncode, process.stderr)
        assert len(data) >= 5 and data[0] == 0, (name, data[:40])
        status = struct.unpack_from('<i', data, 1)[0]
        if expected[0] == 'refuse':
            assert status == expected[1] and len(data) == 9, (name, data[:40])
        else:
            assert status == 0 and len(data) > 5, (name, data[:40])
            tape = out / (name + '.tape')
            tape.write_bytes(data[5:])
            image = out / name
            stamp(tape, image)
            assert execute(image, timeout=args.seconds) == (expected[1], b''), name
        print(f'{name}: passed', flush=True)


if __name__ == '__main__':
    main()
