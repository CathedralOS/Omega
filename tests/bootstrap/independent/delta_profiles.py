"""Exercise both DCREQ byte profiles using a seed-built Delta and native Gamma compiler."""
import argparse
import hashlib
from pathlib import Path
import struct
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[3]
MIB = 1024 * 1024


def execute(program, data, status=0):
    result = subprocess.run([program], input=data, capture_output=True, timeout=120)
    if result.returncode != status or result.stderr:
        raise AssertionError((program, result.returncode, result.stderr[:80], result.stdout[:40]))
    return result.stdout


def check(actual, expected, label):
    if actual != expected:
        raise AssertionError((label, len(actual), actual[:40], len(expected), expected[:40]))


def header(profile, length):
    return b'DCREQ\x01\0\0' + struct.pack('<II', profile, length)


def failure(tag, space, code, coordinate, limit=0, requested=0):
    return struct.pack('<8sBBHIQQQ', b'\xffDCOUT\x01\0', tag, space, 0,
                       code, coordinate, limit, requested)


def run(delta, gamma, work, full_output):
    root = ROOT / 'bootstrap/delta/compiler'
    members = []
    for row in (root / 'support/support.gamma.sources').read_text().splitlines()[1:]:
        _, _, size, digest, name = row.split()
        member = (root / 'support' / name).read_bytes()
        if len(member) != int(size) or hashlib.sha256(member).hexdigest() != digest:
            raise ValueError(f'changed support member {name}')
        members.append(member)
    support = b''.join(members)

    # A complete header reports its first mismatching identity byte. A short
    # header reports physical EOF before any of its fields can be trusted.
    frame = header(1, 0)
    for end in range(len(frame)):
        check(execute(delta, frame[:end], 1), failure(1, 4, 1, end), f'truncated header at {end}')
    for offset in range(8):
        broken = bytearray(frame)
        broken[offset] ^= 1
        check(execute(delta, broken, 1), failure(1, 4, 1, offset), f'changed identity at {offset}')
    broken = bytearray(frame)
    broken[2] ^= 1
    broken[5] ^= 1
    check(execute(delta, broken, 1), failure(1, 4, 1, 2), 'first changed identity byte')
    check(execute(delta, broken[:-1], 1), failure(1, 4, 1, 15), 'short header before identity')

    def request(profile, source, trailer=support):
        return header(profile, len(source)) + source + trailer

    def build(profile, source, name):
        receipt = execute(delta, request(profile, source))
        binary = work / name
        binary.write_bytes(execute(gamma, receipt))
        binary.chmod(0o700)
        return binary, receipt

    # A checked single-constructor type carries its product directly. Put one
    # inside a tagged sum and destructure both shapes on the same call path.
    records = b'''(data Byte (ByteValue Int))
(data Record (Fields Byte Bytes))
(data Choice (Absent) (Present Record))
(def unpack ((r Record)) Bytes
  (match r ((Fields b suffix)
    (match b ((ByteValue n) (bytes_concat (bytes_single n) suffix))))))
(def main ((s Bytes)) Bytes
  (match (Present (Fields (ByteValue 42) s))
    ((Present r) (unpack r)) (Absent "unused")))
'''
    program, _ = build(1, records, 'records')
    check(execute(program, b'\x00\xff'), b'*\x00\xff', 'nested records in a tagged sum')
    # Exhaustiveness does not allow dropping evaluation of the scrutinee.
    unused = b'''(data Byte (ByteValue Int))
(def main ((s Bytes)) Bytes (match (ByteValue (/ 1 0)) ((ByteValue unused) s)))'''
    program, _ = build(1, unused, 'unused-record')
    check(execute(program, b'x', 249), b'', 'unused record field still traps')
    nominal = b'''(data Left (L Int))
(data Right (R Int))
(def accept ((x Left)) Int (match x ((L n) n)))
(def main ((s Bytes)) Bytes (bytes_single (accept (R 42))))'''
    check(execute(delta, request(1, nominal), 1),
          failure(1, 1, 15, nominal.index(b'(R 42)')), 'distinct one-field record types')

    # Both profiles publish the same shared value. Each copy has 1,024 empty
    # prefixes, so logical length and the number of visited nodes differ.
    wrapped = Path(__file__).with_name('shared_empty_rope.delta').read_bytes()
    for profile in (1, 2):
        program, _ = build(profile, wrapped, f'shared-rope-{profile}')
        for doublings in (8, 10):
            check(execute(program, bytes(doublings)), b'Z' * (1 << doublings),
                  f'shared rope with empty prefixes, profile {profile}, {doublings} doublings')

    source = b'(def main ((s Bytes)) Bytes (bytes_concat (bytes_empty) s))\n'
    receipts = []
    for profile, source_limit, input_limit in [(1, 4 * MIB, 4 * MIB), (2, 64 * MIB, 8 * MIB)]:
        check(execute(delta, header(profile, source_limit), 1), failure(1, 4, 1, 16), 'exact source limit')
        check(execute(delta, header(profile, source_limit + 1), 2),
              failure(2, 4, 1, 12, source_limit, source_limit + 1), 'source limit + 1')
        wrong = b'(def main ((n Int)) Int n)\n'
        check(execute(delta, request(profile, wrong), 1), failure(1, 1, 20, wrong.index(b'main')),
              'Bytes entry schema')
        broken = support[:-1] + bytes([support[-1] ^ 1])
        check(execute(delta, request(profile, source, broken), 1),
              failure(1, 5, 2, 16 + len(source) + len(support) - len(members[-1])), 'adapter custody')
        program, receipt = build(profile, source, f'identity-{profile}')
        receipts.append(receipt)
        payload = bytes(range(256)) * (input_limit // 256)
        check(execute(program, payload), payload, 'exact input limit')
        check(execute(program, payload + b'a', 253), b'', 'input limit + 1')
        check(execute(program, b''), b'', 'empty rope')
    check(execute(delta, header(3, 0xffffffff), 1), failure(1, 4, 2, 8), 'unknown ID before provision')

    # The original request and receipt were measured with the three-member
    # compiler. Replaying the exact old packet exercises source_length and
    # support offsets for every emitted member, without upgrading its trailer.
    old_support = b''.join(members[:3])
    old_request = request(1, source, old_support)
    old_receipt = execute(delta, old_request)
    if hashlib.sha256(old_request).hexdigest() != 'ca979a67b0321234af726bf80b7992f799151b0a3597e9c43527a41fba8f4581':
        raise AssertionError('legacy request bytes changed')
    if hashlib.sha256(old_receipt).hexdigest() != '62b0bd1f0d8df36ff15875cfcbddb57abf6af1b27d20b7a2fa3f153a2d6545a4':
        raise AssertionError('legacy emitted Gamma changed')
    check(receipts[0], old_receipt, 'both supported profile-1 trailers')
    check(execute(delta, request(2, source, old_support), 1),
          failure(1, 5, 1, len(old_request)), 'profile 2 requires its adapter')
    for trailer in (old_support[:-1], old_support + b'X', support[:-1]):
        packet = request(1, source, trailer)
        check(execute(delta, packet, 1), failure(1, 5, 1, len(packet)), 'partial trailer')
    broken = old_support[:-1] + bytes([old_support[-1] ^ 1])
    check(execute(delta, request(1, source, broken), 1),
          failure(1, 5, 2, 16 + len(source) + len(members[0]) + len(members[1])), 'legacy adapter custody')
    full = request(1, source)
    check(execute(delta, full + b'X', 1), failure(1, 4, 1, len(full)), 'extra byte')

    original = (root / 'support/adapter.gamma').read_bytes()
    if receipts[0].count(original) != 1:
        raise AssertionError('expected one complete profile-1 adapter')
    check(receipts[1], receipts[0].replace(original, (root / 'support/construction.gamma').read_bytes()),
          'receipt differs only by the selected adapter')

    # Repeat a sealed input sixteen times without duplicating runtime storage,
    # then append a singleton. Its complete length must be checked before writing.
    source = (b'(def twice ((s Bytes)) Bytes (bytes_concat s s))\n'
              b'(def main ((s Bytes)) Bytes (bytes_concat (twice (twice (twice (twice s)))) '
              b'(if (eq (bytes_get s 0) 0) (bytes_empty) (bytes_single (bytes_get s 0)))))\n')
    for profile, output_limit in [(1, 4 * MIB), (2, 128 * MIB)]:
        program, _ = build(profile, source, f'repeat-{profile}')
        check(execute(program, b'abc'), b'abc' * 16 + b'a', 'mixed tag traversal and input indexing')
        payload = bytes(range(256)) * (output_limit // (16 * 256))
        if full_output or profile == 1:
            with tempfile.TemporaryFile() as stream:
                result = subprocess.run([program], input=payload, stdout=stream,
                                        stderr=subprocess.PIPE, timeout=600)
                if result.returncode or result.stderr or stream.tell() != output_limit:
                    raise AssertionError(('exact output limit', profile, result.returncode, stream.tell()))
                stream.seek(0)
                for _ in range(16):
                    check(stream.read(len(payload)), payload, 'ordered full-size publication')
        check(execute(program, b'x' + payload[1:], 254), b'', 'output limit + 1')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('delta', type=Path)
    parser.add_argument('gamma', type=Path)
    parser.add_argument('--full-output', action='store_true',
                        help='also publish and compare all 128 MiB at the profile-2 limit')
    args = parser.parse_args()
    with tempfile.TemporaryDirectory() as directory:
        run(args.delta.resolve(), args.gamma.resolve(), Path(directory), args.full_output)
    print('Both DCREQ byte profiles passed source, schema, custody, traversal and extent checks.')
