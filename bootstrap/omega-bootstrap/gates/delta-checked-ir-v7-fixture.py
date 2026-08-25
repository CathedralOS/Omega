#!/usr/bin/env python3
"""Handcrafted CKIR7 LogicalAnd/LogicalOr truth carriers and mutations."""

from __future__ import annotations

import argparse
import copy
import importlib.util
import struct
from pathlib import Path

import checked_ir_v7_reference as ir7


HERE = Path(__file__).resolve().parent
NO_ID = ir7.NO_ID


def load(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, HERE / filename)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


v6_fixture = load("delta_checked_ir_v6_fixture", "delta-checked-ir-v6-fixture.py")


def replace(row: tuple[int, ...], index: int, value: int) -> tuple[int, ...]:
    return row[:index] + (value,) + row[index + 1:]


def tables(opcode: int, left: int, right: int) -> dict[str, list[tuple[int, ...]]]:
    expected = left & right if opcode == 16 else left | right
    result: dict[str, list[tuple[int, ...]]] = {name: [] for name in ir7.TABLE_ORDER}
    result["types"] = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 3, 0, 0, 0, 0, 0, 1),
        (2, 2, 0, 0, 0, 0, 0, 2_147_483_647),
    ]
    result["records"] = [(0, 0, 0, 0, 0, 0, 0, 0)]
    result["machines"] = [(0, 0, 2, 0, 0, 2, 0, 0, 0, 3, 0)]
    result["blocks"] = [
        (0, 0, 2, 0, 0, 0, 0, 0, 3, 0),
        (1, 0, 2, 0, 0, 0, 0, 3, 1, 1),
        (2, 0, 2, 0, 0, 0, 0, 4, 1, 2),
    ]
    result["operations"] = [
        (0, 0, 0, 1, 1, 0, 0, 1, 0, 0, left, 0),
        (1, 0, 0, 1, 1, 0, 1, 1, 0, 0, right, 0),
        (2, 0, 0, opcode, 1, 0, 2, 1, 0, 2, 0, 0),
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


def encode(raw_tables: dict[str, list[tuple[int, ...]]], *, major: int = 7,
           values: int = 5, places: int = 0) -> bytes:
    counts = {name: len(raw_tables[name]) for name in ir7.TABLE_ORDER}
    counts.update(values=values, places=places)
    payload = b"".join(
        ir7.ROWS[name].pack(*row)
        for name in ir7.TABLE_ORDER
        for row in raw_tables[name]
    )
    return ir7.HEADER.pack(
        b"OMGCKIR\0", major, 0, 1, 1, 0, ir7.HEADER.size + len(payload),
        *(counts[name] for name in ir7.COUNT_NAMES),
    ) + payload


def mutate_count(contents: bytes, name: str, value: int) -> bytes:
    changed = bytearray(contents)
    struct.pack_into("<I", changed, 24 + 4 * ir7.COUNT_NAMES.index(name), value)
    return bytes(changed)


def emit(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    positives: list[tuple[str, int]] = []
    for opcode, label in ((16, "and"), (17, "or")):
        for left in (0, 1):
            for right in (0, 1):
                name = f"{label}-{left}{right}"
                (directory / f"{name}.ckir7").write_bytes(
                    encode(tables(opcode, left, right))
                )
                positives.append((name, 70))

    base = tables(16, 1, 1)
    canonical = encode(base)
    (directory / "canonical.ckir7").write_bytes(canonical)
    manifest: list[tuple[str, int]] = []

    def mutation(name: str, change, expected: int = 251, major: int = 7) -> None:
        changed = copy.deepcopy(base)
        change(changed)
        (directory / f"{name}.ckir7").write_bytes(encode(changed, major=major))
        manifest.append((name, expected))

    mutation("schema-major-4", lambda _: None, major=4)
    mutation("schema-major-5", lambda _: None, major=5)
    mutation("schema-major-6", lambda _: None, major=6)
    (directory / "missing-logical-binary.ckir7").write_bytes(
        v6_fixture.encode(v6_fixture.canonical_tables(), major=7)
    )
    manifest.append(("missing-logical-binary", 251))
    mutation("arity-one", lambda t: t["operations"].__setitem__(
        2, replace(t["operations"][2], 9, 1)
    ))
    mutation("arity-three", lambda t: t["operations"].__setitem__(
        2, replace(t["operations"][2], 9, 3)
    ))
    mutation("immediate-zero", lambda t: t["operations"].__setitem__(
        2, replace(t["operations"][2], 10, 1)
    ))
    mutation("immediate-one", lambda t: t["operations"].__setitem__(
        2, replace(t["operations"][2], 11, 1)
    ))
    mutation("reserved", lambda t: t["operations"].__setitem__(
        2, replace(t["operations"][2], 5, 1)
    ))
    mutation("invisible-left", lambda t: t["operands"].__setitem__(0, (2,)))
    mutation("invisible-right", lambda t: t["operands"].__setitem__(1, (2,)))
    mutation("non-bool-left", lambda t: t["operations"].__setitem__(
        0, replace(t["operations"][0], 7, 2)
    ))
    mutation("non-bool-right", lambda t: t["operations"].__setitem__(
        1, replace(t["operations"][1], 7, 2)
    ))
    mutation("non-bool-result", lambda t: t["operations"].__setitem__(
        2, replace(t["operations"][2], 7, 2)
    ))
    mutation("wrong-result-kind", lambda t: t["operations"].__setitem__(
        2, replace(t["operations"][2], 4, 2)
    ))
    mutation("wrong-result-id", lambda t: t["operations"].__setitem__(
        2, replace(t["operations"][2], 6, 3)
    ))
    for name, count, value in (
        ("operations-over", "operations", 32_769),
        ("operands-over", "operands", 94_209),
    ):
        (directory / f"{name}.ckir7").write_bytes(mutate_count(canonical, count, value))
        manifest.append((name, 252))

    (directory / "positives.tsv").write_text(
        "".join(f"{name}\t{result}\n" for name, result in positives), encoding="ascii"
    )
    (directory / "manifest.tsv").write_text(
        "".join(f"{name}\t{status}\n" for name, status in manifest), encoding="ascii"
    )


def check(path: Path) -> None:
    module = ir7.decode(path.read_bytes())
    ir7.v5.require(ir7.interpret(module) == 70, "logical binary carrier result")
    binary = [op for op in module.tables["operations"] if op[3] in (16, 17)]
    ir7.v5.require(len(binary) == 1, "logical binary operation count")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("emit", "check"))
    parser.add_argument("path", type=Path)
    args = parser.parse_args()
    emit(args.path) if args.command == "emit" else check(args.path)


if __name__ == "__main__":
    main()
