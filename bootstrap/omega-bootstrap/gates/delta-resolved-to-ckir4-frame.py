#!/usr/bin/env python3
"""Focused OMGLOW4/5 framing for the two source relations producing CKIR4."""

from __future__ import annotations

import struct
import sys
from pathlib import Path


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
    if witness[:9] == b"OMGRSW1\0\x01":
        magic, major = b"OMGLOW4\0", 4
    elif witness[:9] == b"OMGRSW2\0\x02":
        magic, major = b"OMGLOW5\0", 5
    else:
        raise ValueError("unsupported resolution witness")
    return HEADER.pack(magic, major, 0, 0, HEADER.size, total, len(compilation), len(witness), 0) + compilation + witness


def decode(raw: bytes) -> tuple[bytes, bytes]:
    if len(raw) < HEADER.size:
        raise ValueError("truncated frame")
    magic, major, minor, flags, size, total, comp, witness, reserved = HEADER.unpack_from(raw)
    if (magic, major) not in ((b"OMGLOW4\0", 4), (b"OMGLOW5\0", 5)):
        raise ValueError("frame relation")
    if (minor, flags, size, reserved) != (0, 0, HEADER.size, 0):
        raise ValueError("fixed header")
    if comp > MAX_COMPILATION or witness > MAX_WITNESS or total != len(raw) or total != HEADER.size + comp + witness:
        raise ValueError("length or capacity")
    compilation = raw[HEADER.size:HEADER.size + comp]
    resolution = raw[HEADER.size + comp:]
    expected = (b"OMGRSW1\0", 1) if major == 4 else (b"OMGRSW2\0", 2)
    if len(resolution) < 10 or resolution[:8] != expected[0] or struct.unpack_from("<H", resolution, 8)[0] != expected[1]:
        raise ValueError("frame/witness relation")
    return compilation, resolution


def main(args: list[str]) -> int:
    if len(args) == 3 and args[0] == "pack":
        sys.stdout.buffer.write(encode(Path(args[1]).read_bytes(), Path(args[2]).read_bytes()))
        return 0
    if len(args) == 2 and args[0] == "verify":
        decode(Path(args[1]).read_bytes())
        return 0
    raise ValueError("usage: pack OMGCOMP OMGRSW1_OR_2 | verify OMGLOW4_OR_5")


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (OSError, ValueError) as error:
        print(f"delta-resolved-to-ckir4-frame: {error}", file=sys.stderr)
        raise SystemExit(2)
