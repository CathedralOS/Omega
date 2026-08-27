#!/usr/bin/env python3
"""Handcrafted CKIR18 SourceUnit fixed-buffer carrier and mutations."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import struct
from pathlib import Path

import checked_ir_v18_reference as ir18


NO_ID = ir18.NO_ID


def replace(row: tuple[int, ...], index: int, value: int) -> tuple[int, ...]:
    return row[:index] + (value,) + row[index + 1:]


class Builder:
    def __init__(self) -> None:
        self.operations: list[tuple[int, ...]] = []
        self.operands: list[tuple[int]] = []
        self.blocks: list[tuple[int, ...]] = []
        self.values = 2  # append(byte), byte_or_nul(index)
        self.places = 0

    def operation(self, owner: int, block: int, opcode: int,
                  result_type: int | None, args: tuple[int, ...] = (),
                  *, place: bool = False, imm0: int = 0, imm1: int = 0) -> int:
        op_id = len(self.operations)
        start = len(self.operands)
        self.operands.extend((value,) for value in args)
        if result_type is None:
            result_kind, result_id, wire_type = 0, NO_ID, NO_ID
            returned = NO_ID
        elif place:
            result_kind, result_id, wire_type = 2, self.places, result_type
            returned = self.places
            self.places += 1
        else:
            result_kind, result_id, wire_type = 1, self.values, result_type
            returned = self.values
            self.values += 1
        self.operations.append((
            op_id, owner, block, opcode, result_kind, 0, result_id, wire_type,
            start, len(args), imm0, imm1,
        ))
        return returned

    def block(self, owner: int, access: int, body) -> int:
        block_id = len(self.blocks)
        start = len(self.operations)
        body(block_id)
        self.blocks.append((
            block_id, owner, access, 0, 0, 0, 0, start,
            len(self.operations) - start, block_id,
        ))
        return block_id


def tables(*, extra: str = "none", array_length: int = 65_536) \
        -> dict[str, list[tuple[int, ...]]]:
    t = {name: [] for name in ir18.TABLE_ORDER}
    t["types"] = [
        (0, 1, 0, 0, 0, 0, 0, 255),                         # u8
        (1, 3, 0, 0, 0, 0, 0, 1),                           # bool
        (2, 8, 0, 0, 0, 0, 0xFFFF_FFFF, 0xFFFF_FFFF),       # u64
        (3, 8, 0, 0, 0, 0, 65_536, 0),                     # u64 [0..=N]
        (4, 8, 0, 0, 0, 0, 0, 0),                          # u64 0
        (5, 8, 0, 0, 1, 0, 1, 0),                          # u64 1
        (6, 8, 0, 0, 65_536, 0, 65_536, 0),                # u64 N
        (7, 5, 0, 0, 0, array_length, 0, 0),                # [u8; N]
        (8, 4, 0, 0, 0, 0, 0, 0),                          # SourceUnit
        (9, 4, 0, 0, 1, 0, 0, 0),                          # Main
    ]
    t["records"] = [
        (0, 8, 0, 3, 1, 0, 0, 0),
        (1, 9, 3, 3, 1, 0, 0, 0),
    ]
    t["fields"] = [
        (0, 0, 0, 7), (1, 0, 1, 3), (2, 0, 2, 1),
        (3, 1, 0, 8), (4, 1, 1, 0), (5, 1, 2, 0),
    ]
    t["machines"] = [
        (0, 0, 2, 0, 0, NO_ID, 0, 0, 0, 1, 0),
        (1, 0, 2, 0, 0, NO_ID, 0, 1, 1, 3, 1),
        (2, 0, 1, 0, 0, 0, 1, 1, 4, 3, 4),
        (3, 1, 2, 0, 0, 0, 2, 0, 7, 1, 7),
    ]
    t["machine_params"] = [
        (0, 1, 0, 0, 0),
        (1, 2, 0, 2, 1),
    ]
    b = Builder()
    term_shapes: dict[int, tuple[int, int]] = {}

    def clear(block: int) -> None:
        self_p = b.operation(0, block, 2, 8, place=True)
        length_p = b.operation(0, block, 3, 3, (self_p,), place=True, imm0=1)
        zero = b.operation(0, block, 1, 4)
        b.operation(0, block, 6, None, (length_p, zero))
        retained_p = b.operation(0, block, 3, 1, (self_p,), place=True, imm0=2)
        true = b.operation(0, block, 1, 1, imm0=1)
        b.operation(0, block, 6, None, (retained_p, true))
    b.block(0, 2, clear)

    def append_entry(block: int) -> None:
        self_p = b.operation(1, block, 2, 8, place=True)
        length_p = b.operation(1, block, 3, 3, (self_p,), place=True, imm0=1)
        retained_p = b.operation(1, block, 3, 1, (self_p,), place=True, imm0=2)
        false = b.operation(1, block, 1, 1)
        b.operation(1, block, 6, None, (retained_p, false))
        length = b.operation(1, block, 5, 3, (length_p,))
        capacity = b.operation(1, block, 1, 6, imm0=65_536)
        condition = b.operation(1, block, 9, 1, (length, capacity))
        term_shapes[block] = (2, condition)
    b.block(1, 2, append_entry)

    def append_retain(block: int) -> None:
        self_p = b.operation(1, block, 2, 8, place=True)
        bytes_p = b.operation(1, block, 3, 7, (self_p,), place=True, imm0=0)
        length_p = b.operation(1, block, 3, 3, (self_p,), place=True, imm0=1)
        retained_p = b.operation(1, block, 3, 1, (self_p,), place=True, imm0=2)
        length = b.operation(1, block, 5, 3, (length_p,))
        byte_p = b.operation(1, block, 4, 0, (bytes_p, length), place=True)
        b.operation(1, block, 6, None, (byte_p, 0))
        one = b.operation(1, block, 1, 5, imm0=1)
        incremented = b.operation(1, block, 8, 3, (length, one))
        b.operation(1, block, 6, None, (length_p, incremented))
        true = b.operation(1, block, 1, 1, imm0=1)
        b.operation(1, block, 6, None, (retained_p, true))
    b.block(1, 2, append_retain)

    def append_full(block: int) -> None:
        self_p = b.operation(1, block, 2, 8, place=True)
        retained_p = b.operation(1, block, 3, 1, (self_p,), place=True, imm0=2)
        false = b.operation(1, block, 1, 1)
        b.operation(1, block, 6, None, (retained_p, false))
    b.block(1, 2, append_full)

    def read_entry(block: int) -> None:
        self_p = b.operation(2, block, 2, 8, place=True)
        length_p = b.operation(2, block, 3, 3, (self_p,), place=True, imm0=1)
        length = b.operation(2, block, 5, 3, (length_p,))
        condition = b.operation(2, block, 9, 1, (1, length))
        term_shapes[block] = (2, condition)
    b.block(2, 1, read_entry)

    def read_present(block: int) -> None:
        self_p = b.operation(2, block, 2, 8, place=True)
        bytes_p = b.operation(2, block, 3, 7, (self_p,), place=True, imm0=0)
        byte_p = b.operation(2, block, 4, 0, (bytes_p, 1), place=True)
        value = b.operation(2, block, 5, 0, (byte_p,))
        term_shapes[block] = (4, value)
    b.block(2, 1, read_present)

    def read_absent(block: int) -> None:
        value = b.operation(2, block, 1, 0)
        term_shapes[block] = (4, value)
    b.block(2, 1, read_absent)

    def run(block: int) -> None:
        main_p = b.operation(3, block, 2, 9, place=True)
        source_p = b.operation(3, block, 3, 8, (main_p,), place=True, imm0=3)
        b.operation(3, block, 10, None, (source_p,), imm0=0)
        byte70 = b.operation(3, block, 1, 0, imm0=70)
        b.operation(3, block, 10, None, (source_p, byte70), imm0=1)
        zero = b.operation(3, block, 1, 2)
        observed = b.operation(3, block, 10, 0, (source_p, zero), imm0=2)
        observed_p = b.operation(3, block, 3, 0, (main_p,), place=True, imm0=4)
        b.operation(3, block, 6, None, (observed_p, observed))
        length_p = b.operation(3, block, 3, 3, (source_p,), place=True, imm0=1)
        capacity = b.operation(3, block, 1, 6, imm0=65_536)
        b.operation(3, block, 6, None, (length_p, capacity))
        byte71 = b.operation(3, block, 1, 0, imm0=71)
        b.operation(3, block, 10, None, (source_p, byte71), imm0=1)
        absent_index = b.operation(3, block, 1, 2, imm0=65_536)
        absent = b.operation(3, block, 10, 0, (source_p, absent_index), imm0=2)
        absent_p = b.operation(3, block, 3, 0, (main_p,), place=True, imm0=5)
        b.operation(3, block, 6, None, (absent_p, absent))
        if extra == "high-half-add":
            low_max = b.operation(3, block, 1, 2, imm0=0xFFFF_FFFF)
            one = b.operation(3, block, 1, 5, imm0=1)
            b.operation(3, block, 8, 2, (low_max, one))
        elif extra == "carry":
            maximum = b.operation(3, block, 1, 2,
                                  imm0=0xFFFF_FFFF, imm1=0xFFFF_FFFF)
            one = b.operation(3, block, 1, 5, imm0=1)
            b.operation(3, block, 8, 2, (maximum, one))
        elif extra == "interval":
            one = b.operation(3, block, 1, 5, imm0=1)
            b.operation(3, block, 8, 3, (capacity, one))
        elif extra == "index-oob-high":
            high = b.operation(3, block, 1, 2, imm1=1)
            bytes_p = b.operation(3, block, 3, 7, (source_p,), place=True,
                                  imm0=0)
            b.operation(3, block, 4, 0, (bytes_p, high), place=True)
        result = b.operation(3, block, 5, 0, (observed_p,))
        term_shapes[block] = (4, result)
    b.block(3, 2, run)

    t["blocks"] = b.blocks
    t["operations"] = b.operations
    t["operands"] = b.operands
    edge_start = len(b.operands)
    t["terminators"] = []
    for block_id, block in enumerate(b.blocks):
        owner = block[1]
        if block_id == 1:
            kind, value, target0, target1 = 2, term_shapes[block_id][1], 2, 3
        elif block_id == 4:
            kind, value, target0, target1 = 2, term_shapes[block_id][1], 5, 6
        elif block_id in term_shapes:
            kind, value = term_shapes[block_id]
            target0 = target1 = NO_ID
        else:
            kind, value, target0, target1 = 3, NO_ID, NO_ID, NO_ID
        t["terminators"].append((
            block_id, owner, block_id, kind, 0, 0, value,
            target0, edge_start, 0, target1, edge_start, 0, 0, 0,
        ))
    t["_counts"] = [(b.values, b.places)]
    return t


def encode(raw: dict[str, list[tuple[int, ...]]], *, major: int = 18) -> bytes:
    values, places = raw["_counts"][0]
    counts = {name: len(raw[name]) for name in ir18.TABLE_ORDER}
    counts.update(values=values, places=places)
    payload = b"".join(
        ir18.ROWS[name].pack(*row)
        for name in ir18.TABLE_ORDER for row in raw[name]
    )
    return ir18.HEADER.pack(
        b"OMGCKIR\0", major, 0, 1, 1, 3,
        ir18.HEADER.size + len(payload),
        *(counts[name] for name in ir18.COUNT_NAMES),
    ) + payload


def mutate_count(contents: bytes, name: str, value: int) -> bytes:
    changed = bytearray(contents)
    struct.pack_into("<I", changed, 24 + 4 * ir18.COUNT_NAMES.index(name), value)
    return bytes(changed)


def emit(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    base = tables()
    canonical = encode(base)
    (directory / "canonical.ckir18").write_bytes(canonical)
    high = encode(tables(extra="high-half-add"))
    (directory / "high-half-add.ckir18").write_bytes(high)
    runtime = {
        "runtime-u64-carry": encode(tables(extra="carry")),
        "runtime-result-interval": encode(tables(extra="interval")),
        "runtime-index-high-half": encode(tables(extra="index-oob-high")),
    }
    for name, contents in runtime.items():
        (directory / f"{name}.ckir18").write_bytes(contents)

    manifest: list[tuple[str, int]] = []
    def mutation(name: str, change, status: int = 251,
                 *, major: int = 18) -> None:
        changed = copy.deepcopy(base)
        change(changed)
        (directory / f"{name}.ckir18").write_bytes(encode(changed, major=major))
        manifest.append((name, status))

    mutation("kind8-policy-flag", lambda x: x["types"].__setitem__(2,
             replace(x["types"][2], 2, 1)))
    mutation("array-65537", lambda x: x["types"].__setitem__(7,
             replace(x["types"][7], 5, 65_537)))
    mutation("index-non-u8-array", lambda x: x["types"].__setitem__(7,
             replace(x["types"][7], 4, 1)))
    index_op = next(i for i, row in enumerate(base["operations"]) if row[3] == 4)
    add_op = next(i for i, row in enumerate(base["operations"]) if row[3] == 8)
    mutation("index-immediate", lambda x: x["operations"].__setitem__(index_op,
             replace(x["operations"][index_op], 10, 1)))
    mutation("index-wrong-result", lambda x: x["operations"].__setitem__(index_op,
             replace(x["operations"][index_op], 7, 1)))
    mutation("add-immediate", lambda x: x["operations"].__setitem__(add_op,
             replace(x["operations"][add_op], 11, 1)))
    mutation("add-u8-result", lambda x: x["operations"].__setitem__(add_op,
             replace(x["operations"][add_op], 7, 0)))
    mutation("missing-u64-index", lambda x: x["operations"].__setitem__(index_op,
             replace(x["operations"][index_op], 3, 3)))
    mutation("missing-u64-add", lambda x: x["operations"].__setitem__(add_op,
             replace(x["operations"][add_op], 3, 9)))
    mutation("schema-major-16", lambda _: None, major=16)
    mutation("static-view-type", lambda x: x["types"].append(
        (len(x["types"]), 7, 0, 0, 0, 0, 0, 0)))
    less_op = next(i for i, row in enumerate(base["operations"]) if row[3] == 9)
    mutation("historical-opcode-15", lambda x: x["operations"].__setitem__(
        less_op, replace(x["operations"][less_op], 3, 15)))
    entry_absent = bytearray(canonical)
    struct.pack_into("<H", entry_absent, 14, 0)
    struct.pack_into("<I", entry_absent, 16, NO_ID)
    (directory / "entry-absent.ckir18").write_bytes(entry_absent)
    manifest.append(("entry-absent", 251))
    (directory / "operations-over.ckir18").write_bytes(
        mutate_count(canonical, "operations", 32_769)
    )
    manifest.append(("operations-over", 252))
    (directory / "trailing-byte.ckir18").write_bytes(canonical + b"\0")
    manifest.append(("trailing-byte", 251))
    (directory / "positives.tsv").write_text(
        "canonical\t70\nhigh-half-add\t70\n", encoding="ascii"
    )
    (directory / "runtime.tsv").write_text(
        "runtime-u64-carry\nruntime-result-interval\nruntime-index-high-half\n",
        encoding="ascii",
    )
    (directory / "manifest.tsv").write_text(
        "".join(f"{name}\t{status}\n" for name, status in manifest),
        encoding="ascii",
    )
    (directory / "identity.json").write_text(json.dumps({
        "byte_length": len(canonical),
        "sha256": hashlib.sha256(canonical).hexdigest(),
        "counts": {name: len(base[name]) for name in ir18.TABLE_ORDER}
                  | {"values": base["_counts"][0][0],
                     "places": base["_counts"][0][1]},
    }, indent=2, sort_keys=True) + "\n", encoding="ascii")


def check(path: Path, expected: int) -> None:
    module = ir18.decode(path.read_bytes())
    ir18.v5.require(ir18.interpret(module) == expected, "CKIR18 result")
    selected = ir18.selected_operations(module)
    ir18.v5.require(all(selected.values()), "complete CKIR18 selected relation")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("emit", "check"))
    parser.add_argument("path", type=Path)
    parser.add_argument("expected", nargs="?", type=int)
    args = parser.parse_args()
    emit(args.path) if args.command == "emit" else check(args.path, int(args.expected))


if __name__ == "__main__":
    main()
