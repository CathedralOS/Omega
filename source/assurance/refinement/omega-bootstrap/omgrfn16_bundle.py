#!/usr/bin/env python3
"""Untrusted exact-byte packer for the private OMGRFN16 refinement frame."""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

from omgrfn6_bundle import (
    HEADER, MAX_CKIR, MAX_ELF, MAX_FRAME, MAX_OMGCOMP, MAX_WITNESS, NO_RESULT,
)


MAGIC = b"OMGRFNG\0"
WITNESS_MAGIC = b"OMGRSW7\0"
CKIR_MAGIC = b"OMGCKIR\0"
FLAG_PROPOSITION = 1
FLAG_TRAP = 2


def bounded(path: Path, ceiling: int, label: str) -> bytes:
    contents = path.read_bytes()
    if len(contents) > ceiling:
        raise SystemExit(f"{label} exceeds OMGRFN16 ceiling")
    return contents


def require_identity(contents: bytes, magic: bytes, major: int, label: str) -> None:
    if (len(contents) < 12 or contents[:8] != magic
            or struct.unpack_from("<HH", contents, 8) != (major, 0)):
        raise SystemExit(f"OMGRFN16 requires {label} schema {major}.0")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("omgcomp", type=Path)
    parser.add_argument("witness", type=Path)
    parser.add_argument("ckir", type=Path)
    parser.add_argument("elf", type=Path)
    outcome = parser.add_mutually_exclusive_group(required=True)
    outcome.add_argument("--result", type=int)
    outcome.add_argument("--trap", action="store_true")
    args = parser.parse_args()

    omgcomp = bounded(args.omgcomp, MAX_OMGCOMP, "OMGCOMP")
    witness = bounded(args.witness, MAX_WITNESS, "OMGRSW7")
    ckir = bounded(args.ckir, MAX_CKIR, "CKIR14")
    elf = bounded(args.elf, MAX_ELF, "ELF")
    if not omgcomp or not witness or not ckir or not elf:
        raise SystemExit("OMGCOMP, OMGRSW7, CKIR14, and ELF must be nonempty")
    require_identity(witness, WITNESS_MAGIC, 7, "OMGRSW")
    require_identity(ckir, CKIR_MAGIC, 14, "CKIR")
    if HEADER.size + len(omgcomp) + len(witness) + len(ckir) + len(elf) > MAX_FRAME:
        raise SystemExit("OMGRFN16 frame exceeds whole-frame ceiling")

    if args.trap:
        flags = FLAG_PROPOSITION | FLAG_TRAP
        result = exit_code = NO_RESULT
    else:
        if args.result is None or not 0 <= args.result <= NO_RESULT:
            raise SystemExit("successful frame requires a u32 result")
        flags = FLAG_PROPOSITION
        result = args.result
        exit_code = result & 255

    sys.stdout.buffer.write(HEADER.pack(
        MAGIC, 16, flags, len(omgcomp), len(witness), len(ckir), len(elf),
        result, exit_code,
    ))
    sys.stdout.buffer.write(omgcomp + witness + ckir + elf)


if __name__ == "__main__":
    try:
        main()
    except (OSError, struct.error) as error:
        print(f"OMGRFN16 bundle: {error}", file=sys.stderr)
        raise SystemExit(251)
