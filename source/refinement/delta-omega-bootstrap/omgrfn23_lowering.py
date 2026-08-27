#!/usr/bin/env python3
"""Exact independent OMGRSWC12-to-CKIR20 lowering join."""

from __future__ import annotations

from omgrfn23_ckir import arguments, definitions, place_paths, producer_decode
from omgrfn23_frame import require
from omgrfn23_source import check_witness_relation


def check_lowering(omgcomp: bytes, witness_raw: bytes, ckir_raw: bytes) -> None:
    witness, _ = check_witness_relation(omgcomp, witness_raw)
    module = producer_decode(ckir_raw)
    t = module.tables

    type_map = {**{type_id: type_id for type_id in range(18)},
                18: 16, 19: 21, 20: 22, 21: 23}
    require(tuple(type_map[row[3]] for row in witness.tables["fields"]) ==
            tuple(row[3] for row in t["fields"]),
            "source/CKIR exact field carrier join")
    require(tuple(row[7] for row in witness.tables["records"]) ==
            tuple(row[4] for row in t["records"]),
            "source/CKIR recursive record-copy join")
    require(tuple(row[7] for row in witness.tables["sums"]) ==
            tuple(row[4] for row in t["sums"]),
            "source/CKIR recursive sum-copy join")
    require(tuple(row[3] for row in witness.tables["params"]) ==
            tuple(row[3] for row in t["machine_params"]),
            "source/CKIR machine parameter join")
    require(tuple(row[3] for row in witness.tables["block_params"]) ==
            tuple(row[3] for row in t["block_params"]),
            "source/CKIR payload/control binder join")

    paths = place_paths(module)
    retain = [op for op in t["operations"] if op[1] == 0 and op[2] == 1]
    data_ops = [op for op in retain if op[3] in (6, 7)
                and (("index", 3) in paths[arguments(module, op)[0]]
                     or ("index", 5) in paths[arguments(module, op)[0]])]
    lowered_destinations = set()
    for op in data_ops:
        path = paths[arguments(module, op)[0]]
        array_at = next(index for index, step in enumerate(path)
                        if step in (("field", 20), ("field", 21)))
        lowered_destinations.add((path[array_at][1],
                                  tuple(step[1] for step in path[array_at + 2:])))
    store_paths = tuple(row[0] for row in witness.tables["store_paths"])
    authored_destinations = {
        (row[3], tuple(store_paths[row[4]:row[4] + row[5]]))
        for row in witness.tables["stores"]
    }
    require(len(data_ops) == 15 and lowered_destinations == authored_destinations,
            "all 15 authored data assignments lower exactly once")

    calls = [op for op in t["operations"] if op[3] == 10]
    require(tuple(row[8] for row in witness.tables["calls"]) ==
            tuple(op[9] - 1 for op in calls),
            "source/CKIR exact 10+1 call-argument join")
    defs = definitions(module)
    push_values = arguments(module, calls[0])[1:]
    read_values = arguments(module, calls[1])[1:]
    require(len(push_values) == 10 and len(read_values) == 1,
            "exact root call values")
    source_constructor = defs.get(push_values[0])
    kind_constructor = defs.get(push_values[1])
    require(source_constructor is not None and source_constructor[3] == 13
            and source_constructor[7] == 0 and source_constructor[10] == 0,
            "SourceId constructor lowering")
    source_args = arguments(module, source_constructor)
    require(len(source_args) == 1 and defs[source_args[0]][3] == 1
            and defs[source_args[0]][10:12] == (4, 0),
            "SourceId value 4 lowering")
    require(kind_constructor is not None and kind_constructor[3] == 14
            and kind_constructor[7] == 11 and kind_constructor[10] == 78,
            "TokenKind::Float constructor lowering")
    kind_args = arguments(module, kind_constructor)
    require(tuple(defs[value][10:12] for value in kind_args) ==
            ((1, 0), (0, 0), (1, 0)),
            "Float(true,false,true) lowering")
    literal_values = push_values[2:] + read_values
    require(tuple(defs[value][10:12] for value in literal_values) ==
            tuple((value, 0) for value in (5, 6, 7, 8, 70, 1, 2, 3, 0)),
            "full-width/root literal identity without high-half drift")
    require([(row[3], row[9]) for row in witness.tables["stores"][:2]] ==
            [(20, 22), (20, 22)] and [op[3] for op in data_ops[:2]] == [7, 7],
            "whole TokenKind/SourceId assignments lower as structural Copy")
    require(witness.owner_size == module.layouts[module.tables["records"][7][1]][0]
            == 1_638_456,
            "source/CKIR exact selected owner layout join")
