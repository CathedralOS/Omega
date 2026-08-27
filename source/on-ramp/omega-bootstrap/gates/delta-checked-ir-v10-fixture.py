#!/usr/bin/env python3
"""Handcrafted CKIR10 IntegerWiden carriers and isolated mutations."""

from __future__ import annotations

import argparse
import copy
import importlib.util
import struct
from pathlib import Path

import checked_ir_v10_reference as ir10


HERE = Path(__file__).resolve().parent
NO_ID = ir10.NO_ID


def load(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, HERE / filename)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


v9_fixture = load("delta_checked_ir_v9_fixture", "delta-checked-ir-v9-fixture.py")


def replace(row: tuple[int, ...], index: int, value: int) -> tuple[int, ...]:
    return row[:index] + (value,) + row[index + 1:]


def tables(value: int) -> dict[str, list[tuple[int, ...]]]:
    result = {name: [] for name in ir10.TABLE_ORDER}
    result["types"] = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 3, 0, 0, 0, 0, 0, 1),
        (2, 2, 0, 0, 0, 0, 0, 2_147_483_647),
        (3, 1, 0, 0, 0, 0, 0, 255),
        (4, 2, 1, 0, 0, 0, 0, 2_147_483_647),
        (5, 1, 1, 0, 0, 0, 0, 255),
        (6, 1, 0, 0, 0, 0, 0, 254),
        (7, 2, 1, 0, 0, 0, 0, 255),
    ]
    result["records"] = [(0, 0, 0, 0, 0, 0, 0, 0)]
    result["machines"] = [(0, 0, 2, 0, 0, 2, 0, 0, 0, 3, 0)]
    result["blocks"] = [
        (0, 0, 2, 0, 0, 0, 0, 0, 4, 0),
        (1, 0, 2, 0, 0, 0, 0, 4, 1, 1),
        (2, 0, 2, 0, 0, 0, 0, 5, 1, 2),
    ]
    result["operations"] = [
        (0, 0, 0, 1, 1, 0, 0, 3, 0, 0, value, 0),
        (1, 0, 0, 21, 1, 0, 1, 4, 0, 1, 0, 0),
        (2, 0, 0, 1, 1, 0, 2, 4, 1, 0, value, 0),
        (3, 0, 0, 18, 1, 0, 3, 1, 1, 2, 0, 0),
        (4, 0, 1, 1, 1, 0, 4, 2, 3, 0, 70, 0),
        (5, 0, 2, 1, 1, 0, 5, 2, 3, 0, 0, 0),
    ]
    result["operands"] = [(0,), (1,), (2,)]
    result["terminators"] = [
        (0, 0, 0, 2, 0, 0, 3, 1, 3, 0, 2, 3, 0, 0, 0),
        (1, 0, 1, 4, 0, 0, 4, NO_ID, 3, 0, NO_ID, 3, 0, 0, 0),
        (2, 0, 2, 4, 0, 0, 5, NO_ID, 3, 0, NO_ID, 3, 0, 0, 0),
    ]
    return result


def encode(raw_tables: dict[str, list[tuple[int, ...]]], *, major: int = 10,
           values: int = 6, places: int = 0) -> bytes:
    counts = {name: len(raw_tables[name]) for name in ir10.TABLE_ORDER}
    counts.update(values=values, places=places)
    payload = b"".join(
        ir10.ROWS[name].pack(*row)
        for name in ir10.TABLE_ORDER
        for row in raw_tables[name]
    )
    return ir10.HEADER.pack(
        b"OMGCKIR\0", major, 0, 1, 1, 0, ir10.HEADER.size + len(payload),
        *(counts[name] for name in ir10.COUNT_NAMES),
    ) + payload


def mutate_count(contents: bytes, name: str, value: int) -> bytes:
    changed = bytearray(contents)
    struct.pack_into("<I", changed, 24 + 4 * ir10.COUNT_NAMES.index(name), value)
    return bytes(changed)


def emit(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    positives = {"widen-0": tables(0), "widen-70": tables(70), "widen-255": tables(255)}
    for name, value in positives.items():
        (directory / f"{name}.ckir10").write_bytes(encode(value))

    base = tables(70)
    canonical = encode(base)
    (directory / "canonical.ckir10").write_bytes(canonical)
    manifest: list[tuple[str, int]] = []

    def mutation(name: str, change, expected: int = 251, major: int = 10) -> None:
        changed = copy.deepcopy(base)
        change(changed)
        (directory / f"{name}.ckir10").write_bytes(encode(changed, major=major))
        manifest.append((name, expected))

    for major in range(4, 10):
        mutation(f"new-op-schema-major-{major}", lambda _: None, major=major)

    inherited = v9_fixture.encode(v9_fixture.tables(71, 70, 3, 3, 19), major=10)
    (directory / "missing-integer-widen.ckir10").write_bytes(inherited)
    manifest.append(("missing-integer-widen", 251))
    mutation("arity-zero", lambda t: t["operations"].__setitem__(1, replace(t["operations"][1], 9, 0)))
    mutation("arity-two", lambda t: t["operations"].__setitem__(1, replace(t["operations"][1], 9, 2)))
    mutation("immediate-zero", lambda t: t["operations"].__setitem__(1, replace(t["operations"][1], 10, 1)))
    mutation("immediate-one", lambda t: t["operations"].__setitem__(1, replace(t["operations"][1], 11, 1)))
    mutation("reserved", lambda t: t["operations"].__setitem__(1, replace(t["operations"][1], 5, 1)))
    mutation("invisible-source", lambda t: t["operands"].__setitem__(0, (1,)))
    mutation("source-bool", lambda t: t["operations"].__setitem__(0, replace(replace(t["operations"][0], 7, 1), 10, 1)))
    mutation("source-u32", lambda t: t["operations"].__setitem__(0, replace(t["operations"][0], 7, 2)))
    mutation("source-u8-trapping", lambda t: t["operations"].__setitem__(0, replace(t["operations"][0], 7, 5)))
    mutation("source-u8-constrained", lambda t: t["operations"].__setitem__(0, replace(t["operations"][0], 7, 6)))
    mutation("result-u32-exact", lambda t: t["operations"].__setitem__(1, replace(t["operations"][1], 7, 2)))
    mutation("result-bool", lambda t: t["operations"].__setitem__(1, replace(t["operations"][1], 7, 1)))
    mutation("result-u8", lambda t: t["operations"].__setitem__(1, replace(t["operations"][1], 7, 3)))
    mutation("result-u32-trapping-constrained", lambda t: t["operations"].__setitem__(1, replace(t["operations"][1], 7, 7)))
    mutation("wrong-result-kind", lambda t: t["operations"].__setitem__(1, replace(t["operations"][1], 4, 2)))
    mutation("wrong-result-id", lambda t: t["operations"].__setitem__(1, replace(t["operations"][1], 6, 2)))
    mutation("opcode-load", lambda t: t["operations"].__setitem__(1, replace(t["operations"][1], 3, 5)))
    mutation("opcode-scalar-equal", lambda t: t["operations"].__setitem__(1, replace(t["operations"][1], 3, 18)))

    for name, count, value in (
        ("operations-over", "operations", 32_769),
        ("operands-over", "operands", 94_209),
    ):
        (directory / f"{name}.ckir10").write_bytes(mutate_count(canonical, count, value))
        manifest.append((name, 252))

    (directory / "positives.tsv").write_text(
        "".join(f"{name}\t70\n" for name in positives), encoding="ascii"
    )
    (directory / "manifest.tsv").write_text(
        "".join(f"{name}\t{status}\n" for name, status in manifest), encoding="ascii"
    )


def check(path: Path) -> None:
    module = ir10.decode(path.read_bytes())
    ir10.v5.require(ir10.interpret(module) == 70, "IntegerWiden carrier result")
    ir10.v5.require(sum(op[3] == 21 for op in module.tables["operations"]) == 1,
                     "IntegerWiden operation count")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("emit", "check"))
    parser.add_argument("path", type=Path)
    args = parser.parse_args()
    emit(args.path) if args.command == "emit" else check(args.path)


if __name__ == "__main__":
    main()
