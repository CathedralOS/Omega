#!/usr/bin/env python3
"""Responsibility-local Python controls for the structural OMGRFN19 owners."""

from __future__ import annotations

import importlib.util
import struct
import subprocess
import sys
from pathlib import Path

from omgrfn19_bundle import pack
from omgrfn19_frame import HEADER, MAX_FRAME, MAX_OMGCOMP, MAX_WITNESS
from omgrfn19_witness import ROWS, decode


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
REFERENCE_PATH = (
    ROOT / "source/on-ramp/omega-bootstrap/gates/omgrsw9_provider_plan_reference.py"
)
OWNERS = {name: HERE / f"omgrfn19-{name}.py" for name in ("r1", "r2", "r3", "r4", "r5")}


def load_reference():
    spec = importlib.util.spec_from_file_location("omgrsw9_v9_reference", REFERENCE_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load OMGRSW9 reference")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def replace_u16(raw: bytes, offset: int, value: int) -> bytes:
    changed = bytearray(raw)
    struct.pack_into("<H", changed, offset, value)
    return bytes(changed)


def replace_u32(raw: bytes, offset: int, value: int) -> bytes:
    changed = bytearray(raw)
    struct.pack_into("<I", changed, offset, value)
    return bytes(changed)


def mutate_row(witness: bytes, table: str, row: int, word: int, value: int) -> bytes:
    parsed = decode(witness)
    start, _ = parsed.offsets[table]
    return replace_u32(witness, start + row * ROWS[table].size + word * 4, value)


def run(owner: str, raw: bytes, expected: int, name: str) -> None:
    result = subprocess.run(
        [sys.executable, "-B", str(OWNERS[owner])], input=raw,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False, timeout=5,
    )
    if result.returncode != expected:
        raise RuntimeError(
            f"{name}/{owner}: status {result.returncode}, expected {expected}: "
            f"{result.stderr.decode('utf-8', 'replace')}"
        )
    if result.stdout:
        raise RuntimeError(f"{name}/{owner}: owner published stdout")


def main() -> None:
    reference = load_reference()
    envelope = reference.encode_envelope(reference.fixture_contents())
    witness = reference.encode_witness(envelope)
    canonical = pack(envelope, witness)

    for owner in OWNERS:
        run(owner, canonical, 0, "canonical")

    # R1 owns carrier/component identity, extents, ceilings, structural input, and EOF.
    r1_cases = {
        "outer-magic": b"X" + canonical[1:],
        "outer-version": replace_u16(canonical, 8, 18),
        "outer-eof": canonical + b"x",
        "component-extent": replace_u32(canonical, 20, len(envelope) + 1),
        "input-version": replace_u16(canonical, HEADER.size + 8, 2),
        "witness-version": replace_u16(canonical, HEADER.size + len(envelope) + 8, 8),
    }
    for name, raw in r1_cases.items():
        run("r1", raw, 251, name)

    # The header checks each declared component ceiling before slicing it.
    run("r1", replace_u32(canonical, 20, MAX_OMGCOMP + 1), 252,
        "input-component-resource")
    run("r1", replace_u32(canonical, 24, MAX_WITNESS + 1), 252,
        "witness-component-resource")
    oversized = canonical + bytes(MAX_FRAME + 1 - len(canonical))
    run("r1", oversized, 252, "whole-frame-resource")

    local = {
        "r2": ("selection-provenance", mutate_row(witness, "selections", 0, 7, 0)),
        "r3": ("requirement-result", mutate_row(witness, "requirements", 3, 8, 0)),
        "r4": ("helper-rank", mutate_row(witness, "helpers", 0, 8, 2)),
        "r5": ("plan-incomplete", mutate_row(witness, "plans", 0, 8, 0)),
    }
    for owner, (name, changed_witness) in local.items():
        run(owner, pack(envelope, changed_witness), 251, name)

    # A second tooth per semantic owner catches an independently owned join.
    second = {
        "r2": ("selection-span", mutate_row(witness, "selections", 0, 6, 1)),
        "r3": ("reach-trait", mutate_row(witness, "reaches", 4, 2, 1)),
        "r4": ("ordinary-call-target", mutate_row(witness, "ordinary_calls", 0, 4, 1)),
        "r5": ("requirement-call-target",
               mutate_row(witness, "requirement_calls", 4, 4, 4)),
    }
    for owner, (name, changed_witness) in second.items():
        run(owner, pack(envelope, changed_witness), 251, name)

    # R5 owns the exact witness publication extent and its inherited ceiling.
    run("r5", pack(envelope, witness[:-1]), 251, "witness-truncated-eof")
    run("r5", pack(envelope, witness + b"x"), 251, "witness-trailing-eof")
    run("r5", replace_u32(canonical, 24, MAX_WITNESS + 1), 252,
        "witness-publication-resource")

    print("OMGRFN19 modular Python owners: positive, local negatives, resources PASS")


if __name__ == "__main__":
    main()
