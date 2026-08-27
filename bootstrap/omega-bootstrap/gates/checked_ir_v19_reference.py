#!/usr/bin/env python3
"""Independent CKIR19 record-array/full-width-u64 reference."""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

import checked_ir_v2_reference as v2
import checked_ir_v3_reference as v3
import checked_ir_v4_reference as v4
import checked_ir_v5_reference as v5


Ckir19Error = v5.Ckir5Error
Ckir19ResourceError = v5.Ckir5ResourceError
Module = v5.Module
HEADER = v5.HEADER
ROWS = v5.ROWS
TABLE_ORDER = v5.TABLE_ORDER
COUNT_NAMES = v5.COUNT_NAMES
NO_ID = v5.NO_ID
interpret = v5.interpret

CAPABILITIES = v5.SchemaCapabilities(
    frozenset(range(1, 11)),
    full_width_u32=True,
    full_width_u64_less=True,
    full_width_u64_index_add=True,
    full_width_u64_record_index=True,
    entry_layout_ceiling=2 * 1024 * 1024,
    machine_parameter_ceiling=16,
    entry_layout_exhaustion_is_resource=True,
)


def _record_fields(module: Module, record_id: int) -> list[tuple[int, ...]]:
    record = module.tables["records"][record_id]
    return module.tables["fields"][record[2]:record[2] + record[3]]


def profile(module: Module) -> dict[str, object]:
    """Validate and return the selected CKIR19 record-array relation."""
    tables = module.tables
    types = tables["types"]
    records = tables["records"]
    machines = tables["machines"]
    operations = tables["operations"]
    operands = [row[0] for row in tables["operands"]]

    v5.require(len(records) == 3 and len(tables["fields"]) == 13,
               "CKIR19 exact record profile")
    observation_record = records[0]
    stream_record = records[1]
    owner_record = records[2]
    observation_type = observation_record[1]
    stream_type = stream_record[1]
    owner_type = owner_record[1]
    observation_fields = _record_fields(module, 0)
    stream_fields = _record_fields(module, 1)
    owner_fields = _record_fields(module, 2)
    v5.require(observation_record[4] == 1
               and stream_record[4] == 0 and owner_record[4] == 0,
               "CKIR19 record copy flags")
    v5.require([types[row[3]][1] for row in observation_fields] ==
               [1, 1, 1, 1, 2, 8, 8, 8, 8],
               "CKIR19 TokenObservation shape")
    v5.require(len(stream_fields) == 3 and len(owner_fields) == 1,
               "CKIR19 owner field count")
    array_type, count_type, retained_type = (row[3] for row in stream_fields)
    v5.require(types[array_type][1] == 5
               and types[array_type][4] == observation_type
               and types[array_type][5] == 16_384,
               "CKIR19 observation array")
    v5.require(types[count_type][1] == 8
               and v5._u64_type_bounds(types[count_type]) == (0, 16_384),
               "CKIR19 count custody")
    v5.require(types[retained_type][1] == 3,
               "CKIR19 retained custody")
    v5.require(owner_fields[0][3] == stream_type,
               "CKIR19 Main stream ownership")
    v5.require(len(machines) == 3 and module.entry == 2,
               "CKIR19 machine profile")
    writer, reader, entry = machines
    v5.require(writer[1] == 1 and writer[2] == 2
               and writer[5] == NO_ID and writer[7] == 9,
               "CKIR19 observation writer")
    v5.require(reader[1] == 1 and reader[2] == 1
               and reader[5] < len(types) and types[reader[5]][1] == 1
               and reader[7] == 1,
               "CKIR19 read_tag machine")
    parameter_kinds = [types[row[3]][1] for row in tables["machine_params"]]
    v5.require(parameter_kinds[:9] == [1, 1, 1, 1, 2, 8, 8, 8, 8],
               "CKIR19 writer parameter custody")
    v5.require(len(parameter_kinds) == 10 and parameter_kinds[9] == 8,
               "CKIR19 read_tag index custody")
    v5.require(entry[1] == 2 and entry[2] == 2
               and entry[5] < len(types)
               and types[entry[5]][1] == 1 and entry[7] == 0,
               "CKIR19 exact entry machine")

    indexed_places: set[int] = set()
    selected_indexes: list[tuple[int, ...]] = []
    observation_places: dict[int, int] = {}
    for operation in operations:
        opcode = operation[3]
        args = operands[operation[8]:operation[8] + operation[9]]
        if opcode == 4 and len(args) == 2:
            base_type = module.place_types[args[0]]
            index_type = module.value_types[args[1]]
            if (types[base_type][1] == 5
                    and types[base_type][4] == observation_type
                    and types[index_type][1] == 8):
                selected_indexes.append(operation)
                indexed_places.add(operation[6])
        elif opcode == 3 and len(args) == 1 and args[0] in indexed_places:
            field_id = operation[10]
            if field_id in range(observation_record[2],
                                 observation_record[2] + observation_record[3]):
                observation_places[operation[6]] = field_id

    stored_fields: set[int] = set()
    loaded_fields: set[int] = set()
    for operation in operations:
        args = operands[operation[8]:operation[8] + operation[9]]
        if operation[3] == 6 and args and args[0] in observation_places:
            stored_fields.add(observation_places[args[0]])
        elif operation[3] == 5 and args and args[0] in observation_places:
            loaded_fields.add(observation_places[args[0]])
    add_count = sum(
        operation[3] == 8
        and types[module.value_types[operands[operation[8]]]][1] == 8
        for operation in operations
    )
    less_count = sum(
        operation[3] == 9
        and types[module.value_types[operands[operation[8]]]][1] == 8
        for operation in operations
    )
    expected_fields = set(range(observation_record[2],
                                observation_record[2] + observation_record[3]))
    v5.require(len(selected_indexes) >= 10,
               "CKIR19 selected record indexes")
    v5.require(stored_fields == expected_fields,
               "CKIR19 complete observation stores")
    v5.require(observation_record[2] in loaded_fields,
               "CKIR19 tag readback")
    v5.require(add_count >= 1 and less_count >= 1,
               "CKIR19 exact count Add/Less")
    calls = [operation for operation in operations if operation[3] == 10]
    v5.require(len(calls) == 2
               and [(call[10], call[9], call[4]) for call in calls]
                   == [(0, 10, 0), (1, 2, 1)],
               "CKIR19 push/read_tag calls")
    v5.require(module.layouts[observation_type] == (40, 8),
               "CKIR19 observation private layout")
    v5.require(module.layouts[owner_type][0] <= 2 * 1024 * 1024,
               "CKIR19 owner ceiling")
    return {
        "indexes": selected_indexes,
        "stored_fields": stored_fields,
        "loaded_fields": loaded_fields,
        "adds": add_count,
        "lesses": less_count,
        "observation_type": observation_type,
        "array_type": array_type,
    }


def decode(contents: bytes) -> Module:
    module = v5.decode(contents, expected_major=19, capabilities=CAPABILITIES)
    v5.require(module.entry != NO_ID, "CKIR19 requires an entry machine")
    for name in (
        "sums", "cases", "case_payloads", "constants",
        "constant_children", "case_arms", "case_arm_args",
    ):
        v5.require(not module.tables[name], f"CKIR19 excludes {name}")
    v5.require(all(row[1] != 7 for row in module.tables["types"]),
               "CKIR19 excludes static byte views")
    v5.require(all(1 <= row[3] <= 10 for row in module.tables["operations"]),
               "CKIR19 excludes historical opcode families")
    profile(module)
    return module


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("validate", "run"))
    parser.add_argument("ckir", type=Path)
    args = parser.parse_args()
    module = decode(args.ckir.read_bytes())
    if args.command == "validate":
        selected = profile(module)
        print(
            "CKIR19 valid: "
            f"{len(selected['indexes'])} record indexes, "
            f"{len(selected['stored_fields'])} stored fields, "
            f"{len(selected['loaded_fields'])} loaded fields"
        )
    else:
        result = interpret(module)
        print("library" if result is None else result)


if __name__ == "__main__":
    try:
        main()
    except Ckir19ResourceError as error:
        print(f"checked IR v19 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(252)
    except (Ckir19Error, v4.Ckir4Error, v3.Ckir3Error, v2.CkirError,
            OSError, struct.error) as error:
        print(f"checked IR v19 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(251)
