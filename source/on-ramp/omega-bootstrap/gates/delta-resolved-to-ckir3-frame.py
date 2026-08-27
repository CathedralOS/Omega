#!/usr/bin/env python3
"""Focused OMGLOW3 framing and CKIR3 structural inspection helper."""

from __future__ import annotations

import struct
import sys
from pathlib import Path


MAGIC = b"OMGLOW3\0"
HEADER = struct.Struct("<8sHHHH4I")
MAX_COMPILATION = 267_280
MAX_WITNESS = 524_288
MAX_FRAME = 791_600


def encode(compilation: bytes, witness: bytes) -> bytes:
    if len(compilation) > MAX_COMPILATION or len(witness) > MAX_WITNESS:
        raise ValueError("component capacity")
    total = HEADER.size + len(compilation) + len(witness)
    if total > MAX_FRAME:
        raise ValueError("frame capacity")
    return HEADER.pack(MAGIC, 3, 0, 0, HEADER.size, total, len(compilation), len(witness), 0) + compilation + witness


def decode(raw: bytes) -> tuple[bytes, bytes]:
    if len(raw) < HEADER.size:
        raise ValueError("truncated frame")
    magic, major, minor, flags, size, total, comp, witness, reserved = HEADER.unpack_from(raw)
    if (magic, major, minor, flags, size, reserved) != (MAGIC, 3, 0, 0, HEADER.size, 0):
        raise ValueError("fixed header")
    if comp > MAX_COMPILATION or witness > MAX_WITNESS or total != len(raw) or total != HEADER.size + comp + witness:
        raise ValueError("length or capacity")
    return raw[HEADER.size:HEADER.size + comp], raw[HEADER.size + comp:]


def main(args: list[str]) -> int:
    if len(args) == 3 and args[0] == "pack":
        sys.stdout.buffer.write(encode(Path(args[1]).read_bytes(), Path(args[2]).read_bytes()))
        return 0
    if len(args) == 2 and args[0] == "verify":
        decode(Path(args[1]).read_bytes())
        return 0
    raise ValueError("usage: pack OMGCOMP OMGRSW1 | verify OMGLOW3")


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (OSError, ValueError) as error:
        print(f"delta-resolved-to-ckir3-frame: {error}", file=sys.stderr)
        raise SystemExit(2)
