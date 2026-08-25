#!/usr/bin/env python3
"""Handcrafted CKIR5 carrier, resource teeth, and isolated mutations."""

from __future__ import annotations

import argparse
import copy
import struct
from pathlib import Path

import checked_ir_v5_reference as ir5


NO_ID = ir5.NO_ID


def encode(tables: dict[str, list[tuple[int, ...]]], values: int,
           places: int, entry: int = 0) -> bytes:
    counts = {name: len(tables[name]) for name in ir5.TABLE_ORDER}
    counts.update(values=values, places=places)
    payload = b"".join(
        ir5.ROWS[name].pack(*row)
        for name in ir5.TABLE_ORDER
        for row in tables[name]
    )
    flags = int(entry != NO_ID)
    return ir5.HEADER.pack(
        b"OMGCKIR\0", 5, 0, 1, flags, entry,
        ir5.HEADER.size + len(payload),
        *(counts[name] for name in ir5.COUNT_NAMES),
    ) + payload


def canonical_tables() -> dict[str, list[tuple[int, ...]]]:
    # Owner layout is u8 padding, then an 8-byte sum at nonzero offset 4,
    # then the callee result.  Sum cases model payload-free, three-Boolean
    # TokenKind, and one-scalar ByteRead shapes without depending on names.
    types = [
        (0, 1, 0, 0, 0, 0, 0, 255),       # u8
        (1, 2, 0, 0, 0, 0, 0, 100),       # u32
        (2, 3, 0, 0, 0, 0, 0, 1),         # bool
        (3, 4, 0, 0, 0, 0, 0, 0),         # Owner
        (4, 6, 0, 0, 0, 0, 0, 0),         # Event sum
        (5, 2, 0, 0, 0, 0, 70, 70),       # exact result carrier
    ]
    records = [(0, 3, 0, 3, 0, 0, 0, 0)]
    fields = [(0, 0, 0, 0), (1, 0, 1, 4), (2, 0, 2, 1)]
    sums = [(0, 4, 0, 3, 1, 0, 0, 0)]
    cases = [
        (0, 0, 0, 0, 0),
        (1, 0, 1, 0, 3),
        (2, 0, 2, 3, 1),
    ]
    case_payloads = [
        (0, 1, 0, 2), (1, 1, 1, 2), (2, 1, 2, 2),
        (3, 2, 0, 1),
    ]
    machines = [
        (0, 0, 2, 0, 0, 1, 0, 0, 0, 6, 0),
        (1, 0, 2, 0, 0, 1, 0, 1, 6, 6, 6),
    ]
    machine_params = [(0, 1, 0, 4, 0)]
    blocks = [
        (0, 0, 2, 0, 0, 0, 0, 0, 9, 0),
        (1, 0, 2, 0, 0, 0, 3, 9, 0, 1),
        (2, 0, 2, 0, 0, 3, 1, 9, 0, 2),
        (3, 0, 2, 0, 0, 4, 0, 9, 1, 3),
        (4, 0, 2, 0, 0, 4, 0, 10, 3, 4),
        (5, 0, 2, 0, 0, 4, 0, 13, 1, 5),
        (6, 1, 2, 0, 0, 4, 0, 14, 0, 6),
        (7, 1, 2, 0, 0, 4, 3, 14, 0, 7),
        (8, 1, 2, 0, 0, 7, 1, 14, 0, 8),
        (9, 1, 2, 0, 0, 8, 0, 14, 1, 9),
        (10, 1, 2, 0, 0, 8, 0, 15, 1, 10),
        (11, 1, 2, 0, 0, 8, 0, 16, 1, 11),
    ]
    block_params = [
        (0, 1, 0, 2, 1), (1, 1, 1, 2, 2), (2, 1, 2, 2, 3),
        (3, 2, 0, 1, 4),
        (4, 7, 0, 2, 5), (5, 7, 1, 2, 6), (6, 7, 2, 2, 7),
        (7, 8, 0, 1, 8),
    ]
    operations = [
        (0, 0, 0, 2, 2, 0, 0, 3, 0, 0, 0, 0),
        (1, 0, 0, 3, 2, 0, 1, 4, 0, 1, 1, 0),
        (2, 0, 0, 3, 2, 0, 2, 1, 1, 1, 2, 0),
        (3, 0, 0, 1, 1, 0, 9, 2, 2, 0, 1, 0),
        (4, 0, 0, 1, 1, 0, 10, 2, 2, 0, 0, 0),
        (5, 0, 0, 14, 1, 0, 11, 4, 2, 3, 1, 0),
        (6, 0, 0, 10, 1, 0, 12, 1, 5, 2, 1, 0),
        (7, 0, 0, 6, 0, 0, NO_ID, NO_ID, 7, 2, 0, 0),
        (8, 0, 0, 7, 0, 0, NO_ID, NO_ID, 9, 2, 1, 0),
        (9, 0, 3, 1, 1, 0, 13, 1, 11, 0, 71, 0),
        (10, 0, 4, 2, 2, 0, 3, 3, 11, 0, 0, 0),
        (11, 0, 4, 3, 2, 0, 4, 1, 11, 1, 2, 0),
        (12, 0, 4, 5, 1, 0, 14, 1, 12, 1, 0, 0),
        (13, 0, 5, 1, 1, 0, 15, 1, 13, 0, 72, 0),
        (14, 1, 9, 1, 1, 0, 16, 1, 13, 0, 73, 0),
        (15, 1, 10, 1, 1, 0, 17, 5, 13, 0, 70, 0),
        (16, 1, 11, 1, 1, 0, 18, 1, 13, 0, 74, 0),
    ]
    operands = [(0,), (0,), (9,), (10,), (10,), (0,), (11,), (2,), (12,), (1,), (11,), (3,), (4,)]
    # Every inherited edge starts at the fully consumed ordinary operand cursor.
    def term(block: int, owner: int, kind: int, value: int = NO_ID,
             target0: int = NO_ID, target1: int = NO_ID,
             arm_start: int = 0, arm_count: int = 0, flags: int = 0):
        return (block, owner, block, kind, flags, 0, value,
                target0, 13, 0, target1, 13, 0, arm_start, arm_count)
    terminators = [
        term(0, 0, 5, 1, arm_start=0, arm_count=3, flags=2),
        term(1, 0, 2, 1, 4, 5, arm_start=3),
        term(2, 0, 4, 4, arm_start=3),
        term(3, 0, 4, 13, arm_start=3),
        term(4, 0, 4, 14, arm_start=3),
        term(5, 0, 4, 15, arm_start=3),
        term(6, 1, 5, 0, arm_start=3, arm_count=3, flags=1),
        term(7, 1, 2, 5, 10, 11, arm_start=6),
        term(8, 1, 4, 8, arm_start=6),
        term(9, 1, 4, 16, arm_start=6),
        term(10, 1, 4, 17, arm_start=6),
        term(11, 1, 4, 18, arm_start=6),
    ]
    case_arms = [
        (0, 0, 0, 3, 0, 0),
        (1, 0, 1, 1, 0, 3),
        (2, 0, 2, 2, 3, 1),
        (3, 6, 0, 9, 4, 0),
        (4, 6, 1, 7, 4, 3),
        (5, 6, 2, 8, 7, 1),
    ]
    case_arm_args = [
        (0, 2, 0), (1, 2, 1), (2, 2, 2), (3, 2, 3),
        (4, 2, 0), (5, 2, 1), (6, 2, 2), (7, 2, 3),
    ]
    return {
        "types": types, "records": records, "fields": fields, "sums": sums,
        "cases": cases, "case_payloads": case_payloads, "machines": machines,
        "machine_params": machine_params, "blocks": blocks,
        "block_params": block_params, "constants": [], "constant_children": [],
        "operations": operations, "operands": operands, "terminators": terminators,
        "case_arms": case_arms, "case_arm_args": case_arm_args,
    }


def declaration_fixture(case_count: int, payload_count: int) -> bytes:
    tables = {name: [] for name in ir5.TABLE_ORDER}
    tables["types"] = [
        (0, 1, 0, 0, 0, 0, 0, 255),
        (1, 6, 0, 0, 0, 0, 0, 0),
    ]
    tables["sums"] = [(0, 1, 0, case_count, 1, 0, 0, 0)]
    tables["cases"] = [(index, 0, index, 0 if index == 0 else payload_count,
                         payload_count if index == 0 else 0)
                        for index in range(case_count)]
    tables["case_payloads"] = [(index, 0, index, 0) for index in range(payload_count)]
    return encode(tables, 0, 0, NO_ID)


def replace(row: tuple[int, ...], index: int, value: int) -> tuple[int, ...]:
    result = list(row)
    result[index] = value
    return tuple(result)


def malformed_cases() -> dict[str, bytes]:
    base = canonical_tables()
    result: dict[str, bytes] = {}

    def one(name: str, change) -> None:
        tables = copy.deepcopy(base)
        change(tables)
        result[name] = encode(tables, 19, 5)

    one("sum-nominal", lambda t: t["sums"].__setitem__(0, replace(t["sums"][0], 1, 3)))
    one("sum-case-start", lambda t: t["sums"].__setitem__(0, replace(t["sums"][0], 2, 1)))
    one("case-owner", lambda t: t["cases"].__setitem__(1, replace(t["cases"][1], 1, 1)))
    one("case-ordinal", lambda t: t["cases"].__setitem__(1, replace(t["cases"][1], 2, 2)))
    one("payload-owner", lambda t: t["case_payloads"].__setitem__(0, replace(t["case_payloads"][0], 1, 2)))
    one("payload-ordinal", lambda t: t["case_payloads"].__setitem__(1, replace(t["case_payloads"][1], 2, 2)))
    one("payload-type", lambda t: t["case_payloads"].__setitem__(0, replace(t["case_payloads"][0], 3, 1)))
    one("construct-case-owner", lambda t: t["operations"].__setitem__(5, replace(t["operations"][5], 10, 2)))
    one("construct-case-reserved", lambda t: t["operations"].__setitem__(5, replace(t["operations"][5], 11, 1)))
    one("construct-case-arity", lambda t: t["operations"].__setitem__(5, replace(t["operations"][5], 9, 2)))
    one("dispatch-flags", lambda t: t["terminators"].__setitem__(0, replace(t["terminators"][0], 4, 0)))
    one("dispatch-arm-count", lambda t: t["terminators"].__setitem__(0, replace(t["terminators"][0], 14, 2)))
    one("arm-owner", lambda t: t["case_arms"].__setitem__(1, replace(t["case_arms"][1], 1, 6)))
    one("arm-case-order", lambda t: t["case_arms"].__setitem__(1, replace(t["case_arms"][1], 2, 2)))
    one("arm-target-owner", lambda t: t["case_arms"].__setitem__(1, replace(t["case_arms"][1], 3, 7)))
    one("arm-argument-start", lambda t: t["case_arms"].__setitem__(1, replace(t["case_arms"][1], 4, 1)))
    one("argument-kind", lambda t: t["case_arm_args"].__setitem__(0, replace(t["case_arm_args"][0], 1, 3)))
    one("argument-wrong-case", lambda t: t["case_arm_args"].__setitem__(0, replace(t["case_arm_args"][0], 2, 3)))
    one("argument-duplicate", lambda t: t["case_arm_args"].__setitem__(1, replace(t["case_arm_args"][1], 2, 0)))

    schema = bytearray(encode(base, 19, 5))
    struct.pack_into("<H", schema, 8, 4)
    result["schema-major-4"] = bytes(schema)
    reserved = bytearray(encode(base, 19, 5))
    # Sum row begins after header/types/records/fields; mutate its first reserved byte.
    sum_at = (ir5.HEADER.size + len(base["types"]) * ir5.ROWS["types"].size
              + len(base["records"]) * ir5.ROWS["records"].size
              + len(base["fields"]) * ir5.ROWS["fields"].size)
    reserved[sum_at + 17] = 1
    result["sum-reserved"] = bytes(reserved)
    return result


def emit(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    base = canonical_tables()
    (directory / "canonical.ckir5").write_bytes(encode(base, 19, 5))
    manifest: list[tuple[str, int]] = []
    for name, contents in malformed_cases().items():
        (directory / f"{name}.ckir5").write_bytes(contents)
        manifest.append((name, 251))
    for name, case_count, payload_count, status in (
        ("payload-four", 1, 4, 0), ("payload-five", 1, 5, 252),
        ("cases-64", 64, 0, 0), ("cases-65", 65, 0, 252),
    ):
        (directory / f"{name}.ckir5").write_bytes(
            declaration_fixture(case_count, payload_count)
        )
        manifest.append((name, status))
    for name, count_name, value in (
        ("combined-raw-types", "fields", 4_097),
        ("case-arms-total", "case_arms", 4_097),
    ):
        contents = bytearray(encode(base, 19, 5))
        count_offset = 24 + 4 * ir5.COUNT_NAMES.index(count_name)
        struct.pack_into("<I", contents, count_offset, value)
        if name == "combined-raw-types":
            payload_offset = 24 + 4 * ir5.COUNT_NAMES.index("case_payloads")
            struct.pack_into("<I", contents, payload_offset, 4_096)
        (directory / f"{name}.ckir5").write_bytes(contents)
        manifest.append((name, 252))
    (directory / "manifest.tsv").write_text(
        "".join(f"{name}\t{status}\n" for name, status in manifest), encoding="ascii"
    )


def check(path: Path) -> None:
    module = ir5.decode(path.read_bytes())
    ir5.require(module.layouts[4] == (8, 4), f"sum layout {module.layouts[4]}")
    ir5.require(module.layouts[3] == (16, 4), f"owner layout {module.layouts[3]}")
    ir5.require(module.field_offsets == (0, 4, 12),
                f"owner field offsets {module.field_offsets}")
    ir5.require(module.sum_payload_offsets == (4,),
                f"sum payload base {module.sum_payload_offsets}")
    ir5.require(module.payload_offsets == (0, 1, 2, 0),
                f"case payload offsets {module.payload_offsets}")
    ir5.require(ir5.interpret(module) == 70, "canonical meaning")

    # A validated module cannot produce this tag, but the interpreter must
    # still fail closed if its in-memory declaration is corrupted after decode.
    corrupted = copy.deepcopy(module)
    corrupted.tables["cases"][1] = replace(corrupted.tables["cases"][1], 2, 99)
    try:
        ir5.interpret(corrupted)
    except ir5.Ckir5Error as error:
        ir5.require("invalid sum tag" in str(error), f"wrong runtime guard: {error}")
    else:
        raise ir5.Ckir5Error("runtime invalid sum tag accepted")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("emit", "check"))
    parser.add_argument("path", type=Path)
    args = parser.parse_args()
    if args.command == "emit":
        emit(args.path)
    else:
        check(args.path)


if __name__ == "__main__":
    main()
