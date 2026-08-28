#!/usr/bin/env python3
"""Bounded OMGRFN18 framing shared by responsibility-local owners."""

from __future__ import annotations

import dataclasses
import sys
from pathlib import Path

from omgrfn6_bundle import HEADER, MAX_CKIR, MAX_ELF, MAX_FRAME, MAX_OMGCOMP, MAX_WITNESS

HERE = Path(__file__).resolve().parent
COMPILER = HERE.parents[2] / "source/on-ramp/omega-bootstrap/compiler"
sys.path.insert(0, str(COMPILER))
import omega_bootstrap_compilation as compilation  # noqa: E402

MAGIC = b"OMGRFNI\0"
VERSION = 18
FLAG_PROPOSITION = 1


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
    magic: bytes
    version: int
    flags: int
    omgcomp: bytes
    witness: bytes
    ckir: bytes
    elf: bytes
    result: int
    exit_code: int


def split(raw: bytes) -> Frame:
    if len(raw) > MAX_FRAME:
        raise RefinementResourceError("whole-frame ceiling")
    require(len(raw) >= HEADER.size, "truncated frame header")
    magic, version, flags, *words = HEADER.unpack_from(raw)
    omg_len, witness_len, ckir_len, elf_len, result, exit_code = words
    ceilings = (
        (omg_len, MAX_OMGCOMP, "OMGCOMP1"),
        (witness_len, MAX_WITNESS, "OMGRSW8"),
        (ckir_len, MAX_CKIR, "CKIR16"),
        (elf_len, MAX_ELF, "ELF"),
    )
    cursor = HEADER.size
    parts: list[bytes] = []
    for length, ceiling, label in ceilings:
        if length > ceiling:
            raise RefinementResourceError(f"{label} ceiling")
        require(length <= len(raw) - cursor, f"{label} extent")
        parts.append(raw[cursor:cursor + length])
        cursor += length
    require(cursor == len(raw), "exact frame EOF")
    return Frame(raw, magic, version, flags, *parts, result, exit_code)


def check_r1(frame: Frame) -> None:
    require(frame.magic == MAGIC and frame.version == VERSION,
            "OMGRFNI version 18")
    require(frame.flags == FLAG_PROPOSITION,
            "exact successful proposition flags")
    require(all((frame.omgcomp, frame.witness, frame.ckir, frame.elf)),
            "nonempty proposition components")
    require(frame.exit_code == frame.result & 255,
            "successful result/exit publication")
    try:
        envelope = compilation.decode(frame.omgcomp)
    except compilation.CompilationError as error:
        if getattr(error, "status", 251) == 252:
            raise RefinementResourceError(f"OMGCOMP1 custody: {error}") from error
        raise RefinementError(f"OMGCOMP1 custody: {error}") from error
    require(bool(envelope.sources) and bool(envelope.bundle_entries),
            "nonempty OMGCOMP1 source closure")
