#!/usr/bin/env python3
"""Handcrafted CKIR19 flat TokenObservation record-array carrier."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import struct
from pathlib import Path

import checked_ir_v19_reference as ir19


NO_ID = ir19.NO_ID


def replace(row: tuple[int, ...], index: int, value: int) -> tuple[int, ...]:
    return row[:index] + (value,) + row[index + 1:]


class Builder:
    def __init__(self) -> None:
        self.operations: list[tuple[int, ...]] = []
        self.operands: list[tuple[int]] = []
        self.blocks: list[tuple[int, ...]] = []
        self.values = 10  # push's nine parameters plus read_tag's index
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


def tables(*, extra: str = "none", array_length: int = 16_384) \
        -> dict[str, list[tuple[int, ...]]]:
    t = {name: [] for name in ir19.TABLE_ORDER}
    t["types"] = [
        (0, 1, 0, 0, 0, 0, 0, 255),                         # u8
        (1, 3, 0, 0, 0, 0, 0, 1),                           # bool
        (2, 2, 1, 0, 0, 0, 0, 0xFFFF_FFFF),                 # u32 in Trapping
        (3, 8, 0, 0, 0, 0, 0xFFFF_FFFF, 0xFFFF_FFFF),       # u64
        (4, 8, 0, 0, 0, 0, 16_384, 0),                     # count
        (5, 8, 0, 0, 0, 0, 0, 0),                          # zero
        (6, 8, 0, 0, 1, 0, 1, 0),                          # one
        (7, 8, 0, 0, 16_384, 0, 16_384, 0),                # capacity
        (8, 5, 1, 0, 9, array_length, 0, 0),                # [Observation;N]
        (9, 4, 0, 0, 0, 0, 0, 0),                          # Observation
        (10, 4, 0, 0, 1, 0, 0, 0),                         # ObservationStream
        (11, 4, 0, 0, 2, 0, 0, 0),                         # Main
    ]
    t["records"] = [
        (0, 9, 0, 9, 1, 0, 0, 0),
        (1, 10, 9, 3, 0, 0, 0, 0),
        (2, 11, 12, 1, 0, 0, 0, 0),
    ]
    t["fields"] = [
        (0, 0, 0, 0), (1, 0, 1, 0), (2, 0, 2, 0),
        (3, 0, 3, 0), (4, 0, 4, 2),
        (5, 0, 5, 3), (6, 0, 6, 3), (7, 0, 7, 3),
        (8, 0, 8, 3),
        (9, 1, 0, 8), (10, 1, 1, 4), (11, 1, 2, 1),
        (12, 2, 0, 10),
    ]
    t["machines"] = [
        (0, 1, 2, 0, 0, NO_ID, 0, 9, 0, 3, 0),
        (1, 1, 1, 0, 0, 0, 9, 1, 3, 3, 3),
        (2, 2, 2, 0, 0, 0, 10, 0, 6, 1, 6),
    ]
    parameter_types = (0, 0, 0, 0, 2, 3, 3, 3, 3, 3)
    t["machine_params"] = [
        (parameter_id, 0 if parameter_id < 9 else 1,
         parameter_id if parameter_id < 9 else 0,
         type_id, parameter_id)
        for parameter_id, type_id in enumerate(parameter_types)
    ]

    b = Builder()
    term_shapes: dict[int, tuple[int, int]] = {}

    def writer_entry(block: int) -> None:
        self_p = b.operation(0, block, 2, 10, place=True)
        retained_p = b.operation(0, block, 3, 1, (self_p,), place=True, imm0=11)
        false = b.operation(0, block, 1, 1)
        b.operation(0, block, 6, None, (retained_p, false))
        self_p = b.operation(0, block, 2, 10, place=True)
        count_p = b.operation(0, block, 3, 4, (self_p,), place=True, imm0=10)
        count = b.operation(0, block, 5, 4, (count_p,))
        capacity = b.operation(0, block, 1, 7, imm0=16_384)
        condition = b.operation(0, block, 9, 1, (count, capacity))
        term_shapes[block] = (2, condition)
    b.block(0, 2, writer_entry)

    def writer_retain(block: int) -> None:
        for field_id, parameter_value in enumerate(range(9)):
            self_p = b.operation(0, block, 2, 10, place=True)
            rows_p = b.operation(0, block, 3, 8, (self_p,), place=True, imm0=9)
            count_p = b.operation(0, block, 3, 4, (self_p,), place=True, imm0=10)
            count = b.operation(0, block, 5, 4, (count_p,))
            row_p = b.operation(0, block, 4, 9, (rows_p, count), place=True)
            field_p = b.operation(0, block, 3, parameter_types[field_id],
                                  (row_p,), place=True, imm0=field_id)
            b.operation(0, block, 6, None, (field_p, parameter_value))
        self_p = b.operation(0, block, 2, 10, place=True)
        count_p = b.operation(0, block, 3, 4, (self_p,), place=True, imm0=10)
        count = b.operation(0, block, 5, 4, (count_p,))
        one = b.operation(0, block, 1, 6, imm0=1)
        incremented = b.operation(0, block, 8, 4, (count, one))
        b.operation(0, block, 6, None, (count_p, incremented))
        retained_p = b.operation(0, block, 3, 1, (self_p,), place=True, imm0=11)
        true = b.operation(0, block, 1, 1, imm0=1)
        b.operation(0, block, 6, None, (retained_p, true))
        term_shapes[block] = (3, NO_ID)
    b.block(0, 2, writer_retain)

    def writer_full(block: int) -> None:
        self_p = b.operation(0, block, 2, 10, place=True)
        retained_p = b.operation(0, block, 3, 1, (self_p,), place=True, imm0=11)
        false = b.operation(0, block, 1, 1)
        b.operation(0, block, 6, None, (retained_p, false))
        term_shapes[block] = (3, NO_ID)
    b.block(0, 2, writer_full)

    def read_entry(block: int) -> None:
        self_p = b.operation(1, block, 2, 10, place=True)
        count_p = b.operation(1, block, 3, 4, (self_p,), place=True, imm0=10)
        count = b.operation(1, block, 5, 4, (count_p,))
        condition = b.operation(1, block, 9, 1, (9, count))
        term_shapes[block] = (2, condition)
    b.block(1, 1, read_entry)

    def read_present(block: int) -> None:
        self_p = b.operation(1, block, 2, 10, place=True)
        rows_p = b.operation(1, block, 3, 8, (self_p,), place=True, imm0=9)
        row_p = b.operation(1, block, 4, 9, (rows_p, 9), place=True)
        tag_p = b.operation(1, block, 3, 0, (row_p,), place=True, imm0=0)
        result = b.operation(1, block, 5, 0, (tag_p,))
        term_shapes[block] = (4, result)
    b.block(1, 1, read_present)

    def read_absent(block: int) -> None:
        result = b.operation(1, block, 1, 0)
        term_shapes[block] = (4, result)
    b.block(1, 1, read_absent)

    def run(block: int) -> None:
        self_p = b.operation(2, block, 2, 11, place=True)
        stream_p = b.operation(2, block, 3, 10, (self_p,), place=True, imm0=12)
        if extra == "full-path":
            count_p = b.operation(2, block, 3, 4, (stream_p,), place=True, imm0=10)
            capacity = b.operation(2, block, 1, 7, imm0=16_384)
            b.operation(2, block, 6, None, (count_p, capacity))
        high_transport = extra == "high-half-transport"
        arguments = [
            b.operation(2, block, 1, 0, imm0=70),
            b.operation(2, block, 1, 0, imm0=71 if high_transport else 1),
            b.operation(2, block, 1, 0, imm0=72 if high_transport else 2),
            b.operation(2, block, 1, 0, imm0=73 if high_transport else 3),
            b.operation(2, block, 1, 2,
                        imm0=0xA5A5_5A5A if high_transport else 4),
            b.operation(2, block, 1, 3,
                        imm0=2 if high_transport else 5,
                        imm1=1 if high_transport else 0),
            b.operation(2, block, 1, 3,
                        imm0=3 if high_transport else 6,
                        imm1=2 if high_transport else 0),
            b.operation(2, block, 1, 3,
                        imm0=4 if high_transport else 7,
                        imm1=3 if high_transport else 0),
            b.operation(2, block, 1, 3,
                        imm0=5 if high_transport else 8,
                        imm1=4 if high_transport else 0),
        ]
        b.operation(2, block, 10, None, (stream_p, *arguments), imm0=0)
        zero = b.operation(2, block, 1, 3)
        result = b.operation(2, block, 10, 0, (stream_p, zero), imm0=1)
        if extra in ("index-oob-high", "index-oob-bound"):
            rows_p = b.operation(2, block, 3, 8, (stream_p,), place=True, imm0=9)
            bad_index = (b.operation(2, block, 1, 3, imm1=1)
                         if extra == "index-oob-high" else
                         b.operation(2, block, 1, 7, imm0=16_384))
            bad_row = b.operation(2, block, 4, 9, (rows_p, bad_index), place=True)
            bad_tag = b.operation(2, block, 3, 0, (bad_row,), place=True, imm0=0)
            b.operation(2, block, 5, 0, (bad_tag,))
        term_shapes[block] = (4, result)
    b.block(2, 2, run)

    t["blocks"] = b.blocks
    t["operations"] = b.operations
    t["operands"] = b.operands
    edge_start = len(b.operands)
    t["terminators"] = []
    for block_id, block in enumerate(b.blocks):
        owner = block[1]
        kind, value = term_shapes[block_id]
        if block_id == 0:
            target0, target1 = 1, 2
        elif block_id == 3:
            target0, target1 = 4, 5
        else:
            target0 = target1 = NO_ID
        t["terminators"].append((
            block_id, owner, block_id, kind, 0, 0, value,
            target0, edge_start, 0, target1, edge_start, 0, 0, 0,
        ))
    t["_counts"] = [(b.values, b.places)]
    return t


def encode(raw: dict[str, list[tuple[int, ...]]], *, major: int = 19) -> bytes:
    values, places = raw["_counts"][0]
    counts = {name: len(raw[name]) for name in ir19.TABLE_ORDER}
    counts.update(values=values, places=places)
    payload = b"".join(
        ir19.ROWS[name].pack(*row)
        for name in ir19.TABLE_ORDER for row in raw[name]
    )
    return ir19.HEADER.pack(
        b"OMGCKIR\0", major, 0, 1, 1, 2,
        ir19.HEADER.size + len(payload),
        *(counts[name] for name in ir19.COUNT_NAMES),
    ) + payload


def mutate_count(contents: bytes, name: str, value: int) -> bytes:
    changed = bytearray(contents)
    struct.pack_into("<I", changed, 24 + 4 * ir19.COUNT_NAMES.index(name), value)
    return bytes(changed)


def emit(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    base = tables()
    canonical = encode(base)
    (directory / "canonical.ckir19").write_bytes(canonical)
    (directory / "full-path.ckir19").write_bytes(encode(tables(extra="full-path")))
    (directory / "high-half-transport.ckir19").write_bytes(
        encode(tables(extra="high-half-transport"))
    )
    (directory / "runtime-index-high-half.ckir19").write_bytes(
        encode(tables(extra="index-oob-high"))
    )
    (directory / "runtime-index-bound.ckir19").write_bytes(
        encode(tables(extra="index-oob-bound"))
    )

    manifest: list[tuple[str, int]] = []

    def mutation(name: str, change, status: int = 251,
                 *, major: int = 19) -> None:
        changed = copy.deepcopy(base)
        change(changed)
        (directory / f"{name}.ckir19").write_bytes(encode(changed, major=major))
        manifest.append((name, status))

    index_ops = [i for i, row in enumerate(base["operations"]) if row[3] == 4]
    store_ops = [i for i, row in enumerate(base["operations"]) if row[3] == 6]
    call_op = next(i for i, row in enumerate(base["operations"]) if row[3] == 10)
    add_op = next(i for i, row in enumerate(base["operations"]) if row[3] == 8)
    less_op = next(i for i, row in enumerate(base["operations"]) if row[3] == 9)
    field_from_index = next(
        i for i, row in enumerate(base["operations"])
        if row[3] == 3 and row[10] == 0 and i > index_ops[0]
    )
    mutation("kind8-policy-flag", lambda x: x["types"].__setitem__(3,
             replace(x["types"][3], 2, 1)))
    mutation("array-65537", lambda x: x["types"].__setitem__(8,
             replace(x["types"][8], 5, 65_537)))
    mutation("array-65536", lambda x: x["types"].__setitem__(8,
             replace(x["types"][8], 5, 65_536)), status=252)
    mutation("owner-over-2m", lambda x: x["types"].__setitem__(8,
             replace(x["types"][8], 5, 52_429)), status=252)
    mutation("index-scalar-element", lambda x: x["types"].__setitem__(8,
             replace(x["types"][8], 4, 0)))
    mutation("index-wrong-result", lambda x: x["operations"].__setitem__(
             index_ops[0], replace(x["operations"][index_ops[0]], 7, 0)))
    mutation("index-immediate", lambda x: x["operations"].__setitem__(
             index_ops[0], replace(x["operations"][index_ops[0]], 10, 1)))
    mutation("wrong-nested-field-owner", lambda x: x["operations"].__setitem__(
             field_from_index, replace(x["operations"][field_from_index], 10, 10)))
    nested_fields = [
        i for i, row in enumerate(base["operations"])
        if row[3] == 3 and row[10] in range(9) and i > index_ops[0]
    ]
    mutation("duplicate-field-store", lambda x: x["operations"].__setitem__(
             nested_fields[1], replace(x["operations"][nested_fields[1]], 10, 0)))
    mutation("missing-field-store", lambda x: x["operations"].__setitem__(
             store_ops[1], replace(x["operations"][store_ops[1]], 3, 5)))
    mutation("writer-arity-eight", lambda x: x["machines"].__setitem__(0,
             replace(x["machines"][0], 7, 8)))
    mutation("writer-arity-seventeen", lambda x: x["machines"].__setitem__(0,
             replace(x["machines"][0], 7, 17)))
    mutation("call-arity-nine", lambda x: x["operations"].__setitem__(call_op,
             replace(x["operations"][call_op], 9, 9)))
    mutation("call-wrong-target", lambda x: x["operations"].__setitem__(call_op,
             replace(x["operations"][call_op], 10, 1)))
    mutation("add-not-u64", lambda x: x["operations"].__setitem__(add_op,
             replace(x["operations"][add_op], 3, 9)))
    mutation("less-not-u64", lambda x: x["operations"].__setitem__(less_op,
             replace(x["operations"][less_op], 3, 3)))
    mutation("constructor-opcode-13", lambda x: x["operations"].__setitem__(
             less_op, replace(x["operations"][less_op], 3, 13)))
    mutation("schema-major-18", lambda _: None, major=18)
    mutation("static-view-type", lambda x: x["types"].append(
        (len(x["types"]), 7, 0, 0, 0, 0, 0, 0)))
    entry_absent = bytearray(canonical)
    struct.pack_into("<H", entry_absent, 14, 0)
    struct.pack_into("<I", entry_absent, 16, NO_ID)
    (directory / "entry-absent.ckir19").write_bytes(entry_absent)
    manifest.append(("entry-absent", 251))
    (directory / "operations-over.ckir19").write_bytes(
        mutate_count(canonical, "operations", 32_769)
    )
    manifest.append(("operations-over", 252))
    (directory / "sums-count-one.ckir19").write_bytes(
        mutate_count(canonical, "sums", 1)
    )
    manifest.append(("sums-count-one", 251))
    (directory / "trailing-byte.ckir19").write_bytes(canonical + b"\0")
    manifest.append(("trailing-byte", 251))
    (directory / "positives.tsv").write_text(
        "canonical\t70\nfull-path\t0\nhigh-half-transport\t70\n",
        encoding="ascii"
    )
    (directory / "runtime.tsv").write_text(
        "runtime-index-high-half\nruntime-index-bound\n", encoding="ascii"
    )
    (directory / "manifest.tsv").write_text(
        "".join(f"{name}\t{status}\n" for name, status in manifest),
        encoding="ascii",
    )
    (directory / "identity.json").write_text(json.dumps({
        "byte_length": len(canonical),
        "sha256": hashlib.sha256(canonical).hexdigest(),
        "counts": {name: len(base[name]) for name in ir19.TABLE_ORDER}
                  | {"values": base["_counts"][0][0],
                     "places": base["_counts"][0][1]},
    }, indent=2, sort_keys=True) + "\n", encoding="ascii")


def check(path: Path, expected: int) -> None:
    module = ir19.decode(path.read_bytes())
    ir19.v5.require(ir19.interpret(module) == expected, "CKIR19 result")
    ir19.profile(module)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("emit", "check"))
    parser.add_argument("path", type=Path)
    parser.add_argument("expected", nargs="?", type=int)
    args = parser.parse_args()
    emit(args.path) if args.command == "emit" else check(args.path, int(args.expected))


if __name__ == "__main__":
    main()
