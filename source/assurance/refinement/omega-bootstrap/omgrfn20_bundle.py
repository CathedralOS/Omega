#!/usr/bin/env python3
"""Untrusted exact-component OMGRFN20 packer."""

from __future__ import annotations

import struct

from omgrfn20_frame import (
    HEADER, MAGIC, MAX_CKIR, MAX_FRAME, MAX_OMGCOMP, MAX_WITNESS, VERSION,
)


def pack(omgcomp: bytes, witness: bytes, ckir: bytes) -> bytes:
    specs = ((omgcomp, MAX_OMGCOMP, b"OMGCOMP\0", 3),
             (witness, MAX_WITNESS, b"OMGRSW9\0", 9),
             (ckir, MAX_CKIR, b"OMGCKIR\0", 17))
    for raw, ceiling, magic, major in specs:
        if not raw or len(raw) > ceiling or len(raw) < 12 or raw[:8] != magic \
                or struct.unpack_from("<HH", raw, 8) != (major, 0):
            raise ValueError("exact OMGRFN20 component required")
    total = HEADER.size + len(omgcomp) + len(witness) + len(ckir)
    if total > MAX_FRAME:
        raise ValueError("OMGRFN20 whole-frame ceiling")
    return HEADER.pack(MAGIC, VERSION, 0, 0, HEADER.size, total,
                       len(omgcomp), len(witness), len(ckir), 0) \
        + omgcomp + witness + ckir
