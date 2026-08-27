#!/usr/bin/env python3
"""Focused historical and selected resolved-source lowering frames."""

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


def encode_v11(compilation: bytes, witness: bytes) -> bytes:
    return encode_selected(compilation, witness, 11)


def encode_v12(compilation: bytes, witness: bytes) -> bytes:
    return encode_selected(compilation, witness, 12)


def encode_v16(compilation: bytes, witness: bytes) -> bytes:
    return encode_selected(compilation, witness, 16)


def encode_selected(compilation: bytes, witness: bytes, major: int) -> bytes:
    if len(compilation) > MAX_COMPILATION or len(witness) > MAX_WITNESS:
        raise ValueError("component capacity")
    total = HEADER.size + len(compilation) + len(witness)
    if total > MAX_FRAME:
        raise ValueError("frame capacity")
    if len(witness) < 10 or witness[:6] != b"OMGRSW":
        raise ValueError("unsupported resolution witness")
    resolution = struct.unpack_from("<H", witness, 8)[0]
    allowed = (4, 7) if major == 16 else (1, 2, 3)
    if resolution not in allowed or witness[:8] != f"OMGRSW{resolution}".encode("ascii") + b"\0":
        raise ValueError("unsupported resolution witness")
    magic = {
        10: b"OMGLOWA\0", 11: b"OMGLOWB\0", 12: b"OMGLOWC\0",
        16: b"OMGLOWG\0",
    }.get(major, f"OMGLOW{major}".encode("ascii") + b"\0")
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
                              (b"OMGLOW9\0", 9), (b"OMGLOWA\0", 10),
                              (b"OMGLOWB\0", 11), (b"OMGLOWC\0", 12),
                              (b"OMGLOWG\0", 16)):
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
    selected = (7, 8, 9, 10, 11, 12, 16)
    allowed = (4, 7) if major == 16 else (1, 2, 3)
    if (major not in selected and reserved != 0) or (major in selected and reserved not in allowed):
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
    if len(args) == 3 and args[0] == "pack-v11":
        sys.stdout.buffer.write(encode_v11(Path(args[1]).read_bytes(), Path(args[2]).read_bytes()))
        return 0
    if len(args) == 3 and args[0] == "pack-v12":
        sys.stdout.buffer.write(encode_v12(Path(args[1]).read_bytes(), Path(args[2]).read_bytes()))
        return 0
    if len(args) == 3 and args[0] == "pack-v16":
        sys.stdout.buffer.write(encode_v16(Path(args[1]).read_bytes(), Path(args[2]).read_bytes()))
        return 0
    if len(args) == 2 and args[0] == "verify":
        decode(Path(args[1]).read_bytes())
        return 0
    raise ValueError("usage: pack OMGCOMP OMGRSW1_OR_2 | pack-v7|pack-v8|pack-v9|pack-v10|pack-v11|pack-v12 OMGCOMP OMGRSW1_OR_2_OR_3 | pack-v16 OMGCOMP OMGRSW4_OR_7 | verify OMGLOW")


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (OSError, ValueError) as error:
        print(f"delta-resolved-to-ckir4-frame: {error}", file=sys.stderr)
        raise SystemExit(2)
