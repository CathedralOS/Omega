#!/usr/bin/env python3
"""Deterministic untrusted OMGRFN18 reference carriers and boundaries."""

from __future__ import annotations

import struct
import sys
from pathlib import Path

from omgrfn18_bundle import pack
from omgrfn18_ckir import V5
from omgrfn18_source import COUNT_NAMES, WIDTHS
from omgrfn18_u64 import U64

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
GATES = REPO / "bootstrap/omega-bootstrap/gates"
sys.path.insert(0, str(GATES))
import shared_byte_view_resolution_fixture as shared  # noqa: E402

NO_ID = 0xFFFF_FFFF


def source(stored: int, ceiling: int) -> bytes:
    bound = ceiling - 1
    return f'''module app;
data Probe {{ stored: u64; }}
machine Probe::echo(&self, value: u64 [0..={bound}]) -> u64 [0..={bound}] {{ value }}
machine Probe::run(&mut self) -> u8 {{
    self.stored = {stored};
    transition self.stored < {ceiling} {{
        true -> bounded(self.stored)
        false -> failed()
    }}
    state bounded(&mut self, value: u64 [0..={bound}]) {{ self.stored = self.echo(value); 70 }}
    state failed(&mut self) {{ 0 }}
}}
'''.encode("ascii")


def witness(authored: bytes, ceiling: int) -> bytes:
    module_at = authored.index(b"app")
    owner_at = authored.index(b"Probe")
    echo_at = authored.index(b"echo")
    run_at = authored.index(b"run")
    stored_at = authored.index(b"stored")
    echo_value_at = authored.index(b"value")
    bounded_at = authored.index(b"bounded")
    bounded_value_at = authored.index(b"value", bounded_at)
    bound = U64.from_int(ceiling - 1)
    rows = {name: [] for name, _ in WIDTHS}
    rows["units"] = [struct.pack("<9I", 0, 0, 0, module_at, 3, 0, 0, 0, 0)]
    rows["declarations"] = [
        struct.pack("<IBBH5I", 0, 1, 0, 0, 0, 0, owner_at, 5, 0),
        struct.pack("<IBBH5I", 1, 2, 0, 0, 0, 1, echo_at, 4, 0),
        struct.pack("<IBBH5I", 2, 2, 0, 0, 0, 2, run_at, 3, 1),
    ]
    rows["types"] = [
        struct.pack("<IBBHIIII", 0, 4, 0, 0, 0, 0, 0, 0),
        struct.pack("<IBBHIIII", 1, 1, 0, 0, 0, 0, 0, 255),
        struct.pack("<IBBHIIII", 2, 10, 0, 0, 0, 0, 0xFFFF_FFFF, 0xFFFF_FFFF),
        struct.pack("<IBBHIIII", 3, 10, 0, 0, 0, 0, bound.lo, bound.hi),
    ]
    rows["records"] = [struct.pack("<5IB3x", 0, 0, 0, 0, 1, 0)]
    rows["fields"] = [struct.pack("<6I", 0, 0, 0, 2, stored_at, 6)]
    rows["machines"] = [
        struct.pack("<3IBBH6I", 0, 1, 0, 1, 0, 0, 3, 0, 1, 0, 1, 0),
        struct.pack("<3IBBH6I", 1, 2, 0, 2, 0, 0, 1, 1, 0, 1, 3, 1),
    ]
    rows["machine_parameters"] = [
        struct.pack("<6I", 0, 0, 0, 3, echo_value_at, 5),
    ]
    rows["blocks"] = [
        struct.pack("<3IBBH6I", 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0),
        struct.pack("<3IBBH6I", 1, 1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0),
        struct.pack("<3IBBH6I", 2, 1, 1, 2, 0, 0, 0, 0, 0, 1, 0, 0),
        struct.pack("<3IBBH6I", 3, 1, 2, 2, 0, 0, 0, 0, 0, 0, 0, 0),
    ]
    rows["block_parameters"] = [
        struct.pack("<6I", 0, 2, 0, 3, bounded_value_at, 5),
    ]
    counts = {"sources": 1, "selected": 1, "reserved": 0}
    counts.update({name: len(rows[name]) for name, _ in WIDTHS})
    payload = b"".join(row for name, _ in WIDTHS for row in rows[name])
    return struct.pack("<8s4H17I", b"OMGRSW8\0", 8, 0, 0, 84,
                       84 + len(payload),
                       *(counts.get(name, 0) for name in COUNT_NAMES)) + payload


def replace(row: tuple[int, ...], index: int, value: int) -> tuple[int, ...]:
    return row[:index] + (value,) + row[index + 1:]


def ckir_tables(stored: int, ceiling: int) -> dict[str, list[tuple[int, ...]]]:
    stored_words, ceiling_words = U64.from_int(stored), U64.from_int(ceiling)
    bound = ceiling_words.predecessor()
    tables = {name: [] for name in V5.TABLE_ORDER}
    tables["types"] = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 1, 0, 0, 0, 0, 0, 255),
        (2, 3, 0, 0, 0, 0, 0, 1),
        (3, 8, 0, 0, 0, 0, 0xFFFF_FFFF, 0xFFFF_FFFF),
        (4, 8, 0, 0, 0, 0, bound.lo, bound.hi),
    ]
    tables["records"] = [(0, 0, 0, 1, 0, 0, 0, 0)]
    tables["fields"] = [(0, 0, 0, 3)]
    tables["machines"] = [
        (0, 0, 2, 0, 0, 1, 0, 0, 0, 3, 0),
        (1, 0, 1, 0, 0, 4, 0, 1, 3, 1, 3),
    ]
    tables["machine_params"] = [(0, 1, 0, 4, 0)]
    tables["blocks"] = [
        (0, 0, 2, 0, 0, 0, 0, 0, 8, 0),
        (1, 0, 2, 0, 0, 0, 1, 8, 5, 1),
        (2, 0, 2, 0, 0, 1, 0, 13, 1, 2),
        (3, 1, 1, 0, 0, 1, 0, 14, 0, 3),
    ]
    tables["block_params"] = [(0, 1, 0, 4, 1)]
    tables["operations"] = [
        (0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 0),
        (1, 0, 0, 3, 2, 0, 1, 3, 0, 1, 0, 0),
        (2, 0, 0, 1, 1, 0, 2, 3, 1, 0, stored_words.lo, stored_words.hi),
        (3, 0, 0, 6, 0, 0, NO_ID, NO_ID, 1, 2, 0, 0),
        (4, 0, 0, 5, 1, 0, 3, 3, 3, 1, 0, 0),
        (5, 0, 0, 1, 1, 0, 4, 3, 4, 0, ceiling_words.lo, ceiling_words.hi),
        (6, 0, 0, 9, 1, 0, 5, 2, 4, 2, 0, 0),
        (7, 0, 0, 5, 1, 0, 6, 3, 6, 1, 0, 0),
        (8, 0, 1, 2, 2, 0, 2, 0, 7, 0, 0, 0),
        (9, 0, 1, 3, 2, 0, 3, 3, 7, 1, 0, 0),
        (10, 0, 1, 10, 1, 0, 7, 4, 8, 2, 1, 0),
        (11, 0, 1, 6, 0, 0, NO_ID, NO_ID, 10, 2, 0, 0),
        (12, 0, 1, 1, 1, 0, 8, 1, 12, 0, 70, 0),
        (13, 0, 2, 1, 1, 0, 9, 1, 12, 0, 0, 0),
    ]
    tables["operands"] = [
        (0,), (1,), (2,), (1,), (3,), (4,), (1,), (2,), (2,), (1,), (3,), (7,),
        (6,),
    ]
    tables["terminators"] = [
        (0, 0, 0, 2, 0, 0, 5, 1, 12, 1, 2, 13, 0, 0, 0),
        (1, 0, 1, 4, 0, 0, 8, NO_ID, 13, 0, NO_ID, 13, 0, 0, 0),
        (2, 0, 2, 4, 0, 0, 9, NO_ID, 13, 0, NO_ID, 13, 0, 0, 0),
        (3, 1, 3, 4, 0, 0, 0, NO_ID, 13, 0, NO_ID, 13, 0, 0, 0),
    ]
    return tables


def encode_ckir(tables: dict[str, list[tuple[int, ...]]], *, values: int = 10,
                places: int = 4) -> bytes:
    counts = {name: len(tables[name]) for name in V5.TABLE_ORDER}
    counts.update(values=values, places=places)
    payload = b"".join(V5.ROWS[name].pack(*row)
                       for name in V5.TABLE_ORDER for row in tables[name])
    return V5.HEADER.pack(
        b"OMGCKIR\0", 16, 0, 1, 1, 0, V5.HEADER.size + len(payload),
        *(counts[name] for name in V5.COUNT_NAMES),
    ) + payload


def ckir(stored: int, ceiling: int) -> bytes:
    return encode_ckir(ckir_tables(stored, ceiling))


def definitions() -> dict[str, tuple[int, int]]:
    return {
        "borrow": (0x00000001_FFFFFFFF, 0x00000002_00000000),
        "same-high": (0x00000001_00000001, 0x00000001_00000002),
        "bit63": (0x7FFF_FFFF_FFFF_FFFF, 0x8000_0000_0000_0000),
        "max-neighbor": (0xFFFF_FFFF_FFFF_FFFE, 0xFFFF_FFFF_FFFF_FFFF),
        "max-equal": (0xFFFF_FFFF_FFFF_FFFF, 0xFFFF_FFFF_FFFF_FFFF),
        "reversed-high": (0x8000_0000_0000_0000, 0x7FFF_FFFF_FFFF_FFFF),
    }


def profiles() -> dict[str, bytes]:
    from omgrfn18_elf import reconstruct

    result: dict[str, bytes] = {}
    for name, (stored, ceiling) in definitions().items():
        authored = source(stored, ceiling)
        omgcomp = shared.encode(authored)
        normalized = witness(authored, ceiling)
        checked = ckir(stored, ceiling)
        expected = 70 if stored < ceiling else 0
        result[name] = pack(omgcomp, normalized, checked,
                            reconstruct(checked), expected)
    return result
