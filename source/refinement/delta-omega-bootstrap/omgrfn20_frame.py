#!/usr/bin/env python3
"""Bounded three-component framing for OMGRFN20."""

from __future__ import annotations

import dataclasses
import struct
import sys
from pathlib import Path

MAGIC = b"OMGRFNK\0"
VERSION = 20
HEADER = struct.Struct("<8s4H5I")
MAX_OMGCOMP = 267_280
MAX_WITNESS = 524_288
MAX_CKIR = 2_654_288
MAX_FRAME = HEADER.size + MAX_OMGCOMP + MAX_WITNESS + MAX_CKIR

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
    ckir: bytes


def split(raw: bytes) -> Frame:
    if len(raw) > MAX_FRAME:
        raise RefinementResourceError("whole-frame ceiling")
    require(len(raw) >= HEADER.size, "truncated OMGRFN20 header")
    (magic, major, minor, flags, header_size, total, omg_len, witness_len,
     ckir_len, reserved) = HEADER.unpack_from(raw)
    require(magic == MAGIC, "OMGRFNK identity")
    require((major, minor, flags, header_size) == (VERSION, 0, 0, HEADER.size),
            "OMGRFN20 version/flags/header size")
    require(reserved == 0 and total == len(raw), "OMGRFN20 extent/reserved")
    for length, ceiling, label in (
        (omg_len, MAX_OMGCOMP, "OMGCOMP3"),
        (witness_len, MAX_WITNESS, "OMGRSW9"),
        (ckir_len, MAX_CKIR, "CKIR17"),
    ):
        if length > ceiling:
            raise RefinementResourceError(f"{label} ceiling")
        require(length > 0, f"nonempty {label}")
    require(HEADER.size + omg_len + witness_len + ckir_len == total,
            "OMGRFN20 component extents")
    first = HEADER.size + omg_len
    second = first + witness_len
    return Frame(raw, raw[HEADER.size:first], raw[first:second], raw[second:])


def check_r1(frame: Frame) -> None:
    try:
        envelope = compilation_v3.decode(frame.omgcomp)
    except compilation_v3.CompilationError as error:
        if getattr(error, "status", 251) == 252:
            raise RefinementResourceError(f"OMGCOMP3 custody: {error}") from error
        raise RefinementError(f"OMGCOMP3 custody: {error}") from error
    require(envelope.sources[envelope.build_source_id].owner_package_id
            == envelope.root_package_id, "build-source/root-package custody")
    require(len(frame.witness) >= 12 and frame.witness[:8] == b"OMGRSW9\0"
            and struct.unpack_from("<HH", frame.witness, 8) == (9, 0),
            "exact OMGRSW9 identity")
    require(len(frame.ckir) >= 12 and frame.ckir[:8] == b"OMGCKIR\0"
            and struct.unpack_from("<HH", frame.ckir, 8) == (17, 0),
            "exact CKIR17 identity")
