#!/usr/bin/env python3
"""OMGLOWJ19 frame builder and focused lowering controls."""

from __future__ import annotations

import argparse
import struct
from pathlib import Path


HEADER = struct.Struct("<8sHHHH4I")


def pack(compilation: bytes, witness: bytes, *, major: int = 19,
         selector: int = 10) -> bytes:
    total = HEADER.size + len(compilation) + len(witness)
    return HEADER.pack(b"OMGLOWJ\0", major, 0, 0, HEADER.size, total,
                       len(compilation), len(witness), selector) + compilation + witness


def mutate_u32(contents: bytes, offset: int, value: int) -> bytes:
    changed = bytearray(contents)
    struct.pack_into("<I", changed, offset, value)
    return bytes(changed)


def cases(compilation: Path, witness: Path, cross_compilation: Path,
          output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    comp = compilation.read_bytes()
    resolution = witness.read_bytes()
    base = pack(comp, resolution)
    controls = {
        "wrong-outer-version": pack(comp, resolution, major=18),
        "wrong-selector": pack(comp, resolution, selector=9),
        "trailing-byte": base + b"\0",
        "source-witness-cross-pair": pack(cross_compilation.read_bytes(), resolution),
        "wrong-witness-major": mutate_u32(base, HEADER.size + len(comp) + 8, 9),
        "wrong-index-policy": bytes(bytearray(base[:HEADER.size + len(comp) + 148 + 3 * 32 + 5])
                                      + bytes([0])
                                      + base[HEADER.size + len(comp) + 148 + 3 * 32 + 6:]),
        "wrong-call-target": mutate_u32(
            base, HEADER.size + len(comp) + 1196 + 12, 2),
    }
    lines = []
    for name, contents in controls.items():
        path = output / f"{name}.omglowj"
        path.write_bytes(contents)
        lines.append(f"{name}\t251\t{path}\n")
    exhausted = output / "input-exhausted.omglowj"
    exhausted.write_bytes(b"\0" * 268690)
    lines.append(f"input-exhausted\t252\t{exhausted}\n")
    (output / "manifest.tsv").write_text("".join(lines), encoding="ascii")


def main() -> None:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    packer = sub.add_parser("pack")
    packer.add_argument("compilation", type=Path)
    packer.add_argument("witness", type=Path)
    packer.add_argument("output", type=Path)
    controls = sub.add_parser("cases")
    controls.add_argument("compilation", type=Path)
    controls.add_argument("witness", type=Path)
    controls.add_argument("cross_compilation", type=Path)
    controls.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.command == "pack":
        args.output.write_bytes(pack(args.compilation.read_bytes(), args.witness.read_bytes()))
    else:
        cases(args.compilation, args.witness, args.cross_compilation, args.output)


if __name__ == "__main__":
    main()
