#!/usr/bin/env python3
"""CKIR15 loading and view-family facts for OMGRFN17 owners."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

from omgrfn17_frame import RefinementError, require

HERE = Path(__file__).resolve().parent
GATES = HERE.parents[2] / "source/on-ramp/omega-bootstrap/gates"
sys.path.insert(0, str(GATES))


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RefinementError(f"cannot load {path.name}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


IR15 = load("omgrfn17_checked_ir_v15_reference", GATES / "checked_ir_v15_reference.py")
V5 = IR15.v5


def decode(contents: bytes):
    try:
        return IR15.decode(contents)
    except V5.Ckir5ResourceError:
        raise
    except Exception as error:
        raise RefinementError(f"CKIR15 structure: {error}") from error


def selected(module) -> dict[int, int]:
    return IR15.selected_counts(module)


def producer_decode(contents: bytes):
    """Rebuild R3's producer-facing conclusion without importing R5's verdict."""
    try:
        module = V5.decode(contents, expected_major=15,
                           capabilities=V5.SCHEMA_CAPABILITIES[15])
    except V5.Ckir5ResourceError:
        raise
    except Exception as error:
        raise RefinementError(f"CKIR15 producer structure: {error}") from error
    counts = selected(module)
    require(counts[23] == counts[24] == counts[25] and counts[23] >= 2,
            "complete recurrent guarded-view family")
    require(counts[22] in (0, 1), "StaticByteView is optional and unique")
    require(sum(row[3] == 1 for row in module.tables["blocks"]) == counts[23],
            "one synthetic owner per guarded edge")
    arithmetic = IR15.selected_arithmetic_counts(module)
    require(not any(arithmetic.values()) or arithmetic == {8: 1, 26: 1, 27: 1},
            "optional full-width arithmetic is complete when present")
    return module


def static_view_bytes(module) -> bytes | None:
    rows = [row for row in module.tables["operations"] if row[3] == 22]
    if not rows:
        return None
    require(len(rows) == 1, "unique StaticByteView")
    root = rows[0][10]
    constants = module.tables["constants"]
    children = module.tables["constant_children"]
    require(root < len(constants), "StaticByteView root")
    node = constants[root]
    start, count = node[2], node[3]
    require(start <= len(children) and count <= len(children) - start,
            "StaticByteView children")
    values = []
    types = module.tables["types"]
    for child in children[start:start + count]:
        child_id = child[0]
        require(child_id < len(constants), "StaticByteView child ID")
        scalar = constants[child_id]
        require(scalar[1] < len(types) and types[scalar[1]][1] == 1
                and types[scalar[1]][6:8] == (0, 255) and scalar[3] == 0,
                "StaticByteView u8 child")
        values.append(scalar[4])
    return bytes(values)
