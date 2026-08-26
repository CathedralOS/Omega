#!/usr/bin/env python3
"""Independent CKIR14 loading and arithmetic relation helpers for OMGRFN16."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

from omgrfn16_frame import RefinementError, require


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
GATES = REPO / "bootstrap/omega-bootstrap/gates"
sys.path.insert(0, str(GATES))


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RefinementError(f"cannot load {path.name}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


IR14 = load("omgrfn16_checked_ir_v14_reference", GATES / "checked_ir_v14_reference.py")
V5 = IR14.v5
FULL_U32 = (2, 1, 0, 0, 0, 0, 0xFFFF_FFFF)
ARITHMETIC = {8: "add", 26: "subtract", 27: "multiply"}


def decode(contents: bytes):
    try:
        return IR14.decode(contents)
    except V5.Ckir5ResourceError:
        raise
    except Exception as error:
        raise RefinementError(f"CKIR14 structure: {error}") from error


def check_arithmetic_closure(module) -> None:
    selected = 0
    values = module.value_types
    types = module.tables["types"]
    operands = module.tables["operands"]
    for operation in module.tables["operations"]:
        opcode = operation[3]
        if opcode not in ARITHMETIC:
            continue
        selected += 1
        result_type = operation[7]
        start, count = operation[8:10]
        require(count == 2, "arithmetic arity")
        left, right = operands[start][0], operands[start + 1][0]
        require(types[result_type][1:] == FULL_U32, "full-u32 semantic words")
        require(values[left] == result_type and values[right] == result_type,
                "same-carrier arithmetic operands")
    require(selected > 0, "at least one selected arithmetic row")
