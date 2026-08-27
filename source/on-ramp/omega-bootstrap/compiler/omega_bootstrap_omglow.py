#!/usr/bin/env python3
"""Pure framing helper for the private OMGLOW1 resolver/lowerer handoff."""

from __future__ import annotations

import json
import struct
import sys
from dataclasses import dataclass
from pathlib import Path


MAGIC = b"OMGLOW1\0"
SCHEMA_MAJOR = 1
SCHEMA_MINOR = 0
HEADER = struct.Struct("<8sHHHH4I")
MAX_COMPILATION_BYTES = 267_280
MAX_WITNESS_BYTES = 524_288
MAX_FRAME_BYTES = 791_600


class FrameError(ValueError):
    pass


@dataclass(frozen=True)
class Frame:
    raw: bytes
    compilation: bytes
    witness: bytes


def encode(compilation: bytes, witness: bytes) -> bytes:
    if len(compilation) > MAX_COMPILATION_BYTES:
        raise FrameError("OMGCOMP component exceeds 267,280 bytes")
    if len(witness) > MAX_WITNESS_BYTES:
        raise FrameError("OMGRSW1 component exceeds 524,288 bytes")
    total = HEADER.size + len(compilation) + len(witness)
    if total > MAX_FRAME_BYTES:
        raise FrameError("OMGLOW1 frame exceeds 791,600 bytes")
    return HEADER.pack(
        MAGIC, SCHEMA_MAJOR, SCHEMA_MINOR, 0, HEADER.size,
        total, len(compilation), len(witness), 0,
    ) + compilation + witness


def decode(raw: bytes) -> Frame:
    if len(raw) < HEADER.size:
        raise FrameError("truncated OMGLOW1 header")
    magic, major, minor, flags, header_size, total, comp_len, witness_len, reserved = HEADER.unpack_from(raw)
    if (magic, major, minor, flags, header_size, reserved) != (
        MAGIC, SCHEMA_MAJOR, SCHEMA_MINOR, 0, HEADER.size, 0,
    ):
        raise FrameError("unsupported or malformed OMGLOW1 fixed header")
    if comp_len > MAX_COMPILATION_BYTES:
        raise FrameError("OMGCOMP component exceeds 267,280 bytes")
    if witness_len > MAX_WITNESS_BYTES:
        raise FrameError("OMGRSW1 component exceeds 524,288 bytes")
    computed = HEADER.size + comp_len + witness_len
    if total != computed or total != len(raw):
        raise FrameError("OMGLOW1 declared/computed/exact length mismatch")
    if computed > MAX_FRAME_BYTES:
        raise FrameError("OMGLOW1 frame exceeds 791,600 bytes")
    split = HEADER.size + comp_len
    return Frame(raw, raw[HEADER.size:split], raw[split:])


def read(path: str | None) -> bytes:
    return sys.stdin.buffer.read() if path in (None, "-") else Path(path).read_bytes()


def usage() -> FrameError:
    return FrameError(
        "usage: omega_bootstrap_omglow.py pack OMGCOMP OMGRSW1 | "
        "verify [OMGLOW1] | inspect [OMGLOW1]"
    )


def main(arguments: list[str]) -> int:
    if not arguments:
        raise usage()
    command, *rest = arguments
    if command == "pack" and len(rest) == 2:
        sys.stdout.buffer.write(encode(Path(rest[0]).read_bytes(), Path(rest[1]).read_bytes()))
        return 0
    if command == "verify" and len(rest) <= 1:
        decode(read(rest[0] if rest else None))
        return 0
    if command == "inspect" and len(rest) <= 1:
        frame = decode(read(rest[0] if rest else None))
        print(json.dumps({
            "frame_bytes": len(frame.raw),
            "omgcomp_bytes": len(frame.compilation),
            "omgrsw1_bytes": len(frame.witness),
        }, sort_keys=True))
        return 0
    raise usage()


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (FrameError, OSError) as error:
        print(f"omega-bootstrap OMGLOW1: {error}", file=sys.stderr)
        raise SystemExit(2)
