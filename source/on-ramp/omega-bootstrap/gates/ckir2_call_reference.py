#!/usr/bin/env python3
"""Pinned reference bytes and observation for the first CKIR schema-2 call DAG."""

from __future__ import annotations

import argparse
import struct
from pathlib import Path


NO_ID = 0xFFFF_FFFF
HEADER = struct.Struct("<8sHHHH14I")
ROWS = (
    struct.Struct("<IBBHIIII"), struct.Struct("<IIIIB3x"),
    struct.Struct("<IIII"), struct.Struct("<IIBBHIIIIII"),
    struct.Struct("<IIIII"), struct.Struct("<IIBBHIIIII"),
    struct.Struct("<IIIII"), struct.Struct("<IIIBBHIIIIII"),
    struct.Struct("<I"), struct.Struct("<IIIBBHIIIIIII"),
)


def expected() -> bytes:
    types = [
        (0, 4, 0, 0, 0, 0, 0, 0), (1, 3, 0, 0, 0, 0, 0, 1),
        (2, 2, 0, 0, 0, 0, 0, 0x7FFF_FFFF),
        (3, 1, 0, 0, 0, 0, 0, 255),
    ]
    records = [(0, 0, 0, 0, 0)]
    machines = [
        (0, 0, 2, 0, 0, 3, 0, 0, 0, 1, 0),
        (1, 0, 2, 0, 0, 3, 0, 1, 1, 1, 1),
        (2, 0, 2, 0, 0, 3, 1, 1, 2, 1, 2),
        (3, 0, 1, 0, 0, 3, 2, 0, 3, 1, 3),
    ]
    machine_parameters = [(0, 1, 0, 3, 0), (1, 2, 0, 3, 1)]
    blocks = [
        (0, 0, 2, 0, 0, 0, 0, 0, 3, 0),
        (1, 1, 2, 0, 0, 0, 0, 3, 2, 1),
        (2, 2, 2, 0, 0, 0, 0, 5, 2, 2),
        (3, 3, 1, 0, 0, 0, 0, 7, 1, 3),
    ]
    operations = [
        (0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 0),
        (1, 0, 0, 1, 1, 0, 2, 3, 0, 0, 68, 0),
        (2, 0, 0, 10, 1, 0, 3, 3, 0, 2, 1, 0),
        (3, 1, 1, 2, 2, 0, 1, 0, 2, 0, 0, 0),
        (4, 1, 1, 10, 1, 0, 4, 3, 2, 2, 2, 0),
        (5, 2, 2, 1, 1, 0, 5, 3, 4, 0, 2, 0),
        (6, 2, 2, 8, 1, 0, 6, 3, 4, 2, 0, 0),
        (7, 3, 3, 1, 1, 0, 7, 3, 6, 0, 7, 0),
    ]
    operands = [(0,), (2,), (1,), (0,), (1,), (5,)]
    terminators = [
        (0, 0, 0, 4, 0, 0, 3, NO_ID, 6, 0, NO_ID, 6, 0),
        (1, 1, 1, 4, 0, 0, 4, NO_ID, 6, 0, NO_ID, 6, 0),
        (2, 2, 2, 4, 0, 0, 6, NO_ID, 6, 0, NO_ID, 6, 0),
        (3, 3, 3, 4, 0, 0, 7, NO_ID, 6, 0, NO_ID, 6, 0),
    ]
    tables = (
        types, records, [], machines, machine_parameters, blocks, [],
        operations, operands, terminators,
    )
    payload = b"".join(
        row_type.pack(*row)
        for table, row_type in zip(tables, ROWS)
        for row in table
    )
    counts = tuple(len(table) for table in tables)
    return HEADER.pack(
        b"OMGCKIR\0", 2, 0, 1, 1, 0, HEADER.size + len(payload),
        *counts, 8, 2,
    ) + payload


def check(path: Path) -> None:
    actual = path.read_bytes()
    wanted = expected()
    if actual != wanted:
        limit = min(len(actual), len(wanted))
        offset = next((i for i in range(limit) if actual[i] != wanted[i]), limit)
        raise ValueError(
            f"CKIR2 mismatch at {offset}: actual={actual[offset:offset+16].hex()} "
            f"expected={wanted[offset:offset+16].hex()} lengths={len(actual)}/{len(wanted)}"
        )
    print("70")


def main() -> None:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    emit = sub.add_parser("emit")
    emit.add_argument("output", type=Path, nargs="?")
    verify = sub.add_parser("check")
    verify.add_argument("ckir", type=Path)
    args = parser.parse_args()
    if args.command == "emit":
        if args.output:
            args.output.write_bytes(expected())
        else:
            import sys
            sys.stdout.buffer.write(expected())
    else:
        check(args.ckir)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, struct.error) as error:
        raise SystemExit(f"CKIR2 call reference: {error}")
