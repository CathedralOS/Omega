#!/usr/bin/env python3
"""Deterministic untrusted carriers for cheap OMGRFN17 owner joins."""

from __future__ import annotations

import importlib.util
import struct
import sys
from pathlib import Path

from omgrfn17_bundle import pack
from omgrfn17_elf import reconstruct
from omgrfn17_source import COUNT_NAMES, WIDTHS

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
GATES = REPO / "source/on-ramp/omega-bootstrap/gates"
sys.path[:0] = [str(GATES), str(REPO / "source/on-ramp/omega-bootstrap/compiler")]
import shared_byte_view_resolution_fixture as shared  # noqa: E402


def load_fixture():
    path = GATES / "delta-checked-ir-v15-fixture.py"
    spec = importlib.util.spec_from_file_location("omgrfn17_profile_fixture", path)
    if spec is None or spec.loader is None:
        raise ValueError("cannot load CKIR15 fixture")
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    return module


def source(literal: str) -> bytes:
    return f'''module app;
data Probe {{ result: u8; }}
machine Probe::run(&mut self) -> u8 {{
    self.result = 70;
    transition {{ _ -> inspect(true, "{literal}", false) }}
    state inspect(&mut self, before: bool, view: &[u8], after: bool) {{
        transition view.len > 0 {{
            true -> emit(before, view[0], view[1..], after)
            false -> finish(before, after)
        }}
    }}
    state emit(&mut self, before: bool, head: u8, view: &[u8], after: bool) {{
        self.result = head;
        transition view.len > 0 {{
            true -> emit(before, view[0], view[1..], after)
            false -> finish(before, after)
        }}
    }}
    state finish(&mut self, before: bool, after: bool) {{ self.result }}
}}
'''.encode("ascii")


def witness(source_bytes: bytes) -> bytes:
    module_at = source_bytes.index(b"app")
    owner_at = source_bytes.index(b"Probe")
    machine_at = source_bytes.index(b"run")
    rows = {name: [] for name, _ in WIDTHS}
    rows["units"] = [struct.pack("<9I", 0, 0, 0, module_at, 3, 0, 0, 0, 0)]
    rows["declarations"] = [
        struct.pack("<IBBH5I", 0, 1, 0, 0, 0, 0, owner_at, 5, 0),
        struct.pack("<IBBH5I", 1, 2, 0, 0, 0, 1, machine_at, 3, 0),
    ]
    rows["records"] = [struct.pack("<6I", 0, 0, 0, 0, 0, 0)]
    rows["machines"] = [struct.pack("<10I", 0, 1, 0, 0, 0, 0, 0, 0, 0, 0)]
    counts = {"sources": 1, "selected": 0, "reserved": 0}
    counts.update({name: len(rows[name]) for name, _ in WIDTHS})
    payload = b"".join(row for name, _ in WIDTHS for row in rows[name])
    return struct.pack("<8s4H17I", b"OMGRSW4\0", 4, 0, 0, 84,
                       84 + len(payload),
                       *(counts.get(name, 0) for name in COUNT_NAMES)) + payload


def profiles() -> dict[str, bytes]:
    fixture = load_fixture()
    definitions = {
        "recurrent": ("FG", (70, 71), 71),
        "one-byte": ("F", (70,), 70),
        "empty": ("", (), 70),
    }
    result: dict[str, bytes] = {}
    for name, (literal, values, expected) in definitions.items():
        authored = source(literal)
        omgcomp = shared.encode(authored)
        normalized = witness(authored)
        if normalized[:12] != b"OMGRSW4\0\x04\0\0\0":
            raise ValueError(f"{name} resolver did not publish exact OMGRSW4")
        tables = fixture.tables(values)
        if expected != 70:
            row = tables["operations"][5]
            tables["operations"][5] = row[:10] + (expected,) + row[11:]
        ckir = fixture.encode(tables)
        result[name] = pack(omgcomp, normalized, ckir,
                            reconstruct(ckir), expected)
    return result
