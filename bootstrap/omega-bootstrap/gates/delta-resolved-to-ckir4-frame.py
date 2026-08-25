#!/usr/bin/env python3
"""Focused OMGLOW4/5 and explicit OMGLOW7/8/9/A resolved-source framing."""

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


def encode_v7(compilation: bytes, witness: bytes) -> bytes:
    return encode_selected(compilation, witness, 7)


def encode_v8(compilation: bytes, witness: bytes) -> bytes:
    return encode_selected(compilation, witness, 8)


def encode_v9(compilation: bytes, witness: bytes) -> bytes:
    return encode_selected(compilation, witness, 9)


def encode_v10(compilation: bytes, witness: bytes) -> bytes:
    return encode_selected(compilation, witness, 10)


def encode_selected(compilation: bytes, witness: bytes, major: int) -> bytes:
    if len(compilation) > MAX_COMPILATION or len(witness) > MAX_WITNESS:
        raise ValueError("component capacity")
    total = HEADER.size + len(compilation) + len(witness)
    if total > MAX_FRAME:
        raise ValueError("frame capacity")
    if len(witness) < 10 or witness[:6] != b"OMGRSW":
        raise ValueError("unsupported resolution witness")
    resolution = struct.unpack_from("<H", witness, 8)[0]
    if resolution not in (1, 2, 3) or witness[:8] != f"OMGRSW{resolution}".encode("ascii") + b"\0":
        raise ValueError("unsupported resolution witness")
    magic = b"OMGLOWA\0" if major == 10 else f"OMGLOW{major}".encode("ascii") + b"\0"
    return HEADER.pack(
        magic, major, 0, 0, HEADER.size, total,
        len(compilation), len(witness), resolution,
    ) + compilation + witness


def decode(raw: bytes) -> tuple[bytes, bytes]:
    if len(raw) < HEADER.size:
        raise ValueError("truncated frame")
    magic, major, minor, flags, size, total, comp, witness, reserved = HEADER.unpack_from(raw)
    if (magic, major) not in ((b"OMGLOW4\0", 4), (b"OMGLOW5\0", 5),
                              (b"OMGLOW7\0", 7), (b"OMGLOW8\0", 8),
                              (b"OMGLOW9\0", 9), (b"OMGLOWA\0", 10)):
        raise ValueError("frame relation")
    if (minor, flags, size) != (0, 0, HEADER.size):
        raise ValueError("fixed header")
    if comp > MAX_COMPILATION or witness > MAX_WITNESS or total != len(raw) or total != HEADER.size + comp + witness:
        raise ValueError("length or capacity")
    compilation = raw[HEADER.size:HEADER.size + comp]
    resolution = raw[HEADER.size + comp:]
    if major == 4:
        expected = (b"OMGRSW1\0", 1)
    elif major == 5:
        expected = (b"OMGRSW2\0", 2)
    else:
        expected = (f"OMGRSW{reserved}".encode("ascii") + b"\0", reserved)
    if (major not in (7, 8, 9, 10) and reserved != 0) or (major in (7, 8, 9, 10) and reserved not in (1, 2, 3)):
        raise ValueError("frame selector")
    if len(resolution) < 10 or resolution[:8] != expected[0] or struct.unpack_from("<H", resolution, 8)[0] != expected[1]:
        raise ValueError("frame/witness relation")
    return compilation, resolution


def main(args: list[str]) -> int:
    if len(args) == 3 and args[0] == "pack":
        sys.stdout.buffer.write(encode(Path(args[1]).read_bytes(), Path(args[2]).read_bytes()))
        return 0
    if len(args) == 3 and args[0] == "pack-v7":
        sys.stdout.buffer.write(encode_v7(Path(args[1]).read_bytes(), Path(args[2]).read_bytes()))
        return 0
    if len(args) == 3 and args[0] == "pack-v8":
        sys.stdout.buffer.write(encode_v8(Path(args[1]).read_bytes(), Path(args[2]).read_bytes()))
        return 0
    if len(args) == 3 and args[0] == "pack-v9":
        sys.stdout.buffer.write(encode_v9(Path(args[1]).read_bytes(), Path(args[2]).read_bytes()))
        return 0
    if len(args) == 3 and args[0] == "pack-v10":
        sys.stdout.buffer.write(encode_v10(Path(args[1]).read_bytes(), Path(args[2]).read_bytes()))
        return 0
    if len(args) == 2 and args[0] == "verify":
        decode(Path(args[1]).read_bytes())
        return 0
    raise ValueError("usage: pack OMGCOMP OMGRSW1_OR_2 | pack-v7|pack-v8|pack-v9|pack-v10 OMGCOMP OMGRSW1_OR_2_OR_3 | verify OMGLOW")


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (OSError, ValueError) as error:
        print(f"delta-resolved-to-ckir4-frame: {error}", file=sys.stderr)
        raise SystemExit(2)
