#!/usr/bin/env python3
"""Untrusted exact-byte packer for the private OMGRFN15 frame."""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

from omgrfn6_bundle import (
    HEADER, MAX_CKIR, MAX_ELF, MAX_FRAME, MAX_OMGCOMP, MAX_WITNESS,
)


MAGIC = b"OMGRFNF\0"


def bounded(path: Path, ceiling: int, label: str) -> bytes:
    contents = path.read_bytes()
    if len(contents) > ceiling:
        raise SystemExit(f"{label} exceeds OMGRFN15 ceiling")
    return contents


def identity(contents: bytes, magic: bytes, major: int, label: str) -> None:
    if (len(contents) < 12 or contents[:8] != magic
            or struct.unpack_from("<HH", contents, 8) != (major, 0)):
        raise SystemExit(f"OMGRFN15 requires {label} {major}.0")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("omgcomp", type=Path)
    parser.add_argument("witness", type=Path)
    parser.add_argument("ckir", type=Path)
    parser.add_argument("elf", type=Path)
    parser.add_argument("--result", type=int, required=True)
    args = parser.parse_args()
    omgcomp = bounded(args.omgcomp, MAX_OMGCOMP, "OMGCOMP")
    witness = bounded(args.witness, MAX_WITNESS, "OMGRSW5")
    ckir = bounded(args.ckir, MAX_CKIR, "CKIR13")
    elf = bounded(args.elf, MAX_ELF, "ELF")
    if not all((omgcomp, witness, ckir, elf)):
        raise SystemExit("OMGRFN15 entry components must be nonempty")
    identity(witness, b"OMGRSW5\0", 5, "OMGRSW")
    identity(ckir, b"OMGCKIR\0", 13, "CKIR")
    if not 0 <= args.result <= 0xFFFF_FFFF:
        raise SystemExit("OMGRFN15 result is not u32")
    if HEADER.size + sum(map(len, (omgcomp, witness, ckir, elf))) > MAX_FRAME:
        raise SystemExit("OMGRFN15 frame exceeds whole-frame ceiling")
    sys.stdout.buffer.write(HEADER.pack(
        MAGIC, 15, 1, len(omgcomp), len(witness), len(ckir), len(elf),
        args.result, args.result & 255,
    ))
    sys.stdout.buffer.write(omgcomp + witness + ckir + elf)


if __name__ == "__main__":
    main()
