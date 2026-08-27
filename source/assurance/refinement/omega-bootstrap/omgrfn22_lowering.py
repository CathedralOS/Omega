#!/usr/bin/env python3
"""Independent OMGRSWB-to-CKIR19 authored lowering correspondence."""

from __future__ import annotations

from omgrfn22_ckir import (arguments, definitions, direct_self_field,
                           place_definitions, producer_decode)
from omgrfn22_frame import require
from omgrfn22_source import check_witness_relation


def _constant(module, value: int) -> int | None:
    operation = definitions(module).get(value)
    if operation is None or operation[3] != 1:
        return None
    kind = module.tables["types"][module.value_types[value]][1]
    return operation[10] | (operation[11] << 32) if kind == 8 else operation[10]


def _source_scalar_kind(source_type: int) -> int:
    """Map the normalized OMGRSWB scalar IDs to carrier kinds, not CKIR IDs."""
    try:
        return {1: 1, 2: 2, 4: 8}[source_type]
    except KeyError as error:
        raise AssertionError(f"unexpected OMGRSWB scalar type {source_type}") from error


def check_lowering(omgcomp: bytes, witness_bytes: bytes, ckir_bytes: bytes) -> None:
    witness, _ = check_witness_relation(omgcomp, witness_bytes)
    module = producer_decode(ckir_bytes)
    selected = __import__("omgrfn22_ckir").check_selected_structure(module)
    types, fields = module.tables["types"], module.tables["fields"]
    require([types[row[3]][1] for row in fields[:9]] ==
            [1, 1, 1, 1, 2, 8, 8, 8, 8],
            "source field carriers join CKIR19 field carriers")
    require(types[fields[4][3]][2] == 1,
            "authored u32 Trapping policy retained in CKIR19")
    require(all(types[fields[index][3]][2] == 0 for index in range(5, 9)),
            "selected u64 policy consumed only into checked operations")
    require(types[selected.array_type][2] == 1
            and types[selected.array_type][5] == witness.length,
            "authored trapping record array and N join")

    pdefs = place_definitions(module)
    for store_path in witness.tables["stores"]:
        _, machine, block, array_field, count_field, element_field, parameter, scalar = store_path
        candidates = []
        for store in selected.stores:
            destination, source = arguments(module, store)
            field_op = pdefs[destination]
            if field_op[10] == element_field and source == parameter:
                candidates.append((store, field_op))
        require(len(candidates) == 1, "one CKIR Store per authored selected store")
        store, field_op = candidates[0]
        require(store[1:3] == (machine, block), "store machine/true-block identity")
        destination_type = field_op[7]
        parameter_type = module.value_types[parameter]
        require(destination_type == parameter_type
                and types[destination_type][1] == _source_scalar_kind(scalar),
                "store destination/parameter exact carrier")
        row_place = arguments(module, field_op)[0]
        index_op = pdefs[row_place]
        base, index = arguments(module, index_op)
        require(index_op[3] == 4 and index_op[7] == selected.observation_type,
                "authored row path maps to record IndexPlace")
        require(direct_self_field(module, base, pdefs) == array_field,
                "record IndexPlace base is direct rows field")
        index_load = definitions(module).get(index)
        require(index_load is not None and index_load[3] == 5
                and direct_self_field(module, arguments(module, index_load)[0], pdefs)
                    == count_field,
                "writer IndexPlace index is the direct count leaf")

    # The reader uses its exact parameter both in the pure guard and the
    # trapping IndexPlace, then loads the selected first field on the true edge.
    read_index = selected.indexes[-1]
    read_base, read_value = arguments(module, read_index)
    require(read_index[1:3] == (witness.lookup_machine, 4)
            and read_value == 9
            and direct_self_field(module, read_base, pdefs) == 9,
            "reader true-edge path and parameter custody")
    read_less_args = arguments(module, selected.reader_less)
    require(read_less_args[0] == read_value,
            "reader Less and IndexPlace share exact index value")
    count_load = definitions(module).get(read_less_args[1])
    require(count_load is not None and count_load[3] == 5
            and direct_self_field(module, arguments(module, count_load)[0], pdefs) == 10,
            "reader guard compares direct index < count")

    calls = [operation for operation in module.tables["operations"]
             if operation[3] == 10]
    require(len(calls) == 2, "two exact source calls lower without flattening")
    push_call, read_call = calls
    require((push_call[10], read_call[10]) ==
            (witness.push_machine, witness.lookup_machine),
            "call targets join source witness")
    for call in calls:
        receiver = arguments(module, call)[0]
        require(direct_self_field(module, receiver, pdefs) == 12,
                "both calls retain Main.stream receiver path")
    push_values = arguments(module, push_call)[1:]
    require(tuple(_constant(module, value) for value in push_values) ==
            (70, 1, 2, 3, 4, 5, 6, 7, 8),
            "push call literal identity and order")
    require(tuple(types[module.value_types[value]][1] for value in push_values) ==
            tuple(_source_scalar_kind(row[3])
                  for row in witness.tables["arguments"]),
            "push call literal contextual carriers")
    read_values = arguments(module, read_call)[1:]
    require(len(read_values) == 1 and _constant(module, read_values[0]) == 0,
            "read_tag index-zero call")
    root_term = module.tables["terminators"][6]
    require(root_term[3] == 4 and root_term[6] == read_call[6],
            "root returns exact read_tag result")
