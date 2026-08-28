#!/usr/bin/env python3
"""Independent CKIR20 structure and lowering observations for OMGRFN23."""

from __future__ import annotations

import importlib.util
import sys
from dataclasses import dataclass
from pathlib import Path

from omgrfn23_frame import RefinementError, RefinementResourceError, require

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


V5 = load("omgrfn23_checked_ir_v5_reference", GATES / "checked_ir_v5_reference.py")
IR20 = load("omgrfn23_checked_ir_v20_reference", GATES / "checked_ir_v20_reference.py")
NO_ID = V5.NO_ID


def _translate(label: str, action):
    try:
        return action()
    except (V5.Ckir5ResourceError, IR20.Ckir20ResourceError) as error:
        raise RefinementResourceError(f"{label}: {error}") from error
    except Exception as error:
        raise RefinementError(f"{label}: {error}") from error


def meaning_decode(contents: bytes):
    return _translate("CKIR20 meaning", lambda: IR20.decode(contents))


def producer_decode(contents: bytes):
    # V5 owns generic schema reconstruction.  The selected profile below is
    # deliberately duplicated here instead of importing IR20.profile/verdicts.
    module = _translate(
        "CKIR20 producer structure",
        lambda: V5.decode(contents, expected_major=20,
                          capabilities=IR20.CAPABILITIES),
    )
    check_selected_structure(module)
    return module


def arguments(module, operation: tuple[int, ...]) -> tuple[int, ...]:
    return tuple(module.tables["operands"][index][0]
                 for index in range(operation[8], operation[8] + operation[9]))


def definitions(module) -> dict[int, tuple[int, ...]]:
    return {op[6]: op for op in module.tables["operations"] if op[4] == 1}


def place_definitions(module) -> dict[int, tuple[int, ...]]:
    return {op[6]: op for op in module.tables["operations"] if op[4] == 2}


def place_paths(module) -> dict[int, tuple[object, ...]]:
    paths: dict[int, tuple[object, ...]] = {}
    for op in module.tables["operations"]:
        args = arguments(module, op)
        if op[3] == 2:
            paths[op[6]] = ("self", op[1])
        elif op[3] == 3 and len(args) == 1 and args[0] in paths:
            paths[op[6]] = paths[args[0]] + (("field", op[10]),)
        elif op[3] == 4 and len(args) == 2 and args[0] in paths:
            paths[op[6]] = paths[args[0]] + (("index", op[7]),)
    return paths


def direct_self_field(module, place: int, pdefs=None) -> int | None:
    pdefs = place_definitions(module) if pdefs is None else pdefs
    operation = pdefs.get(place)
    if operation is None or operation[3] != 3:
        return None
    args = arguments(module, operation)
    base = pdefs.get(args[0]) if len(args) == 1 else None
    return operation[10] if base is not None and base[3] == 2 else None


@dataclass(frozen=True)
class Selected:
    source_id_record: int
    span_record: int
    source_span_record: int
    token_record: int
    diagnostic_record: int
    observation_record: int
    stream_record: int
    root_record: int
    token_kind_sum: int
    stream_type: int
    root_type: int
    writer: int
    reader: int
    entry: int
    copies: tuple[tuple[int, ...], ...]
    stores: tuple[tuple[int, ...], ...]
    indexes: tuple[tuple[int, ...], ...]
    source_load: tuple[int, ...]
    add: tuple[int, ...]
    lesses: tuple[tuple[int, ...], ...]
    calls: tuple[tuple[int, ...], ...]


def _record_fields(module, record_id: int) -> tuple[tuple[int, ...], ...]:
    record = module.tables["records"][record_id]
    return tuple(module.tables["fields"][record[2]:record[2] + record[3]])


def check_selected_structure(module) -> Selected:
    t = module.tables
    types, records, fields = t["types"], t["records"], t["fields"]
    sums, cases, payloads = t["sums"], t["cases"], t["case_payloads"]
    require((len(types), len(records), len(fields), len(sums), len(cases),
             len(payloads)) == (24, 8, 29, 5, 105, 8),
            "exact 24-type/8-record/29-field/5-sum/105-case census")
    require({name: len(t[name]) for name in V5.TABLE_ORDER} == {
        "types": 24, "records": 8, "fields": 29, "sums": 5,
        "cases": 105, "case_payloads": 8, "constants": 0,
        "constant_children": 0, "machines": 3, "machine_params": 11,
        "blocks": 14, "block_params": 11, "operations": 183,
        "operands": 180, "terminators": 14, "case_arms": 9,
        "case_arm_args": 8,
    } and (len(module.value_types), len(module.place_types)) == (67, 118),
            "complete CKIR20 table/value/place census")
    require([row[0] for row in records] == list(range(8))
            and [row[1] for row in records] == list(range(8)),
            "dense exact record/nominal identities")
    require([row[0] for row in sums] == list(range(5))
            and [row[1] for row in sums] == list(range(8, 13)),
            "dense exact sum/nominal identities")
    require([row[4] for row in records] == [1, 1, 1, 1, 1, 1, 0, 0]
            and all(row[4] == 1 for row in sums),
            "recursive nested copy declarations")
    require([row[3] for row in sums] == [4, 30, 42, 9, 20],
            "exact sum case partitions")
    token_kind_sum = 3
    token_kind_cases = cases[sums[token_kind_sum][2]:
                             sums[token_kind_sum][2] + sums[token_kind_sum][3]]
    require([row[4] for row in token_kind_cases] == [0, 3, 3, 0, 1, 1, 0, 0, 0],
            "TokenKind exact payload arities")
    require([row[3] for row in payloads] == [8, 15, 15, 15, 15, 15, 9, 10],
            "TokenKind exact payload carriers")
    expected_fields = (
        (14,), (16, 16), (0, 1), (11, 2, 16, 16), (12, 2),
        (13, 13, 13, 13, 14, 16, 16, 16, 16),
        (21, 22, 17, 23, 16, 4, 15, 15), (6,),
    )
    require(tuple(tuple(row[3] for row in _record_fields(module, rid))
                  for rid in range(8)) == expected_fields,
            "exact record field ownership/types")
    require(types[21][1:6] == (5, 1, 0, 3, 16_384)
            and types[22][1:6] == (5, 1, 0, 5, 16_384)
            and types[23][1:6] == (5, 1, 0, 13, 65_536),
            "exact trapping Token/Observation/decoded arrays")
    require(V5._u64_type_bounds(types[17]) == (0, 16_384),
            "Exact token-count interval")
    expected_layouts = {
        0: (4, 4), 1: (16, 8), 2: (24, 8), 3: (56, 8),
        4: (32, 8), 5: (40, 8), 6: (1_638_456, 8),
        7: (1_638_456, 8), 8: (4, 4), 9: (4, 4),
        10: (4, 4), 11: (12, 4), 12: (4, 4),
    }
    require(all(module.layouts[type_id] == layout
                for type_id, layout in expected_layouts.items()),
            "exact private aggregate layouts")
    require(tuple(module.field_offsets) == (
        0, 0, 8, 0, 8, 0, 16, 40, 48, 0, 8,
        0, 1, 2, 3, 4, 8, 16, 24, 32,
        0, 917_504, 1_572_864, 1_572_872, 1_638_408,
        1_638_416, 1_638_448, 1_638_449, 0,
    ), "exact private field offsets")
    require(module.layouts[7][0] <= 2 * 1024 * 1024,
            "selected owner at most 2 MiB")

    machines = t["machines"]
    require(len(machines) == 3 and module.entry == 2,
            "push/read_kind/entry exact machine selection")
    require([(m[1], m[2], m[5], m[7], m[9]) for m in machines] == [
        (6, 2, NO_ID, 10, 3), (6, 1, 13, 1, 10), (7, 2, 13, 0, 1),
    ], "exact machine signatures")
    require([row[3] for row in t["machine_params"]] ==
            [0, 11, 16, 16, 16, 16, 13, 13, 13, 13, 16],
            "two structural plus scalar parameter custody")
    require(len(t["block_params"]) == 11,
            "Float payload/control binders")

    paths = place_paths(module)
    retain = [op for op in t["operations"] if op[1] == 0 and op[2] == 1]
    copies = tuple(op for op in retain if op[3] == 7)
    all_stores = tuple(op for op in retain if op[3] == 6)
    token_kind_path = ("self", 0, ("field", 20), ("index", 3), ("field", 5))
    source_id_path = token_kind_path[:-1] + (("field", 6), ("field", 3))
    copy_paths = {paths[arguments(module, op)[0]] for op in copies}
    require(len(copies) == 2 and copy_paths == {token_kind_path, source_id_path}
            and all(op[10] == 1 for op in copies),
            "whole TokenKind/SourceId Copy into indexed Token")
    stores = tuple(op for op in all_stores
                   if ("index", 3) in paths[arguments(module, op)[0]]
                   or ("index", 5) in paths[arguments(module, op)[0]])
    store_paths = [paths[arguments(module, op)[0]] for op in stores]
    token_fields = {path[-1][1] for path in store_paths
                    if len(path) >= 5 and path[2:4] ==
                    (("field", 20), ("index", 3)) and path[-1][0] == "field"}
    observation_fields = {path[-1][1] for path in store_paths
                          if len(path) == 5 and path[2:4] ==
                          (("field", 21), ("index", 5))}
    require(len(all_stores) == 15 and len(stores) == 13
            and token_fields == {1, 2, 7, 8}
            and observation_fields == set(range(11, 20)),
            "exact 13 scalar data stores plus Exact/status stores")
    source_value_path = source_id_path + (("field", 0),)
    source_loads = tuple(op for op in retain if op[3] == 5
                         and paths[arguments(module, op)[0]] == source_value_path)
    require(len(source_loads) == 1, "exact source.value projection")
    source_stores = [op for op in stores
                     if paths[arguments(module, op)[0]][-1] == ("field", 15)]
    require(len(source_stores) == 1
            and arguments(module, source_stores[0])[1] == source_loads[0][6],
            "source.value feeds observation source")

    indexes = tuple(op for op in t["operations"] if op[3] == 4
                    and types[op[7]][1] in (4, 6))
    require(len(indexes) >= 17 and all(op[10] == op[11] == 0 for op in indexes),
            "full-u64 aggregate indexes have no narrowing immediates")
    adds = tuple(op for op in t["operations"] if op[3] == 8
                 and types[module.value_types[arguments(module, op)[0]]][1] == 8)
    lesses = tuple(op for op in t["operations"] if op[3] == 9)
    require(len(adds) == 1 and len(lesses) >= 2,
            "guarded full-u64 index and Exact increment")
    dispatches = [term for term in t["terminators"] if term[3] == 5]
    require(len(dispatches) == 1 and dispatches[0][4] == 2
            and dispatches[0][14] == 9,
            "indexed TokenKind exact CaseDispatch")
    require(t["case_arms"][2][2] == 78 and t["case_arms"][2][5] == 3,
            "Float(true,false,true) payload readback binders")
    calls = tuple(op for op in t["operations"] if op[3] == 10)
    require([(op[10], op[9], op[4]) for op in calls] == [(0, 11, 0), (1, 2, 1)],
            "10-argument push and indexed read_kind calls")
    constructors = [op for op in t["operations"] if op[3] in (13, 14)]
    require([(op[3], op[7], op[10]) for op in constructors] ==
            [(13, 0, 0), (14, 11, 78)],
            "SourceId and Float(true,false,true) construction")
    return Selected(0, 1, 2, 3, 4, 5, 6, 7, token_kind_sum,
                    records[6][1], records[7][1], 0, 1, 2,
                    copies, stores, indexes, source_loads[0], adds[0],
                    lesses, calls)
