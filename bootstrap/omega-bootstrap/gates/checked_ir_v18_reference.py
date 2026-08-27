#!/usr/bin/env python3
"""Independent CKIR18 fixed-buffer/full-width-u64 reference."""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

import checked_ir_v2_reference as v2
import checked_ir_v3_reference as v3
import checked_ir_v4_reference as v4
import checked_ir_v5_reference as v5


Ckir18Error = v5.Ckir5Error
Ckir18ResourceError = v5.Ckir5ResourceError
Module = v5.Module
HEADER = v5.HEADER
ROWS = v5.ROWS
TABLE_ORDER = v5.TABLE_ORDER
COUNT_NAMES = v5.COUNT_NAMES
NO_ID = v5.NO_ID
interpret = v5.interpret

CAPABILITIES = v5.SchemaCapabilities(
    frozenset(range(1, 11)),
    full_width_u64_less=True,
    full_width_u64_index_add=True,
)


def decode(contents: bytes) -> Module:
    module = v5.decode(
        contents,
        expected_major=18,
        capabilities=CAPABILITIES,
    )
    v5.require(module.entry != NO_ID, "CKIR18 requires an entry machine")
    for name in (
        "sums", "cases", "case_payloads", "constants",
        "constant_children", "case_arms", "case_arm_args",
    ):
        v5.require(not module.tables[name], f"CKIR18 excludes {name}")
    v5.require(all(row[1] != 7 for row in module.tables["types"]),
               "CKIR18 excludes static byte views")
    v5.require(all(1 <= row[3] <= 10 for row in module.tables["operations"]),
               "CKIR18 excludes historical opcode families")
    selected = selected_operations(module)
    v5.require(all(selected.values()),
               "CKIR18 requires u64 IndexPlace, Add, and Less")
    return module


def selected_operations(module: Module) -> dict[str, list[tuple[int, ...]]]:
    types = module.tables["types"]
    operands = [row[0] for row in module.tables["operands"]]
    values = module.value_types
    places = module.place_types
    selected = {"index": [], "add": [], "less": []}
    for operation in module.tables["operations"]:
        opcode, start, count = operation[3], operation[8], operation[9]
        args = operands[start:start + count]
        if opcode == 4 and len(args) == 2:
            base_type = places[args[0]]
            index_type = values[args[1]]
            if types[base_type][1] == 5 and types[index_type][1] == 8:
                selected["index"].append(operation)
        elif opcode in (8, 9) and len(args) == 2:
            if all(types[values[value]][1] == 8 for value in args):
                selected["add" if opcode == 8 else "less"].append(operation)
    return selected


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("validate", "run"))
    parser.add_argument("ckir", type=Path)
    args = parser.parse_args()
    module = decode(args.ckir.read_bytes())
    if args.command == "validate":
        selected = selected_operations(module)
        print(
            "CKIR18 valid: "
            f"{len(selected['index'])} u64 indexes, "
            f"{len(selected['add'])} u64 adds, "
            f"{len(selected['less'])} u64 less operations"
        )
    else:
        result = interpret(module)
        print("library" if result is None else result)


if __name__ == "__main__":
    try:
        main()
    except Ckir18ResourceError as error:
        print(f"checked IR v18 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(252)
    except (Ckir18Error, v4.Ckir4Error, v3.Ckir3Error, v2.CkirError,
            OSError, struct.error) as error:
        print(f"checked IR v18 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(251)
