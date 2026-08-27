#!/usr/bin/env python3
"""OMGLOWK20 frame builder and focused lowering controls."""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

HEADER = struct.Struct("<8sHHHH4I")


def pack(compilation: bytes, witness: bytes, *, major: int = 20,
         selector: int = 11) -> bytes:
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
        "wrong-outer-version": pack(comp, resolution, major=19),
        "wrong-selector": pack(comp, resolution, selector=10),
        "trailing-byte": base + b"\0",
        "source-witness-cross-pair": pack(cross.read_bytes(), resolution),
        "wrong-witness-major": mutate_u32(base, wb + 8, 10),
        "wrong-copy-flag": mutate_u32(base, wb + 500 + 28, 0),
        "wrong-store-field": mutate_u32(base, wb + 1668 + 20, 1),
        "wrong-argument-value": mutate_u32(base, wb + 1956 + 16, 69),
        "wrong-call-target": mutate_u32(base, wb + 1596 + 12, 1),
    }
    lines = []
    for name, contents in controls.items():
        path = output / f"{name}.omglowk"; path.write_bytes(contents)
        lines.append(f"{name}\t251\t{path}\n")
    exhausted = output / "input-exhausted.omglowk"
    exhausted.write_bytes(b"\0" * 269486)
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


if __name__ == "__main__": main()
