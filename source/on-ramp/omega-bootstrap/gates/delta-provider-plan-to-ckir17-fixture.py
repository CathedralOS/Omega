#!/usr/bin/env python3
"""Focused OMGLOWI18 packing and source/resolution mutation corpus."""

from __future__ import annotations

import argparse
import struct
from pathlib import Path


HEADER = struct.Struct("<8sHHHH4I")


def pack(compilation: bytes, witness: bytes, *, major: int = 18,
         selector: int = 9) -> bytes:
    total = HEADER.size + len(compilation) + len(witness)
    return HEADER.pack(
        b"OMGLOWI\0", major, 0, 0, HEADER.size, total,
        len(compilation), len(witness), selector,
    ) + compilation + witness


def replace_u16(raw: bytes, offset: int, value: int) -> bytes:
    changed = bytearray(raw)
    struct.pack_into("<H", changed, offset, value)
    return bytes(changed)


def replace_u32(raw: bytes, offset: int, value: int) -> bytes:
    changed = bytearray(raw)
    struct.pack_into("<I", changed, offset, value)
    return bytes(changed)


def build_cases(compilation: bytes, witness: bytes, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    canonical = pack(compilation, witness)
    (output / "canonical.omglowi").write_bytes(canonical)
    cases: list[tuple[str, int, bytes]] = []

    def semantic(name: str, relative: int, value: int) -> None:
        changed = replace_u32(witness, relative, value)
        cases.append((name, 251, pack(compilation, changed)))

    cases.append(("old-outer-major", 251, replace_u16(canonical, 8, 17)))
    cases.append(("old-selector", 251, replace_u32(canonical, 28, 8)))
    cases.append(("trailing-byte", 251, canonical + b"\0"))
    cases.append(("truncated-frame", 251, canonical[:-1]))
    cases.append(("old-witness-major", 251,
                  pack(compilation, replace_u16(witness, 8, 8))))

    marker = b"output as i32"
    if compilation.count(marker) != 2:
        raise SystemExit("expected two explicit output-as-i32 source sites")
    wrong_cast = compilation.replace(marker, b"output as u32", 1)
    cases.append(("source-witness-cross-pair", 251,
                  pack(wrong_cast, witness)))

    semantic("helper-console-type", 1056 + 16, 6)
    semantic("adapter-requirement", 1104 + 12, 0)
    semantic("intrinsic-requirement", 1208 + 4 * 56 + 16, 5)
    semantic("incomplete-plan-row", 1824 + 4 * 24 + 16, 5)
    semantic("cast-call-requirement", 1968 + 16, 5)
    semantic("ordinary-call-helper", 2232 + 16, 1)

    cases.append(("compilation-declared-exhaustion", 252,
                  replace_u32(canonical, 20, 267_281)))
    cases.append(("witness-declared-exhaustion", 252,
                  replace_u32(canonical, 24, 524_289)))
    cases.append(("carrier-exhaustion", 252, bytes(269_617)))

    manifest: list[str] = []
    for name, status, contents in cases:
        path = output / f"{name}.omglowi"
        path.write_bytes(contents)
        manifest.append(f"{name}\t{status}\t{path}\n")
    (output / "manifest.tsv").write_text("".join(manifest), encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    pack_parser = sub.add_parser("pack")
    pack_parser.add_argument("compilation", type=Path)
    pack_parser.add_argument("witness", type=Path)
    pack_parser.add_argument("output", type=Path)
    cases_parser = sub.add_parser("cases")
    cases_parser.add_argument("compilation", type=Path)
    cases_parser.add_argument("witness", type=Path)
    cases_parser.add_argument("output", type=Path)
    args = parser.parse_args()
    compilation = args.compilation.read_bytes()
    witness = args.witness.read_bytes()
    if args.command == "pack":
        args.output.write_bytes(pack(compilation, witness))
    else:
        build_cases(compilation, witness, args.output)


if __name__ == "__main__":
    main()
