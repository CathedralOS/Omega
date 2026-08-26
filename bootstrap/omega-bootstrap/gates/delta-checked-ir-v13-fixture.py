#!/usr/bin/env python3
"""Handcrafted CKIR13 full-u32 Subtract carriers, mutations, and artifact checks."""

from __future__ import annotations

import argparse
import copy
import re
import struct
from pathlib import Path

import checked_ir_v13_reference as ir13


NO_ID = ir13.NO_ID


def replace(row: tuple[int, ...], index: int, value: int) -> tuple[int, ...]:
    return row[:index] + (value,) + row[index + 1:]


def tables(left: int, right: int, expected: int) -> dict[str, list[tuple[int, ...]]]:
    result = {name: [] for name in ir13.TABLE_ORDER}
    result["types"] = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 3, 0, 0, 0, 0, 0, 1),
        (2, 2, 0, 0, 0, 0, 0, 0xFFFF_FFFF),
        (3, 1, 0, 0, 0, 0, 0, 255),
        (4, 2, 1, 0, 0, 0, 0, 0xFFFF_FFFF),
        (5, 2, 1, 0, 0, 0, 0, 255),
    ]
    result["records"] = [(0, 0, 0, 0, 0, 0, 0, 0)]
    result["machines"] = [(0, 0, 2, 0, 0, 2, 0, 0, 0, 3, 0)]
    result["blocks"] = [
        (0, 0, 2, 0, 0, 0, 0, 0, 5, 0),
        (1, 0, 2, 0, 0, 0, 0, 5, 1, 1),
        (2, 0, 2, 0, 0, 0, 0, 6, 1, 2),
    ]
    result["operations"] = [
        (0, 0, 0, 1, 1, 0, 0, 4, 0, 0, left, 0),
        (1, 0, 0, 1, 1, 0, 1, 4, 0, 0, right, 0),
        (2, 0, 0, 26, 1, 0, 2, 4, 0, 2, 0, 0),
        (3, 0, 0, 1, 1, 0, 3, 4, 2, 0, expected, 0),
        (4, 0, 0, 18, 1, 0, 4, 1, 2, 2, 0, 0),
        (5, 0, 1, 1, 1, 0, 5, 2, 4, 0, 70, 0),
        (6, 0, 2, 1, 1, 0, 6, 2, 4, 0, 0, 0),
    ]
    result["operands"] = [(0,), (1,), (2,), (3,)]
    result["terminators"] = [
        (0, 0, 0, 2, 0, 0, 4, 1, 4, 0, 2, 4, 0, 0, 0),
        (1, 0, 1, 4, 0, 0, 5, NO_ID, 4, 0, NO_ID, 4, 0, 0, 0),
        (2, 0, 2, 4, 0, 0, 6, NO_ID, 4, 0, NO_ID, 4, 0, 0, 0),
    ]
    return result


def encode(raw: dict[str, list[tuple[int, ...]]], *, major: int = 13) -> bytes:
    counts = {name: len(raw[name]) for name in ir13.TABLE_ORDER}
    counts.update(values=7, places=0)
    payload = b"".join(ir13.ROWS[name].pack(*row)
                       for name in ir13.TABLE_ORDER for row in raw[name])
    return ir13.HEADER.pack(
        b"OMGCKIR\0", major, 0, 1, 1, 0, ir13.HEADER.size + len(payload),
        *(counts[name] for name in ir13.COUNT_NAMES),
    ) + payload


def mutate_count(contents: bytes, name: str, value: int) -> bytes:
    changed = bytearray(contents)
    struct.pack_into("<I", changed, 24 + 4 * ir13.COUNT_NAMES.index(name), value)
    return bytes(changed)


def emit(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    positives = {
        "max-minus-near-max": tables(0xFFFF_FFFF, 0xFFFF_FFB9, 70),
        "seventy-minus-zero": tables(70, 0, 70),
        "equal-minus-equal": tables(0xFFFF_FFFF, 0xFFFF_FFFF, 0),
    }
    for name, value in positives.items():
        (directory / f"{name}.ckir13").write_bytes(encode(value))
    (directory / "underflow.ckir13").write_bytes(encode(tables(0, 1, 0)))

    base = tables(0xFFFF_FFFF, 0xFFFF_FFB9, 70)
    canonical = encode(base)
    (directory / "canonical.ckir13").write_bytes(canonical)
    manifest: list[tuple[str, int]] = []

    def mutation(name: str, change, status: int = 251, major: int = 13) -> None:
        changed = copy.deepcopy(base); change(changed)
        (directory / f"{name}.ckir13").write_bytes(encode(changed, major=major))
        manifest.append((name, status))

    for major in (10, 11, 12):
        mutation(f"old-major-{major}", lambda _: None, major=major)
    mutation("missing-subtract", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 3, 8)))
    mutation("arity-one", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 9, 1)))
    mutation("nonzero-imm0", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 10, 1)))
    mutation("nonzero-imm1", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 11, 1)))
    mutation("wrong-result-type", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 7, 5)))
    mutation("right-constrained", lambda t: t["operations"].__setitem__(1, replace(t["operations"][1], 7, 5)))
    mutation("reversed-visibility", lambda t: t["operands"].__setitem__(0, (2,)))
    mutation("narrow-canonical-u32", lambda t: t["types"].__setitem__(2, replace(t["types"][2], 7, 0x7FFF_FFFF)))
    for name, count, value in (("operations-over", "operations", 32_769),
                               ("operands-over", "operands", 94_209)):
        (directory / f"{name}.ckir13").write_bytes(mutate_count(canonical, count, value))
        manifest.append((name, 252))
    (directory / "positives.tsv").write_text(
        "max-minus-near-max\t70\nseventy-minus-zero\t70\nequal-minus-equal\t70\n",
        encoding="ascii",
    )
    (directory / "manifest.tsv").write_text(
        "".join(f"{name}\t{status}\n" for name, status in manifest), encoding="ascii")


def check_ir(path: Path, expected: int) -> None:
    module = ir13.decode(path.read_bytes())
    ir13.v5.require(ir13.interpret(module) == expected, "CKIR13 result")
    ir13.v5.require(ir13.selected_subtract_count(module) == 1, "Subtract count")


def check_artifact(path: Path) -> None:
    artifact = path.read_bytes()
    ir13.v5.require(artifact.startswith(b"\x7fELF\x02\x01\x01"), "ELF64 header")
    template = re.compile(
        rb"\x8b\x85....\x2b\x85....\x0f\x82...."
        rb"\x3d\x00\x00\x00\x00\x0f\x82...."
        rb"\x3d\xff\xff\xff\xff\x0f\x87....\x89\x85....", re.DOTALL)
    ir13.v5.require(template.search(artifact) is not None,
                     "canonical SUB/borrow/range/store template")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("emit", "check-ir", "check-artifact"))
    parser.add_argument("path", type=Path)
    parser.add_argument("expected", type=int, nargs="?")
    args = parser.parse_args()
    if args.command == "emit": emit(args.path)
    elif args.command == "check-ir": check_ir(args.path, 70 if args.expected is None else args.expected)
    else: check_artifact(args.path)


if __name__ == "__main__":
    main()
