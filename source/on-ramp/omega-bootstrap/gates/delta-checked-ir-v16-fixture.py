#!/usr/bin/env python3
"""Handcrafted CKIR16 u64 Less carrier and isolated mutations."""

from __future__ import annotations

import argparse
import copy
import struct
from pathlib import Path

import checked_ir_v16_reference as ir16

NO_ID = ir16.NO_ID


def replace(row: tuple[int, ...], index: int, value: int) -> tuple[int, ...]:
    return row[:index] + (value,) + row[index + 1:]


def tables(left: int = 0x80000000FFFFFFFE,
           right: int = 0x80000000FFFFFFFF) -> dict[str, list[tuple[int, ...]]]:
    t = {name: [] for name in ir16.TABLE_ORDER}
    ll, lh = left & 0xFFFFFFFF, left >> 32
    rl, rh = right & 0xFFFFFFFF, right >> 32
    t["types"] = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 3, 0, 0, 0, 0, 0, 1),
        (2, 2, 0, 0, 0, 0, 0, 0x7FFFFFFF),
        (3, 8, 0, 0, 0, 0, 0xFFFFFFFF, 0xFFFFFFFF),
        (4, 8, 0, 0, 0, 0, ll, lh),
    ]
    t["records"] = [(0, 0, 0, 1, 1, 0, 0, 0)]
    t["fields"] = [(0, 0, 0, 4)]
    t["machines"] = [
        (0, 0, 2, 0, 0, 2, 0, 0, 0, 3, 0),
        (1, 0, 1, 0, 0, 4, 0, 1, 3, 1, 3),
    ]
    t["machine_params"] = [(0, 1, 0, 4, 0)]
    t["blocks"] = [
        (0, 0, 2, 0, 0, 0, 0, 0, 11, 0),
        (1, 0, 2, 0, 0, 0, 1, 11, 1, 1),
        (2, 0, 2, 0, 0, 1, 1, 12, 1, 2),
        (3, 1, 1, 0, 0, 2, 0, 13, 0, 3),
    ]
    t["block_params"] = [(0, 1, 0, 4, 1), (1, 2, 0, 3, 2)]
    t["operations"] = [
        (0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 0),
        (1, 0, 0, 3, 2, 0, 1, 4, 0, 1, 0, 0),
        (2, 0, 0, 1, 1, 0, 3, 4, 1, 0, ll, lh),
        (3, 0, 0, 6, 0, 0, NO_ID, NO_ID, 1, 2, 0, 0),
        (4, 0, 0, 5, 1, 0, 4, 4, 3, 1, 0, 0),
        (5, 0, 0, 10, 1, 0, 5, 4, 4, 2, 1, 0),
        (6, 0, 0, 6, 0, 0, NO_ID, NO_ID, 6, 2, 0, 0),
        (7, 0, 0, 5, 1, 0, 6, 4, 8, 1, 0, 0),
        (8, 0, 0, 1, 1, 0, 7, 3, 9, 0, rl, rh),
        (9, 0, 0, 9, 1, 0, 8, 1, 9, 2, 0, 0),
        (10, 0, 0, 13, 1, 0, 9, 0, 11, 1, 0, 0),
        (11, 0, 1, 1, 1, 0, 10, 2, 12, 0, 70, 0),
        (12, 0, 2, 1, 1, 0, 11, 2, 12, 0, 0, 0),
    ]
    t["operands"] = [
        (0,), (1,), (3,), (1,), (0,), (4,), (1,), (5,), (1,),
        (6,), (7,), (6,), (6,), (6,),
    ]
    t["terminators"] = [
        (0, 0, 0, 2, 0, 0, 8, 1, 12, 1, 2, 13, 1, 0, 0),
        (1, 0, 1, 4, 0, 0, 10, NO_ID, 14, 0, NO_ID, 14, 0, 0, 0),
        (2, 0, 2, 4, 0, 0, 11, NO_ID, 14, 0, NO_ID, 14, 0, 0, 0),
        (3, 1, 3, 4, 0, 0, 0, NO_ID, 14, 0, NO_ID, 14, 0, 0, 0),
    ]
    return t


def encode(tables_: dict[str, list[tuple[int, ...]]], *, major: int = 16,
           flags: int = 1) -> bytes:
    counts = {name: len(tables_[name]) for name in ir16.TABLE_ORDER}
    counts.update(values=12, places=2)
    payload = b"".join(ir16.ROWS[name].pack(*row)
                       for name in ir16.TABLE_ORDER for row in tables_[name])
    return ir16.HEADER.pack(
        b"OMGCKIR\0", major, 0, 1, flags, 0 if flags else NO_ID,
        ir16.HEADER.size + len(payload),
        *(counts[name] for name in ir16.COUNT_NAMES),
    ) + payload


def emit(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    positives = {
        "cross-high-true": tables(),
        "cross-high-false": tables(0x8000000100000000, 0x80000000FFFFFFFF),
        "low-half-true": tables(0x00000000FFFFFFFE, 0x00000000FFFFFFFF),
    }
    for name, carrier in positives.items():
        (directory / f"{name}.ckir16").write_bytes(encode(carrier))
    base = tables()
    mutations: list[tuple[str, int]] = []
    def mutation(name: str, change, expected: int = 251, major: int = 16) -> None:
        changed = copy.deepcopy(base); change(changed)
        (directory / f"{name}.ckir16").write_bytes(encode(changed, major=major))
        mutations.append((name, expected))
    mutation("kind8-trapping-flag", lambda t: t["types"].__setitem__(3, replace(t["types"][3], 2, 1)))
    mutation("reversed-u64-range", lambda t: t["types"].__setitem__(4, (4, 8, 0, 0, 0, 1, 0, 0)))
    mutation("const-above-range", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 10, 0xFFFFFFFF)))
    mutation("u64-less-equal", lambda t: t["operations"].__setitem__(9, replace(t["operations"][9], 3, 12)))
    mutation("u64-equal", lambda t: t["operations"].__setitem__(9, replace(t["operations"][9], 3, 18)))
    mutation("u64-greater", lambda t: t["operations"].__setitem__(9, replace(t["operations"][9], 3, 19)))
    mutation("missing-u64-less", lambda t: t["operations"].__setitem__(9, replace(t["operations"][9], 3, 9)), major=15)
    (directory / "positives.tsv").write_text(
        "cross-high-true\t70\ncross-high-false\t0\nlow-half-true\t70\n",
        encoding="ascii")
    (directory / "manifest.tsv").write_text(
        "".join(f"{name}\t{status}\n" for name, status in mutations), encoding="ascii")


def check(path: Path, expected: int) -> None:
    module = ir16.decode(path.read_bytes())
    ir16.v5.require(ir16.interpret(module) == expected, "u64 carrier result")
    ir16.v5.require(ir16.selected_count(module) == 1, "one u64 Less")
    ir16.v5.require(not any(row[1] == 7 for row in module.tables["types"]),
                   "no-view CKIR16 positive")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("emit", "check"))
    parser.add_argument("path", type=Path)
    parser.add_argument("expected", nargs="?", type=int)
    args = parser.parse_args()
    emit(args.path) if args.command == "emit" else check(args.path, int(args.expected))


if __name__ == "__main__":
    main()
