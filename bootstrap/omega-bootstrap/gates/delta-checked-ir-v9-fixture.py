#!/usr/bin/env python3
"""Handcrafted CKIR9 Greater/GreaterEqual carriers and isolated mutations."""

from __future__ import annotations

import argparse
import copy
import importlib.util
from pathlib import Path

import checked_ir_v9_reference as ir9


HERE = Path(__file__).resolve().parent
NO_ID = ir9.NO_ID


def load(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, HERE / filename)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


v8_fixture = load("delta_checked_ir_v8_fixture", "delta-checked-ir-v8-fixture.py")


def replace(row: tuple[int, ...], index: int, value: int) -> tuple[int, ...]:
    return row[:index] + (value,) + row[index + 1:]


def tables(left: int, right: int, left_type: int, right_type: int,
           opcode: int) -> dict[str, list[tuple[int, ...]]]:
    result = v8_fixture.tables(left, right, left_type, right_type)
    expected = left > right if opcode == 19 else left >= right
    result["operations"][2] = replace(result["operations"][2], 3, opcode)
    result["operations"][3] = replace(
        result["operations"][3], 10, 70 if expected else 0
    )
    result["operations"][4] = replace(
        result["operations"][4], 10, 0 if expected else 70
    )
    return result


def mixed_tables() -> dict[str, list[tuple[int, ...]]]:
    result = {name: [] for name in ir9.TABLE_ORDER}
    result["types"] = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 3, 0, 0, 0, 0, 0, 1),
        (2, 2, 0, 0, 0, 0, 0, 2_147_483_647),
        (3, 1, 0, 0, 0, 0, 0, 255),
    ]
    result["records"] = [(0, 0, 0, 0, 0, 0, 0, 0)]
    result["machines"] = [(0, 0, 2, 0, 0, 2, 0, 0, 0, 3, 0)]
    result["blocks"] = [
        (0, 0, 2, 0, 0, 0, 0, 0, 9, 0),
        (1, 0, 2, 0, 0, 0, 0, 9, 1, 1),
        (2, 0, 2, 0, 0, 0, 0, 10, 1, 2),
    ]
    result["operations"] = [
        (0, 0, 0, 1, 1, 0, 0, 2, 0, 0, 70, 0),
        (1, 0, 0, 1, 1, 0, 1, 2, 0, 0, 69, 0),
        (2, 0, 0, 19, 1, 0, 2, 1, 0, 2, 0, 0),
        (3, 0, 0, 1, 1, 0, 3, 2, 2, 0, 70, 0),
        (4, 0, 0, 1, 1, 0, 4, 2, 2, 0, 70, 0),
        (5, 0, 0, 18, 1, 0, 5, 1, 2, 2, 0, 0),
        (6, 0, 0, 16, 1, 0, 6, 1, 4, 2, 0, 0),
        (7, 0, 0, 15, 1, 0, 7, 1, 6, 1, 0, 0),
        (8, 0, 0, 17, 1, 0, 8, 1, 7, 2, 0, 0),
        (9, 0, 1, 1, 1, 0, 9, 2, 9, 0, 70, 0),
        (10, 0, 2, 1, 1, 0, 10, 2, 9, 0, 0, 0),
    ]
    result["operands"] = [
        (0,), (1,), (3,), (4,), (2,), (5,), (6,), (6,), (7,),
    ]
    result["terminators"] = [
        (0, 0, 0, 2, 0, 0, 8, 1, 9, 0, 2, 9, 0, 0, 0),
        (1, 0, 1, 4, 0, 0, 9, NO_ID, 9, 0, NO_ID, 9, 0, 0, 0),
        (2, 0, 2, 4, 0, 0, 10, NO_ID, 9, 0, NO_ID, 9, 0, 0, 0),
    ]
    return result


def encode(raw_tables: dict[str, list[tuple[int, ...]]], *, major: int = 9,
           values: int | None = None, places: int = 0) -> bytes:
    if values is None:
        values = sum(row[4] == 1 for row in raw_tables["operations"])
    return v8_fixture.encode(raw_tables, major=major, values=values, places=places)


def emit(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    positives = {
        "u8-greater-true": tables(71, 70, 3, 3, 19),
        "u8-greater-equal": tables(70, 70, 3, 3, 19),
        "u8-greater-less": tables(69, 70, 3, 3, 19),
        "u8-greater-equal-greater": tables(71, 70, 3, 3, 20),
        "u8-greater-equal-equal": tables(70, 70, 3, 3, 20),
        "u8-greater-equal-less": tables(69, 70, 3, 3, 20),
        "u32-greater-true": tables(71, 70, 2, 2, 19),
        "u32-greater-equal": tables(70, 70, 2, 2, 19),
        "u32-greater-less": tables(69, 70, 2, 2, 19),
        "u32-greater-equal-greater": tables(71, 70, 2, 2, 20),
        "u32-greater-equal-equal": tables(70, 70, 2, 2, 20),
        "u32-greater-equal-less": tables(69, 70, 2, 2, 20),
        "u32-constrained-greater": tables(71, 70, 4, 5, 19),
        "u32-constrained-greater-equal": tables(70, 70, 4, 5, 20),
        "mixed-inherited": mixed_tables(),
    }
    for name, value in positives.items():
        (directory / f"{name}.ckir9").write_bytes(encode(value))

    base = tables(71, 70, 2, 2, 19)
    canonical = encode(base)
    (directory / "canonical.ckir9").write_bytes(canonical)
    manifest: list[tuple[str, int]] = []

    def mutation(name: str, change, expected: int = 251, major: int = 9) -> None:
        changed = copy.deepcopy(base)
        change(changed)
        (directory / f"{name}.ckir9").write_bytes(encode(changed, major=major))
        manifest.append((name, expected))

    for major in (4, 5, 6, 7, 8):
        mutation(f"new-op-schema-major-{major}", lambda _: None, major=major)

    inherited = v8_fixture.encode(v8_fixture.tables(70, 70, 2, 2), major=9)
    (directory / "missing-greater.ckir9").write_bytes(inherited)
    manifest.append(("missing-greater", 251))

    mutation("arity-one", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 9, 1)))
    mutation("arity-three", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 9, 3)))
    mutation("immediate-zero", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 10, 1)))
    mutation("immediate-one", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 11, 1)))
    mutation("reserved", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 5, 1)))
    mutation("invisible-left", lambda t: t["operands"].__setitem__(0, (2,)))
    mutation("invisible-right", lambda t: t["operands"].__setitem__(1, (2,)))
    mutation("mixed-u8-u32", lambda t: t["operations"].__setitem__(1, replace(t["operations"][1], 7, 3)))

    def bool_operands(value: dict[str, list[tuple[int, ...]]]) -> None:
        value["operations"][0] = replace(replace(value["operations"][0], 7, 1), 10, 1)
        value["operations"][1] = replace(replace(value["operations"][1], 7, 1), 10, 0)

    mutation("bool-operands", bool_operands)
    mutation("non-bool-result", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 7, 2)))
    mutation("wrong-result-kind", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 4, 2)))
    mutation("wrong-result-id", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 6, 3)))
    mutation("opcode-less", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 3, 9)))
    mutation("opcode-less-equal", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 3, 12)))
    mutation("opcode-scalar-equal", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 3, 18)))

    for name, count, value in (
        ("operations-over", "operations", 32_769),
        ("operands-over", "operands", 94_209),
    ):
        (directory / f"{name}.ckir9").write_bytes(
            v8_fixture.mutate_count(canonical, count, value)
        )
        manifest.append((name, 252))

    (directory / "positives.tsv").write_text(
        "".join(f"{name}\t70\n" for name in positives), encoding="ascii"
    )
    (directory / "manifest.tsv").write_text(
        "".join(f"{name}\t{status}\n" for name, status in manifest), encoding="ascii"
    )


def check(path: Path) -> None:
    module = ir9.decode(path.read_bytes())
    ir9.v5.require(ir9.interpret(module) == 70, "ordered greater carrier result")
    ir9.v5.require(any(op[3] in (19, 20) for op in module.tables["operations"]),
                   "ordered greater operation count")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("emit", "check"))
    parser.add_argument("path", type=Path)
    args = parser.parse_args()
    emit(args.path) if args.command == "emit" else check(args.path)


if __name__ == "__main__":
    main()
