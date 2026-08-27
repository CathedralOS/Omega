#!/usr/bin/env python3
"""CKIR6 LogicalNot carrier derived from the frozen CKIR5 composition fixture."""

from __future__ import annotations

import argparse
import copy
import importlib.util
import struct
from pathlib import Path

import checked_ir_v6_reference as ir6


HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location(
    "delta_checked_ir_v5_fixture", HERE / "delta-checked-ir-v5-fixture.py"
)
assert SPEC is not None and SPEC.loader is not None
v5_fixture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(v5_fixture)


def replace(row: tuple[int, ...], index: int, value: int) -> tuple[int, ...]:
    result = list(row)
    result[index] = value
    return tuple(result)


def encode(tables: dict[str, list[tuple[int, ...]]], major: int = 6) -> bytes:
    counts = {name: len(tables[name]) for name in ir6.TABLE_ORDER}
    counts.update(values=19, places=5)
    payload = b"".join(
        ir6.ROWS[name].pack(*row)
        for name in ir6.TABLE_ORDER
        for row in tables[name]
    )
    return ir6.HEADER.pack(
        b"OMGCKIR\0", major, 0, 1, 1, 0, ir6.HEADER.size + len(payload),
        *(counts[name] for name in ir6.COUNT_NAMES),
    ) + payload


def add_logical_not(
    inherited: dict[str, list[tuple[int, ...]]],
) -> dict[str, list[tuple[int, ...]]]:
    tables = copy.deepcopy(inherited)
    # The inherited carrier's operation 3 publishes canonical true as value 9.
    # Replace its following false constant with `!value9`, retaining value 10
    # and every downstream sum/call/copy/dispatch dependency.
    operation = tables["operations"][4]
    operation = replace(operation, 3, 15)
    operation = replace(operation, 9, 1)
    tables["operations"][4] = operation
    tables["operands"].insert(2, (9,))
    for index in range(5, len(tables["operations"])):
        tables["operations"][index] = replace(
            tables["operations"][index], 8,
            tables["operations"][index][8] + 1,
        )
    for index, term in enumerate(tables["terminators"]):
        term = replace(term, 8, term[8] + 1)
        term = replace(term, 11, term[11] + 1)
        tables["terminators"][index] = term
    return tables


def canonical_tables() -> dict[str, list[tuple[int, ...]]]:
    return add_logical_not(v5_fixture.canonical_tables())


def malformed_cases() -> dict[str, bytes]:
    base = canonical_tables()
    result: dict[str, bytes] = {}

    def one(name: str, change, major: int = 6) -> None:
        tables = copy.deepcopy(base)
        change(tables)
        result[name] = encode(tables, major)

    one("schema-major-5", lambda _: None, 5)
    result["missing-logical-not"] = encode(v5_fixture.canonical_tables())
    one("arity-zero", lambda t: t["operations"].__setitem__(
        4, replace(t["operations"][4], 9, 0)
    ))
    one("arity-two", lambda t: t["operations"].__setitem__(
        4, replace(t["operations"][4], 9, 2)
    ))
    one("immediate-zero", lambda t: t["operations"].__setitem__(
        4, replace(t["operations"][4], 10, 1)
    ))
    one("immediate-one", lambda t: t["operations"].__setitem__(
        4, replace(t["operations"][4], 11, 1)
    ))
    one("invisible-operand", lambda t: t["operands"].__setitem__(2, (10,)))
    one("non-bool-operand", lambda t: t["operations"].__setitem__(
        3, replace(t["operations"][3], 7, 0)
    ))
    one("non-bool-result", lambda t: t["operations"].__setitem__(
        4, replace(t["operations"][4], 7, 0)
    ))
    one("wrong-result-kind", lambda t: t["operations"].__setitem__(
        4, replace(t["operations"][4], 4, 2)
    ))
    one("wrong-result-id", lambda t: t["operations"].__setitem__(
        4, replace(t["operations"][4], 6, 11)
    ))
    return result


def mutate_count(contents: bytes, name: str, value: int) -> bytes:
    result = bytearray(contents)
    struct.pack_into("<I", result, 24 + 4 * ir6.COUNT_NAMES.index(name), value)
    return bytes(result)


def emit(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    canonical = encode(canonical_tables())
    (directory / "canonical.ckir6").write_bytes(canonical)
    manifest = []
    for name, contents in malformed_cases().items():
        (directory / f"{name}.ckir6").write_bytes(contents)
        manifest.append((name, 251))
    for name, count, value in (
        ("operations-over", "operations", 32_769),
        ("operands-over", "operands", 94_209),
    ):
        (directory / f"{name}.ckir6").write_bytes(mutate_count(canonical, count, value))
        manifest.append((name, 252))
    (directory / "manifest.tsv").write_text(
        "".join(f"{name}\t{status}\n" for name, status in manifest), encoding="ascii"
    )


def check(path: Path) -> None:
    module = ir6.decode(path.read_bytes())
    ir6.v5.require(sum(op[3] == 15 for op in module.tables["operations"]) == 1,
                   "LogicalNot count")
    ir6.v5.require(ir6.interpret(module) == 70, "LogicalNot carrier meaning")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("emit", "check"))
    parser.add_argument("path", type=Path)
    args = parser.parse_args()
    emit(args.path) if args.command == "emit" else check(args.path)


if __name__ == "__main__":
    main()
