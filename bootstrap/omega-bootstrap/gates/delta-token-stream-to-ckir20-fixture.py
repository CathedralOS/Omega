#!/usr/bin/env python3
"""OMGLOWL21 frame builder and focused OMGRSWC12 lowering controls."""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

HEADER = struct.Struct("<8sHHHH4I")


def pack(compilation: bytes, witness: bytes, *, major: int = 21,
         selector: int = 12) -> bytes:
    total = HEADER.size + len(compilation) + len(witness)
    return HEADER.pack(b"OMGLOWK\0", major, 0, 0, HEADER.size, total,
                       len(compilation), len(witness), selector) + compilation + witness


def mutate_u32(contents: bytes, offset: int, value: int) -> bytes:
    changed = bytearray(contents)
    struct.pack_into("<I", changed, offset, value)
    return bytes(changed)


def cases(compilation: Path, witness: Path, cross: Path, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    comp = compilation.read_bytes(); resolution = witness.read_bytes()
    base = pack(comp, resolution); wb = HEADER.size + len(comp)
    controls = {
        "wrong-outer-version": pack(comp, resolution, major=20),
        "wrong-selector": pack(comp, resolution, selector=11),
        "trailing-byte": base + b"\0",
        "source-witness-cross-pair": pack(cross.read_bytes(), resolution),
        "wrong-witness-major": mutate_u32(base, wb + 8, 11),
        "wrong-record-copy": mutate_u32(base, wb + 916 + 28, 0),
        "wrong-sum-copy": mutate_u32(base, wb + 1868 + 28, 0),
        "wrong-store-owner": mutate_u32(base, wb + 6488 + 12, 21),
        "wrong-float-constructor": mutate_u32(base, wb + 7168 + 32 + 20, 77),
        "wrong-observation-tag": mutate_u32(base, wb + 7168 + 6 * 32 + 20, 69),
        "wrong-call-target": mutate_u32(base, wb + 6416 + 12, 1),
    }
    lines = []
    for name, contents in controls.items():
        path = output / f"{name}.omglowl"; path.write_bytes(contents)
        lines.append(f"{name}\t251\t{path}\n")
    exhausted = output / "input-exhausted.omglowl"
    exhausted.write_bytes(b"\0" * 274_834)
    lines.append(f"input-exhausted\t252\t{exhausted}\n")
    (output / "manifest.tsv").write_text("".join(lines), encoding="ascii")


def main() -> None:
    parser = argparse.ArgumentParser(); sub = parser.add_subparsers(dest="command", required=True)
    item = sub.add_parser("pack"); item.add_argument("compilation", type=Path); item.add_argument("witness", type=Path); item.add_argument("output", type=Path)
    item = sub.add_parser("cases"); item.add_argument("compilation", type=Path); item.add_argument("witness", type=Path); item.add_argument("cross", type=Path); item.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.command == "pack":
        args.output.write_bytes(pack(args.compilation.read_bytes(), args.witness.read_bytes()))
    else:
        cases(args.compilation, args.witness, args.cross, args.output)


if __name__ == "__main__":
    main()
