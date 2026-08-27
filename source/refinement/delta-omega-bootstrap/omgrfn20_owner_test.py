#!/usr/bin/env python3
"""Responsibility-local controls for the modular OMGRFN20 owners."""

from __future__ import annotations

import importlib.util
import struct
import subprocess
import sys
from pathlib import Path

from omgrfn19_witness import ROWS as WITNESS_ROWS, decode as decode_witness
from omgrfn20_bundle import pack
from omgrfn20_frame import HEADER, MAX_CKIR, MAX_FRAME, MAX_OMGCOMP, MAX_WITNESS

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
GATES = ROOT / "source/on-ramp/omega-bootstrap/gates"
OWNERS = {name: HERE / f"omgrfn20-{name}.py" for name in ("r1", "r2", "r3", "r4", "r5")}
sys.path.insert(0, str(GATES))


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def u16(raw: bytes, offset: int, value: int) -> bytes:
    changed = bytearray(raw)
    struct.pack_into("<H", changed, offset, value)
    return bytes(changed)


def u32(raw: bytes, offset: int, value: int) -> bytes:
    changed = bytearray(raw)
    struct.pack_into("<I", changed, offset, value)
    return bytes(changed)


def witness_row(raw: bytes, table: str, row: int, word: int, value: int) -> bytes:
    parsed = decode_witness(raw)
    start, _ = parsed.offsets[table]
    return u32(raw, start + row * WITNESS_ROWS[table].size + 4 * word, value)


def observe(owner: str, raw: bytes, expected: int, label: str) -> None:
    result = subprocess.run(
        [sys.executable, "-B", str(OWNERS[owner])], input=raw,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=5,
    )
    if result.returncode != expected:
        raise RuntimeError(
            f"{label}/{owner}: {result.returncode} != {expected}: "
            f"{result.stderr.decode('utf-8', 'replace')}"
        )
    if result.stdout:
        raise RuntimeError(f"{label}/{owner}: rejection/acceptance published stdout")


def main() -> None:
    v9 = load("omgrfn20_v9_reference", GATES / "omgrsw9_provider_plan_reference.py")
    fixture = load("omgrfn20_ckir17_fixture", GATES / "delta-checked-ir-v17-fixture.py")
    envelope = v9.encode_envelope(v9.fixture_contents())
    witness = v9.encode_witness(envelope)
    base_tables = fixture.tables()
    ckir = fixture.encode(base_tables)
    canonical = pack(envelope, witness, ckir)

    for owner in OWNERS:
        observe(owner, canonical, 0, "canonical")

    # R1 framing, component identity, EOF, and all declared ceilings.
    r1 = {
        "outer-magic": b"X" + canonical[1:],
        "outer-version": u16(canonical, 8, 19),
        "outer-eof": canonical + b"x",
        "component-extent": u32(canonical, 20, len(envelope) + 1),
        "omg-major": u16(canonical, HEADER.size + 8, 2),
        "witness-major": u16(canonical, HEADER.size + len(envelope) + 8, 8),
        "ckir-major": u16(canonical, HEADER.size + len(envelope) + len(witness) + 8, 16),
    }
    for label, raw in r1.items():
        observe("r1", raw, 251, label)
    for label, offset, value in (
        ("omg-resource", 20, MAX_OMGCOMP + 1),
        ("witness-resource", 24, MAX_WITNESS + 1),
        ("ckir-resource", 28, MAX_CKIR + 1),
    ):
        observe("r1", u32(canonical, offset, value), 252, label)
    observe("r1", canonical + bytes(MAX_FRAME + 1 - len(canonical)), 252,
            "whole-frame-resource")

    # R2 owns exact explicit-cast source and the selected structural rows.
    changed_contents = dict(v9.fixture_contents())
    portable = v9.LABELS[2]
    changed_contents[portable] = changed_contents[portable].replace(
        b"output as i32", b"output as u32", 1)
    changed_envelope = v9.encode_envelope(changed_contents)
    observe("r2", pack(changed_envelope, witness, ckir), 251,
            "explicit-cast-source")
    observe("r2", pack(envelope,
                        witness_row(witness, "plan_rows", 1, 4, 0), ckir),
            251, "selected-write-plan-row")
    observe("r2", pack(envelope,
                        witness_row(witness, "requirement_calls", 0, 4, 3), ckir),
            251, "helper-requirement-target")

    # R3 complete CKIR relation teeth.
    def changed_ckir(table: str, row: int, field: int, value: int) -> bytes:
        tables = fixture.copy.deepcopy(base_tables)
        tables[table][row] = fixture.replace(tables[table][row], field, value)
        return fixture.encode(tables)

    for label, raw in (
        ("service-provider", changed_ckir("services", 0, 2, 1)),
        ("ranking-measure", changed_ckir("rankings", 0, 3, 2)),
        ("static-receiver", changed_ckir("machines", 1, 2, 1)),
        ("boundary-binding", changed_ckir("boundary_targets", 0, 8, 1)),
    ):
        observe("r3", pack(envelope, witness, raw), 251, label)

    # R4 cross-component identities and explicit authored widening.
    observe("r4", pack(envelope,
                        witness_row(witness, "plan_rows", 4, 4, 3), ckir),
            251, "plan-to-boundary-candidate")
    observe("r4", pack(envelope, witness,
                        changed_ckir("operations", 3, 3, 21)),
            251, "missing-explicit-widen")
    observe("r4", pack(envelope, witness,
                        changed_ckir("boundary_targets", 0, 2, 5)),
            251, "boundary-requirement")

    # R5 observes exact traces beyond structural well-formedness: changing the
    # newline literal remains structurally valid but changes both newline paths.
    newline = changed_ckir("operations", 8, 10, 11)
    module = fixture.ir17.decode(newline)  # prove this is not an R3 rejection
    if fixture.ir17.invoke(module, "write_line", b"") != (11,):
        raise RuntimeError("newline mutation did not isolate trace meaning")
    observe("r5", pack(envelope, witness, newline), 251, "newline-trace")
    observe("r5", pack(envelope, witness, ckir + b"x"), 251, "ckir-eof")
    exhausted = fixture.mutate_count(ckir, "services", fixture.ir17.CEILINGS["services"] + 1)
    observe("r5", pack(envelope, witness, exhausted), 252, "ckir-table-resource")

    print("OMGRFN20 modular owners: positive, local negatives, resources PASS")


if __name__ == "__main__":
    main()
