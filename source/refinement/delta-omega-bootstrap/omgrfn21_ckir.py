#!/usr/bin/env python3
"""Independent CKIR18 fixed-buffer observations and source lowering join."""

from __future__ import annotations

import importlib.util
import sys
from dataclasses import dataclass
from pathlib import Path

from omgrfn18_u64 import U64, bounds
from omgrfn21_frame import RefinementError, RefinementResourceError, require

HERE = Path(__file__).resolve().parent
GATES = HERE.parents[3] / "source/on-ramp/omega-bootstrap/gates"
sys.path.insert(0, str(GATES))


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RefinementError(f"cannot load {path.name}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


V5 = load("omgrfn21_checked_ir_v5_reference", GATES / "checked_ir_v5_reference.py")
IR18 = load("omgrfn21_checked_ir_v18_reference", GATES / "checked_ir_v18_reference.py")
NO_ID = V5.NO_ID


def _translate(label: str, action):
    try:
        return action()
    except (V5.Ckir5ResourceError, IR18.Ckir18ResourceError) as error:
        raise RefinementResourceError(f"{label}: {error}") from error
    except Exception as error:
        raise RefinementError(f"{label}: {error}") from error


def decode(contents: bytes):
    return _translate("CKIR18 structure", lambda: IR18.decode(contents))


def producer_decode(contents: bytes):
    module = _translate(
        "CKIR18 producer structure",
        lambda: V5.decode(contents, expected_major=18,
                          capabilities=IR18.CAPABILITIES),
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


def constant_u64(module, value: int, defs=None) -> U64 | None:
    operation = (definitions(module) if defs is None else defs).get(value)
    if operation is None or operation[3] != 1:
        return None
    if module.tables["types"][module.value_types[value]][1] != 8:
        return None
    return U64(operation[10], operation[11])


def direct_field(module, place: int, pdefs=None) -> int | None:
    pdefs = place_definitions(module) if pdefs is None else pdefs
    operation = pdefs.get(place)
    if operation is None or operation[3] != 3:
        return None
    args = arguments(module, operation)
    if len(args) != 1:
        return None
    base = pdefs.get(args[0])
    return operation[10] if base is not None and base[3] == 2 else None


def direct_field_load(module, value: int, defs=None, pdefs=None) -> int | None:
    defs = definitions(module) if defs is None else defs
    operation = defs.get(value)
    if operation is None or operation[3] != 5:
        return None
    args = arguments(module, operation)
    return direct_field(module, args[0], pdefs) if len(args) == 1 else None


@dataclass(frozen=True)
class Selected:
    append_machine: int
    lookup_machine: int
    array_field: int
    length_field: int
    append_less: tuple[int, ...]
    lookup_less: tuple[int, ...]
    store_index: tuple[int, ...]
    load_index: tuple[int, ...]
    add: tuple[int, ...]
    length_type: int
    index_type: int
    array_type: int
    capacity: U64


def check_selected_structure(module) -> Selected:
    types = module.tables["types"]
    operations = module.tables["operations"]
    defs, pdefs = definitions(module), place_definitions(module)
    require(not any(row[1] == 7 for row in types),
            "fixed-buffer slice excludes shared/mutable views")
    require(all(row[2] == 0 and row[3] == 0 for row in types if row[1] == 8),
            "CKIR18 kind-8 policy-neutral rows")
    arrays = [row for row in types if row[1] == 5]
    require(len(arrays) == 1 and 1 <= arrays[0][5] <= 65_536,
            "one bounded fixed-array type")
    array = arrays[0]
    require(types[array[4]][1:] == (1, 0, 0, 0, 0, 0, 255),
            "fixed array has exact u8 elements")

    indexes = [op for op in operations if op[3] == 4
               and types[module.value_types[arguments(module, op)[1]]][1] == 8]
    adds = [op for op in operations if op[3] == 8
            and types[module.value_types[arguments(module, op)[0]]][1] == 8]
    lesses = [op for op in operations if op[3] == 9
              and types[module.value_types[arguments(module, op)[0]]][1] == 8]
    require((len(indexes), len(adds), len(lesses)) == (2, 1, 2),
            "exact two IndexPlace, one Add, two Less u64 slice")
    require(not any(op[3] in (12, 18, 19, 20, 26, 27)
                    and any(types[module.value_types[value]][1] == 8
                            for value in arguments(module, op))
                    for op in operations), "unrelated u64 operation exclusion")

    index_uses: dict[int, list[tuple[int, ...]]] = {op[6]: [] for op in indexes}
    for operation in operations:
        for value in arguments(module, operation):
            if value in index_uses:
                index_uses[value].append(operation)
    store_index = next((op for op in indexes
                        if any(use[3] == 6 and arguments(module, use)[0] == op[6]
                               for use in index_uses[op[6]])), None)
    load_index = next((op for op in indexes
                       if any(use[3] == 5 and arguments(module, use)[0] == op[6]
                              for use in index_uses[op[6]])), None)
    require(store_index is not None and load_index is not None
            and store_index != load_index, "one indexed Store and one indexed Load")
    require(all(op[10] == op[11] == 0 for op in indexes + adds + lesses),
            "selected operations have zero immediates")

    store_args, load_args = arguments(module, store_index), arguments(module, load_index)
    store_array = direct_field(module, store_args[0], pdefs)
    load_array = direct_field(module, load_args[0], pdefs)
    require(store_array is not None and store_array == load_array,
            "indexed Store/Load share direct array field")
    require(module.place_types[store_args[0]] == module.place_types[load_args[0]]
            == array[0], "indexed bases use selected fixed array")
    require(store_index[7] == load_index[7] == array[4],
            "IndexPlace result is exact u8 element")

    add = adds[0]
    add_args = arguments(module, add)
    literal_values = [value for value in add_args
                      if constant_u64(module, value, defs) == U64(1, 0)]
    load_values = [value for value in add_args
                   if direct_field_load(module, value, defs, pdefs) is not None]
    require(len(literal_values) == len(load_values) == 1
            and set(add_args) == {literal_values[0], load_values[0]},
            "direct u64 leaf-plus-literal-one Add")
    length_field = direct_field_load(module, load_values[0], defs, pdefs)
    length_type = module.value_types[load_values[0]]
    require(add[7] == length_type
            and bounds(types[length_type]) == (U64(0, 0), U64(array[5], 0)),
            "Add exact retained-length result interval")
    add_stores = [op for op in operations if op[3] == 6
                  and arguments(module, op)[1] == add[6]]
    require(len(add_stores) == 1
            and direct_field(module, arguments(module, add_stores[0])[0], pdefs)
                == length_field, "increment returns to retained-length field")

    append_machine = add[1]
    require(store_index[1] == append_machine, "append owns indexed Store and Add")
    store_index_value = store_args[1]
    require(direct_field_load(module, store_index_value, defs, pdefs) == length_field,
            "append index is direct retained-length load")
    append_less = next((op for op in lesses if op[1] == append_machine), None)
    require(append_less is not None, "append owns one u64 guard")
    append_args = arguments(module, append_less)
    require(direct_field_load(module, append_args[0], defs, pdefs) == length_field
            and constant_u64(module, append_args[1], defs) == U64(array[5], 0),
            "append guard is direct length < N")

    lookup_machine = load_index[1]
    require(lookup_machine != append_machine, "lookup is distinct shared machine")
    lookup_less = next((op for op in lesses if op[1] == lookup_machine), None)
    require(lookup_less is not None, "lookup owns one u64 guard")
    lookup_args = arguments(module, lookup_less)
    index_value = load_args[1]
    require(lookup_args[0] == index_value
            and direct_field_load(module, lookup_args[1], defs, pdefs) == length_field,
            "lookup guard/index preserve direct parameter identity")
    require(index_value < len(module.value_types)
            and module.tables["types"][module.value_types[index_value]][1] == 8,
            "lookup index full-u64 carrier")

    branches = {term[6]: term for term in module.tables["terminators"] if term[3] == 2}
    append_branch, lookup_branch = branches.get(append_less[6]), branches.get(lookup_less[6])
    require(append_branch is not None and append_branch[7] == store_index[2],
            "append true edge owns IndexPlace/Add block")
    require(lookup_branch is not None and lookup_branch[7] == load_index[2],
            "lookup true edge owns IndexPlace/Load block")

    return Selected(append_machine, lookup_machine, store_array, length_field,
                    append_less, lookup_less, store_index, load_index, add,
                    length_type, module.value_types[index_value], array[0],
                    U64(array[5], 0))


def check_lowering(module, witness, source) -> Selected:
    selected = check_selected_structure(module)
    defs, pdefs = definitions(module), place_definitions(module)
    require(selected.append_machine == witness.append_machine
            and selected.lookup_machine == witness.lookup_machine,
            "source/witness machine identities reach CKIR18")
    witness_fields = witness.tables["fields"]
    source_array = next(row for row in witness_fields
                        if row[1] == witness.selected_record
                        and row[3] == witness.array_type)
    source_length = next(row for row in witness_fields
                         if row[1] == witness.selected_record
                         and row[3] == witness.length_type)
    require((selected.array_field, selected.length_field) ==
            (source_array[2], source_length[2]),
            "source field ordinals reach indexed array/length CKIR places")
    types = module.tables["types"]
    require(bounds(types[selected.index_type]) == (U64(0, 0),
                                                   U64(0xFFFF_FFFF, 0xFFFF_FFFF)),
            "trapping source index maps to policy-neutral full-u64 CKIR")
    require(bounds(types[selected.length_type]) == (U64(0, 0),
                                                    U64(source.length, 0)),
            "exact source length interval reaches CKIR")
    require(selected.capacity == U64(source.length, 0),
            "authored fixed-array N reaches both guards and array layout")
    append_branch = next(term for term in module.tables["terminators"]
                         if term[3] == 2 and term[6] == selected.append_less[6])
    require(selected.store_index[2] == selected.add[2] == append_branch[7],
            "append true edge owns both indexed Store and Exact Add")
    add_args = arguments(module, selected.add)
    require(direct_field_load(module, add_args[0], defs, pdefs)
                == selected.length_field
            and constant_u64(module, add_args[1], defs) == U64(1, 0),
            "authored leaf-plus-literal Add operand order")
    # Independent erased-fact proof.  Append true means length <= N-1, so its
    # index is in bounds and exact length+1 has neither u64 carry nor interval
    # failure. Lookup true means index < length <= N, hence index < N. CKIR's
    # defensive IndexPlace/Add traps remain present but unreachable here.
    require(source.length > 0 and source.length - 1 < source.length
            and source.length <= 0xFFFF_FFFF_FFFF_FFFF,
            "true-edge IndexPlace/Add safety proof")
    return selected


def interpret(module) -> int | None:
    return _translate("CKIR18 execution", lambda: IR18.interpret(module))
