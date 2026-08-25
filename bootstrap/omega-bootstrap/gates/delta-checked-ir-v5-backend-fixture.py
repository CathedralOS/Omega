#!/usr/bin/env python3
"""Backend-local canonical CKIR5 carrier and isolated controls."""

from __future__ import annotations

import argparse
import copy
import importlib.util
import struct
from pathlib import Path

import checked_ir_v5_reference as ir5


HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location(
    "ckir5_fixture_base", HERE / "delta-checked-ir-v5-fixture.py"
)
assert SPEC and SPEC.loader
BASE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BASE)
NO_ID = ir5.NO_ID


def canonical_tables() -> dict[str, list[tuple[int, ...]]]:
    """Remap the independent meaning carrier to CKIR5 canonical type order."""
    tables = copy.deepcopy(BASE.canonical_tables())
    remap = {0: 4, 1: 5, 2: 2, 3: 0, 4: 1, 5: 6}
    tables["types"] = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 6, 0, 0, 0, 0, 0, 0),
        (2, 3, 0, 0, 0, 0, 0, 1),
        (3, 2, 0, 0, 0, 0, 0, 2_147_483_647),
        (4, 1, 0, 0, 0, 0, 0, 255),
        (5, 2, 0, 0, 0, 0, 0, 100),
        (6, 2, 0, 0, 0, 0, 70, 70),
    ]

    def at(name: str, column: int) -> None:
        tables[name] = [
            row if row[column] == NO_ID else
            row[:column] + (remap[row[column]],) + row[column + 1:]
            for row in tables[name]
        ]

    tables["records"] = [row[:1] + (remap[row[1]],) + row[2:]
                           for row in tables["records"]]
    tables["sums"] = [row[:1] + (remap[row[1]],) + row[2:]
                        for row in tables["sums"]]
    for name, column in (("fields", 3), ("case_payloads", 3),
                         ("machine_params", 3), ("block_params", 3),
                         ("operations", 7)):
        at(name, column)
    tables["machines"] = [
        row if row[5] == NO_ID else row[:5] + (remap[row[5]],) + row[6:]
        for row in tables["machines"]
    ]
    return tables


def encode(tables: dict[str, list[tuple[int, ...]]], values: int = 19,
           places: int = 5, entry: int = 0) -> bytes:
    return BASE.encode(tables, values, places, entry)


def replace(row: tuple[int, ...], index: int, value: int) -> tuple[int, ...]:
    result = list(row)
    result[index] = value
    return tuple(result)


def declarations(case_count: int, payload_count: int) -> bytes:
    tables = {name: [] for name in ir5.TABLE_ORDER}
    tables["types"] = [
        (0, 6, 0, 0, 0, 0, 0, 0),
        (1, 3, 0, 0, 0, 0, 0, 1),
        (2, 2, 0, 0, 0, 0, 0, 2_147_483_647),
        (3, 1, 0, 0, 0, 0, 0, 255),
    ]
    tables["sums"] = [(0, 0, 0, case_count, 1, 0, 0, 0)]
    tables["cases"] = [
        (index, 0, index, 0 if index == 0 else payload_count,
         payload_count if index == 0 else 0)
        for index in range(case_count)
    ]
    tables["case_payloads"] = [(index, 0, index, 3)
                                for index in range(payload_count)]
    return encode(tables, 0, 0, NO_ID)


def emit(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    base = canonical_tables()
    (directory / "canonical.ckir5").write_bytes(encode(base))
    manifest: list[tuple[str, int]] = []

    def mutation(name: str, table: str, row: int, column: int, value: int) -> None:
        changed = copy.deepcopy(base)
        changed[table][row] = replace(changed[table][row], column, value)
        (directory / f"{name}.ckir5").write_bytes(encode(changed))
        manifest.append((name, 251))

    mutation("construct-wrong-case", "operations", 5, 10, 2)
    mutation("construct-reserved", "operations", 5, 11, 1)
    mutation("dispatch-flags", "terminators", 0, 4, 0)
    mutation("arm-order", "case_arms", 1, 2, 2)
    mutation("payload-wrong-case", "case_arm_args", 0, 2, 3)
    mutation("payload-duplicate", "case_arm_args", 1, 2, 0)

    for name, cases, payloads, expected in (
        ("payload-four", 1, 4, 0),
        ("payload-five", 1, 5, 252),
        ("cases-64", 64, 0, 0),
        ("cases-65", 65, 0, 252),
    ):
        (directory / f"{name}.ckir5").write_bytes(declarations(cases, payloads))
        manifest.append((name, expected))

    raw = bytearray(encode(base))
    struct.pack_into("<I", raw, 24 + 4 * ir5.COUNT_NAMES.index("case_arms"), 4_097)
    (directory / "case-arms-4097.ckir5").write_bytes(raw)
    manifest.append(("case-arms-4097", 252))

    (directory / "manifest.tsv").write_text(
        "".join(f"{name}\t{status}\n" for name, status in manifest), encoding="ascii"
    )


def check_ir(path: Path) -> None:
    module = ir5.decode(path.read_bytes())
    ir5.require(ir5.interpret(module) == 70, "canonical result")
    ir5.require(module.layouts[1] == (8, 4), f"sum layout {module.layouts[1]}")
    ir5.require(module.layouts[0] == (16, 4), f"owner layout {module.layouts[0]}")


def check_artifact(path: Path) -> None:
    artifact = path.read_bytes()
    ir5.require(artifact.startswith(b"\x7fELF\x02\x01\x01"), "ELF64 header")
    # mov eax,[r11]; cmp eax,3; jae trap -- both dispatch sites must validate.
    tag_check = b"\x41\x8b\x03\x3d\x03\x00\x00\x00\x0f\x83"
    ir5.require(artifact.count(tag_check) >= 2, "runtime invalid-tag checks")
    # ConstructCase case-1 tag store into its derived object.
    ir5.require(b"\x41\xc7\x02\x01\x00\x00\x00" in artifact,
                "ConstructCase tag store")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("emit", "check-ir", "check-artifact"))
    parser.add_argument("path", type=Path)
    args = parser.parse_args()
    if args.command == "emit":
        emit(args.path)
    elif args.command == "check-ir":
        check_ir(args.path)
    else:
        check_artifact(args.path)


if __name__ == "__main__":
    main()
