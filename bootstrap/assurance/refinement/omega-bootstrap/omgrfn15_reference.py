#!/usr/bin/env python3
"""Focused independent reference for the OMGRFN15 candidate frame."""

from __future__ import annotations

import argparse
import re
import struct
import sys
from pathlib import Path


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
GATES = REPO / "bootstrap/omega-bootstrap/gates"
COMPILER = REPO / "bootstrap/omega-bootstrap/compiler"
sys.path.insert(0, str(GATES))
sys.path.insert(0, str(COMPILER))
import checked_ir_v13_reference as ir13  # noqa: E402
import omega_bootstrap_compilation as compilation  # noqa: E402
from omgrfn6_bundle import HEADER, MAX_CKIR, MAX_ELF, MAX_FRAME, MAX_OMGCOMP, MAX_WITNESS  # noqa: E402


class ReferenceError(ValueError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ReferenceError(message)


def witness_types(raw: bytes) -> list[tuple[int, ...]]:
    require(len(raw) >= 84 and raw[:16] == b"OMGRSW5\0\x05\0\0\0\0\0T\0",
            "OMGRSW5 identity")
    words = struct.unpack_from("<17I", raw, 16)
    require(words[0] == len(raw), "OMGRSW5 length")
    counts = words[1:15]
    ordered = (counts[0], counts[1], counts[2], counts[3], counts[4], counts[5],
               counts[6], counts[11], counts[12], counts[13], counts[7], counts[8],
               counts[9], counts[10])
    widths = (36, 48, 28, 28, 24, 24, 24, 24, 28, 24, 40, 24, 40, 24)
    at = 84
    rows: list[tuple[int, ...]] = []
    for index, (count, width) in enumerate(zip(ordered, widths)):
        end = at + count * width
        require(end <= len(raw), "OMGRSW5 table extent")
        if index == 4:
            rows = [struct.unpack_from("<IBBHIIII", raw, at + row * width)
                    for row in range(count)]
        at = end
    require(at == len(raw), "OMGRSW5 EOF")
    return rows


def check(path: Path) -> None:
    raw = path.read_bytes()
    require(len(raw) <= MAX_FRAME and len(raw) >= HEADER.size, "frame extent")
    magic, version, flags, omg_len, witness_len, ckir_len, elf_len, result, exit_code = HEADER.unpack_from(raw)
    require((magic, version, flags) == (b"OMGRFNF\0", 15, 1), "frame identity")
    require(0 < omg_len <= MAX_OMGCOMP and 0 < witness_len <= MAX_WITNESS
            and 0 < ckir_len <= MAX_CKIR and 0 < elf_len <= MAX_ELF, "component ceiling")
    require(HEADER.size + omg_len + witness_len + ckir_len + elf_len == len(raw), "exact EOF")
    require((result, exit_code) == (70, 70), "result claim")
    at = HEADER.size
    omg = raw[at:at + omg_len]; at += omg_len
    witness = raw[at:at + witness_len]; at += witness_len
    ckir = raw[at:at + ckir_len]; at += ckir_len
    elf = raw[at:]

    decoded = compilation.decode(omg)
    expected = (GATES / "fixtures/ckir13-full-u32-subtract/success.omg").read_bytes()
    require(any(entry.content == expected for entry in decoded.bundle_entries), "exact source fixture custody")
    types = witness_types(witness)
    require(any(row[1:] == (2, 1, 0, 0, 0, 0, 0xFFFF_FFFF) for row in types),
            "full u32 in Trapping witness")
    module = ir13.decode(ckir)
    require(ir13.selected_subtract_count(module) == 1, "selected subtraction count")
    require(ir13.interpret(module) == 70, "independent CKIR result")
    pattern = re.compile(
        rb"\x8b\x85....\x2b\x85....\x0f\x82...."
        rb"\x3d\x00\x00\x00\x00\x0f\x82...."
        rb"\x3d\xff\xff\xff\xff\x0f\x87....\x89\x85....", re.S)
    require(elf.startswith(b"\x7fELF\x02\x01\x01") and pattern.search(elf) is not None,
            "selected ELF subtraction template")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("frame", type=Path)
    args = parser.parse_args()
    check(args.frame)
    print("OMGRFN15 candidate reference: frame/source/OMGRSW5/CKIR13/result/template valid")


if __name__ == "__main__":
    try:
        main()
    except (ReferenceError, ir13.Ckir13Error, OSError, struct.error, ValueError) as error:
        print(f"OMGRFN15 candidate reference: {error}", file=sys.stderr)
        raise SystemExit(251)
