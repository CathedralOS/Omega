#!/usr/bin/env python3
"""Handcrafted CKIR11 canonical u32 Trapping Add carriers and mutations."""

from __future__ import annotations

import argparse
import copy
import importlib.util
import struct
from pathlib import Path

import checked_ir_v11_reference as ir11


HERE = Path(__file__).resolve().parent
NO_ID = ir11.NO_ID


def load(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, HERE / filename)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


v10_fixture = load("delta_checked_ir_v10_fixture_for_v11", "delta-checked-ir-v10-fixture.py")


def replace(row: tuple[int, ...], index: int, value: int) -> tuple[int, ...]:
    return row[:index] + (value,) + row[index + 1:]


def tables(left: int, right: int, expected: int) -> dict[str, list[tuple[int, ...]]]:
    result = {name: [] for name in ir11.TABLE_ORDER}
    result["types"] = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 3, 0, 0, 0, 0, 0, 1),
        (2, 2, 0, 0, 0, 0, 0, 2_147_483_647),
        (3, 1, 0, 0, 0, 0, 0, 255),
        (4, 2, 1, 0, 0, 0, 0, 2_147_483_647),
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
        (2, 0, 0, 8, 1, 0, 2, 4, 0, 2, 0, 0),
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


def encode(raw_tables: dict[str, list[tuple[int, ...]]], *, major: int = 11,
           values: int = 7, places: int = 0) -> bytes:
    counts = {name: len(raw_tables[name]) for name in ir11.TABLE_ORDER}
    counts.update(values=values, places=places)
    payload = b"".join(
        ir11.ROWS[name].pack(*row)
        for name in ir11.TABLE_ORDER
        for row in raw_tables[name]
    )
    return ir11.HEADER.pack(
        b"OMGCKIR\0", major, 0, 1, 1, 0, ir11.HEADER.size + len(payload),
        *(counts[name] for name in ir11.COUNT_NAMES),
    ) + payload


def mutate_count(contents: bytes, name: str, value: int) -> bytes:
    changed = bytearray(contents)
    struct.pack_into("<I", changed, 24 + 4 * ir11.COUNT_NAMES.index(name), value)
    return bytes(changed)


def emit(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    positives = {
        "add-0-plus-70": tables(0, 70, 70),
        "add-69-plus-1": tables(69, 1, 70),
        "add-near-limit": tables(2_147_483_646, 1, 2_147_483_647),
    }
    for name, value in positives.items():
        (directory / f"{name}.ckir11").write_bytes(encode(value))
    (directory / "runtime-overflow.ckir11").write_bytes(
        encode(tables(2_147_483_647, 1, 0))
    )

    base = tables(69, 1, 70)
    canonical = encode(base)
    (directory / "canonical.ckir11").write_bytes(canonical)
    manifest: list[tuple[str, int]] = []

    def mutation(name: str, change, expected_status: int = 251,
                 major: int = 11) -> None:
        changed = copy.deepcopy(base)
        change(changed)
        (directory / f"{name}.ckir11").write_bytes(encode(changed, major=major))
        manifest.append((name, expected_status))

    for major in (4, 5, 6, 7, 9, 10):
        mutation(f"old-schema-major-{major}", lambda _: None, major=major)
    (directory / "old-schema-major-8.ckir11").write_bytes(encode(base, major=8))

    inherited = v10_fixture.encode(v10_fixture.tables(70), major=11)
    (directory / "missing-selected-add.ckir11").write_bytes(inherited)
    manifest.append(("missing-selected-add", 251))

    mutation("arity-one", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 9, 1)))
    mutation("arity-three", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 9, 3)))
    mutation("immediate-zero", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 10, 1)))
    mutation("immediate-one", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 11, 1)))
    mutation("reserved", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 5, 1)))
    mutation("invisible-left", lambda t: t["operands"].__setitem__(0, (2,)))
    mutation("right-u32-exact", lambda t: t["operations"].__setitem__(1, replace(t["operations"][1], 7, 2)))
    mutation("left-u32-constrained", lambda t: t["operations"].__setitem__(0, replace(t["operations"][0], 7, 5)))
    mutation("result-u32-exact", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 7, 2)))
    mutation("result-u32-constrained", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 7, 5)))
    mutation("wrong-result-kind", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 4, 2)))
    mutation("wrong-result-id", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 6, 3)))
    mutation("opcode-less", lambda t: t["operations"].__setitem__(2, replace(t["operations"][2], 3, 9)))

    for name, count, value in (
        ("operations-over", "operations", 32_769),
        ("operands-over", "operands", 94_209),
    ):
        (directory / f"{name}.ckir11").write_bytes(mutate_count(canonical, count, value))
        manifest.append((name, 252))

    (directory / "positives.tsv").write_text(
        "".join(f"{name}\t70\n" for name in positives), encoding="ascii"
    )
    (directory / "manifest.tsv").write_text(
        "".join(f"{name}\t{status}\n" for name, status in manifest), encoding="ascii"
    )


def check(path: Path) -> None:
    module = ir11.decode(path.read_bytes())
    ir11.v5.require(ir11.interpret(module) == 70, "canonical trapping Add carrier result")
    ir11.v5.require(ir11.selected_add_count(module) == 1,
                     "canonical u32 Trapping Add operation count")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("emit", "check"))
    parser.add_argument("path", type=Path)
    args = parser.parse_args()
    emit(args.path) if args.command == "emit" else check(args.path)


if __name__ == "__main__":
    main()
