#!/usr/bin/env python3
"""Handcrafted CKIR8 primitive ScalarEqual carriers and mutations."""

from __future__ import annotations

import argparse
import copy
import importlib.util
import struct
from pathlib import Path

import checked_ir_v8_reference as ir8


HERE = Path(__file__).resolve().parent
NO_ID = ir8.NO_ID


def load(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, HERE / filename)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


v7_fixture = load("delta_checked_ir_v7_fixture", "delta-checked-ir-v7-fixture.py")


def replace(row: tuple[int, ...], index: int, value: int) -> tuple[int, ...]:
    return row[:index] + (value,) + row[index + 1:]


def tables(left: int, right: int, left_type: int, right_type: int) -> dict[str, list[tuple[int, ...]]]:
    expected = left == right
    result: dict[str, list[tuple[int, ...]]] = {name: [] for name in ir8.TABLE_ORDER}
    result["types"] = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 3, 0, 0, 0, 0, 0, 1),
        (2, 2, 0, 0, 0, 0, 0, 2_147_483_647),
        (3, 1, 0, 0, 0, 0, 0, 255),
        (4, 2, 0, 0, 0, 0, 0, 100),
        (5, 2, 0, 0, 0, 0, 50, 100),
    ]
    result["records"] = [(0, 0, 0, 0, 0, 0, 0, 0)]
    result["machines"] = [(0, 0, 2, 0, 0, 2, 0, 0, 0, 3, 0)]
    result["blocks"] = [
        (0, 0, 2, 0, 0, 0, 0, 0, 3, 0),
        (1, 0, 2, 0, 0, 0, 0, 3, 1, 1),
        (2, 0, 2, 0, 0, 0, 0, 4, 1, 2),
    ]
    result["operations"] = [
        (0, 0, 0, 1, 1, 0, 0, left_type, 0, 0, left, 0),
        (1, 0, 0, 1, 1, 0, 1, right_type, 0, 0, right, 0),
        (2, 0, 0, 18, 1, 0, 2, 1, 0, 2, 0, 0),
        (3, 0, 1, 1, 1, 0, 3, 2, 2, 0, 70 if expected else 0, 0),
        (4, 0, 2, 1, 1, 0, 4, 2, 2, 0, 0 if expected else 70, 0),
    ]
    result["operands"] = [(0,), (1,)]
    result["terminators"] = [
        (0, 0, 0, 2, 0, 0, 2, 1, 2, 0, 2, 2, 0, 0, 0),
        (1, 0, 1, 4, 0, 0, 3, NO_ID, 2, 0, NO_ID, 2, 0, 0, 0),
        (2, 0, 2, 4, 0, 0, 4, NO_ID, 2, 0, NO_ID, 2, 0, 0, 0),
    ]
    return result


def encode(raw_tables: dict[str, list[tuple[int, ...]]], *, major: int = 8,
           values: int = 5, places: int = 0) -> bytes:
    counts = {name: len(raw_tables[name]) for name in ir8.TABLE_ORDER}
    counts.update(values=values, places=places)
    payload = b"".join(
        ir8.ROWS[name].pack(*row)
        for name in ir8.TABLE_ORDER
        for row in raw_tables[name]
    )
    return ir8.HEADER.pack(
        b"OMGCKIR\0", major, 0, 1, 1, 0, ir8.HEADER.size + len(payload),
        *(counts[name] for name in ir8.COUNT_NAMES),
    ) + payload


def mutate_count(contents: bytes, name: str, value: int) -> bytes:
    changed = bytearray(contents)
    struct.pack_into("<I", changed, 24 + 4 * ir8.COUNT_NAMES.index(name), value)
    return bytes(changed)


def emit(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    positives = {
        "bool-00": tables(0, 0, 1, 1),
        "bool-01": tables(0, 1, 1, 1),
        "bool-10": tables(1, 0, 1, 1),
        "bool-11": tables(1, 1, 1, 1),
        "u8-equal": tables(70, 70, 3, 3),
        "u8-unequal": tables(69, 70, 3, 3),
        "u32-equal": tables(70, 70, 2, 2),
        "u32-unequal": tables(70, 71, 2, 2),
        "u32-constrained-compatible": tables(70, 70, 4, 5),
    }
    for name, value in positives.items():
        (directory / f"{name}.ckir8").write_bytes(encode(value))

    base = tables(70, 70, 2, 2)
    canonical = encode(base)
    (directory / "canonical.ckir8").write_bytes(canonical)
    manifest: list[tuple[str, int]] = []

    def mutation(name: str, change, expected: int = 251, major: int = 8) -> None:
        changed = copy.deepcopy(base)
        change(changed)
        (directory / f"{name}.ckir8").write_bytes(encode(changed, major=major))
        manifest.append((name, expected))

    for major in (4, 5, 6, 7):
        mutation(f"schema-major-{major}", lambda _: None, major=major)
    inherited = v7_fixture.encode(v7_fixture.tables(16, 1, 1), major=8)
    (directory / "missing-scalar-equal.ckir8").write_bytes(inherited)
    manifest.append(("missing-scalar-equal", 251))
    mutation("arity-one", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 9, 1)))
    mutation("arity-three", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 9, 3)))
    mutation("immediate-zero", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 10, 1)))
    mutation("immediate-one", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 11, 1)))
    mutation("reserved", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 5, 1)))
    mutation("invisible-left", lambda t: t["operands"].__setitem__(0, (2,)))
    mutation("invisible-right", lambda t: t["operands"].__setitem__(1, (2,)))
    mutation("mixed-u8-u32", lambda t: t["operations"].__setitem__(1, replace(t["operations"][1], 7, 3)))
    mutation("mixed-bool-u32", lambda t: t["operations"].__setitem__(1, replace(t["operations"][1], 7, 1)))
    mutation("non-bool-result", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 7, 2)))
    mutation("wrong-result-kind", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 4, 2)))
    mutation("wrong-result-id", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 6, 3)))
    mutation("opcode-less", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 3, 9)))
    mutation("opcode-logical-and", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 3, 16)))
    for name, count, value in (
        ("operations-over", "operations", 32_769),
        ("operands-over", "operands", 94_209),
    ):
        (directory / f"{name}.ckir8").write_bytes(mutate_count(canonical, count, value))
        manifest.append((name, 252))

    (directory / "positives.tsv").write_text(
        "".join(f"{name}\t70\n" for name in positives), encoding="ascii"
    )
    (directory / "manifest.tsv").write_text(
        "".join(f"{name}\t{status}\n" for name, status in manifest), encoding="ascii"
    )


def check(path: Path) -> None:
    module = ir8.decode(path.read_bytes())
    ir8.v5.require(ir8.interpret(module) == 70, "ScalarEqual carrier result")
    ir8.v5.require(sum(op[3] == 18 for op in module.tables["operations"]) == 1,
                   "ScalarEqual operation count")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("emit", "check"))
    parser.add_argument("path", type=Path)
    args = parser.parse_args()
    emit(args.path) if args.command == "emit" else check(args.path)


if __name__ == "__main__":
    main()
