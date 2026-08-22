#!/usr/bin/env python3
"""Mechanically enlarge the audited Windows seed's zero-only tape section.

This deliberately is not a general PE forge. It accepts exactly the committed
32 KiB seed shape, changes the three capacity fields audited below, and appends
zeros until the .tape raw extent is 256 KiB. Executable code is untouched.
"""

from __future__ import annotations

import os
from pathlib import Path
import sys
import tempfile
from typing import NoReturn


OLD_FILE_SIZE = 0x9400
NEW_FILE_SIZE = 0x41400
TAPE_RAW_OFFSET = 0x1400
OLD_TAPE_SIZE = 0x8000
NEW_TAPE_SIZE = 0x40000

# file offset: (old bytes, new bytes)
PATCHES = {
    0x90: (bytes.fromhex("00 c0 00 04"), bytes.fromhex("00 40 04 04")),
    0x1C8: (bytes.fromhex("00 80 00 00"), bytes.fromhex("00 00 04 00")),
    0x1D0: (bytes.fromhex("00 80 00 00"), bytes.fromhex("00 00 04 00")),
}


def fail(message: str) -> NoReturn:
    raise SystemExit(f"resize-x64-tape-hole: {message}")


def main() -> None:
    if len(sys.argv) != 2:
        fail("usage: resize-x64-tape-hole.py SEED")

    seed = Path(sys.argv[1])
    original = seed.read_bytes()
    if len(original) != OLD_FILE_SIZE:
        fail(f"expected {OLD_FILE_SIZE:#x}-byte input, got {len(original):#x}")
    if any(original[TAPE_RAW_OFFSET : TAPE_RAW_OFFSET + OLD_TAPE_SIZE]):
        fail("existing .tape section is not entirely zero")

    resized = bytearray(original)
    for offset, (expected, replacement) in PATCHES.items():
        actual = bytes(resized[offset : offset + len(expected)])
        if actual != expected:
            fail(
                f"unexpected bytes at {offset:#x}: "
                f"expected {expected.hex()}, got {actual.hex()}"
            )
        resized[offset : offset + len(expected)] = replacement

    resized.extend(bytes(NEW_FILE_SIZE - len(resized)))
    if len(resized) != TAPE_RAW_OFFSET + NEW_TAPE_SIZE:
        fail("internal extent calculation disagrees with requested tape size")

    mode = seed.stat().st_mode
    with tempfile.NamedTemporaryFile(dir=seed.parent, delete=False) as output:
        temporary = Path(output.name)
        output.write(resized)
    os.chmod(temporary, mode)
    os.replace(temporary, seed)


if __name__ == "__main__":
    main()
