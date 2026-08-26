#!/usr/bin/env python3
"""Shared bounded framing primitives for independent OMGRFN16 owners."""

from __future__ import annotations

import dataclasses
import struct
import sys
from pathlib import Path

from omgrfn6_bundle import (
    HEADER, MAX_CKIR, MAX_ELF, MAX_FRAME, MAX_OMGCOMP, MAX_WITNESS, NO_RESULT,
)


HERE = Path(__file__).resolve().parent
COMPILER = HERE.parents[3] / "bootstrap/omega-bootstrap/compiler"
sys.path.insert(0, str(COMPILER))
import omega_bootstrap_compilation as compilation  # noqa: E402


MAGIC = b"OMGRFNG\0"
OUTER_VERSION = 16
FLAG_PROPOSITION = 1
FLAG_TRAP = 2


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

    @property
    def traps(self) -> bool:
        return self.flags == FLAG_PROPOSITION | FLAG_TRAP


def split(raw: bytes) -> Frame:
    """Safely split components without granting any R1 acceptance claim."""
    if len(raw) > MAX_FRAME:
        raise RefinementResourceError("whole-frame ceiling")
    require(len(raw) >= HEADER.size, "truncated frame header")
    fields = HEADER.unpack_from(raw)
    magic, version, flags, omg_len, witness_len, ckir_len, elf_len, result, exit_code = fields
    ceilings = (
        (omg_len, MAX_OMGCOMP, "OMGCOMP"),
        (witness_len, MAX_WITNESS, "OMGRSW7"),
        (ckir_len, MAX_CKIR, "CKIR14"),
        (elf_len, MAX_ELF, "ELF"),
    )
    for length, ceiling, label in ceilings:
        if length > ceiling:
            raise RefinementResourceError(f"{label} ceiling")
    total = HEADER.size
    for length, _, _ in ceilings:
        require(length <= len(raw) - total, "component extent")
        total += length
    require(total == len(raw), "exact EOF")
    cursor = HEADER.size
    components: list[bytes] = []
    for length, _, _ in ceilings:
        components.append(raw[cursor:cursor + length])
        cursor += length
    return Frame(
        raw, magic, version, flags,
        components[0], components[1], components[2], components[3],
        result, exit_code,
    )


def check_r1(frame: Frame) -> None:
    require(frame.magic == MAGIC, "OMGRFNG identity")
    require(frame.version == OUTER_VERSION, "outer version 16")
    require(all((frame.omgcomp, frame.witness, frame.ckir, frame.elf)),
            "nonempty proposition components")
    require(frame.flags in (FLAG_PROPOSITION, FLAG_PROPOSITION | FLAG_TRAP),
            "exact proposition flags")
    if frame.flags == FLAG_PROPOSITION:
        require(frame.exit_code == frame.result & 255, "successful result/exit")
    else:
        require(frame.result == NO_RESULT and frame.exit_code == NO_RESULT,
                "trapping no-result sentinels")
    try:
        envelope = compilation.decode(frame.omgcomp)
    except Exception as error:
        raise RefinementError(f"complete OMGCOMP1 custody: {error}") from error
    require(getattr(envelope, "version", 1) == 1, "OMGCOMP1 identity")
    require(bool(envelope.sources) and bool(envelope.bundle_entries),
            "nonempty OMGCOMP1 source closure")
