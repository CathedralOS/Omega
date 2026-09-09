#!/usr/bin/env python3
"""Hand-encoded Alpha bounds cases; no assembler or higher bootstrap rung."""
import argparse
from pathlib import Path
import platform
import struct
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
MEMORY = 0x70000000
ORIGIN = 0x10000000
MAX_TAPE = 0xfffffc
MASK = (1 << 64) - 1


def word(value):
    return struct.pack('<Q', value)


def imm(register, value):
    return bytes((1, register)) + word(value)


def jump(target):
    return b'\x0c' + word(target)


# Address 9 holds halt r255. Execution starts at 11 and writes an exact prefix.
PREFIX = jump(11) + b'\x00\xff' + imm(0, 65) + b'\x12\x00' + imm(1, 1)


def put(address, data):
    # Ordinary mutable-code stores; scratch registers do not alias Windows I/O.
    return b''.join(imm(200, address + i) + imm(201, byte) + b'\x09\xc8\xc9'
                    for i, byte in enumerate(data))


def cases():
    # The semantic memory boundary, not the end of the loaded tape, controls
    # decoding. Exercise every opcode's exact full width and one missing byte.
    instructions = [
        b'\x00\xff', imm(0, 42),
        *(bytes((op, 0, 1)) for op in range(2, 12)),
        jump(9), b'\x0d\x02' + word(9), b'\x0e\x01' + word(9),
        b'\x0f\x02\x01' + word(9), b'\x10\x01\x01' + word(9),
        b'\x11\x00', b'\x12\x00', b'\x13' + word(9), b'\x14',
    ]
    for op, instruction in enumerate(instructions):
        setup = PREFIX
        if op == 20:
            setup += imm(200, ORIGIN) + imm(201, 9) + b'\x0b\xc8\xc9'
        exact = MEMORY - len(instruction)
        status = 0 if op in (0, 12, 13, 14, 15, 16, 19, 20) else 'trap'
        output = b'AA' if op == 18 else b'A'
        yield f'op-{op:02x}-exact', setup + put(exact, instruction) + jump(exact), status, output
        # ret has no operand: its adjacent control is a failed opcode fetch.
        adjacent = exact + 1
        yield f'op-{op:02x}-adjacent', setup + put(adjacent, instruction[:-1]) + jump(adjacent), 'trap', b'A'

    for target in (MEMORY, MASK, MASK - 0x1000):
        yield f'fetch-{target:x}', PREFIX + jump(target), 'trap', b'A'
    yield 'unknown-final-byte', PREFIX + put(MEMORY - 1, b'\xff') + jump(MEMORY - 1), 'trap', b'A'
    # Untaken branches decode their full operands but never validate the target.
    for op, operands in ((13, b'\x01'), (14, b'\x02'), (15, b'\x01\x02'), (16, b'\x01\x02')):
        yield f'untaken-{op:02x}', PREFIX + bytes((op,)) + operands + word(MASK) + b'\x00\xff', 0, b'A'
    for op, width in ((8, 1), (9, 1), (10, 8), (11, 8)):
        for address in dict.fromkeys((MEMORY - width + 1, MEMORY, MASK, MASK - 3)):
            operands = b'\x02\x01' if op in (8, 10) else b'\x01\x00'
            yield f'data-{op:02x}-{address:x}', PREFIX + imm(1, address) + bytes((op,)) + operands + b'\x00\xff', 'trap', b'A'
    for width, store, load in ((1, 9, 8), (8, 11, 10)):
        value = 0x41 if width == 1 else 0x8877665544332241
        body = PREFIX + imm(1, MEMORY - width) + imm(2, value) + bytes((store, 1, 2, load, 3, 1))
        # Compare all 64 bits: a byte-only implementation must not pass the word case.
        body += b'\x10\x02\x03' + word(len(body) + 12) + b'\xff'
        yield f'data-{width}-exact', body + b'\x12\x03\x00\xff', 0, b'AA'
    yield 'unaligned-word', PREFIX + imm(1, 257) + imm(2, 66) + b'\x0b\x01\x02\x0a\x03\x01\x12\x03\x00\xff', 0, b'AB'
    # ret can read above its initial origin without a preceding call.
    yield 'ret-above-origin', PREFIX + put(256, b'\x14') + imm(200, ORIGIN) + imm(201, 256) + b'\x0b\xc8\xc9' + imm(200, ORIGIN + 8) + imm(201, 9) + b'\x0b\xc8\xc9\x14', 0, b'A'
    # Read the call target before the stack store overwrites that same operand.
    location = ORIGIN - 9
    yield 'call-operand-overlap', PREFIX + put(location, b'\x13' + word(9)) + jump(location), 0, b'A'
    yield 'call-invalid-target', PREFIX + b'\x13' + word(MASK), 'trap', b'A'
    yield 'loader-exact', b'\x00\xff' + bytes(MAX_TAPE - 2), 0, b''


def stack_cases():
    # Real profile boundaries, reached by ordinary Alpha instructions. Keep
    # recursive code above the stack origin so the last push may overwrite M[0].
    location = ORIGIN + 4096
    for adjacent in (False, True):
        loop = b'\x04\x04\x01' + b'\x0d\x04' + word(location + 22)
        loop += b'\x13' + word(location)
        done = b'\x13' + word(location) if adjacent else b'\x00\xff'
        yield f'call-stack-{ "adjacent" if adjacent else "exact" }', PREFIX + imm(4, ORIGIN // 8) + put(location, loop + done) + b'\x13' + word(location), 'trap' if adjacent else 0, b'A'
    # Zero-filled stack words return to M[0], changed to ret after setup. The
    # exact case's final legal word instead returns to halt; the adjacent case
    # attempts one more ret at sp == MEMORY. No private test-only stack pointer.
    for adjacent in (False, True):
        setup = PREFIX + put(0, b'\x14')
        if not adjacent:
            setup += imm(200, MEMORY - 8) + imm(201, 9) + b'\x0b\xc8\xc9'
        yield f'ret-stack-{ "adjacent" if adjacent else "exact" }', setup + jump(0), 'trap' if adjacent else 0, b'A'
    # After the final legal ret, sp == MEMORY: a call may push back into the
    # final word. The initial stack origin is not an upper stack partition.
    setup = PREFIX + put(0, b'\x14') + put(4096, b'\x13' + word(9))
    setup += imm(200, MEMORY - 8) + imm(201, 4096) + b'\x0b\xc8\xc9'
    yield 'call-at-memory-end', setup + jump(0), 0, b'A'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference', action='store_true')
    parser.add_argument('--stack-only', action='store_true')
    args = parser.parse_args()
    if args.reference and args.stack_only:
        parser.error('full-profile stack loops run in the native seed only')
    mac = sys.platform == 'darwin' and platform.machine() == 'arm64'
    seed_path = ROOT / 'bootstrap/0_alpha' / ('alpha_arm64_macos' if mac else 'alpha_x64_windows.exe')
    offset = 32768 if mac else 5120
    seed = seed_path.read_bytes() if not args.reference else None
    failures = 0
    count = 0
    with tempfile.TemporaryDirectory(prefix='omega-alpha-bounds-') as directory:
        scratch = Path(directory)

        def check(name, tape, status, expected, declared=None):
            nonlocal failures, count
            count += 1
            if args.reference:
                subject = scratch / 'case.tape'
                subject.write_bytes(tape)
                command = [sys.executable, str(ROOT / 'tests/alpha/reference/alpha_ref.py'), str(subject)]
            else:
                subject = scratch / ('case' if mac else 'case.exe')
                container = bytearray(seed)
                container[offset:offset+4] = struct.pack('<I', len(tape) if declared is None else declared)
                container[offset+4:offset+4+len(tape)] = tape
                subject.write_bytes(container)
                subject.chmod(0o755)
                if mac:
                    subprocess.run(['codesign', '-f', '-s', '-', str(subject)], check=True, capture_output=True)
                command = [str(subject)]
            try:
                result = subprocess.run(command, input=b'Z', capture_output=True, timeout=300)
                trap_codes = (132,) if args.reference else ((-4,) if mac else (0xc000001d, -1073741795))
                correct_status = result.returncode in trap_codes if status == 'trap' else result.returncode == status
                if not correct_status or result.stdout != expected or result.stderr:
                    failures += 1
                    print(f'FAIL {name}: status={result.returncode} stdout={result.stdout.hex()} stderr={result.stderr!r}; want {status}, {expected.hex()}', flush=True)
            except subprocess.TimeoutExpired:
                failures += 1
                print(f'FAIL {name}: exceeded 300-second host watchdog', flush=True)

        if not args.stack_only:
            for case in cases():
                check(*case)
            for length in (MAX_TAPE + 1, 0xffffffff):
                # Bypass the stamper deliberately: exercise the loader itself.
                if args.reference:
                    if length == 0xffffffff:
                        continue  # raw-tape reference has no stamped length field
                    check('loader-adjacent', bytes(length), 'trap', b'')
                else:
                    check(f'loader-{length:x}', b'\x00\xff', 'trap', b'', declared=length)
        if not args.reference:
            for case in stack_cases():
                check(*case)
        print(f'Alpha bounds ({"reference; native-only full-stack loops excluded" if args.reference else "native"}): {count-failures} passed, {failures} failed', flush=True)
    return bool(failures)


if __name__ == '__main__':
    sys.exit(main())
