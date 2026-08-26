#!/usr/bin/env python3
"""Untrusted exact-byte packer for the private OMGRFN8 refinement frame."""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

from omgrfn6_bundle import (
    HEADER,
    MAX_CKIR,
    MAX_ELF,
    MAX_FRAME,
    MAX_OMGCOMP,
    MAX_WITNESS,
    NO_RESULT,
)


MAGIC = b"OMGRFN8\0"
CKIR_MAGIC = b"OMGCKIR\0"
WITNESS_MAJORS = {
    b"OMGRSW1\0": 1,
    b"OMGRSW2\0": 2,
    b"OMGRSW3\0": 3,
}


def bounded(path: Path, ceiling: int, label: str) -> bytes:
    contents = path.read_bytes()
    if len(contents) > ceiling:
        raise SystemExit(f"{label} exceeds OMGRFN8 ceiling")
    return contents


def require_witness_identity(witness: bytes) -> int:
    if len(witness) < 12:
        raise SystemExit(
            "OMGRFN8 requires an OMGRSW1, OMGRSW2, or OMGRSW3 witness"
        )
    major = WITNESS_MAJORS.get(witness[:8])
    if major is None:
        raise SystemExit(
            "OMGRFN8 requires an OMGRSW1, OMGRSW2, or OMGRSW3 witness"
        )
    if struct.unpack_from("<H", witness, 8)[0] != major:
        raise SystemExit("OMGRFN8 witness magic and schema major disagree")
    if struct.unpack_from("<H", witness, 10)[0] != 0:
        raise SystemExit("OMGRFN8 requires OMGRSW schema minor 0")
    return major


def require_ckir6_identity(ckir: bytes) -> None:
    if len(ckir) < 12 or ckir[:8] != CKIR_MAGIC:
        raise SystemExit("OMGRFN8 requires a valid CKIR6 component")
    if struct.unpack_from("<H", ckir, 8)[0] != 6:
        raise SystemExit("OMGRFN8 requires CKIR schema major 6")
    if struct.unpack_from("<H", ckir, 10)[0] != 0:
        raise SystemExit("OMGRFN8 requires CKIR schema minor 0")


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
    witness = bounded(args.witness, MAX_WITNESS, "OMGRSW")
    ckir = bounded(args.ckir, MAX_CKIR, "CKIR6")
    elf = bounded(args.elf, MAX_ELF, "ELF")
    if not omgcomp or not witness or not ckir:
        raise SystemExit("OMGCOMP, selected OMGRSW, and CKIR6 must be nonempty")
    require_witness_identity(witness)
    require_ckir6_identity(ckir)
    if HEADER.size + len(omgcomp) + len(witness) + len(ckir) + len(elf) > MAX_FRAME:
        raise SystemExit("OMGRFN8 frame exceeds whole-frame ceiling")

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

    sys.stdout.buffer.write(
        HEADER.pack(
            MAGIC,
            8,
            flags,
            len(omgcomp),
            len(witness),
            len(ckir),
            len(elf),
            result,
            exit_code,
        )
    )
    sys.stdout.buffer.write(omgcomp)
    sys.stdout.buffer.write(witness)
    sys.stdout.buffer.write(ckir)
    sys.stdout.buffer.write(elf)


if __name__ == "__main__":
    main()
