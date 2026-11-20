#!/usr/bin/env python3
"""Exercise the actual Delta parser at both DCREQ profiles' syntax limits."""
import argparse
from pathlib import Path
import struct
import subprocess


def check(compiler, support, profile, source, status, expected):
    header = b'DCREQ\x01\0\0' + struct.pack('<II', profile, len(source))
    result = subprocess.run([compiler], input=header + source + support, capture_output=True)
    assert (result.returncode, result.stdout, result.stderr) == (status, expected, b''), (
        profile, len(source), result.returncode, result.stdout[:80], result.stderr[:80])


def outcome(tag, code, position, limit=0, requested=0):
    return struct.pack('<8sBBHIQQQ', b'\xffDCOUT\x01\0', tag, 1, 0, code, position,
                       limit, requested)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path, help='source-built Delta executable')
    parser.add_argument('support', type=Path, help='packed support closure bound by this executable')
    args = parser.parse_args()
    compiler, support = args.compiler.resolve(), args.support.read_bytes()

    # Each opening delimiter requires a three-pair frame, with 40 bytes per pair.
    # At the last complete frame, the parser can still report the unclosed source
    # at EOF. The next delimiter must fail allocation before it creates a frame.
    for profile, limit in ((1, 114_294_752), (2, 1_073_741_824)):
        frames = limit // 120
        check(compiler, support, profile, b'(' * frames, 1, outcome(1, 4, frames))
        check(compiler, support, profile, b'(' * (frames + 1), 2,
              outcome(2, 7, frames, limit, (frames + 1) * 120))
        print(f'Profile {profile} admitted {frames:,} frames and refused the next.', flush=True)


if __name__ == '__main__':
    main()
