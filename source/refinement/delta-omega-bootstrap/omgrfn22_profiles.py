#!/usr/bin/env python3
"""Deterministic independent canonical carrier for OMGRFN22 owner tests."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

from omgrfn22_bundle import pack
from omgrfn22_elf import reconstruct
from omgrfn22_source import HEADER, ORDER, ROWS

HERE = Path(__file__).resolve().parent
GATES = HERE.parents[3] / "source/on-ramp/omega-bootstrap/gates"


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


SOURCE_FIXTURE = load("omgrfn22_source_fixture",
                      GATES / "omgrsw11_record_array_fixture.py")
CKIR_FIXTURE = load("omgrfn22_ckir_fixture",
                    GATES / "delta-checked-ir-v19-fixture.py")


def canonical_witness(omgcomp: bytes) -> bytes:
    """Encode the frozen canonical relation without invoking either producer."""
    tables = {
        "units": ((0, 0, 0, 1690, 0),),
        "types": (
            (0, 0, 0, 0, 0, 0, 0, 0, 0, 0),
            (1, 1, 0, 0, 0, 0, 0, 0, 255, 0),
            (2, 2, 1, 0, 0, 0, 0, 0, 0xFFFF_FFFF, 0),
            (3, 3, 0, 0, 0, 0, 0, 0, 1, 0),
            (4, 10, 1, 0, 0, 0, 0, 0, 0xFFFF_FFFF, 0xFFFF_FFFF),
            (5, 10, 0, 0, 0, 0, 0, 0, 16_384, 0),
            (6, 5, 1, 0, 7, 16_384, 0, 0, 0, 0),
            (7, 4, 0, 0, 0, 0, 0, 0, 0, 0),
            (8, 4, 0, 0, 1, 0, 0, 0, 0, 0),
            (9, 4, 0, 0, 2, 0, 0, 0, 0, 0),
        ),
        "records": (
            (0, 0, 7, 0, 9, 17, 11, 1),
            (1, 0, 8, 9, 3, 260, 17, 0),
            (2, 0, 9, 12, 1, 1537, 4, 0),
        ),
        "fields": (
            (0, 0, 0, 1, 42, 3), (1, 0, 1, 1, 55, 5),
            (2, 0, 2, 1, 70, 6), (3, 0, 3, 1, 86, 5),
            (4, 0, 4, 2, 101, 6), (5, 0, 5, 4, 130, 5),
            (6, 0, 6, 4, 158, 3), (7, 0, 7, 4, 184, 13),
            (8, 0, 8, 4, 220, 14), (9, 1, 0, 6, 284, 4),
            (10, 1, 1, 5, 328, 5), (11, 1, 2, 3, 356, 13),
            (12, 2, 0, 8, 1544, 6),
        ),
        "machines": (
            (0, 0, 1, 2, 0xFFFF_FFFF, 0, 9, 0, 3, 406, 4, 601, 703, 0),
            (1, 0, 1, 1, 1, 9, 1, 3, 3, 1332, 8, 1378, 153, 0),
            (2, 0, 2, 2, 1, 10, 0, 6, 1, 1587, 3, 1608, 81, 0),
        ),
        "params": tuple(
            (index, 0, index, kind, start, length)
            for index, (kind, start, length) in enumerate((
                (1, 422, 3), (1, 431, 5), (1, 442, 6),
                (1, 454, 5), (2, 465, 6), (4, 490, 5),
                (4, 514, 3), (4, 536, 13), (4, 568, 14),
            ))) + ((9, 1, 0, 4, 1348, 5),),
        "blocks": (
            (0, 0, 0, 2, 0, 0, 0, 0, 601, 703),
            (1, 0, 1, 2, 0, 0, 712, 6, 730, 514),
            (2, 0, 2, 2, 0, 0, 1255, 4, 1271, 31),
            (3, 1, 0, 1, 0, 0, 0, 0, 1378, 153),
            (4, 1, 1, 1, 0, 0, 1460, 7, 1475, 24),
            (5, 1, 2, 1, 0, 0, 1510, 6, 1524, 5),
            (6, 2, 0, 2, 0, 0, 0, 0, 1608, 81),
        ),
        "calls": ((0, 0, 2, 0, 12, 1626, 32, 9, 0),
                  (1, 0, 2, 1, 12, 1676, 11, 1, 0)),
        "stores": tuple((index, 0, 1, 9, 10, index, index, kind)
                        for index, kind in enumerate((1, 1, 1, 1, 2, 4, 4, 4, 4))),
        "arguments": tuple((index, 0, index, kind, value, 0)
                           for index, (kind, value) in enumerate(zip(
                               (1, 1, 1, 1, 2, 4, 4, 4, 4),
                               (70, 1, 2, 3, 4, 5, 6, 7, 8)))),
    }
    payload = b"".join(ROWS[name].pack(*row)
                       for name in ORDER for row in tables[name])
    counts = tuple(len(tables[name]) for name in ORDER)
    header = HEADER.pack(
        b"OMGRSWB\0", 11, 0, 0, HEADER.size,
        HEADER.size + len(payload), len(omgcomp), *counts,
        2, 0, 1, 2, 0, 1, 16_384, 1, 2, 3, 4, 5, 6, 7, 8, 9,
        1, 0, 0, 0, 0, 0, 0, 0,
    )
    return header + payload


def components() -> tuple[bytes, bytes, bytes, bytes]:
    omgcomp = SOURCE_FIXTURE.encode(SOURCE_FIXTURE.CANONICAL)
    witness = canonical_witness(omgcomp)
    ckir = CKIR_FIXTURE.encode(CKIR_FIXTURE.tables())
    return omgcomp, witness, ckir, reconstruct(ckir)


def canonical() -> bytes:
    return pack(*components())
