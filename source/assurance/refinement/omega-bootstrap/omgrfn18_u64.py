#!/usr/bin/env python3
"""Exact unsigned two-word arithmetic for the OMGRFN18 assurance cut."""

from __future__ import annotations

import dataclasses


MASK32 = 0xFFFF_FFFF
MAX_U64 = 0xFFFF_FFFF_FFFF_FFFF


@dataclasses.dataclass(frozen=True, order=False)
class U64:
    """One unsigned u64 kept as two semantic u32 words.

    Host integer conversion is confined to serialization and tests.  Ordering
    and predecessor custody operate on the words directly so no signed-host
    interpretation can accidentally enter the refinement relation.
    """

    lo: int
    hi: int

    def __post_init__(self) -> None:
        if not (0 <= self.lo <= MASK32 and 0 <= self.hi <= MASK32):
            raise ValueError("u64 semantic word")

    @classmethod
    def from_int(cls, value: int) -> "U64":
        if not 0 <= value <= MAX_U64:
            raise ValueError("u64 magnitude")
        return cls(value & MASK32, value >> 32)

    def to_int(self) -> int:
        return self.lo | self.hi << 32

    def less(self, other: "U64") -> bool:
        return self.hi < other.hi or self.hi == other.hi and self.lo < other.lo

    def predecessor(self) -> "U64":
        if self.lo:
            return U64(self.lo - 1, self.hi)
        if self.hi:
            return U64(MASK32, self.hi - 1)
        raise ValueError("u64 zero has no predecessor")

    def in_closed(self, low: "U64", high: "U64") -> bool:
        return not self.less(low) and not high.less(self)


FULL_LOW = U64(0, 0)
FULL_HIGH = U64(MASK32, MASK32)


def words(value: int) -> tuple[int, int]:
    pair = U64.from_int(value)
    return pair.lo, pair.hi


def bounds(row: tuple[int, ...]) -> tuple[U64, U64]:
    """Decode a kind-local CKIR/OMGRSW u64 row's four endpoint words."""
    return U64(row[4], row[5]), U64(row[6], row[7])
