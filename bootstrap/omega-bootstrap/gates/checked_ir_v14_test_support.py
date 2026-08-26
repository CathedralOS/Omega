"""Small handcrafted CKIR14 carriers for reference-model unit tests."""

from __future__ import annotations

import copy
import importlib.util
from pathlib import Path

import checked_ir_v14_reference as ir14


NO_ID = ir14.NO_ID
HERE = Path(__file__).resolve().parent


def replace(row: tuple[int, ...], index: int, value: int) -> tuple[int, ...]:
    return row[:index] + (value,) + row[index + 1:]


def encode(tables: dict[str, list[tuple[int, ...]]], *, values: int,
           places: int = 0, major: int = 14) -> bytes:
    counts = {name: len(tables[name]) for name in ir14.TABLE_ORDER}
    counts.update(values=values, places=places)
    payload = b"".join(
        ir14.ROWS[name].pack(*row)
        for name in ir14.TABLE_ORDER
        for row in tables[name]
    )
    return ir14.HEADER.pack(
        b"OMGCKIR\0", major, 0, 1, 1, 0,
        ir14.HEADER.size + len(payload),
        *(counts[name] for name in ir14.COUNT_NAMES),
    ) + payload


def arithmetic_tables(kind: str = "success") -> dict[str, list[tuple[int, ...]]]:
    result = {name: [] for name in ir14.TABLE_ORDER}
    result["types"] = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 2, 1, 0, 0, 0, 0, 0xFFFF_FFFF),
    ]
    result["records"] = [(0, 0, 0, 0, 0, 0, 0, 0)]
    result["machines"] = [(0, 0, 2, 0, 0, 1, 0, 0, 0, 1, 0)]
    result["blocks"] = [(0, 0, 2, 0, 0, 0, 0, 0, 7, 0)]

    if kind == "success":
        constants = (0xFFFF_FFFF, 0xFFFF_FFFA, 13, 5)
        opcodes = (26, 27, 8)
        pairs = ((0, 1), (3, 2), (4, 5))
    elif kind == "add-overflow":
        constants = (0xFFFF_FFFF, 1, 1, 0)
        opcodes = (8, 27, 26)
        pairs = ((0, 1), (4, 2), (5, 3))
    elif kind == "subtract-underflow":
        constants = (0, 1, 1, 0)
        opcodes = (26, 27, 8)
        pairs = ((0, 1), (4, 2), (5, 3))
    elif kind == "multiply-overflow":
        constants = (65_536, 65_536, 0, 0)
        opcodes = (27, 8, 26)
        pairs = ((0, 1), (4, 2), (5, 3))
    else:
        raise ValueError(f"unknown arithmetic carrier {kind}")

    operations: list[tuple[int, ...]] = []
    operands: list[tuple[int, ...]] = []
    for value in constants:
        operations.append((len(operations), 0, 0, 1, 1, 0,
                           len(operations), 1, len(operands), 0, value, 0))
    for opcode, pair in zip(opcodes, pairs):
        operations.append((len(operations), 0, 0, opcode, 1, 0,
                           len(operations), 1, len(operands), 2, 0, 0))
        operands.extend(((pair[0],), (pair[1],)))
    result["operations"] = operations
    result["operands"] = operands
    result["terminators"] = [
        (0, 0, 0, 4, 0, 0, 6, NO_ID, len(operands), 0,
         NO_ID, len(operands), 0, 0, 0),
    ]
    return result


def arithmetic(kind: str = "success") -> bytes:
    return encode(arithmetic_tables(kind), values=7)


def single_arithmetic(opcode: int, left: int, right: int) -> bytes:
    if opcode not in (8, 26, 27):
        raise ValueError("not a selected CKIR14 arithmetic opcode")
    tables = {name: [] for name in ir14.TABLE_ORDER}
    tables["types"] = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 2, 1, 0, 0, 0, 0, 0xFFFF_FFFF),
    ]
    tables["records"] = [(0, 0, 0, 0, 0, 0, 0, 0)]
    tables["machines"] = [(0, 0, 2, 0, 0, 1, 0, 0, 0, 1, 0)]
    tables["blocks"] = [(0, 0, 2, 0, 0, 0, 0, 0, 3, 0)]
    tables["operations"] = [
        (0, 0, 0, 1, 1, 0, 0, 1, 0, 0, left, 0),
        (1, 0, 0, 1, 1, 0, 1, 1, 0, 0, right, 0),
        (2, 0, 0, opcode, 1, 0, 2, 1, 0, 2, 0, 0),
    ]
    tables["operands"] = [(0,), (1,)]
    tables["terminators"] = [
        (0, 0, 0, 4, 0, 0, 2, NO_ID, 2, 0, NO_ID, 2, 0, 0, 0),
    ]
    return encode(tables, values=3)


def parameter_arithmetic() -> bytes:
    """A selected Add whose left leaf is an exact block parameter."""
    tables = {name: [] for name in ir14.TABLE_ORDER}
    tables["types"] = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 2, 1, 0, 0, 0, 0, 0xFFFF_FFFF),
    ]
    tables["records"] = [(0, 0, 0, 0, 0, 0, 0, 0)]
    tables["machines"] = [(0, 0, 2, 0, 0, 1, 0, 0, 0, 2, 0)]
    tables["block_params"] = [(0, 1, 0, 1, 0)]
    tables["blocks"] = [
        (0, 0, 2, 0, 0, 0, 0, 0, 1, 0),
        (1, 0, 2, 0, 0, 0, 1, 1, 2, 1),
    ]
    tables["operations"] = [
        (0, 0, 0, 1, 1, 0, 1, 1, 0, 0, 1, 0),
        (1, 0, 1, 1, 1, 0, 2, 1, 0, 0, 69, 0),
        (2, 0, 1, 8, 1, 0, 3, 1, 0, 2, 0, 0),
    ]
    tables["operands"] = [(0,), (2,), (1,)]
    tables["terminators"] = [
        (0, 0, 0, 1, 0, 0, NO_ID, 1, 2, 1, NO_ID, 3, 0, 0, 0),
        (1, 0, 1, 4, 0, 0, 3, NO_ID, 3, 0, NO_ID, 3, 0, 0, 0),
    ]
    return encode(tables, values=4)


def widen_arithmetic_tables() -> dict[str, list[tuple[int, ...]]]:
    tables = {name: [] for name in ir14.TABLE_ORDER}
    tables["types"] = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 1, 0, 0, 0, 0, 0, 255),
        (2, 2, 1, 0, 0, 0, 0, 0xFFFF_FFFF),
    ]
    tables["records"] = [(0, 0, 0, 0, 0, 0, 0, 0)]
    tables["machines"] = [(0, 0, 2, 0, 0, 2, 0, 0, 0, 1, 0)]
    tables["blocks"] = [(0, 0, 2, 0, 0, 0, 0, 0, 4, 0)]
    tables["operations"] = [
        (0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 69, 0),
        (1, 0, 0, 21, 1, 0, 1, 2, 0, 1, 0, 0),
        (2, 0, 0, 1, 1, 0, 2, 2, 1, 0, 1, 0),
        (3, 0, 0, 8, 1, 0, 3, 2, 1, 2, 0, 0),
    ]
    tables["operands"] = [(0,), (1,), (2,)]
    tables["terminators"] = [
        (0, 0, 0, 4, 0, 0, 3, NO_ID, 3, 0, NO_ID, 3, 0, 0, 0),
    ]
    return tables


def widen_arithmetic() -> bytes:
    return encode(widen_arithmetic_tables(), values=4)


def call_custody_violation() -> bytes:
    """An exact full-u32 Call result illegally feeds selected Add."""
    tables = {name: [] for name in ir14.TABLE_ORDER}
    tables["types"] = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 2, 1, 0, 0, 0, 0, 0xFFFF_FFFF),
    ]
    tables["records"] = [(0, 0, 0, 0, 0, 0, 0, 0)]
    tables["machines"] = [
        (0, 0, 2, 0, 0, 1, 0, 0, 0, 1, 0),
        (1, 0, 1, 0, 0, 1, 0, 0, 1, 1, 1),
    ]
    tables["blocks"] = [
        (0, 0, 2, 0, 0, 0, 0, 0, 4, 0),
        (1, 1, 1, 0, 0, 0, 0, 4, 1, 1),
    ]
    tables["operations"] = [
        (0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 0),
        (1, 0, 0, 10, 1, 0, 0, 1, 0, 1, 1, 0),
        (2, 0, 0, 1, 1, 0, 1, 1, 1, 0, 1, 0),
        (3, 0, 0, 8, 1, 0, 2, 1, 1, 2, 0, 0),
        (4, 1, 1, 1, 1, 0, 3, 1, 3, 0, 69, 0),
    ]
    tables["operands"] = [(0,), (0,), (1,)]
    tables["terminators"] = [
        (0, 0, 0, 4, 0, 0, 2, NO_ID, 3, 0, NO_ID, 3, 0, 0, 0),
        (1, 1, 1, 4, 0, 0, 3, NO_ID, 3, 0, NO_ID, 3, 0, 0, 0),
    ]
    return encode(tables, values=4, places=1)


def _load_v12_fixture():
    path = HERE / "delta-checked-ir-v12-fixture.py"
    spec = importlib.util.spec_from_file_location("ckir14_v12_fixture", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def composed_view_and_arithmetic_tables() -> dict[str, list[tuple[int, ...]]]:
    v12 = _load_v12_fixture()
    tables = copy.deepcopy(v12.tables((70,)))
    full_type = len(tables["types"])
    tables["types"].append(
        (full_type, 2, 1, 0, 0, 0, 0, 0xFFFF_FFFF)
    )

    # Block 4 is reached only after the inherited nonempty/head/tail checks.
    # Append a successful nested arithmetic tree there, leaving its original
    # return-70 observation intact.
    block4 = tables["blocks"][4]
    tables["blocks"][4] = replace(block4, 8, block4[8] + 7)
    tables["blocks"][5] = replace(tables["blocks"][5], 7, 14)
    inserted_operands = [(11,), (12,), (14,), (15,), (13,), (16,)]
    tables["operands"][4:4] = inserted_operands
    for index, term in enumerate(tables["terminators"]):
        changed = term
        for field in (8, 11):
            if changed[field] >= 4:
                changed = replace(changed, field, changed[field] + len(inserted_operands))
        tables["terminators"][index] = changed

    tables["operations"].extend([
        (7, 0, 4, 1, 1, 0, 11, full_type, 4, 0, 0xFFFF_FFFF, 0),
        (8, 0, 4, 1, 1, 0, 12, full_type, 4, 0, 0xFFFF_FFFA, 0),
        (9, 0, 4, 26, 1, 0, 13, full_type, 4, 2, 0, 0),
        (10, 0, 4, 1, 1, 0, 14, full_type, 6, 0, 13, 0),
        (11, 0, 4, 1, 1, 0, 15, full_type, 6, 0, 5, 0),
        (12, 0, 4, 27, 1, 0, 16, full_type, 6, 2, 0, 0),
        (13, 0, 4, 8, 1, 0, 17, full_type, 8, 2, 0, 0),
    ])
    return tables


def composed_view_and_arithmetic() -> bytes:
    return encode(composed_view_and_arithmetic_tables(), values=18)


def view_only() -> bytes:
    v12 = _load_v12_fixture()
    return v12.encode(v12.tables((70,)), major=14)
