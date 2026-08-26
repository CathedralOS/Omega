#!/usr/bin/env python3
"""Construct exact OMGLOWI18 and OMGRFN20 frames for the production gate."""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

from omgrfn20_bundle import pack


def lowering(omgcomp: bytes, witness: bytes) -> bytes:
    total = 32 + len(omgcomp) + len(witness)
    if not omgcomp.startswith(b"OMGCOMP\0\x03\x00"):
        raise ValueError("exact OMGCOMP3 required")
    if not witness.startswith(b"OMGRSW9\0\x09\x00"):
        raise ValueError("exact OMGRSW9 required")
    return struct.pack("<8s4H4I", b"OMGLOWI\0", 18, 0, 0, 32,
                       total, len(omgcomp), len(witness), 9) \
        + omgcomp + witness


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("kind", choices=("lowering", "refinement"))
    parser.add_argument("components", nargs="+", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    raw = [path.read_bytes() for path in args.components]
    if args.kind == "lowering" and len(raw) == 2:
        result = lowering(*raw)
    elif args.kind == "refinement" and len(raw) == 3:
        result = pack(*raw)
    else:
        parser.error(f"{args.kind} requires {'two' if args.kind == 'lowering' else 'three'} components")
    args.output.write_bytes(result)


if __name__ == "__main__":
    main()
