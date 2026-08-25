#!/usr/bin/env python3
"""Thin CKIR7 reference over the inherited CKIR6 table implementation."""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

import checked_ir_v2_reference as v2
import checked_ir_v3_reference as v3
import checked_ir_v4_reference as v4
import checked_ir_v5_reference as v5


Ckir7Error = v5.Ckir5Error
Ckir7ResourceError = v5.Ckir5ResourceError
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
        expected_major=7,
        allow_logical_not=True,
        allow_logical_binary=True,
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("validate", "run"))
    parser.add_argument("ckir", type=Path)
    args = parser.parse_args()
    module = decode(args.ckir.read_bytes())
    if args.command == "validate":
        and_count = sum(op[3] == 16 for op in module.tables["operations"])
        or_count = sum(op[3] == 17 for op in module.tables["operations"])
        print(f"CKIR7 valid: {and_count} LogicalAnd, {or_count} LogicalOr operations")
    else:
        result = interpret(module)
        print("library" if result is None else result)


if __name__ == "__main__":
    try:
        main()
    except Ckir7ResourceError as error:
        print(f"checked IR v7 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(252)
    except (Ckir7Error, v4.Ckir4Error, v3.Ckir3Error, v2.CkirError,
            OSError, struct.error) as error:
        print(f"checked IR v7 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(251)
