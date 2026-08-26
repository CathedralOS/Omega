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


def producer_decode(contents: bytes):
    """R3's producer-contract reconstruction, separate from R5's wrapper.

    The shared frozen table decoder supplies bounded row access, but this path
    independently rechecks every CKIR14-selected row and its producer-facing
    canonical fields rather than importing R5's acceptance conclusion.
    """
    try:
        module = V5.decode(
            contents, expected_major=14,
            capabilities=V5.SCHEMA_CAPABILITIES[14],
        )
    except V5.Ckir5ResourceError:
        raise
    except Exception as error:
        raise RefinementError(f"CKIR14 producer structure: {error}") from error
    check_producer_structure(module)
    return module


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


def check_producer_structure(module) -> None:
    check_arithmetic_closure(module)
    operations = module.tables["operations"]
    operands = module.tables["operands"]
    values = module.value_types
    definitions: dict[int, tuple[int, ...]] = {}
    selected_results: set[int] = set()
    for ordinal, operation in enumerate(operations):
        require(operation[0] == ordinal, "dense operation IDs")
        result = operation[6]
        if operation[4] == 1 and result != 0xFFFF_FFFF:
            require(result not in definitions, "unique value definition")
            definitions[result] = operation
        if operation[3] not in ARITHMETIC:
            continue
        require(operation[4:6] == (1, 0) and operation[10:12] == (0, 0),
                "canonical arithmetic flags/immediates")
        start, count = operation[8:10]
        require(start <= len(operands) and count <= len(operands) - start,
                "arithmetic operand extent")
        selected_results.add(result)
        for index in range(start, start + count):
            operand = operands[index][0]
            require(operand < result and operand < len(values),
                    "recursive postorder visibility")
    # Every selected result is either a later selected operand or one authored
    # expression root consumed by an inherited context operation/terminator.
    consumed = {
        operands[index][0]
        for operation in operations if operation[3] in ARITHMETIC
        for index in range(operation[8], operation[8] + operation[9])
    }
    require(bool(selected_results - consumed), "selected arithmetic expression root")


def _value_definition(module) -> dict[int, tuple[int, ...]]:
    return {
        operation[6]: operation for operation in module.tables["operations"]
        if operation[4] == 1 and operation[6] != 0xFFFF_FFFF
    }


def leaf_value_names(module, witness_names: dict[str, dict[int, bytes]]) -> dict[int, bytes]:
    """Resolve CKIR value IDs back to independently checked source leaf names."""
    result: dict[int, bytes] = {}
    for table, witness_table in (
        ("machine_params", "machine_parameters"),
        ("block_params", "block_parameters"),
    ):
        names = witness_names[witness_table]
        for row in module.tables[table]:
            if row[0] in names:
                result[row[4]] = names[row[0]]

    place_names: dict[int, bytes | None] = {}
    operands = module.tables["operands"]
    field_names = witness_names["fields"]
    for operation in module.tables["operations"]:
        if operation[4] != 2:
            continue
        opcode, place = operation[3], operation[6]
        if opcode == 2:
            place_names[place] = None
        elif opcode == 3:
            require(operation[9] == 1, "field-place arity")
            parent = operands[operation[8]][0]
            require(parent in place_names, "field-place parent")
            require(operation[10] in field_names, "field-place witness name")
            place_names[place] = field_names[operation[10]]
        else:
            place_names[place] = None

    for operation in module.tables["operations"]:
        if operation[3] != 5:
            continue
        require(operation[9] == 1, "load-place arity")
        place = operands[operation[8]][0]
        name = place_names.get(place)
        if name is not None:
            result[operation[6]] = name
    return result


def _normalize_value(module, value: int, definitions: dict[int, tuple[int, ...]],
                     names: dict[int, bytes], active: set[int]) -> tuple:
    require(value < len(module.value_types), "lowering value ID")
    if value in active:
        raise RefinementError("recursive value definition")
    operation = definitions.get(value)
    type_id = module.value_types[value]
    type_row = module.tables["types"][type_id]
    if operation is None:
        require(value in names, "parameter witness name")
        return ("leaf", "u8" if type_row[1] == 1 else "u32", names[value])
    opcode = operation[3]
    operands = module.tables["operands"]
    args = [operands[index][0]
            for index in range(operation[8], operation[8] + operation[9])]
    if opcode == 1:
        return ("literal", operation[10])
    if opcode == 5:
        require(value in names, "loaded-field witness name")
        return ("leaf", "u8" if type_row[1] == 1 else "u32", names[value])
    if opcode == 21:
        require(len(args) == 1, "widen lowering arity")
        return ("widen", _normalize_value(
            module, args[0], definitions, names, active | {value}
        ))
    if opcode in ARITHMETIC:
        require(len(args) == 2, "arithmetic lowering arity")
        return (
            opcode,
            _normalize_value(module, args[0], definitions, names, active | {value}),
            _normalize_value(module, args[1], definitions, names, active | {value}),
        )
    raise RefinementError("excluded arithmetic operand definition")


def arithmetic_trees(module, witness_names: dict[str, dict[int, bytes]]) -> tuple[tuple, ...]:
    definitions = _value_definition(module)
    names = leaf_value_names(module, witness_names)
    arithmetic = [operation for operation in module.tables["operations"]
                  if operation[3] in ARITHMETIC]
    consumed = {
        module.tables["operands"][index][0]
        for operation in arithmetic
        for index in range(operation[8], operation[8] + operation[9])
    }
    roots = [operation for operation in arithmetic if operation[6] not in consumed]
    return tuple(_normalize_value(module, operation[6], definitions, names, set())
                 for operation in roots)


def source_tree(expression, types: dict[bytes, str]) -> tuple:
    if expression.kind == "literal":
        return ("literal", int(expression.value))
    if expression.kind == "leaf":
        name = bytes(expression.value)
        lookup = name[5:] if name.startswith(b"self.") else name
        return ("leaf", types[lookup], lookup)
    if expression.kind == "widen":
        require(expression.left is not None, "source widening child")
        return ("widen", source_tree(expression.left, types))
    require(expression.left is not None and expression.right is not None,
            "source arithmetic children")
    return (int(expression.value), source_tree(expression.left, types),
            source_tree(expression.right, types))


def check_expression_join(module, program, witness_names: dict[str, dict[int, bytes]]) -> None:
    expected = tuple(source_tree(expression, program.field_types)
                     for expression in program.expressions)
    require(arithmetic_trees(module, witness_names) == expected,
            "per-node source/CKIR postorder, operands, literals, and widening")


def _tree_has_arithmetic(tree: tuple) -> bool:
    return (bool(tree) and tree[0] in ARITHMETIC
            or any(_tree_has_arithmetic(child) for child in tree[1:] if isinstance(child, tuple)))


def check_context_join(module, program,
                       witness_names: dict[str, dict[int, bytes]]) -> None:
    """Join the complete argument vectors surrounding selected arithmetic."""
    definitions = _value_definition(module)
    names = leaf_value_names(module, witness_names)
    operands = module.tables["operands"]

    expected_calls: list[tuple[tuple, ...]] = []
    expected_transitions: list[tuple[tuple, ...]] = []
    for binding in program.bindings:
        vector = tuple(source_tree(expression, program.field_types)
                       for _, expression in binding.assignments)
        if not any(_tree_has_arithmetic(tree) for tree in vector):
            continue
        if binding.context == "transition-argument":
            expected_transitions.append(vector)
        else:
            expected_calls.append(vector)

    actual_calls: list[tuple[tuple, ...]] = []
    for operation in module.tables["operations"]:
        if operation[3] != 10:
            continue
        values = [operands[index][0]
                  for index in range(operation[8] + 1, operation[8] + operation[9])]
        vector = tuple(_normalize_value(module, value, definitions, names, set())
                       for value in values)
        if any(_tree_has_arithmetic(tree) for tree in vector):
            actual_calls.append(vector)

    payloads = module.tables["case_payloads"]
    actual_transitions: list[tuple[tuple, ...]] = []
    for arm in module.tables["case_arms"]:
        vector: list[tuple] = []
        for argument_id in range(arm[4], arm[4] + arm[5]):
            argument = module.tables["case_arm_args"][argument_id]
            if argument[1] == 1:
                vector.append(_normalize_value(
                    module, argument[2], definitions, names, set()
                ))
            else:
                payload_id = argument[2]
                require(payload_id in witness_names["payloads"],
                        "case payload witness name")
                type_row = module.tables["types"][payloads[payload_id][3]]
                vector.append((
                    "leaf", "u8" if type_row[1] == 1 else "u32",
                    witness_names["payloads"][payload_id],
                ))
        normalized = tuple(vector)
        if any(_tree_has_arithmetic(tree) for tree in normalized):
            actual_transitions.append(normalized)

    require(tuple(actual_calls) == tuple(expected_calls),
            "source/CKIR call argument values and order")
    require(tuple(actual_transitions) == tuple(expected_transitions),
            "source/CKIR transition argument values and order")


def check_view_join(module, program) -> None:
    operations = module.tables["operations"]
    selected = {opcode: [row for row in operations if row[3] == opcode]
                for opcode in range(22, 26)}
    expected_counts = {
        22: len(program.view_literals), 23: program.view_nonempty,
        24: program.view_heads, 25: program.view_tails,
    }
    require({opcode: len(rows) for opcode, rows in selected.items()} == expected_counts,
            "source/CKIR optional-view operation join")
    if not program.view_literals:
        return
    constants = module.tables["constants"]
    children = module.tables["constant_children"]
    actual: list[bytes] = []
    for operation in selected[22]:
        root = constants[operation[10]]
        actual.append(bytes(constants[children[index][0]][4]
                            for index in range(root[2], root[2] + root[3])))
    require(tuple(actual) == program.view_literals, "source/CKIR static-view literal bytes")
    synthetic = sum(bool(block[3] & 1) for block in module.tables["blocks"])
    require(synthetic >= max(program.view_heads, program.view_tails),
            "source/CKIR optional-view synthetic edge")
