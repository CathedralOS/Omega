#!/usr/bin/env python3
"""Decode omega2gamma's exact `(Pair status stdout)` observation.

The canonical Gamma interpreter prints constructor trees rather than performing
host I/O on their behalf.  Output-capable Delta/Omega programs therefore return
their process status together with a forward `Cons` byte list.  This strict,
untrusted projection is for differential gates: malformed text, non-byte list
elements, trailing syntax, and partial trees all reject.
"""

from __future__ import annotations

import argparse
from pathlib import Path


class DecodeError(ValueError):
    pass


def decode(text: str) -> tuple[int, bytes]:
    position = 0

    def expect(token: str) -> None:
        nonlocal position
        if not text.startswith(token, position):
            raise DecodeError(f"expected {token!r} at byte {position}")
        position += len(token)

    def integer() -> int:
        nonlocal position
        start = position
        if position < len(text) and text[position] == "-":
            position += 1
        digits = position
        while position < len(text) and text[position].isdigit():
            position += 1
        if position == digits:
            raise DecodeError(f"expected integer at byte {start}")
        return int(text[start:position], 10)

    expect("(Pair ")
    status = integer()
    expect(" ")

    output = bytearray()
    cells = 0
    while text.startswith("(Cons ", position):
        position += len("(Cons ")
        value = integer()
        if not 0 <= value <= 255:
            raise DecodeError(f"stdout element {cells} is not a byte: {value}")
        output.append(value)
        cells += 1
        expect(" ")

    expect("Nil")
    expect(")" * cells)
    expect(")")
    if position < len(text) and text[position] == "\n":
        position += 1
    if position != len(text):
        raise DecodeError(f"trailing data at byte {position}")
    return status, bytes(output)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("observation", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    try:
        status, output = decode(args.observation.read_text(encoding="ascii"))
    except (DecodeError, UnicodeDecodeError) as error:
        parser.error(str(error))
    args.output.write_bytes(output)
    print(status)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
