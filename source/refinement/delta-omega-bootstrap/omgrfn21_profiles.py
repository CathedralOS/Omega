#!/usr/bin/env python3
"""Deterministic untrusted canonical carrier for OMGRFN21 owner tests."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

from omgrfn21_bundle import pack
from omgrfn21_elf import reconstruct

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


SOURCE_FIXTURE = load("omgrfn21_source_fixture",
                      GATES / "omgrsw10_u64_buffer_fixture.py")
CKIR_FIXTURE = load("omgrfn21_ckir_fixture",
                    GATES / "delta-checked-ir-v18-fixture.py")


def components() -> tuple[bytes, bytes, bytes, bytes]:
    source = SOURCE_FIXTURE.CANONICAL.encode("ascii")
    omgcomp = SOURCE_FIXTURE.encode_compilation(SOURCE_FIXTURE.CANONICAL)
    witness = SOURCE_FIXTURE.encode_witness(omgcomp, source)
    ckir = CKIR_FIXTURE.encode(CKIR_FIXTURE.tables())
    return omgcomp, witness, ckir, reconstruct(ckir)


def canonical() -> bytes:
    return pack(*components())
