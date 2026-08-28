#!/usr/bin/env python3
"""Independent CKIR19 record-array observations and lowering helpers."""

from __future__ import annotations

import importlib.util
import sys
from dataclasses import dataclass
from pathlib import Path

from omgrfn22_frame import RefinementError, RefinementResourceError, require

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


V5 = load("omgrfn22_checked_ir_v5_reference", GATES / "checked_ir_v5_reference.py")
IR19 = load("omgrfn22_checked_ir_v19_reference", GATES / "checked_ir_v19_reference.py")
NO_ID = V5.NO_ID


def _translate(label: str, action):
    try:
        return action()
    except (V5.Ckir5ResourceError, IR19.Ckir19ResourceError) as error:
        raise RefinementResourceError(f"{label}: {error}") from error
    except Exception as error:
        raise RefinementError(f"{label}: {error}") from error


def meaning_decode(contents: bytes):
    return _translate("CKIR19 meaning", lambda: IR19.decode(contents))


def producer_decode(contents: bytes):
    module = _translate(
        "CKIR19 producer structure",
        lambda: V5.decode(contents, expected_major=19,
                          capabilities=IR19.CAPABILITIES),
    )
    check_selected_structure(module)
    return module


def arguments(module, operation: tuple[int, ...]) -> tuple[int, ...]:
    return tuple(module.tables["operands"][index][0]
                 for index in range(operation[8], operation[8] + operation[9]))


def definitions(module) -> dict[int, tuple[int, ...]]:
    return {operation[6]: operation for operation in module.tables["operations"]
            if operation[4] == 1}


def place_definitions(module) -> dict[int, tuple[int, ...]]:
    return {operation[6]: operation for operation in module.tables["operations"]
            if operation[4] == 2}


def direct_self_field(module, place: int, pdefs=None) -> int | None:
    pdefs = place_definitions(module) if pdefs is None else pdefs
    operation = pdefs.get(place)
    if operation is None or operation[3] != 3:
        return None
    args = arguments(module, operation)
    base = pdefs.get(args[0]) if len(args) == 1 else None
    return operation[10] if base is not None and base[3] == 2 else None


def direct_field_load(module, value: int, pdefs=None) -> int | None:
    operation = definitions(module).get(value)
    if operation is None or operation[3] != 5:
        return None
    args = arguments(module, operation)
    return direct_self_field(module, args[0], pdefs) if len(args) == 1 else None


def constant_u64(module, value: int) -> int | None:
    operation = definitions(module).get(value)
    if operation is None or operation[3] != 1:
        return None
    if module.tables["types"][module.value_types[value]][1] != 8:
        return None
    return operation[10] | (operation[11] << 32)


@dataclass(frozen=True)
class Selected:
    observation_record: int
    stream_record: int
    root_record: int
    observation_type: int
    stream_type: int
    root_type: int
    array_type: int
    count_type: int
    retained_type: int
    writer: int
    reader: int
    entry: int
    array_field: int
    count_field: int
    retained_field: int
    indexes: tuple[tuple[int, ...], ...]
    stores: tuple[tuple[int, ...], ...]
    readback_load: tuple[int, ...]
    writer_less: tuple[int, ...]
    reader_less: tuple[int, ...]
    add: tuple[int, ...]


def check_selected_structure(module) -> Selected:
    tables = module.tables
    types, records, fields = tables["types"], tables["records"], tables["fields"]
    operations, machines = tables["operations"], tables["machines"]
    for name in ("sums", "cases", "case_payloads", "constants",
                 "constant_children", "case_arms", "case_arm_args"):
        require(not tables[name], f"CKIR19 excludes {name}")
    require(all(row[1] != 7 for row in types), "CKIR19 excludes static views")
    require(all(1 <= row[3] <= 10 for row in operations),
            "CKIR19 excludes sibling opcode families")
    require(len(records) == 3 and len(fields) == 13,
            "three-record flat observation profile")
    observation_record, stream_record, root_record = records
    require([row[0] for row in records] == [0, 1, 2],
            "dense selected record IDs")
    observation_type, stream_type, root_type = (row[1] for row in records)
    require(observation_record[2:5] == (0, 9, 1)
            and stream_record[2:5] == (9, 3, 0)
            and root_record[2:5] == (12, 1, 0),
            "only Observation is copyable")
    observation_fields, stream_fields, root_fields = fields[:9], fields[9:12], fields[12:]
    require([row[:3] for row in observation_fields] ==
            [(index, 0, index) for index in range(9)],
            "observation field owner/ordinal identity")
    require([types[row[3]][1] for row in observation_fields] ==
            [1, 1, 1, 1, 2, 8, 8, 8, 8],
            "flat TokenObservation carrier sequence")
    require([row[:3] for row in stream_fields] ==
            [(9 + index, 1, index) for index in range(3)],
            "stream field owner/ordinal identity")
    require(root_fields == [(12, 2, 0, stream_type)],
            "root owns exact stream field")
    array_type, count_type, retained_type = (row[3] for row in stream_fields)
    require(types[array_type][1:6] == (5, 1, 0, observation_type, 16_384),
            "trapping fixed observation array")
    require(types[count_type][1] == 8
            and V5._u64_type_bounds(types[count_type]) == (0, 16_384),
            "retained full-width count interval")
    require(types[retained_type][1] == 3, "retained Boolean field")
    require(module.layouts[observation_type] == (40, 8),
            "observation stride 40 alignment 8")
    require(module.layouts[stream_type] == (655_376, 8)
            and module.layouts[root_type] == (655_376, 8),
            "canonical stream/root private layouts")
    require(module.layouts[root_type][0] <= 2 * 1024 * 1024,
            "selected owner at most 2 MiB")

    require(len(machines) == 3 and module.entry == 2,
            "writer, reader, and selected entry")
    writer, reader, entry = machines
    require(writer[1] == 1 and writer[2] == 2 and writer[5] == NO_ID
            and writer[6:11] == (0, 9, 0, 3, 0),
            "nine-parameter mutable writer")
    require(reader[1] == 1 and reader[2] == 1 and reader[6:11] == (9, 1, 3, 3, 3)
            and types[reader[5]][1] == 1,
            "one-parameter shared read_tag")
    require(entry[1] == 2 and entry[2] == 2 and entry[6:11] == (10, 0, 6, 1, 6)
            and types[entry[5]][1] == 1,
            "u8 selected mutable entry")
    parameter_kinds = [types[row[3]][1] for row in tables["machine_params"]]
    require(parameter_kinds == [1, 1, 1, 1, 2, 8, 8, 8, 8, 8],
            "writer and read-index parameter carriers/order")

    pdefs = place_definitions(module)
    index_rows: list[tuple[int, ...]] = []
    indexed: dict[int, tuple[int, ...]] = {}
    nested: dict[int, tuple[int, ...]] = {}
    for operation in operations:
        args = arguments(module, operation)
        if operation[3] == 4 and len(args) == 2:
            if (module.place_types[args[0]] == array_type
                    and types[module.value_types[args[1]]][1] == 8):
                require(operation[7] == observation_type
                        and operation[10] == operation[11] == 0,
                        "record IndexPlace envelope")
                index_rows.append(operation); indexed[operation[6]] = operation
        elif operation[3] == 3 and len(args) == 1 and args[0] in indexed:
            require(operation[10] < 9 and operation[7] == fields[operation[10]][3]
                    and operation[11] == 0,
                    "indexed observation FieldPlace")
            nested[operation[6]] = operation
    require(len(index_rows) == 10, "nine stores plus one record readback index")

    stores: list[tuple[int, ...]] = []
    stored: dict[int, tuple[int, ...]] = {}
    loads: list[tuple[int, ...]] = []
    for operation in operations:
        args = arguments(module, operation)
        if operation[3] == 6 and len(args) == 2 and args[0] in nested:
            stores.append(operation); stored[nested[args[0]][10]] = operation
        elif operation[3] == 5 and len(args) == 1 and args[0] in nested:
            loads.append(operation)
    require(set(stored) == set(range(9)) and len(stores) == 9,
            "one scalar Store for every observation field")
    require(len(loads) == 1 and loads[0][1] == 1
            and nested[arguments(module, loads[0])[0]][10] == 0
            and loads[0][7] == fields[0][3],
            "exact tag readback Load")
    require(not any(operation[3] == 7 for operation in operations),
            "no structural Copy in flat slice")

    lesses = [operation for operation in operations if operation[3] == 9
              and all(types[module.value_types[value]][1] == 8
                      for value in arguments(module, operation))]
    adds = [operation for operation in operations if operation[3] == 8
            and all(types[module.value_types[value]][1] == 8
                    for value in arguments(module, operation))]
    require(len(lesses) == 2 and len(adds) == 1 and adds[0][1] == 0,
            "writer/read Less and one Exact Add")
    writer_less = next((row for row in lesses if row[1] == 0), None)
    reader_less = next((row for row in lesses if row[1] == 1), None)
    require(writer_less is not None and reader_less is not None,
            "distinct writer/read guard ownership")
    less_args, add_args = arguments(module, writer_less), arguments(module, adds[0])
    require(direct_field_load(module, less_args[0], pdefs) == 10
            and constant_u64(module, less_args[1]) == 16_384,
            "direct count < capacity guard")
    require(direct_field_load(module, add_args[0], pdefs) == 10
            and constant_u64(module, add_args[1]) == 1
            and adds[0][7] == count_type,
            "direct count plus literal one")
    branches = [row for row in tables["terminators"] if row[3] == 2]
    require(len(branches) == 2
            and any(row[6] == writer_less[6] and row[7] == 1 and row[10] == 2
                    for row in branches)
            and any(row[6] == reader_less[6] and row[7] == 4 and row[10] == 5
                    for row in branches),
            "writer/read true-target custody")
    require(all(operation[2] == 1 for operation in (*stores, adds[0])),
            "stores and Exact Add are true-edge dominated")
    require(index_rows[-1][1:3] == (1, 4) and loads[0][2] == 4,
            "readback IndexPlace/Load are reader true-edge dominated")

    calls = [operation for operation in operations if operation[3] == 10]
    require(len(calls) == 2 and all(row[1] == 2 and row[2] == 6 for row in calls)
            and [(row[10], row[9], row[4]) for row in calls] ==
                [(0, 10, 0), (1, 2, 1)],
            "selected entry calls writer then reader")
    return Selected(0, 1, 2, observation_type, stream_type, root_type,
                    array_type, count_type, retained_type, 0, 1, 2, 9, 10, 11,
                    tuple(index_rows), tuple(stores), loads[0], writer_less,
                    reader_less, adds[0])
