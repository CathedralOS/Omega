#!/usr/bin/env python3
"""Thin CKIR11 reference over the inherited CKIR5-10 table implementation."""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

import checked_ir_v2_reference as v2
import checked_ir_v3_reference as v3
import checked_ir_v4_reference as v4
import checked_ir_v5_reference as v5


Ckir11Error = v5.Ckir5Error
Ckir11ResourceError = v5.Ckir5ResourceError
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
        expected_major=11,
        allow_logical_not=True,
        allow_logical_binary=True,
        allow_scalar_equal=True,
        allow_greater=True,
        allow_integer_widen=True,
        require_trapping_add=True,
    )


def selected_add_count(module: Module) -> int:
    types = module.tables["types"]
    values = module.value_types
    return sum(
        operation[3] == 8
        and types[operation[7]][1:] == (2, 1, 0, 0, 0, 0, 0x7FFF_FFFF)
        and values[module.tables["operands"][operation[8]][0]] == operation[7]
        and values[module.tables["operands"][operation[8] + 1][0]] == operation[7]
        for operation in module.tables["operations"]
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("validate", "run"))
    parser.add_argument("ckir", type=Path)
    args = parser.parse_args()
    module = decode(args.ckir.read_bytes())
    if args.command == "validate":
        print(f"CKIR11 valid: {selected_add_count(module)} canonical u32 Trapping Add operations")
    else:
        result = interpret(module)
        print("library" if result is None else result)


if __name__ == "__main__":
    try:
        main()
    except Ckir11ResourceError as error:
        print(f"checked IR v11 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(252)
    except (Ckir11Error, v4.Ckir4Error, v3.Ckir3Error, v2.CkirError,
            OSError, struct.error) as error:
        print(f"checked IR v11 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(251)
