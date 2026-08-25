#!/usr/bin/env python3
"""Untrusted exact-byte packer for the private OMGRFN10 refinement frame."""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

from omgrfn6_bundle import HEADER, MAX_CKIR, MAX_ELF, MAX_FRAME, MAX_OMGCOMP, MAX_WITNESS, NO_RESULT


# The carrier header owns exactly eight magic bytes.  A denotes version ten
# without widening or shifting the frozen forty-byte envelope.
MAGIC = b"OMGRFNA\0"
CKIR_MAGIC = b"OMGCKIR\0"
WITNESS_MAJORS = {b"OMGRSW1\0": 1, b"OMGRSW2\0": 2, b"OMGRSW3\0": 3}


def bounded(path: Path, ceiling: int, label: str) -> bytes:
    contents = path.read_bytes()
    if len(contents) > ceiling:
        raise SystemExit(f"{label} exceeds OMGRFN10 ceiling")
    return contents


def require_witness_identity(witness: bytes) -> int:
    if len(witness) < 12 or witness[:8] not in WITNESS_MAJORS:
        raise SystemExit("OMGRFN10 requires an OMGRSW1, OMGRSW2, or OMGRSW3 witness")
    major = WITNESS_MAJORS[witness[:8]]
    if struct.unpack_from("<H", witness, 8)[0] != major:
        raise SystemExit("OMGRFN10 witness magic and schema major disagree")
    if struct.unpack_from("<H", witness, 10)[0] != 0:
        raise SystemExit("OMGRFN10 requires OMGRSW schema minor 0")
    return major


def require_ckir8_identity(ckir: bytes) -> None:
    if len(ckir) < 12 or ckir[:8] != CKIR_MAGIC:
        raise SystemExit("OMGRFN10 requires a valid CKIR8 component")
    if struct.unpack_from("<H", ckir, 8)[0] != 8:
        raise SystemExit("OMGRFN10 requires CKIR schema major 8")
    if struct.unpack_from("<H", ckir, 10)[0] != 0:
        raise SystemExit("OMGRFN10 requires CKIR schema minor 0")


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
    ckir = bounded(args.ckir, MAX_CKIR, "CKIR8")
    elf = bounded(args.elf, MAX_ELF, "ELF")
    if not omgcomp or not witness or not ckir:
        raise SystemExit("OMGCOMP, selected OMGRSW, and CKIR8 must be nonempty")
    require_witness_identity(witness)
    require_ckir8_identity(ckir)
    if HEADER.size + len(omgcomp) + len(witness) + len(ckir) + len(elf) > MAX_FRAME:
        raise SystemExit("OMGRFN10 frame exceeds whole-frame ceiling")

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
        HEADER.pack(MAGIC, 10, flags, len(omgcomp), len(witness), len(ckir), len(elf), result, exit_code)
    )
    sys.stdout.buffer.write(omgcomp)
    sys.stdout.buffer.write(witness)
    sys.stdout.buffer.write(ckir)
    sys.stdout.buffer.write(elf)


if __name__ == "__main__":
    main()
