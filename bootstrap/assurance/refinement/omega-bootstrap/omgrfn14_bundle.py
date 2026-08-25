#!/usr/bin/env python3
"""Untrusted exact-byte packer for the private OMGRFN14 refinement frame."""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

from omgrfn6_bundle import (
    HEADER, MAX_CKIR, MAX_ELF, MAX_FRAME, MAX_OMGCOMP, MAX_WITNESS, NO_RESULT,
)


MAGIC = b"OMGRFNE\0"
WITNESS_MAGIC = b"OMGRSW4\0"
CKIR_MAGIC = b"OMGCKIR\0"


def bounded(path: Path, ceiling: int, label: str) -> bytes:
    contents = path.read_bytes()
    if len(contents) > ceiling:
        raise SystemExit(f"{label} exceeds OMGRFN14 ceiling")
    return contents


def require_identity(contents: bytes, magic: bytes, major: int, label: str) -> None:
    if (len(contents) < 12 or contents[:8] != magic
            or struct.unpack_from("<HH", contents, 8) != (major, 0)):
        raise SystemExit(f"OMGRFN14 requires {label} schema {major}.0")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("omgcomp", type=Path)
    parser.add_argument("witness", type=Path)
    parser.add_argument("ckir", type=Path)
    parser.add_argument("elf", type=Path)
    parser.add_argument("--result", type=int)
    parser.add_argument("--library", action="store_true")
    args = parser.parse_args()

    omgcomp = bounded(args.omgcomp, MAX_OMGCOMP, "OMGCOMP")
    witness = bounded(args.witness, MAX_WITNESS, "OMGRSW4")
    ckir = bounded(args.ckir, MAX_CKIR, "CKIR12")
    elf = bounded(args.elf, MAX_ELF, "ELF")
    if not omgcomp or not witness or not ckir:
        raise SystemExit("OMGCOMP, OMGRSW4, and CKIR12 must be nonempty")
    require_identity(witness, WITNESS_MAGIC, 4, "OMGRSW")
    require_identity(ckir, CKIR_MAGIC, 12, "CKIR")
    if HEADER.size + len(omgcomp) + len(witness) + len(ckir) + len(elf) > MAX_FRAME:
        raise SystemExit("OMGRFN14 frame exceeds whole-frame ceiling")

    if args.library:
        if args.result is not None or elf:
            raise SystemExit("library frame requires no result and empty ELF")
        flags = 0
        result = exit_code = NO_RESULT
    else:
        if args.result is None or not 0 <= args.result <= NO_RESULT or not elf:
            raise SystemExit("entry frame requires a u32 result and nonempty ELF")
        flags = 1
        result = args.result
        exit_code = result & 255
    sys.stdout.buffer.write(HEADER.pack(
        MAGIC, 14, flags, len(omgcomp), len(witness), len(ckir), len(elf),
        result, exit_code,
    ))
    sys.stdout.buffer.write(omgcomp + witness + ckir + elf)


if __name__ == "__main__":
    main()
