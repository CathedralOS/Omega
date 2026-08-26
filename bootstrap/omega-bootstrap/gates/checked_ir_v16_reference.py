#!/usr/bin/env python3
"""Independent CKIR16 full-width u64 Less reference."""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

import checked_ir_v2_reference as v2
import checked_ir_v3_reference as v3
import checked_ir_v4_reference as v4
import checked_ir_v5_reference as v5


Ckir16Error = v5.Ckir5Error
Ckir16ResourceError = v5.Ckir5ResourceError
Module = v5.Module
HEADER = v5.HEADER
ROWS = v5.ROWS
TABLE_ORDER = v5.TABLE_ORDER
COUNT_NAMES = v5.COUNT_NAMES
NO_ID = v5.NO_ID
interpret = v5.interpret


def decode(contents: bytes) -> Module:
    return v5.decode(
        contents,
        expected_major=16,
        capabilities=v5.SCHEMA_CAPABILITIES[16],
    )


def selected_less(module: Module) -> list[tuple[int, ...]]:
    types = module.tables["types"]
    operands = module.tables["operands"]
    values = module.value_types
    selected: list[tuple[int, ...]] = []
    for operation in module.tables["operations"]:
        if operation[3] != 9 or operation[9] != 2:
            continue
        start = operation[8]
        left_type = values[operands[start][0]]
        right_type = values[operands[start + 1][0]]
        if types[left_type][1] == 8 and types[right_type][1] == 8:
            selected.append(operation)
    return selected


def selected_count(module: Module) -> int:
    return len(selected_less(module))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("validate", "run"))
    parser.add_argument("ckir", type=Path)
    args = parser.parse_args()
    module = decode(args.ckir.read_bytes())
    if args.command == "validate":
        print(f"CKIR16 valid: {len(selected_less(module))} full-u64 Less operations")
    else:
        result = interpret(module)
        print("library" if result is None else result)


if __name__ == "__main__":
    try:
        main()
    except Ckir16ResourceError as error:
        print(f"checked IR v16 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(252)
    except (Ckir16Error, v4.Ckir4Error, v3.Ckir3Error, v2.CkirError,
            OSError, struct.error) as error:
        print(f"checked IR v16 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(251)
