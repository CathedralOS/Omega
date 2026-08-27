#!/usr/bin/env python3
"""Untrusted exact-byte packer for OMGCOMP3 + OMGRSW9 OMGRFN19 frames."""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

from omgrfn19_frame import (
    FLAGS,
    HEADER,
    HEADER_SIZE,
    MAGIC,
    MAX_FRAME,
    MAX_OMGCOMP,
    MAX_WITNESS,
    VERSION,
)


def read(path: Path, ceiling: int, label: str) -> bytes:
    raw = path.read_bytes()
    if not raw:
        raise ValueError(f"{label} must be nonempty")
    if len(raw) > ceiling:
        raise ValueError(f"{label} exceeds its ceiling")
    return raw


def pack(omgcomp: bytes, witness: bytes) -> bytes:
    if not omgcomp or len(omgcomp) > MAX_OMGCOMP:
        raise ValueError("OMGCOMP3 component extent")
    if not witness or len(witness) > MAX_WITNESS:
        raise ValueError("OMGRSW9 component extent")
    if len(omgcomp) < 12 or omgcomp[:8] != b"OMGCOMP\0" \
            or struct.unpack_from("<HH", omgcomp, 8) != (3, 0):
        raise ValueError("exact OMGCOMP3 required")
    if len(witness) < 12 or witness[:8] != b"OMGRSW9\0" \
            or struct.unpack_from("<HH", witness, 8) != (9, 0):
        raise ValueError("exact OMGRSW9 required")
    total = HEADER_SIZE + len(omgcomp) + len(witness)
    if total > MAX_FRAME:
        raise ValueError("OMGRFN19 whole-frame ceiling")
    return HEADER.pack(
        MAGIC, VERSION, 0, FLAGS, HEADER_SIZE, total,
        len(omgcomp), len(witness), 0,
    ) + omgcomp + witness


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("omgcomp", type=Path)
    parser.add_argument("witness", type=Path)
    arguments = parser.parse_args()
    sys.stdout.buffer.write(pack(
        read(arguments.omgcomp, MAX_OMGCOMP, "OMGCOMP3"),
        read(arguments.witness, MAX_WITNESS, "OMGRSW9"),
    ))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, struct.error) as error:
        print(f"OMGRFN19 bundle: {error}", file=sys.stderr)
        raise SystemExit(251)
