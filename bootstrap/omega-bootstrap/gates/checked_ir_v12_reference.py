#!/usr/bin/env python3
"""Independent CKIR12 shared static-byte-view reference."""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

import checked_ir_v2_reference as v2
import checked_ir_v3_reference as v3
import checked_ir_v4_reference as v4
import checked_ir_v5_reference as v5


Ckir12Error = v5.Ckir5Error
Ckir12ResourceError = v5.Ckir5ResourceError
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
        expected_major=12,
        allow_logical_not=True,
        allow_logical_binary=True,
        allow_scalar_equal=True,
        allow_greater=True,
        allow_integer_widen=True,
        allow_static_byte_view=True,
    )


def selected_counts(module: Module) -> dict[int, int]:
    return {
        opcode: sum(operation[3] == opcode for operation in module.tables["operations"])
        for opcode in range(22, 26)
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("validate", "run"))
    parser.add_argument("ckir", type=Path)
    args = parser.parse_args()
    module = decode(args.ckir.read_bytes())
    if args.command == "validate":
        counts = selected_counts(module)
        print(
            "CKIR12 valid: "
            f"{counts[22]} StaticByteView, {counts[23]} SliceNonEmpty, "
            f"{counts[24]} SliceHead, {counts[25]} SliceTailOne operations"
        )
    else:
        result = interpret(module)
        print("library" if result is None else result)


if __name__ == "__main__":
    try:
        main()
    except Ckir12ResourceError as error:
        print(f"checked IR v12 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(252)
    except (Ckir12Error, v4.Ckir4Error, v3.Ckir3Error, v2.CkirError,
            OSError, struct.error) as error:
        print(f"checked IR v12 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(251)
