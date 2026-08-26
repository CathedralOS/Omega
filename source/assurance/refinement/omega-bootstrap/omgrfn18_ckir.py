#!/usr/bin/env python3
"""CKIR16 loading and exact u64-Less observations for OMGRFN18."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

from omgrfn18_frame import RefinementError, RefinementResourceError, require
from omgrfn18_u64 import U64, bounds

HERE = Path(__file__).resolve().parent
GATES = HERE.parents[3] / "bootstrap/omega-bootstrap/gates"
sys.path.insert(0, str(GATES))


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RefinementError(f"cannot load {path.name}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


V5 = load("omgrfn18_checked_ir_v5_reference", GATES / "checked_ir_v5_reference.py")
SELECTOR_PATH = GATES / "checked_ir_v16_reference.py"
IR16 = load("omgrfn18_checked_ir_v16_reference", SELECTOR_PATH) if SELECTOR_PATH.exists() else None


def _translate_error(label: str, action):
    try:
        return action()
    except Exception as error:
        resource_types = [V5.Ckir5ResourceError]
        if IR16 is not None and hasattr(IR16, "Ckir16ResourceError"):
            resource_types.append(IR16.Ckir16ResourceError)
        if isinstance(error, tuple(resource_types)):
            raise RefinementResourceError(f"{label}: {error}") from error
        raise RefinementError(f"{label}: {error}") from error


def decode(contents: bytes):
    """R5's frozen-selector decode; fallback exists only during selector landing."""
    if IR16 is not None:
        return _translate_error("CKIR16 structure", lambda: IR16.decode(contents))
    return _translate_error(
        "CKIR16 structure",
        lambda: V5.decode(contents, expected_major=16,
                          capabilities=V5.SCHEMA_CAPABILITIES[16]),
    )


def producer_decode(contents: bytes):
    """R3 producer-facing decode without importing an R5 verdict."""
    module = _translate_error(
        "CKIR16 producer structure",
        lambda: V5.decode(contents, expected_major=16,
                          capabilities=V5.SCHEMA_CAPABILITIES[16]),
    )
    types = module.tables["types"]
    operations = module.tables["operations"]
    operands = module.tables["operands"]
    selected = []
    for operation in operations:
        if operation[3] != 9:
            continue
        values = [operands[index][0]
                  for index in range(operation[8], operation[8] + operation[9])]
        if (len(values) == 2
                and types[module.value_types[values[0]]][1] == 8
                and types[module.value_types[values[1]]][1] == 8):
            selected.append(operation)
    require(bool(selected), "complete direct u64 Less family")
    require(not any(op[3] in (4, 8, 12, 18, 19, 20, 26, 27)
                    for op in operations),
            "bounded slice excludes indexing, arithmetic, and other comparisons")
    return module


def _definitions(module) -> dict[int, tuple[int, ...]]:
    return {operation[6]: operation for operation in module.tables["operations"]
            if operation[4] == 1}


def _arguments(module, operation: tuple[int, ...]) -> tuple[int, ...]:
    return tuple(module.tables["operands"][index][0]
                 for index in range(operation[8], operation[8] + operation[9]))


def _field_load(module, value: int, definitions: dict[int, tuple[int, ...]]) -> int | None:
    load_op = definitions.get(value)
    if load_op is None or load_op[3] != 5:
        return None
    load_args = _arguments(module, load_op)
    if len(load_args) != 1:
        return None
    place_ops = {operation[6]: operation for operation in module.tables["operations"]
                 if operation[4] == 2}
    field_place = place_ops.get(load_args[0])
    if field_place is None or field_place[3] != 3:
        return None
    base = _arguments(module, field_place)
    if len(base) != 1 or place_ops.get(base[0], (0, 0, 0, 0))[3] != 2:
        return None
    return field_place[10]


def _constant(module, value: int, definitions: dict[int, tuple[int, ...]]) -> U64 | None:
    operation = definitions.get(value)
    if operation is None or operation[3] != 1:
        return None
    type_id = module.value_types[value]
    if module.tables["types"][type_id][1] != 8:
        return None
    return U64(operation[10], operation[11])


def check_lowering(module, source) -> None:
    """Join authored Less and true-edge fact custody to exact CKIR16 rows."""
    types = module.tables["types"]
    definitions = _definitions(module)
    operands = module.tables["operands"]
    candidates: list[tuple[tuple[int, ...], int, int, int]] = []
    for operation in module.tables["operations"]:
        if operation[3] != 9:
            continue
        args = _arguments(module, operation)
        if len(args) != 2:
            continue
        field_id = _field_load(module, args[0], definitions)
        right = _constant(module, args[1], definitions)
        if field_id is not None and right == source.less.ceiling:
            candidates.append((operation, args[0], args[1], field_id))
    require(len(candidates) == 1,
            "one authored direct field-load/literal Less in operand order")
    comparison, subject_value, _, field_id = candidates[0]

    branches = [term for term in module.tables["terminators"]
                if term[3] == 2 and term[6] == comparison[6]]
    require(len(branches) == 1, "Less result owns one authored branch")
    branch = branches[0]
    true_target, start, count = branch[7], branch[8], branch[9]
    require(true_target != V5.NO_ID and count == 1,
            "true edge carries one refined subject")
    passed = operands[start][0]
    require(_field_load(module, passed, definitions) == field_id
            or passed == subject_value,
            "true edge forwards the compared field identity")
    target = module.tables["blocks"][true_target]
    require(target[6] == 1, "true target has one refined parameter")
    parameter = module.tables["block_params"][target[5]]
    parameter_type = types[parameter[3]]
    require(parameter_type[1] == 8 and parameter_type[2] == 0,
            "true target parameter is unqualified u64")
    low, high = bounds(parameter_type)
    require(low == source.less.fact_low and high == source.less.fact_high,
            "true-edge predecessor/intersection fact custody")

    # The selected true state must exercise same-carrier call transport and
    # storage, but these operations are not allowed to become Less operands.
    true_ops = module.tables["operations"][target[7]:target[7] + target[8]]
    calls = [operation for operation in true_ops if operation[3] == 10]
    require(len(calls) == 1, "true state has one u64 echo Call")
    call = calls[0]
    call_args = _arguments(module, call)
    require(len(call_args) == 2
            and module.tables["types"][call[7]][1] == 8
            and module.tables["types"][module.value_types[call_args[1]]][1] == 8,
            "same-carrier u64 Call argument/result")
    stores = [operation for operation in true_ops if operation[3] == 6]
    require(any(_arguments(module, operation)[1] == call[6] for operation in stores),
            "u64 Call result reaches storage")


def interpret(module) -> int | None:
    runner = IR16.interpret if IR16 is not None else V5.interpret
    return _translate_error("CKIR16 execution", lambda: runner(module))
