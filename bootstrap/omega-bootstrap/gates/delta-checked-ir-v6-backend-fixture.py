#!/usr/bin/env python3
"""Backend-canonical CKIR6 carrier and LogicalNot artifact check."""

from __future__ import annotations

import argparse
import copy
import importlib.util
import re
from pathlib import Path

import checked_ir_v6_reference as ir6


HERE = Path(__file__).resolve().parent


def load(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, HERE / filename)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


v5_backend = load("delta_checked_ir_v5_backend_fixture",
                  "delta-checked-ir-v5-backend-fixture.py")
v6_fixture = load("delta_checked_ir_v6_fixture", "delta-checked-ir-v6-fixture.py")


def replace(row: tuple[int, ...], index: int, value: int) -> tuple[int, ...]:
    return row[:index] + (value,) + row[index + 1:]


def canonical_tables() -> dict[str, list[tuple[int, ...]]]:
    return v6_fixture.add_logical_not(v5_backend.canonical_tables())


def emit(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    base = canonical_tables()
    (directory / "canonical.ckir6").write_bytes(v6_fixture.encode(base))
    manifest = []

    def mutation(name: str, table: str, row: int, column: int, value: int) -> None:
        changed = copy.deepcopy(base)
        changed[table][row] = replace(changed[table][row], column, value)
        (directory / f"{name}.ckir6").write_bytes(v6_fixture.encode(changed))
        manifest.append((name, 251))

    mutation("logical-not-arity", "operations", 4, 9, 0)
    mutation("logical-not-immediate", "operations", 4, 10, 1)
    mutation("logical-not-invisible", "operands", 2, 0, 10)
    mutation("logical-not-result", "operations", 4, 7, 4)
    mutation("logical-not-kind", "operations", 4, 4, 2)
    schema5 = bytearray(v6_fixture.encode(base))
    schema5[8] = 5
    (directory / "schema-major-5.ckir6").write_bytes(schema5)
    manifest.append(("schema-major-5", 251))
    (directory / "manifest.tsv").write_text(
        "".join(f"{name}\t{status}\n" for name, status in manifest), encoding="ascii"
    )


def check_ir(path: Path) -> None:
    module = ir6.decode(path.read_bytes())
    ir6.v5.require(ir6.interpret(module) == 70, "canonical result")
    ir6.v5.require(sum(op[3] == 15 for op in module.tables["operations"]) == 1,
                   "LogicalNot count")


def check_artifact(path: Path) -> None:
    artifact = path.read_bytes()
    ir6.v5.require(artifact.startswith(b"\x7fELF\x02\x01\x01"), "ELF64 header")
    template = re.compile(rb"\x8b\x85....\x83\xf0\x01\x89\x85....", re.DOTALL)
    ir6.v5.require(template.search(artifact) is not None,
                   "canonical load/xor-one/store LogicalNot template")


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
