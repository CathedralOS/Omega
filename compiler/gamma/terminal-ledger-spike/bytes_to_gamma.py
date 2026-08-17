#!/usr/bin/env python3
"""Render a hex fixture as a shallow Gamma Bytes expression.

This is deliberately an untrusted transport helper.  It assigns no semantics
to the bytes: the typed Gamma decoder remains the only spike component that
recognizes PSITERM.  Seven-byte little-endian integers keep source/parser size
bounded while the typed ``unpack_bytes`` helper reconstructs every exact byte.
"""

from __future__ import annotations

import argparse
from pathlib import Path


def render(data: bytes, chunk_size: int) -> str:
    chunks = [data[offset : offset + chunk_size] for offset in range(0, len(data), chunk_size)]
    result = "BNil"
    for chunk in reversed(chunks):
        packed = int.from_bytes(chunk, byteorder="little", signed=False)
        result = f"(unpack_bytes {packed} {len(chunk)} {result})"
    return result


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("fixture", type=Path)
    parser.add_argument("--chunk-size", type=int, default=7)
    parser.add_argument("--set-byte", nargs=2, type=int, metavar=("OFFSET", "VALUE"))
    parser.add_argument("--drop-last", action="store_true")
    parser.add_argument("--append-byte", type=int)
    args = parser.parse_args()

    if not 1 <= args.chunk_size <= 7:
        parser.error("--chunk-size must be between one and seven bytes")
    compact = "".join(args.fixture.read_text(encoding="ascii").split())
    if len(compact) % 2:
        parser.error("fixture has an odd number of hexadecimal digits")
    try:
        data = bytearray.fromhex(compact)
    except ValueError as error:
        parser.error(f"fixture is not hexadecimal: {error}")

    if args.set_byte is not None:
        offset, value = args.set_byte
        if not 0 <= offset < len(data):
            parser.error("--set-byte offset is outside the fixture")
        if not 0 <= value <= 255:
            parser.error("--set-byte value must fit in one byte")
        data[offset] = value
    if args.drop_last:
        if not data:
            parser.error("cannot drop a byte from an empty fixture")
        data.pop()
    if args.append_byte is not None:
        if not 0 <= args.append_byte <= 255:
            parser.error("--append-byte must fit in one byte")
        data.append(args.append_byte)

    print(render(bytes(data), args.chunk_size))


if __name__ == "__main__":
    main()
