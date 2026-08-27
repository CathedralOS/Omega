#!/usr/bin/env python3
"""Independent CKIR20 TokenStream/pure-sum record-array reference."""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

import checked_ir_v2_reference as v2
import checked_ir_v3_reference as v3
import checked_ir_v4_reference as v4
import checked_ir_v5_reference as v5


Ckir20Error = v5.Ckir5Error
Ckir20ResourceError = v5.Ckir5ResourceError
Module = v5.Module
HEADER = v5.HEADER
ROWS = v5.ROWS
TABLE_ORDER = v5.TABLE_ORDER
COUNT_NAMES = v5.COUNT_NAMES
NO_ID = v5.NO_ID
interpret = v5.interpret

CAPABILITIES = v5.SchemaCapabilities(
    frozenset(range(1, 15)),
    full_width_u32=True,
    full_width_u64_less=True,
    full_width_u64_index_add=True,
    full_width_u64_record_index=True,
    entry_layout_ceiling=2 * 1024 * 1024,
    machine_parameter_ceiling=16,
    entry_layout_exhaustion_is_resource=True,
)


def _fields(module: Module, record_id: int) -> list[tuple[int, ...]]:
    row = module.tables["records"][record_id]
    return module.tables["fields"][row[2]:row[2] + row[3]]


def _place_paths(module: Module) -> dict[int, tuple[object, ...]]:
    operands = [row[0] for row in module.tables["operands"]]
    paths: dict[int, tuple[object, ...]] = {}
    for op in module.tables["operations"]:
        args = operands[op[8]:op[8] + op[9]]
        if op[3] == 2:
            paths[op[6]] = ("self", op[1])
        elif op[3] == 3 and args[0] in paths:
            paths[op[6]] = paths[args[0]] + (("field", op[10]),)
        elif op[3] == 4 and args[0] in paths:
            paths[op[6]] = paths[args[0]] + (("index", op[7]),)
    return paths


def profile(module: Module) -> dict[str, object]:
    """Validate the exact full TokenStream::push selected relation."""
    t = module.tables
    types, records, sums = t["types"], t["records"], t["sums"]
    cases, payloads = t["cases"], t["case_payloads"]
    v5.require(len(types) == 24 and len(records) == 8 and len(t["fields"]) == 29,
               "CKIR20 exact nominal/type profile")
    v5.require(len(sums) == 5 and len(cases) == 105 and len(payloads) == 8,
               "CKIR20 exact sum profile")
    v5.require([row[1] for row in records] == list(range(8)),
               "CKIR20 record nominal order")
    v5.require([row[1] for row in sums] == list(range(8, 13)),
               "CKIR20 sum nominal order")
    v5.require([row[4] for row in records] == [1, 1, 1, 1, 1, 1, 0, 0]
               and all(row[4] == 1 for row in sums),
               "CKIR20 recursive copy declarations")
    v5.require([row[3] for row in sums] == [4, 30, 42, 9, 20],
               "CKIR20 exact case families")
    token_kind_cases = cases[sums[3][2]:sums[3][2] + sums[3][3]]
    v5.require([row[4] for row in token_kind_cases] == [0, 3, 3, 0, 1, 1, 0, 0, 0],
               "CKIR20 TokenKind payload shape")
    v5.require([row[3] for row in payloads] == [8, 15, 15, 15, 15, 15, 9, 10],
               "CKIR20 TokenKind payload types")

    expected_field_types = [
        [14], [16, 16], [0, 1], [11, 2, 16, 16], [12, 2],
        [13, 13, 13, 13, 14, 16, 16, 16, 16],
        [21, 22, 17, 23, 16, 4, 15, 15], [6],
    ]
    v5.require([[row[3] for row in _fields(module, rid)]
                for rid in range(8)] == expected_field_types,
               "CKIR20 exact record fields")
    v5.require(types[21][1:6] == (5, 1, 0, 3, 16_384)
               and types[22][1:6] == (5, 1, 0, 5, 16_384)
               and types[23][1:6] == (5, 1, 0, 13, 65_536),
               "CKIR20 exact fixed arrays")
    v5.require(v5._u64_type_bounds(types[17]) == (0, 16_384),
               "CKIR20 token count interval")
    expected_layouts = {
        0: (4, 4), 1: (16, 8), 2: (24, 8), 3: (56, 8),
        4: (32, 8), 5: (40, 8), 6: (1_638_456, 8),
        7: (1_638_456, 8), 8: (4, 4), 9: (4, 4),
        10: (4, 4), 11: (12, 4), 12: (4, 4),
    }
    v5.require(all(module.layouts[type_id] == layout
                   for type_id, layout in expected_layouts.items()),
               "CKIR20 private layouts")
    v5.require(tuple(module.field_offsets) == (
        0, 0, 8, 0, 8, 0, 16, 40, 48, 0, 8,
        0, 1, 2, 3, 4, 8, 16, 24, 32,
        0, 917_504, 1_572_864, 1_572_872, 1_638_408,
        1_638_416, 1_638_448, 1_638_449, 0,
    ), "CKIR20 private field offsets")

    machines = t["machines"]
    v5.require(len(machines) == 3 and module.entry == 2,
               "CKIR20 exact machines/entry")
    v5.require([(m[1], m[2], m[5], m[7], m[9]) for m in machines] == [
        (6, 2, NO_ID, 10, 3), (6, 1, 13, 1, 10), (7, 2, 13, 0, 1),
    ], "CKIR20 machine signatures")
    v5.require([row[3] for row in t["machine_params"]] ==
               [0, 11, 16, 16, 16, 16, 13, 13, 13, 13, 16],
               "CKIR20 machine parameter custody")
    v5.require(len(t["block_params"]) == 11,
               "CKIR20 selected-case/edge binders")

    operands = [row[0] for row in t["operands"]]
    paths = _place_paths(module)
    operations = t["operations"]
    retain_ops = [op for op in operations if op[2] == 1]
    copies = [op for op in retain_ops if op[3] == 7]
    stores = [op for op in retain_ops if op[3] == 6]
    v5.require(len(copies) >= 2 and {op[10] for op in copies} >= {1},
               "CKIR20 structural Copy operations")
    copy_destinations = {paths[operands[op[8]]] for op in copies}
    token_kind_path = ("self", 0, ("field", 20), ("index", 3), ("field", 5))
    source_id_path = token_kind_path[:-1] + (("field", 6), ("field", 3))
    v5.require(token_kind_path in copy_destinations and source_id_path in copy_destinations,
               "CKIR20 whole-sum/nested-record copies")

    store_destinations = [paths[operands[op[8]]] for op in stores]
    token_scalar_fields = {1, 2, 7, 8}
    observed_token_fields = {
        path[-1][1] for path in store_destinations
        if len(path) >= 5 and path[2] == ("field", 20)
        and path[3] == ("index", 3) and path[-1][0] == "field"
    }
    observed_observation_fields = {
        path[-1][1] for path in store_destinations
        if len(path) == 5 and path[2] == ("field", 21)
        and path[3] == ("index", 5) and path[-1][0] == "field"
    }
    v5.require(token_scalar_fields <= observed_token_fields
               and observed_observation_fields == set(range(11, 20)),
               "CKIR20 fifteen selected data stores")

    source_value_path = source_id_path + (("field", 0),)
    source_loads = [op for op in retain_ops if op[3] == 5
                    and paths[operands[op[8]]] == source_value_path]
    v5.require(len(source_loads) == 1, "CKIR20 source.value projection")
    source_value = source_loads[0][6]
    source_stores = [op for op in stores
                     if paths[operands[op[8]]][-1] == ("field", 15)]
    v5.require(len(source_stores) == 1
               and operands[source_stores[0][8] + 1] == source_value,
               "CKIR20 source.value observation join")

    adds = [op for op in operations if op[3] == 8
            and types[module.value_types[operands[op[8]]]][1] == 8]
    lesses = [op for op in operations if op[3] == 9]
    indexes = [op for op in operations if op[3] == 4
               and types[op[7]][1] == 4]
    v5.require(len(adds) == 1 and len(lesses) >= 2 and len(indexes) >= 17,
               "CKIR20 u64 Add/Less/record indexes")
    dispatches = [term for term in t["terminators"] if term[3] == 5]
    v5.require(len(dispatches) == 1 and dispatches[0][4] == 2
               and dispatches[0][14] == 9,
               "CKIR20 indexed TokenKind CaseDispatch")
    float_arm = t["case_arms"][2]
    v5.require(float_arm[2] == 78 and float_arm[5] == 3,
               "CKIR20 Float payload readback")
    calls = [op for op in operations if op[3] == 10]
    v5.require([(op[10], op[9], op[4]) for op in calls] == [(0, 11, 0), (1, 2, 1)],
               "CKIR20 real push/read_kind calls")
    constructors = [op for op in operations if op[3] in (13, 14)]
    v5.require([(op[3], op[7], op[10]) for op in constructors] ==
               [(13, 0, 0), (14, 11, 78)],
               "CKIR20 SourceId/Float construction")
    return {
        "copies": copies, "stores": stores, "indexes": indexes,
        "adds": adds, "lesses": lesses, "dispatches": dispatches,
    }


def decode(contents: bytes) -> Module:
    module = v5.decode(contents, expected_major=20, capabilities=CAPABILITIES)
    v5.require(module.entry != NO_ID, "CKIR20 requires entry")
    v5.require(not module.tables["constants"] and not module.tables["constant_children"],
               "CKIR20 excludes public constants")
    v5.require(all(row[1] != 7 for row in module.tables["types"]),
               "CKIR20 excludes static byte views")
    v5.require(all(1 <= row[3] <= 14 for row in module.tables["operations"]),
               "CKIR20 focused opcode profile")
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
        print(f"CKIR20 valid: {len(selected['stores'])} retain stores, "
              f"{len(selected['indexes'])} record indexes")
    else:
        print(interpret(module))


if __name__ == "__main__":
    try:
        main()
    except Ckir20ResourceError as error:
        print(f"checked IR v20 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(252)
    except (Ckir20Error, v4.Ckir4Error, v3.Ckir3Error, v2.CkirError,
            OSError, struct.error) as error:
        print(f"checked IR v20 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(251)
