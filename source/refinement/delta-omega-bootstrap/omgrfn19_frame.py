#!/usr/bin/env python3
"""Bounded two-component framing for structural OMGRFN19 assurance."""

from __future__ import annotations

import dataclasses
import struct
import sys
from pathlib import Path


MAGIC = b"OMGRFNJ\0"
VERSION = 19
FLAGS = 0
HEADER = struct.Struct("<8s4H4I")
HEADER_SIZE = HEADER.size
MAX_OMGCOMP = 267_280
MAX_WITNESS = 524_288
MAX_FRAME = HEADER_SIZE + MAX_OMGCOMP + MAX_WITNESS

HERE = Path(__file__).resolve().parent
COMPILER = HERE.parents[3] / "source/on-ramp/omega-bootstrap/compiler"
sys.path.insert(0, str(COMPILER))
import omega_bootstrap_compilation_v3 as compilation_v3  # noqa: E402


class RefinementError(ValueError):
    pass


class RefinementResourceError(RefinementError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RefinementError(message)


@dataclasses.dataclass(frozen=True)
class Frame:
    raw: bytes
    omgcomp: bytes
    witness: bytes


def split(raw: bytes) -> Frame:
    if len(raw) > MAX_FRAME:
        raise RefinementResourceError("whole-frame ceiling")
    require(len(raw) >= HEADER_SIZE, "truncated OMGRFN19 header")
    magic, major, minor, flags, header_size, total, omg_len, witness_len, reserved = (
        HEADER.unpack_from(raw)
    )
    require(magic == MAGIC, "OMGRFNJ identity")
    require((major, minor, flags, header_size) == (VERSION, 0, FLAGS, HEADER_SIZE),
            "OMGRFN19 version/flags/header size")
    require(reserved == 0, "OMGRFN19 reserved word")
    require(total == len(raw), "OMGRFN19 total extent")
    if omg_len > MAX_OMGCOMP:
        raise RefinementResourceError("OMGCOMP3 ceiling")
    if witness_len > MAX_WITNESS:
        raise RefinementResourceError("OMGRSW9 ceiling")
    require(omg_len > 0 and witness_len > 0, "nonempty structural components")
    require(HEADER_SIZE + omg_len + witness_len == total,
            "OMGRFN19 component extents")
    boundary = HEADER_SIZE + omg_len
    return Frame(raw, raw[HEADER_SIZE:boundary], raw[boundary:])


def check_r1(frame: Frame) -> None:
    try:
        envelope = compilation_v3.decode(frame.omgcomp)
    except compilation_v3.CompilationError as error:
        if getattr(error, "status", 251) == 252:
            raise RefinementResourceError(f"OMGCOMP3 custody: {error}") from error
        raise RefinementError(f"OMGCOMP3 custody: {error}") from error
    require(envelope.build_source_id < len(envelope.sources),
            "OMGCOMP3 selected build source")
    require(envelope.sources[envelope.build_source_id].owner_package_id
            == envelope.root_package_id,
            "OMGCOMP3 build-source/root-package custody")
    require(frame.witness[:8] == b"OMGRSW9\0" and len(frame.witness) >= 12,
            "OMGRSW9 component identity")
    require(struct.unpack_from("<HH", frame.witness, 8) == (9, 0),
            "OMGRSW9 component version")
