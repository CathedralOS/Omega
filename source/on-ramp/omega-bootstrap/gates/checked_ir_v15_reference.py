#!/usr/bin/env python3
"""Independent CKIR15 recurrent shared-view edge reference."""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

import checked_ir_v2_reference as v2
import checked_ir_v3_reference as v3
import checked_ir_v4_reference as v4
import checked_ir_v5_reference as v5
import checked_ir_v14_reference as v14


Ckir15Error = v5.Ckir5Error
Ckir15ResourceError = v5.Ckir5ResourceError
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
        expected_major=15,
        capabilities=v5.SCHEMA_CAPABILITIES[15],
    )


def selected_counts(module: Module) -> dict[int, int]:
    return {
        opcode: sum(operation[3] == opcode for operation in module.tables["operations"])
        for opcode in range(22, 26)
    }


def selected_arithmetic_counts(module: Module) -> dict[int, int]:
    """Return the independently selected optional CKIR14 arithmetic rows."""
    return v14.selected_counts(module)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("validate", "run"))
    parser.add_argument("ckir", type=Path)
    args = parser.parse_args()
    module = decode(args.ckir.read_bytes())
    if args.command == "validate":
        counts = selected_counts(module)
        arithmetic = selected_arithmetic_counts(module)
        synthetic = sum(block[3] == 1 for block in module.tables["blocks"])
        print(
            "CKIR15 valid: "
            f"{synthetic} recurrent synthetic edges, "
            f"{counts[22]} StaticByteView, {counts[23]} SliceNonEmpty, "
            f"{counts[24]} SliceHead, {counts[25]} SliceTailOne, "
            f"optional arithmetic {arithmetic}"
        )
    else:
        result = interpret(module)
        print("library" if result is None else result)


if __name__ == "__main__":
    try:
        main()
    except Ckir15ResourceError as error:
        print(f"checked IR v15 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(252)
    except (Ckir15Error, v4.Ckir4Error, v3.Ckir3Error, v2.CkirError,
            OSError, struct.error) as error:
        print(f"checked IR v15 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(251)
