#!/usr/bin/env python3
"""Untrusted exact-byte packer for the private OMGRFN6 refinement frame."""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path


MAGIC = b"OMGRFN6\0"
WITNESS_MAGIC = b"OMGRSW2\0"
HEADER = struct.Struct("<8s8I")
NO_RESULT = 0xFFFF_FFFF
MAX_OMGCOMP = 267_280
MAX_WITNESS = 524_288
MAX_CKIR = 2_522_192
MAX_ELF = 1_183_744
MAX_FRAME = 4_497_544


def bounded(path: Path, ceiling: int, label: str) -> bytes:
    contents = path.read_bytes()
    if len(contents) > ceiling:
        raise SystemExit(f"{label} exceeds OMGRFN6 ceiling")
    return contents


def require_omgrsw2(witness: bytes) -> None:
    if len(witness) < 12 or witness[:8] != WITNESS_MAGIC:
        raise SystemExit("OMGRFN6 requires an OMGRSW2 witness")
    if struct.unpack_from("<I", witness, 8)[0] != 2:
        raise SystemExit("OMGRFN6 requires OMGRSW schema major 2")


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
    witness = bounded(args.witness, MAX_WITNESS, "OMGRSW2")
    ckir = bounded(args.ckir, MAX_CKIR, "CKIR4")
    elf = bounded(args.elf, MAX_ELF, "ELF")
    if not omgcomp or not witness or not ckir:
        raise SystemExit("OMGCOMP, OMGRSW2, and CKIR4 must be nonempty")
    require_omgrsw2(witness)
    if HEADER.size + len(omgcomp) + len(witness) + len(ckir) + len(elf) > MAX_FRAME:
        raise SystemExit("OMGRFN6 frame exceeds whole-frame ceiling")

    if args.library:
        if args.result is not None or elf:
            raise SystemExit("library frame requires no result and empty ELF")
        flags = 0
        result = exit_code = NO_RESULT
    else:
        if args.result is None or not 0 <= args.result <= NO_RESULT:
            raise SystemExit("entry frame requires one u32 result")
        if not elf:
            raise SystemExit("entry frame requires a nonempty ELF")
        flags = 1
        result = args.result
        exit_code = result & 0xFF

    sys.stdout.buffer.write(HEADER.pack(
        MAGIC, 6, flags, len(omgcomp), len(witness), len(ckir), len(elf), result, exit_code,
    ))
    sys.stdout.buffer.write(omgcomp)
    sys.stdout.buffer.write(witness)
    sys.stdout.buffer.write(ckir)
    sys.stdout.buffer.write(elf)


if __name__ == "__main__":
    main()
