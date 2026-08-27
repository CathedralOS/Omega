#!/usr/bin/env python3
"""Frame exact source-bundle, CKIR1, and ELF bytes for low-rung checkers.

This packer is deliberately untrusted.  It selects no semantic rows and
computes no refinement result; the Beta checkers decode the envelope and bind
every byte themselves.
"""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path


MAGIC = b"OMGRFN1\0"
HEADER = struct.Struct("<8s7I")
NO_RESULT = 0xFFFF_FFFF
MAX_BUNDLE = 131_160
MAX_CKIR = 2_260_040
MAX_ELF = 1_052_672


def bounded(path: Path, ceiling: int, label: str) -> bytes:
    contents = path.read_bytes()
    if len(contents) > ceiling:
        raise SystemExit(f"{label} exceeds refinement-envelope ceiling")
    return contents


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("bundle", type=Path)
    parser.add_argument("ckir", type=Path)
    parser.add_argument("elf", type=Path)
    parser.add_argument("--result", type=int)
    parser.add_argument("--library", action="store_true")
    arguments = parser.parse_args()

    bundle = bounded(arguments.bundle, MAX_BUNDLE, "source bundle")
    ckir = bounded(arguments.ckir, MAX_CKIR, "CKIR")
    elf = bounded(arguments.elf, MAX_ELF, "ELF")
    if not bundle or not ckir:
        raise SystemExit("source bundle and CKIR must be nonempty")

    if arguments.library:
        if arguments.result is not None or elf:
            raise SystemExit("library envelope requires no result and empty ELF")
        flags = 0
        result = exit_code = NO_RESULT
    else:
        if arguments.result is None or not 0 <= arguments.result <= 0xFFFF_FFFF:
            raise SystemExit("entry envelope requires one u32 result")
        if not elf:
            raise SystemExit("entry envelope requires a nonempty ELF")
        flags = 1
        result = arguments.result
        exit_code = result & 0xFF

    sys.stdout.buffer.write(
        HEADER.pack(
            MAGIC,
            1,
            flags,
            len(bundle),
            len(ckir),
            len(elf),
            result,
            exit_code,
        )
    )
    sys.stdout.buffer.write(bundle)
    sys.stdout.buffer.write(ckir)
    sys.stdout.buffer.write(elf)


if __name__ == "__main__":
    main()
