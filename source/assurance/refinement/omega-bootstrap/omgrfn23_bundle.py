#!/usr/bin/env python3
"""Untrusted packer for exact OMGCOMP1 + OMGRSWC12 + CKIR20 + ELF."""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

from omgrfn6_bundle import HEADER, MAX_CKIR, MAX_ELF, MAX_FRAME, MAX_OMGCOMP, MAX_WITNESS
from omgrfn23_frame import FLAG_PROPOSITION, MAGIC, VERSION


def read(path: Path, ceiling: int, label: str) -> bytes:
    raw = path.read_bytes()
    if not raw or len(raw) > ceiling:
        raise ValueError(f"{label} must be nonempty and within its ceiling")
    return raw


def identity(raw: bytes, magic: bytes, major: int, label: str) -> None:
    if (len(raw) < 12 or raw[:8] != magic
            or struct.unpack_from("<HH", raw, 8) != (major, 0)):
        raise ValueError(f"exact {label} {major}.0 required")


def pack(omgcomp: bytes, witness: bytes, ckir: bytes, elf: bytes,
         result: int = 70) -> bytes:
    for raw, ceiling, label in (
        (omgcomp, MAX_OMGCOMP, "OMGCOMP1"),
        (witness, MAX_WITNESS, "OMGRSWC12"),
        (ckir, MAX_CKIR, "CKIR20"),
        (elf, MAX_ELF, "ELF"),
    ):
        if not raw or len(raw) > ceiling:
            raise ValueError(f"{label} must be nonempty and within its ceiling")
    identity(omgcomp, b"OMGCOMP\0", 1, "OMGCOMP")
    identity(witness, b"OMGRSWC\0", 12, "OMGRSWC")
    identity(ckir, b"OMGCKIR\0", 20, "CKIR")
    if not elf.startswith(b"\x7fELF"):
        raise ValueError("conservative ELF identity")
    if result != 70:
        raise ValueError("canonical OMGRFN23 result is 70")
    lengths = (len(omgcomp), len(witness), len(ckir), len(elf))
    if HEADER.size + sum(lengths) > MAX_FRAME:
        raise ValueError("whole-frame ceiling")
    return HEADER.pack(MAGIC, VERSION, FLAG_PROPOSITION, *lengths, 70, 70) + \
        omgcomp + witness + ckir + elf


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("omgcomp", type=Path)
    parser.add_argument("witness", type=Path)
    parser.add_argument("ckir", type=Path)
    parser.add_argument("elf", type=Path)
    args = parser.parse_args()
    sys.stdout.buffer.write(pack(
        read(args.omgcomp, MAX_OMGCOMP, "OMGCOMP1"),
        read(args.witness, MAX_WITNESS, "OMGRSWC12"),
        read(args.ckir, MAX_CKIR, "CKIR20"),
        read(args.elf, MAX_ELF, "ELF"),
    ))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, struct.error) as error:
        print(f"OMGRFN23 bundle: {error}", file=sys.stderr)
        raise SystemExit(251)
