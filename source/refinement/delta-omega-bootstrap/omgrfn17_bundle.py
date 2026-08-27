#!/usr/bin/env python3
"""Untrusted packer for an exact OMGCOMP1 + OMGRSW4 + CKIR15 + ELF frame."""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

from omgrfn6_bundle import HEADER, MAX_CKIR, MAX_ELF, MAX_FRAME, MAX_OMGCOMP, MAX_WITNESS
from omgrfn17_frame import FLAG_PROPOSITION, MAGIC, VERSION


def read(path: Path, ceiling: int, label: str) -> bytes:
    raw = path.read_bytes()
    if not raw or len(raw) > ceiling:
        raise ValueError(f"{label} must be nonempty and within its ceiling")
    return raw


def identity(raw: bytes, magic: bytes, major: int, label: str) -> None:
    if len(raw) < 12 or raw[:8] != magic or struct.unpack_from("<HH", raw, 8) != (major, 0):
        raise ValueError(f"exact {label} {major}.0 required")


def pack(omgcomp: bytes, witness: bytes, ckir: bytes, elf: bytes, result: int) -> bytes:
    for raw, ceiling, label in ((omgcomp, MAX_OMGCOMP, "OMGCOMP1"),
                                (witness, MAX_WITNESS, "OMGRSW4"),
                                (ckir, MAX_CKIR, "CKIR15"),
                                (elf, MAX_ELF, "ELF")):
        if not raw or len(raw) > ceiling:
            raise ValueError(f"{label} must be nonempty and within its ceiling")
    identity(omgcomp, b"OMGCOMP\0", 1, "OMGCOMP")
    identity(witness, b"OMGRSW4\0", 4, "OMGRSW")
    identity(ckir, b"OMGCKIR\0", 15, "CKIR")
    if not elf.startswith(b"\x7fELF"):
        raise ValueError("conservative ELF identity")
    if not 0 <= result <= 0xFFFF_FFFF:
        raise ValueError("u32 result")
    lengths = (len(omgcomp), len(witness), len(ckir), len(elf))
    if HEADER.size + sum(lengths) > MAX_FRAME:
        raise ValueError("whole-frame ceiling")
    return HEADER.pack(MAGIC, VERSION, FLAG_PROPOSITION, *lengths, result, result & 255) + \
        omgcomp + witness + ckir + elf


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("omgcomp", type=Path); parser.add_argument("witness", type=Path)
    parser.add_argument("ckir", type=Path); parser.add_argument("elf", type=Path)
    parser.add_argument("--result", required=True, type=int)
    args = parser.parse_args()
    sys.stdout.buffer.write(pack(
        read(args.omgcomp, MAX_OMGCOMP, "OMGCOMP1"),
        read(args.witness, MAX_WITNESS, "OMGRSW4"),
        read(args.ckir, MAX_CKIR, "CKIR15"),
        read(args.elf, MAX_ELF, "ELF"), args.result,
    ))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, struct.error) as error:
        print(f"OMGRFN17 bundle: {error}", file=sys.stderr)
        raise SystemExit(251)
