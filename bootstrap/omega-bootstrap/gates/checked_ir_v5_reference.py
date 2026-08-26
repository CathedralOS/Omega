#!/usr/bin/env python3
"""Independent CKIR5 decoder and bounded interpreter.

The frozen CKIR4 helpers remain responsible for the inherited constant graph
and declaration projection.  This module independently reconstructs CKIR5's
sum declarations, private layouts, ConstructCase, and CaseDispatch edges.
"""

from __future__ import annotations

import argparse
import dataclasses
import struct
from pathlib import Path

import checked_ir_v2_reference as v2
import checked_ir_v3_reference as v3
import checked_ir_v4_reference as v4


NO_ID = 0xFFFF_FFFF
HEADER = struct.Struct("<8sHHHH21I")
ROWS = {
    "types": v2.ROWS["types"],
    "records": v2.ROWS["records"],
    "fields": v2.ROWS["fields"],
    "sums": struct.Struct("<IIIIBBBB"),
    "cases": struct.Struct("<IIIII"),
    "case_payloads": struct.Struct("<IIII"),
    "machines": v2.ROWS["machines"],
    "machine_params": v2.ROWS["machine_params"],
    "blocks": v2.ROWS["blocks"],
    "block_params": v2.ROWS["block_params"],
    "constants": v3.ROWS["constants"],
    "constant_children": v3.ROWS["constant_children"],
    "operations": v2.ROWS["operations"],
    "operands": v2.ROWS["operands"],
    "terminators": struct.Struct("<IIIBBHIIIIIIIII"),
    "case_arms": struct.Struct("<IIIIII"),
    "case_arm_args": struct.Struct("<IB3xI"),
}
TABLE_ORDER = tuple(ROWS)
COUNT_NAMES = TABLE_ORDER + ("values", "places")


class Ckir5Error(v4.Ckir4Error):
    pass


class Ckir5ResourceError(Ckir5Error):
    """A validated public CKIR5 extent selects status 252."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise Ckir5Error(message)


def span(start: int, count: int, length: int, label: str) -> range:
    require(start <= length and count <= length - start, f"bad {label} span")
    return range(start, start + count)


def align(value: int, alignment: int) -> int:
    return (value + alignment - 1) // alignment * alignment


@dataclasses.dataclass(frozen=True)
class Module:
    entry: int
    tables: dict[str, list[tuple[int, ...]]]
    layouts: tuple[tuple[int, int], ...]
    field_offsets: tuple[int, ...]
    payload_offsets: tuple[int, ...]
    sum_payload_offsets: tuple[int, ...]
    value_types: tuple[int, ...]
    place_types: tuple[int, ...]


def _project_declarations(
    entry: int, flags: int, tables: dict[str, list[tuple[int, ...]]],
) -> None:
    """Ask CKIR4's frozen projection to recheck inherited declarations.

    Nominal sums become opaque empty copyable records only in this validation
    projection.  Their actual identity, copyability, and layout are checked
    independently below and never taken from the projection.
    """
    projected = {name: list(tables[name]) for name in (
        "types", "records", "fields", "machines", "machine_params"
    )}
    rewritten = []
    for row in projected["types"]:
        if row[1] in (6, 7):
            record_id = len(projected["records"])
            projected["records"].append(
                (record_id, row[0], len(projected["fields"]), 0, 1, 0, 0, 0)
            )
            rewritten.append((row[0], 4, 0, 0, record_id, 0, 0, 0))
        else:
            rewritten.append(row)
    projected["types"] = rewritten
    v4._declaration_projection(entry, flags, projected)


def decode(contents: bytes, *, expected_major: int = 5,
           allow_logical_not: bool = False,
           allow_logical_binary: bool = False,
           allow_scalar_equal: bool = False,
           allow_greater: bool = False,
           allow_integer_widen: bool = False,
           require_trapping_add: bool = False,
           allow_static_byte_view: bool = False,
           allow_full_u32_subtract: bool = False) -> Module:
    require(expected_major in (5, 6, 7, 8, 9, 10, 11, 12, 13),
            "internal CKIR schema selection")
    require(allow_logical_not == (expected_major >= 6)
            and allow_logical_binary == (expected_major >= 7)
            and allow_scalar_equal == (expected_major >= 8)
            and allow_greater == (expected_major >= 9)
            and allow_integer_widen == (expected_major >= 10)
            and require_trapping_add == (expected_major == 11)
            and allow_static_byte_view == (expected_major == 12)
            and allow_full_u32_subtract == (expected_major == 13),
            "internal CKIR feature selection")
    require(len(contents) >= HEADER.size, "truncated CKIR header")
    magic, major, minor, target, flags, entry, total, *raw_counts = HEADER.unpack_from(contents)
    require(magic == b"OMGCKIR\0" and (major, minor, target) == (expected_major, 0, 1),
            "bad CKIR schema or target")
    require(flags in (0, 1) and (entry != NO_ID) == bool(flags & 1), "bad entry flags")
    require(total == len(contents), "CKIR length mismatch")
    if len(contents) > 2_522_192:
        raise Ckir5ResourceError("CKIR5 byte exhaustion")
    require(len(raw_counts) == len(COUNT_NAMES), "internal CKIR5 count schema")
    counts = dict(zip(COUNT_NAMES, raw_counts))
    ceilings = {
        "types": 8_192, "records": 128, "fields": 8_192, "sums": 128,
        "cases": 4_096, "case_payloads": 4_096, "machines": 128,
        "machine_params": 896, "blocks": 2_048, "block_params": 4_096,
        "constants": 8_192, "constant_children": 16_384,
        "operations": 32_768, "operands": 94_208, "terminators": 2_048,
        "case_arms": 4_096, "case_arm_args": 94_208,
        "values": 36_864, "places": 32_768,
    }
    if any(counts[name] > ceiling for name, ceiling in ceilings.items()):
        raise Ckir5ResourceError("CKIR5 table exhaustion")
    if counts["records"] + counts["sums"] > 128:
        raise Ckir5ResourceError("combined nominal exhaustion")
    if counts["machine_params"] + counts["block_params"] > 4_096:
        raise Ckir5ResourceError("combined parameter exhaustion")
    if counts["fields"] + counts["case_payloads"] > 8_192:
        raise Ckir5ResourceError("combined raw-type exhaustion")
    if counts["operands"] + counts["case_arm_args"] > 94_208:
        raise Ckir5ResourceError("combined argument exhaustion")
    expected = HEADER.size + sum(ROWS[name].size * counts[name] for name in TABLE_ORDER)
    require(expected == len(contents), "noncanonical CKIR5 table extent")

    tables: dict[str, list[tuple[int, ...]]] = {}
    cursor = HEADER.size
    for name in TABLE_ORDER:
        row = ROWS[name]
        tables[name] = [
            row.unpack_from(contents, cursor + index * row.size)
            for index in range(counts[name])
        ]
        cursor += row.size * counts[name]
    require(cursor == len(contents), "CKIR5 trailing bytes")

    dense = (
        "types", "records", "fields", "sums", "cases", "case_payloads",
        "machines", "machine_params", "blocks", "block_params", "constants",
        "operations", "terminators", "case_arms", "case_arm_args",
    )
    for name in dense:
        for index, row in enumerate(tables[name]):
            require(row[0] == index, f"non-dense {name} ID")

    types, records, fields = tables["types"], tables["records"], tables["fields"]
    sums, cases, payloads = tables["sums"], tables["cases"], tables["case_payloads"]
    nominal_records: dict[int, int] = {}
    nominal_sums: dict[int, int] = {}
    type_keys: set[tuple[int, ...]] = set()
    for type_id, kind, type_flags, reserved, payload0, payload1, low, high in types:
        require(kind in range(1, 8 if allow_static_byte_view else 7)
                and reserved == 0 and type_flags <= 1, "bad type row")
        require(kind not in (3, 4, 6, 7) or type_flags == 0, "forbidden type flag")
        if kind in (1, 2, 3):
            require(payload0 == payload1 == 0 and low <= high, "bad scalar type")
            require(high <= (255 if kind == 1 else 1 if kind == 3 else
                             0xFFFF_FFFF if allow_full_u32_subtract else 0x7FFF_FFFF),
                    "scalar range")
            if kind == 3:
                require((low, high) == (0, 1), "bool range")
        elif kind == 4:
            require(payload0 < len(records) and payload1 == low == high == 0,
                    "bad record type")
            require(payload0 not in nominal_records, "duplicate record nominal")
            nominal_records[payload0] = type_id
        elif kind == 5:
            require(payload0 < len(types) and payload1 <= 65_536 and low == high == 0,
                    "bad array type")
        elif kind == 6:
            require(payload0 < len(sums) and payload1 == low == high == 0,
                    "bad sum type")
            require(payload0 not in nominal_sums, "duplicate sum nominal")
            nominal_sums[payload0] = type_id
        else:
            require(payload0 < len(types) and payload1 == low == high == 0,
                    "bad shared byte-slice type")
            element = types[payload0]
            require(element == (payload0, 1, 0, 0, 0, 0, 0, 255),
                    "shared byte-slice element type")
        key = (kind, type_flags, payload0, payload1, low, high)
        require(key not in type_keys, "duplicate interned type")
        type_keys.add(key)

    next_field = 0
    for record_id, nominal, start, count, record_flags, r0, r1, r2 in records:
        require(record_flags <= 1 and r0 == r1 == r2 == 0 and nominal < len(types),
                "bad record row")
        require(types[nominal][1] == 4 and types[nominal][4] == record_id,
                "record nominal")
        require(start == next_field and count <= 64, "field partition")
        for ordinal, field_id in enumerate(span(start, count, len(fields), "field")):
            field = fields[field_id]
            require(field == (field_id, record_id, ordinal, field[3])
                    and field[3] < len(types), "bad field row")
        next_field += count
    require(next_field == len(fields) and len(nominal_records) == len(records),
            "record field/nominal partition")

    next_case = next_payload = 0
    for sum_id, nominal, case_start, case_count, sum_flags, r0, r1, r2 in sums:
        require(sum_flags <= 1 and r0 == r1 == r2 == 0 and nominal < len(types),
                "bad sum row")
        require(types[nominal][1] == 6 and types[nominal][4] == sum_id, "sum nominal")
        require(case_start == next_case and case_count >= 1, "case partition")
        span(case_start, case_count, len(cases), "sum cases")
        if case_count > 64:
            raise Ckir5ResourceError("cases-per-sum exhaustion")
        for ordinal, case_id in enumerate(range(case_start, case_start + case_count)):
            case = cases[case_id]
            require(case[:3] == (case_id, sum_id, ordinal), "bad case owner/ordinal")
            require(case[3] == next_payload, "payload partition")
            span(case[3], case[4], len(payloads), "case payload")
            if case[4] > 4:
                raise Ckir5ResourceError("payload-fields-per-case exhaustion")
            for payload_ordinal, payload_id in enumerate(range(case[3], case[3] + case[4])):
                payload = payloads[payload_id]
                require(payload == (payload_id, case_id, payload_ordinal, payload[3])
                        and payload[3] < len(types), "bad payload field")
            next_payload += case[4]
        next_case += case_count
    require(next_case == len(cases) and next_payload == len(payloads)
            and len(nominal_sums) == len(sums), "sum case/payload/nominal partition")

    visiting: set[int] = set()
    layout_cache: dict[int, tuple[int, int]] = {}
    field_offsets = [0] * len(fields)
    payload_offsets = [0] * len(payloads)
    sum_payload_offsets = [0] * len(sums)

    def type_layout(type_id: int) -> tuple[int, int]:
        if type_id in layout_cache:
            return layout_cache[type_id]
        require(type_id not in visiting, "recursive by-value layout")
        visiting.add(type_id)
        _, kind, _, _, payload0, payload1, _, _ = types[type_id]
        if kind in (1, 3):
            result = (1, 1)
        elif kind == 2:
            result = (4, 4)
        elif kind == 7:
            result = (16, 8)
        elif kind == 5:
            size, alignment = type_layout(payload0)
            result = (align(size, alignment) * payload1, alignment)
        elif kind == 4:
            record = records[payload0]
            cursor, aggregate_alignment = 0, 1
            for field_id in span(record[2], record[3], len(fields), "layout fields"):
                size, alignment = type_layout(fields[field_id][3])
                cursor = align(cursor, alignment)
                field_offsets[field_id] = cursor
                cursor += size
                aggregate_alignment = max(aggregate_alignment, alignment)
            result = (align(cursor, aggregate_alignment), aggregate_alignment)
        else:
            sum_row = sums[payload0]
            payload_alignment, max_payload_size = 1, 0
            for case_id in span(sum_row[2], sum_row[3], len(cases), "layout cases"):
                case = cases[case_id]
                cursor, case_alignment = 0, 1
                for payload_id in span(case[3], case[4], len(payloads), "layout payloads"):
                    size, alignment = type_layout(payloads[payload_id][3])
                    cursor = align(cursor, alignment)
                    payload_offsets[payload_id] = cursor
                    cursor += size
                    case_alignment = max(case_alignment, alignment)
                payload_alignment = max(payload_alignment, case_alignment)
                max_payload_size = max(max_payload_size, align(cursor, case_alignment))
            payload_base = align(4, payload_alignment)
            sum_payload_offsets[payload0] = payload_base
            sum_alignment = max(4, payload_alignment)
            result = (align(payload_base + max_payload_size, sum_alignment), sum_alignment)
        visiting.remove(type_id)
        require(result[0] <= 0x7FFF_FFFF, "layout overflow")
        layout_cache[type_id] = result
        return result

    layouts = tuple(type_layout(type_id) for type_id in range(len(types)))

    copy_cache: dict[int, bool] = {}
    copy_active: set[int] = set()

    def copyable(type_id: int) -> bool:
        if type_id in copy_cache:
            return copy_cache[type_id]
        require(type_id not in copy_active, "recursive copyability")
        copy_active.add(type_id)
        kind, payload0 = types[type_id][1], types[type_id][4]
        if kind in (1, 2, 3, 7):
            result = True
        elif kind == 5:
            result = copyable(payload0)
        elif kind == 4:
            row = records[payload0]
            result = bool(row[4] & 1) and all(
                copyable(fields[field_id][3])
                for field_id in span(row[2], row[3], len(fields), "copy fields")
            )
        else:
            row = sums[payload0]
            result = bool(row[4] & 1) and all(
                copyable(payloads[payload_id][3])
                for case_id in span(row[2], row[3], len(cases), "copy cases")
                for payload_id in span(cases[case_id][3], cases[case_id][4], len(payloads),
                                       "copy payloads")
            )
        copy_active.remove(type_id)
        copy_cache[type_id] = result
        return result

    for record in records:
        if record[4] & 1:
            require(copyable(record[1]), "invalid [copy] record")
    for sum_row in sums:
        if sum_row[4] & 1:
            require(copyable(sum_row[1]), "invalid [copy] sum")

    if allow_full_u32_subtract:
        canonical_u32 = len(tables["records"]) + len(tables["sums"]) + 1
        require(canonical_u32 < len(types)
                and types[canonical_u32][1:] ==
                (2, 0, 0, 0, 0, 0, 0xFFFF_FFFF),
                "CKIR13 canonical full u32")
        declaration_tables = dict(tables)
        declaration_tables["types"] = [
            row[:7] + (min(row[7], 0x7FFF_FFFF),) if row[1] == 2 else row
            for row in types
        ]
        _project_declarations(entry, flags, declaration_tables)
    else:
        _project_declarations(entry, flags, tables)

    def contains_sum(type_id: int, active: set[int] | None = None) -> bool:
        active = set() if active is None else active
        if type_id in active:
            return False
        active.add(type_id)
        kind, payload0 = types[type_id][1], types[type_id][4]
        if kind == 6:
            result = True
        elif kind == 5:
            result = contains_sum(payload0, active)
        elif kind == 4:
            row = records[payload0]
            result = any(contains_sum(fields[field_id][3], active)
                         for field_id in span(row[2], row[3], len(fields), "constant records"))
        else:
            result = False
        active.remove(type_id)
        return result

    require(all(not contains_sum(row[1]) for row in tables["constants"]),
            "sum constant")
    if not allow_static_byte_view:
        children, nodes = v4._validate_constant_graph(tables)
    else:
        children = [row[0] for row in tables["constant_children"]]
        nodes = tables["constants"]
        heights: list[int] = []
        keys: list[tuple[int, ...]] = []
        next_child = 0
        for index, node in enumerate(nodes):
            node_id, type_id, child_start, child_count, scalar, reserved = node
            require(node_id == index and type_id < len(types) and reserved == 0,
                    "constant node identity")
            require(child_start == next_child, "constant child partition")
            node_children = children[child_start:child_start + child_count]
            require(len(node_children) == child_count
                    and all(child < index for child in node_children),
                    "constant child order")
            kind = types[type_id][1]
            if kind in (1, 2, 3):
                require(child_count == 0 and types[type_id][6] <= scalar <= types[type_id][7],
                        "scalar constant")
                height = 0
                key = (height, type_id, scalar)
            else:
                require(scalar == 0, "structural constant scalar")
                if kind == 4:
                    record = records[types[type_id][4]]
                    expected = [
                        fields[field_id][3]
                        for field_id in span(record[2], record[3], len(fields),
                                             "constant fields")
                    ]
                    require(child_count <= 4, "record constant child exhaustion")
                elif kind == 5:
                    expected = [types[type_id][4]] * types[type_id][5]
                    require(child_count <= 1_024, "array constant child exhaustion")
                elif kind == 7:
                    if child_count > 32:
                        raise Ckir5ResourceError("static byte-view literal exhaustion")
                    expected = [types[type_id][4]] * child_count
                else:
                    raise Ckir5Error("sum constant")
                require(child_count == len(expected), "structural constant arity")
                require(all(nodes[child][1] == wanted
                            for child, wanted in zip(node_children, expected)),
                        "constant child type")
                height = 1 + max((heights[child] for child in node_children), default=-1)
                key = (height, type_id, child_count, *node_children)
            require(not keys or keys[-1] < key, "constant canonical order")
            heights.append(height)
            keys.append(key)
            next_child += child_count
        require(next_child == len(children), "unused constant child")

    machines, machine_params = tables["machines"], tables["machine_params"]
    blocks, block_params = tables["blocks"], tables["block_params"]
    operations = tables["operations"]
    operands = [row[0] for row in tables["operands"]]
    terminators = tables["terminators"]
    arms, arm_args = tables["case_arms"], tables["case_arm_args"]
    require(len(terminators) == len(blocks), "terminator count")

    next_machine_param = next_block = 0
    for machine in machines:
        machine_id, owner, access, machine_flags, reserved, result = machine[:6]
        require(owner < len(records) and access in (1, 2)
                and machine_flags == reserved == 0, "bad machine")
        require(result == NO_ID or result < len(types) and types[result][1] in (1, 2, 3),
                "machine result")
        require(machine[6] == next_machine_param and machine[7] <= 7,
                "machine parameter partition")
        require(machine[8] == next_block and 1 <= machine[9] <= 128
                and machine[10] == machine[8], "machine block partition")
        for ordinal, parameter_id in enumerate(span(machine[6], machine[7], len(machine_params),
                                                     "machine parameters")):
            parameter = machine_params[parameter_id]
            require(parameter[:4] == (parameter_id, machine_id, ordinal, parameter[3])
                    and parameter[3] < len(types) and parameter[4] == parameter_id,
                    "machine parameter")
            require(types[parameter[3]][1] in (1, 2, 3) or copyable(parameter[3]),
                    "noncopyable machine parameter")
        next_machine_param += machine[7]
        next_block += machine[9]
    require(next_machine_param == len(machine_params) and next_block == len(blocks),
            "machine partitions")

    next_block_param = next_operation = 0
    for block in blocks:
        block_id, owner, access, block_flags, reserved = block[:5]
        require(owner < len(machines) and access in (1, 2) and access <= machines[owner][2],
                "bad block")
        require(reserved == 0 and block[9] == block_id
                and (block_flags in (0, 1) if allow_static_byte_view else block_flags == 0),
                "block flags/terminator")
        require(block[5] == next_block_param and block[6] <= 7,
                "block parameter partition")
        require(block[7] == next_operation, "operation partition")
        require(block_id in span(machines[owner][8], machines[owner][9], len(blocks),
                                 "owner blocks"), "block owner")
        if block_id == machines[owner][10]:
            require(access == machines[owner][2] and block[6] == 0,
                    "entry block signature")
        for ordinal, parameter_id in enumerate(span(block[5], block[6], len(block_params),
                                                     "block parameters")):
            parameter = block_params[parameter_id]
            require(parameter[:4] == (parameter_id, block_id, ordinal, parameter[3])
                    and parameter[3] < len(types)
                    and parameter[4] == len(machine_params) + parameter_id,
                    "block parameter")
            require(types[parameter[3]][1] in (1, 2, 3) or copyable(parameter[3]),
                    "noncopyable block parameter")
        next_block_param += block[6]
        next_operation += block[8]
    require(next_block_param == len(block_params) and next_operation == len(operations),
            "block partitions")

    value_types = [row[3] for row in machine_params] + [row[3] for row in block_params]
    value_machines = [row[1] for row in machine_params]
    value_machines.extend(blocks[row[1]][1] for row in block_params)
    value_blocks = [NO_ID] * len(machine_params) + [row[1] for row in block_params]
    value_operations = [NO_ID] * len(value_types)
    place_types: list[int] = []
    place_mutable: list[bool] = []
    place_blocks: list[int] = []
    place_operations: list[int] = []
    constructor_results: set[int] = set()
    roots: list[int] = []
    call_graph: list[set[int]] = [set() for _ in machines]

    def visible_value(value_id: int, owner: int, block: int, operation: int) -> bool:
        return value_id < len(value_types) and value_machines[value_id] == owner and (
            value_blocks[value_id] == NO_ID or value_blocks[value_id] == block and (
                value_operations[value_id] == NO_ID or value_operations[value_id] < operation
            )
        )

    def visible_place(place_id: int, block: int, operation: int) -> bool:
        return (place_id < len(place_types) and place_blocks[place_id] == block
                and place_operations[place_id] < operation)

    next_operand = 0
    logical_not_count = 0
    logical_binary_count = 0
    scalar_equal_count = 0
    greater_count = 0
    integer_widen_count = 0
    trapping_add_count = 0
    subtract_count = 0
    byte_view_counts = {opcode: 0 for opcode in range(22, 26)}
    for operation in operations:
        (op_id, owner, block, opcode, result_kind, op_flags, result_id,
         result_type, operand_start, operand_count, imm0, imm1) = operation
        require(op_flags == 0 and block < len(blocks) and owner == blocks[block][1],
                "operation owner")
        require(op_id in span(blocks[block][7], blocks[block][8], len(operations),
                              "block operations"), "operation block")
        require(operand_start == next_operand, "operation operand partition")
        op_values = operands[operand_start:operand_start + operand_count]
        require(len(op_values) == operand_count, "operation operand extent")
        next_operand += operand_count
        opcode_limit = 27 if allow_full_u32_subtract else 26 if allow_static_byte_view else 22 if allow_integer_widen else 21 if allow_greater else 19 if allow_scalar_equal else 18 if allow_logical_binary else 16 if allow_logical_not else 15
        require(opcode in range(1, opcode_limit), "opcode")
        if opcode == 10:
            require(imm0 < len(machines), "call target")
            expected_kind = 0 if machines[imm0][5] == NO_ID else 1
        elif opcode in (13, 14):
            expected_kind = 1
        else:
            expected_kind = 0 if opcode in (6, 7, 11) else 2 if opcode in (2, 3, 4) else 1
        require(result_kind == expected_kind, "operation result kind")
        if result_kind == 0:
            require(result_id == result_type == NO_ID, "spurious result")
        elif result_kind == 1:
            require(result_id == len(value_types) and result_type < len(types), "value result ID")
            value_types.append(result_type)
            value_machines.append(owner)
            value_blocks.append(block)
            value_operations.append(op_id)
        else:
            require(result_id == len(place_types) and result_type < len(types), "place result ID")
            place_types.append(result_type)
            place_mutable.append(False)
            place_blocks.append(block)
            place_operations.append(op_id)

        expected_operands = (1 + machines[imm0][7] if opcode == 10 else {
            1: 0, 2: 0, 3: 1, 4: 2, 5: 1, 6: 2, 7: 2, 8: 2, 9: 2,
            11: 1, 12: 2, 15: 1, 16: 2, 17: 2, 18: 2, 19: 2, 20: 2,
            21: 1, 22: 0, 23: 1, 24: 1, 25: 1, 26: 2,
        }.get(opcode))
        if opcode not in (13, 14):
            require(operand_count == expected_operands, "operation arity")

        if opcode == 1:
            require(imm1 == 0 and types[result_type][1] in (1, 2, 3), "const type")
            require(types[result_type][6] <= imm0 <= types[result_type][7], "const range")
        elif opcode == 2:
            require(imm0 == imm1 == 0 and result_type == records[machines[owner][1]][1]
                    and types[result_type][1] == 4, "self place")
            place_mutable[result_id] = blocks[block][2] == 2
        elif opcode == 3:
            require(imm0 < len(fields) and imm1 == 0
                    and visible_place(op_values[0], block, op_id), "field place")
            require(place_types[op_values[0]] == records[fields[imm0][1]][1]
                    and result_type == fields[imm0][3], "field type")
            place_mutable[result_id] = place_mutable[op_values[0]]
        elif opcode == 4:
            require(imm0 == imm1 == 0 and visible_place(op_values[0], block, op_id)
                    and visible_value(op_values[1], owner, block, op_id), "index refs")
            base_type = place_types[op_values[0]]
            require(types[base_type][1] == 5 and result_type == types[base_type][4],
                    "index type")
            require(types[value_types[op_values[1]]][1] in (1, 2), "index scalar")
            place_mutable[result_id] = place_mutable[op_values[0]]
        elif opcode == 5:
            require(imm0 == imm1 == 0 and visible_place(op_values[0], block, op_id),
                    "load ref")
            require(result_type == place_types[op_values[0]]
                    and types[result_type][1] in (1, 2, 3), "load type")
        elif opcode == 6:
            require(imm0 == imm1 == 0 and visible_place(op_values[0], block, op_id)
                    and visible_value(op_values[1], owner, block, op_id), "store refs")
            require(types[place_types[op_values[0]]][1] == types[value_types[op_values[1]]][1]
                    and types[place_types[op_values[0]]][1] in (1, 2, 3), "store type")
            require(place_mutable[op_values[0]], "shared place store")
        elif opcode == 7:
            require(imm0 in (1, 2) and imm1 == 0
                    and visible_place(op_values[0], block, op_id), "copy refs")
            require(visible_value(op_values[1], owner, block, op_id) if imm0 == 1
                    else visible_place(op_values[1], block, op_id), "copy source")
            source_type = value_types[op_values[1]] if imm0 == 1 else place_types[op_values[1]]
            require(place_types[op_values[0]] == source_type
                    and types[source_type][1] in (4, 5, 6) and copyable(source_type),
                    "copy type")
            require(place_mutable[op_values[0]], "shared place copy")
        elif opcode in (8, 9, 12, 26):
            require(imm0 == imm1 == 0
                    and all(visible_value(value, owner, block, op_id) for value in op_values),
                    "arithmetic refs")
            require(types[value_types[op_values[0]]][1]
                    == types[value_types[op_values[1]]][1]
                    and types[value_types[op_values[0]]][1] in (1, 2), "arithmetic type")
            if opcode in (8, 26):
                require(types[result_type][1] == types[value_types[op_values[0]]][1],
                        "arithmetic result")
                if (opcode == 8 and expected_major == 11
                        and types[result_type][1:] == (2, 1, 0, 0, 0, 0, 0x7FFF_FFFF)
                        and value_types[op_values[0]] == result_type
                        and value_types[op_values[1]] == result_type):
                    trapping_add_count += 1
                if opcode == 26:
                    require(expected_major == 13
                            and types[result_type][1:] == (2, 1, 0, 0, 0, 0, 0xFFFF_FFFF)
                            and value_types[op_values[0]] == result_type
                            and value_types[op_values[1]] == result_type,
                            "full-u32 Subtract type")
                    subtract_count += 1
            else:
                require(types[result_type][1] == 3, "comparison result")
        elif opcode == 15:
            require(imm0 == imm1 == 0
                    and visible_value(op_values[0], owner, block, op_id),
                    "LogicalNot operand")
            require(value_types[op_values[0]] == result_type
                    and result_type < len(types) and types[result_type][1] == 3,
                    "LogicalNot bool type")
            logical_not_count += 1
        elif opcode in (16, 17):
            require(imm0 == imm1 == 0
                    and all(visible_value(value, owner, block, op_id)
                            for value in op_values),
                    "logical binary operands")
            require(all(value_types[value] == result_type for value in op_values)
                    and result_type < len(types) and types[result_type][1] == 3,
                    "logical binary bool type")
            logical_binary_count += 1
        elif opcode == 18:
            require(imm0 == imm1 == 0
                    and all(visible_value(value, owner, block, op_id)
                            for value in op_values),
                    "ScalarEqual operands")
            left_kind = types[value_types[op_values[0]]][1]
            right_kind = types[value_types[op_values[1]]][1]
            require(left_kind == right_kind and left_kind in (1, 2, 3),
                    "ScalarEqual operand type")
            require(result_type < len(types) and types[result_type][1] == 3,
                    "ScalarEqual result type")
            scalar_equal_count += 1
        elif opcode in (19, 20):
            require(imm0 == imm1 == 0
                    and all(visible_value(value, owner, block, op_id)
                            for value in op_values),
                    "ordered greater operands")
            left_kind = types[value_types[op_values[0]]][1]
            right_kind = types[value_types[op_values[1]]][1]
            require(left_kind == right_kind and left_kind in (1, 2),
                    "ordered greater operand type")
            require(result_type < len(types) and types[result_type][1] == 3,
                    "ordered greater result type")
            greater_count += 1
        elif opcode == 21:
            require(imm0 == imm1 == 0
                    and visible_value(op_values[0], owner, block, op_id),
                    "IntegerWiden operand")
            source_type = value_types[op_values[0]]
            require(types[source_type] == (source_type, 1, 0, 0, 0, 0, 0, 255),
                    "IntegerWiden exact-u8 source")
            require(types[result_type] ==
                    (result_type, 2, 1, 0, 0, 0, 0, 0x7FFF_FFFF),
                    "IntegerWiden canonical u32 Trapping result")
            integer_widen_count += 1
        elif opcode == 22:
            require(imm1 == 0 and imm0 < len(nodes), "StaticByteView root")
            require(types[result_type][1] == 7 and nodes[imm0][1] == result_type,
                    "StaticByteView result/root type")
            roots.append(imm0)
            byte_view_counts[opcode] += 1
        elif opcode in (23, 24, 25):
            require(imm0 == imm1 == 0
                    and visible_value(op_values[0], owner, block, op_id),
                    "byte-view operand")
            source_type = value_types[op_values[0]]
            require(types[source_type][1] == 7, "byte-view source type")
            if opcode == 23:
                require(types[result_type] ==
                        (result_type, 3, 0, 0, 0, 0, 0, 1),
                        "SliceNonEmpty canonical bool result")
            elif opcode == 24:
                require(result_type == types[source_type][4],
                        "SliceHead exact-u8 result")
            else:
                require(result_type == source_type, "SliceTailOne result type")
            byte_view_counts[opcode] += 1
        elif opcode == 10:
            callee = machines[imm0]
            require(imm1 == 0 and visible_place(op_values[0], block, op_id), "call receiver")
            require(place_types[op_values[0]] == records[callee[1]][1], "call receiver type")
            require(callee[2] == 1 or place_mutable[op_values[0]], "mutable call via shared place")
            for argument, parameter_id in zip(op_values[1:],
                    span(callee[6], callee[7], len(machine_params), "call params")):
                require(visible_value(argument, owner, block, op_id), "call argument visibility")
                require(value_types[argument] == machine_params[parameter_id][3],
                        "call argument type")
            if callee[5] != NO_ID:
                require(result_type == callee[5] and types[result_type][1] in (1, 2, 3),
                        "call result type")
            call_graph[owner].add(imm0)
        elif opcode == 11:
            require(imm1 == 0 and len(op_values) == 1
                    and visible_place(op_values[0], block, op_id), "constant copy refs")
            destination = op_values[0]
            require(place_mutable[destination] and imm0 < len(nodes)
                    and nodes[imm0][1] == place_types[destination], "constant copy root")
            require(types[nodes[imm0][1]][1] in (4, 5)
                    and copyable(nodes[imm0][1]), "constant copy type")
            roots.append(imm0)
        elif opcode == 13:
            require(imm0 == imm1 == 0 and types[result_type][1] == 4,
                    "ConstructRecord result")
            record = records[types[result_type][4]]
            require(bool(record[4] & 1) and copyable(result_type)
                    and operand_count == record[3], "ConstructRecord shape")
            wanted_fields = [fields[field_id][3]
                             for field_id in span(record[2], record[3], len(fields),
                                                  "constructor fields")]
            if record[3] > 4:
                raise Ckir5ResourceError("ConstructRecord field exhaustion")
            for argument, wanted in zip(op_values, wanted_fields):
                require(visible_value(argument, owner, block, op_id),
                        "ConstructRecord operand visibility")
                actual = value_types[argument]
                require(actual == wanted if types[wanted][1] not in (1, 2, 3)
                        else types[actual][1] == types[wanted][1]
                        and types[actual][6] >= types[wanted][6]
                        and types[actual][7] <= types[wanted][7],
                        "ConstructRecord operand type")
            constructor_results.add(result_id)
        elif opcode == 14:
            require(imm1 == 0 and types[result_type][1] == 6 and imm0 < len(cases),
                    "ConstructCase result")
            case = cases[imm0]
            require(case[1] == types[result_type][4] and copyable(result_type),
                    "ConstructCase owner/copyability")
            require(operand_count == case[4], "ConstructCase arity")
            if case[4] > 4:
                raise Ckir5ResourceError("ConstructCase field exhaustion")
            for argument, payload_id in zip(op_values,
                    span(case[3], case[4], len(payloads), "constructor payloads")):
                require(visible_value(argument, owner, block, op_id),
                        "ConstructCase operand visibility")
                actual, wanted = value_types[argument], payloads[payload_id][3]
                require(actual == wanted if types[wanted][1] not in (1, 2, 3)
                        else types[actual][1] == types[wanted][1]
                        and types[actual][6] >= types[wanted][6]
                        and types[actual][7] <= types[wanted][7],
                        "ConstructCase operand type")
            constructor_results.add(result_id)
        else:
            raise Ckir5Error("unhandled opcode")

    require(len(value_types) == counts["values"] and len(place_types) == counts["places"],
            "reconstructed result counts")
    if expected_major == 6:
        require(logical_not_count > 0, "CKIR6 requires LogicalNot")
    elif expected_major == 7:
        require(logical_binary_count > 0, "CKIR7 requires LogicalAnd or LogicalOr")
    elif expected_major == 8:
        require(scalar_equal_count > 0, "CKIR8 requires ScalarEqual")
    elif expected_major == 9:
        require(greater_count > 0, "CKIR9 requires Greater or GreaterEqual")
    elif expected_major == 10:
        require(integer_widen_count > 0, "CKIR10 requires IntegerWiden")
    elif expected_major == 11:
        require(trapping_add_count > 0, "CKIR11 requires canonical u32 Trapping Add")
    elif expected_major == 12:
        require(all(byte_view_counts.values()), "CKIR12 requires byte-view operations 22-25")
    elif expected_major == 13:
        require(subtract_count > 0, "CKIR13 requires full-u32 Trapping Subtract")

    next_arm = next_arm_arg = 0
    predecessors: list[list[tuple[int, int, int, tuple[int, ...]]]] = [
        [] for _ in blocks
    ]
    for term in terminators:
        (term_id, owner, block, kind, term_flags, reserved, value,
         target0, start0, count0, target1, start1, count1, arm_start, arm_count) = term
        require(block < len(blocks) and owner == blocks[block][1] and term_id == block
                and reserved == 0, "terminator owner")
        require(kind in range(1, 6), "terminator kind")
        block_end = blocks[block][7] + blocks[block][8]
        if kind != 5:
            require(term_flags == 0 and arm_start == next_arm and arm_count == 0,
                    "inherited terminator arm shape")
            require(start0 == next_operand, "target-0 operand partition")
            next_operand += count0
            require(start1 == next_operand, "target-1 operand partition")
            next_operand += count1
            for target, start, count in ((target0, start0, count0), (target1, start1, count1)):
                if target == NO_ID:
                    require(count == 0, "arguments without target")
                    continue
                require(target < len(blocks) and blocks[target][1] == owner
                        and target != machines[owner][10] and count == blocks[target][6],
                        "bad inherited edge")
                for argument, parameter_id in zip(operands[start:start + count],
                        span(blocks[target][5], count, len(block_params), "edge params")):
                    require(visible_value(argument, owner, block, block_end)
                            and argument not in constructor_results, "edge argument")
                    actual, wanted = value_types[argument], block_params[parameter_id][3]
                    require(actual == wanted if types[wanted][1] in (4, 5, 6, 7)
                            else types[actual][1] == types[wanted][1], "edge argument type")
                predecessors[target].append((block, 0 if target == target0 else 1, value,
                                             tuple(operands[start:start + count])))
            result_type = machines[owner][5]
            if kind == 1:
                require(value == target1 == NO_ID and target0 != NO_ID, "jump shape")
            elif kind == 2:
                require(visible_value(value, owner, block, block_end)
                        and types[value_types[value]][1] == 3
                        and target0 != NO_ID and target1 != NO_ID, "branch shape")
            elif kind == 3:
                require(value == target0 == target1 == NO_ID and result_type == NO_ID,
                        "Unit return")
            else:
                require(target0 == target1 == NO_ID
                        and visible_value(value, owner, block, block_end), "value return")
                require(types[value_types[value]][1] == types[result_type][1], "return carrier")
            continue

        require(term_flags in (1, 2) and target0 == target1 == NO_ID,
                "CaseDispatch flags/targets")
        require(start0 == start1 == next_operand and count0 == count1 == 0,
                "CaseDispatch ordinary spans")
        subject_type = (value_types[value] if term_flags == 1
                        and visible_value(value, owner, block, block_end)
                        else place_types[value] if term_flags == 2
                        and visible_place(value, block, block_end) else NO_ID)
        require(subject_type < len(types) and types[subject_type][1] == 6,
                "CaseDispatch subject")
        sum_row = sums[types[subject_type][4]]
        require(arm_start == next_arm and arm_count == sum_row[3], "CaseDispatch arm span")
        if arm_count > 64:
            raise Ckir5ResourceError("CaseDispatch arm exhaustion")
        for ordinal, arm_id in enumerate(span(arm_start, arm_count, len(arms), "case arms")):
            arm = arms[arm_id]
            case_id = sum_row[2] + ordinal
            require(arm[:3] == (arm_id, term_id, case_id), "case arm owner/order")
            target, arg_start, arg_count = arm[3:]
            require(target < len(blocks) and blocks[target][1] == owner
                    and target != machines[owner][10] and arg_count == blocks[target][6],
                    "case arm target")
            require(arg_start == next_arm_arg, "case-arm argument partition")
            case = cases[case_id]
            bound_payloads: set[int] = set()
            for argument_id, parameter_id in zip(
                span(arg_start, arg_count, len(arm_args), "case-arm arguments"),
                span(blocks[target][5], arg_count, len(block_params), "case target params"),
            ):
                argument = arm_args[argument_id]
                require(argument[0] == argument_id and argument[1] in (1, 2),
                        "case-arm argument kind")
                reference = argument[2]
                if argument[1] == 1:
                    require(visible_value(reference, owner, block, block_end)
                            and reference not in constructor_results,
                            "ordinary case-arm argument")
                    actual = value_types[reference]
                else:
                    require(reference in span(case[3], case[4], len(payloads),
                                              "selected payload"),
                            "inactive/wrong-case payload")
                    require(reference not in bound_payloads, "duplicate payload binding")
                    bound_payloads.add(reference)
                    actual = payloads[reference][3]
                wanted = block_params[parameter_id][3]
                require(actual == wanted if types[wanted][1] in (4, 5, 6, 7)
                        else types[actual][1] == types[wanted][1],
                        "case-arm argument type")
            require(bound_payloads == set(span(case[3], case[4], len(payloads),
                                               "complete payload binding")),
                    "incomplete payload binding")
            next_arm_arg += arg_count
        next_arm += arm_count
    require(next_operand == len(operands), "unused ordinary operands")
    require(next_arm == len(arms) and next_arm_arg == len(arm_args),
            "unused case arm rows")

    if allow_static_byte_view:
        synthetic = [block[0] for block in blocks if block[3] & 1]
        require(len(synthetic) == 1, "unique synthetic nonempty-edge block")
        synthetic_id = synthetic[0]
        synthetic_block = blocks[synthetic_id]
        require(synthetic_block[6] == 1, "synthetic slice parameter shape")
        parameter = block_params[synthetic_block[5]]
        require(types[parameter[3]][1] == 7, "synthetic slice parameter type")
        incoming = predecessors[synthetic_id]
        require(len(incoming) == 1, "synthetic unique predecessor")
        source, edge_slot, condition, arguments = incoming[0]
        source_term = terminators[source]
        require(source_term[3] == 2 and edge_slot == 0 and source_term[10] != synthetic_id,
                "synthetic true-edge-only predecessor")
        require(len(arguments) == 1, "synthetic incoming argument shape")
        condition_op = value_operations[condition]
        require(condition_op != NO_ID and operations[condition_op][3] == 23,
                "synthetic predecessor condition")
        condition_operand = operands[operations[condition_op][8]]
        require(arguments[0] == condition_operand,
                "synthetic condition/passed-slice identity")
        for op_id in span(synthetic_block[7], synthetic_block[8], len(operations),
                          "synthetic operations"):
            operation = operations[op_id]
            require(operation[3] in (24, 25)
                    and operation[9] == 1
                    and operands[operation[8]] == parameter[4],
                    "synthetic operation shape")
        require({operations[op_id][3] for op_id in
                 span(synthetic_block[7], synthetic_block[8], len(operations),
                      "synthetic operations")} == {24, 25},
                "synthetic head/tail coverage")
        synthetic_term = terminators[synthetic_id]
        require(synthetic_term[3] == 1 and synthetic_term[7] != NO_ID
                and blocks[synthetic_term[7]][3] == 0,
                "synthetic authored jump target")

        rooted_views = {root for root in roots if types[nodes[root][1]][1] == 7}
        require(all(types[node[1]][1] != 7 or node[0] in rooted_views for node in nodes),
                "byte-view literal must be a StaticByteView root")

    require((not nodes) == (not roots), "constant/root presence")
    reachable: set[int] = set()
    pending = list(set(roots))
    while pending:
        node = pending.pop()
        if node in reachable:
            continue
        reachable.add(node)
        row = nodes[node]
        pending.extend(children[row[2]:row[2] + row[3]])
    require(reachable == set(range(len(nodes))), "unreachable constant node")

    indegree = [0] * len(machines)
    for callees in call_graph:
        for callee in callees:
            indegree[callee] += 1
    pending_machines = [index for index, degree in enumerate(indegree) if degree == 0]
    removed = 0
    while pending_machines:
        caller = min(pending_machines)
        pending_machines.remove(caller)
        removed += 1
        for callee in sorted(call_graph[caller]):
            indegree[callee] -= 1
            if indegree[callee] == 0:
                pending_machines.append(callee)
    require(removed == len(machines), "cyclic machine calls")
    if entry != NO_ID:
        require(entry < len(machines) and machines[entry][7] == 0
                and machines[entry][5] != NO_ID, "selected entry signature")
        owner_type = records[machines[entry][1]][1]
        require(layouts[owner_type][0] <= 131_072, "entry layout ceiling")

    return Module(entry, tables, layouts, tuple(field_offsets), tuple(payload_offsets),
                  tuple(sum_payload_offsets), tuple(value_types), tuple(place_types))


def interpret(module: Module, step_limit: int = 65_536, frame_limit: int = 64) -> int | None:
    if module.entry == NO_ID:
        return None
    tables = module.tables
    types, records, fields = tables["types"], tables["records"], tables["fields"]
    sums, cases, payloads = tables["sums"], tables["cases"], tables["case_payloads"]
    machines, blocks = tables["machines"], tables["blocks"]
    block_params, operations = tables["block_params"], tables["operations"]
    operands = [row[0] for row in tables["operands"]]
    terminators, arms, arm_args = tables["terminators"], tables["case_arms"], tables["case_arm_args"]
    owner_type = records[machines[module.entry][1]][1]
    memory = bytearray(module.layouts[owner_type][0])
    steps = 0

    def selected_case(type_id: int, image: bytes) -> tuple[int, tuple[int, ...]]:
        sum_row = sums[types[type_id][4]]
        require(len(image) >= 4, "runtime sum extent")
        tag = int.from_bytes(image[:4], "little")
        require(tag < sum_row[3], "runtime invalid sum tag")
        return tag, cases[sum_row[2] + tag]

    def scalar_leaves(type_id: int, image: bytes, base: int = 0):
        kind, payload0, payload1 = types[type_id][1], types[type_id][4], types[type_id][5]
        if kind in (1, 2, 3):
            yield base, module.layouts[type_id][0]
        elif kind == 7:
            yield base, 16
        elif kind == 4:
            record = records[payload0]
            for field_id in span(record[2], record[3], len(fields), "runtime copy fields"):
                offset = module.field_offsets[field_id]
                yield from scalar_leaves(fields[field_id][3], image[offset:], base + offset)
        elif kind == 5:
            stride = module.layouts[payload0][0]
            for index in range(payload1):
                yield from scalar_leaves(payload0, image[index * stride:], base + index * stride)
        else:
            _, case = selected_case(type_id, image)
            yield base, 4
            payload_base = module.sum_payload_offsets[payload0]
            for payload_id in span(case[3], case[4], len(payloads), "runtime sum payload"):
                offset = payload_base + module.payload_offsets[payload_id]
                yield from scalar_leaves(payloads[payload_id][3], image[offset:], base + offset)

    def semantic_copy(output: bytearray, destination: int, type_id: int, image: bytes) -> None:
        staged = [(offset, size, image[offset:offset + size])
                  for offset, size in scalar_leaves(type_id, image)]
        for offset, size, data in staged:
            require(len(data) == size, "runtime structural source extent")
            output[destination + offset:destination + offset + size] = data

    def run_machine(machine_id: int, receiver: int,
                    arguments: list[tuple[int, int | bytes]], depth: int) -> int | None:
        nonlocal steps
        require(depth <= frame_limit, "active machine-frame exhaustion")
        machine = machines[machine_id]
        require(len(arguments) == machine[7], "runtime call arity")
        machine_values: dict[int, tuple[int, int | bytes]] = {}
        for ordinal, argument in enumerate(arguments):
            parameter = tables["machine_params"][machine[6] + ordinal]
            machine_values[parameter[4]] = argument
        block_values: dict[int, tuple[int, int | bytes]] = {}
        block_id = machine[10]
        while True:
            steps += 1
            require(steps <= step_limit, "dynamic block-entry exhaustion")
            block = blocks[block_id]
            values = dict(machine_values)
            values.update(block_values)
            places: dict[int, tuple[int, int]] = {}
            for op_id in span(block[7], block[8], len(operations), "runtime operations"):
                op = operations[op_id]
                opcode, result_id, result_type = op[3], op[6], op[7]
                args = operands[op[8]:op[8] + op[9]]
                if opcode == 1:
                    values[result_id] = (result_type, op[10])
                elif opcode == 2:
                    places[result_id] = (result_type, receiver)
                elif opcode == 3:
                    _, base = places[args[0]]
                    places[result_id] = (result_type, base + module.field_offsets[op[10]])
                elif opcode == 4:
                    base_type, base = places[args[0]]
                    index = int(values[args[1]][1])
                    element_type, length = types[base_type][4], types[base_type][5]
                    require(index < length, "runtime index trap")
                    places[result_id] = (result_type, base + index * module.layouts[element_type][0])
                elif opcode == 5:
                    _, address = places[args[0]]
                    size = module.layouts[result_type][0]
                    values[result_id] = (result_type,
                        int.from_bytes(memory[address:address + size], "little"))
                elif opcode == 6:
                    place_type, address = places[args[0]]
                    value = int(values[args[1]][1])
                    require(types[place_type][6] <= value <= types[place_type][7],
                            "runtime store range")
                    size = module.layouts[place_type][0]
                    memory[address:address + size] = value.to_bytes(size, "little")
                elif opcode == 7:
                    destination_type, destination = places[args[0]]
                    if op[10] == 2:
                        source_type, source = places[args[1]]
                        image = bytes(memory[source:source + module.layouts[source_type][0]])
                    else:
                        source_type, image = values[args[1]]
                        require(isinstance(image, bytes), "runtime structural value")
                    require(source_type == destination_type, "runtime copy type")
                    semantic_copy(memory, destination, destination_type, image)
                elif opcode == 8:
                    value = int(values[args[0]][1]) + int(values[args[1]][1])
                    require(types[result_type][6] <= value <= types[result_type][7],
                            "runtime add range")
                    values[result_id] = (result_type, value)
                elif opcode == 26:
                    left, right = int(values[args[0]][1]), int(values[args[1]][1])
                    require(left >= right, "runtime subtract underflow")
                    value = left - right
                    require(types[result_type][6] <= value <= types[result_type][7],
                            "runtime subtract range")
                    values[result_id] = (result_type, value)
                elif opcode in (9, 12):
                    left, right = int(values[args[0]][1]), int(values[args[1]][1])
                    values[result_id] = (result_type,
                        int(left < right if opcode == 9 else left <= right))
                elif opcode == 15:
                    values[result_id] = (result_type, 1 - int(values[args[0]][1]))
                elif opcode in (16, 17):
                    left, right = int(values[args[0]][1]), int(values[args[1]][1])
                    values[result_id] = (
                        result_type,
                        left & right if opcode == 16 else left | right,
                    )
                elif opcode == 18:
                    left, right = int(values[args[0]][1]), int(values[args[1]][1])
                    values[result_id] = (result_type, int(left == right))
                elif opcode in (19, 20):
                    left, right = int(values[args[0]][1]), int(values[args[1]][1])
                    values[result_id] = (
                        result_type,
                        int(left > right if opcode == 19 else left >= right),
                    )
                elif opcode == 21:
                    values[result_id] = (result_type, int(values[args[0]][1]))
                elif opcode == 22:
                    root = tables["constants"][op[10]]
                    values[result_id] = (
                        result_type,
                        struct.pack("<QII", op[10], 0, root[3]),
                    )
                elif opcode in (23, 24, 25):
                    descriptor = values[args[0]][1]
                    require(isinstance(descriptor, bytes) and len(descriptor) == 16,
                            "runtime byte-view descriptor")
                    root_id, offset, length = struct.unpack("<QII", descriptor)
                    require(root_id < len(tables["constants"]),
                            "runtime byte-view root")
                    root = tables["constants"][root_id]
                    require(types[root[1]][1] == 7 and offset + length <= root[3],
                            "runtime byte-view extent")
                    if opcode == 23:
                        values[result_id] = (result_type, int(length != 0))
                    elif opcode == 24:
                        require(length != 0, "runtime SliceHead on empty view")
                        child_id = tables["constant_children"][root[2] + offset][0]
                        values[result_id] = (result_type, tables["constants"][child_id][4])
                    else:
                        require(length != 0, "runtime SliceTailOne on empty view")
                        values[result_id] = (
                            result_type,
                            struct.pack("<QII", root_id, offset + 1, length - 1),
                        )
                elif opcode == 10:
                    _, callee_receiver = places[args[0]]
                    result = run_machine(op[10], callee_receiver,
                                         [values[value] for value in args[1:]], depth + 1)
                    if op[4] == 1:
                        require(result is not None, "runtime missing call result")
                        values[result_id] = (result_type, result)
                elif opcode == 11:
                    destination_type, destination = places[args[0]]
                    semantic_copy(memory, destination, destination_type,
                                  v4.materialize_constant(module, op[10]))
                elif opcode == 13:
                    record = records[types[result_type][4]]
                    image = bytearray(module.layouts[result_type][0])
                    for argument, field_id in zip(args,
                            span(record[2], record[3], len(fields), "runtime record fields")):
                        field_type = fields[field_id][3]
                        actual, value = values[argument]
                        offset = module.field_offsets[field_id]
                        if types[field_type][1] in (1, 2, 3):
                            size = module.layouts[field_type][0]
                            image[offset:offset + size] = int(value).to_bytes(size, "little")
                        else:
                            require(actual == field_type and isinstance(value, bytes),
                                    "runtime record structural value")
                            semantic_copy(image, offset, field_type, value)
                    values[result_id] = (result_type, bytes(image))
                elif opcode == 14:
                    case = cases[op[10]]
                    sum_id = types[result_type][4]
                    image = bytearray(module.layouts[result_type][0])
                    image[:4] = case[2].to_bytes(4, "little")
                    payload_base = module.sum_payload_offsets[sum_id]
                    for argument, payload_id in zip(args,
                            span(case[3], case[4], len(payloads), "runtime case payloads")):
                        field_type = payloads[payload_id][3]
                        actual, value = values[argument]
                        offset = payload_base + module.payload_offsets[payload_id]
                        if types[field_type][1] in (1, 2, 3):
                            scalar = int(value)
                            require(types[field_type][6] <= scalar <= types[field_type][7],
                                    "runtime case payload range")
                            size = module.layouts[field_type][0]
                            image[offset:offset + size] = scalar.to_bytes(size, "little")
                        else:
                            require(actual == field_type and isinstance(value, bytes),
                                    "runtime case structural payload")
                            semantic_copy(image, offset, field_type, value)
                    values[result_id] = (result_type, bytes(image))
                else:
                    raise Ckir5Error("runtime opcode")

            term = terminators[block_id]
            if term[3] in (1, 2):
                first = term[3] == 1 or bool(int(values[term[6]][1]))
                target, start, count = ((term[7], term[8], term[9]) if first
                                        else (term[10], term[11], term[12]))
                assigned = {
                    block_params[param_id][4]: values[arg]
                    for arg, param_id in zip(operands[start:start + count],
                        span(blocks[target][5], count, len(block_params), "runtime edge"))
                }
                block_values, block_id = assigned, target
            elif term[3] == 3:
                return None
            elif term[3] == 4:
                return int(values[term[6]][1])
            else:
                subject_type, subject = ((values[term[6]]) if term[4] == 1 else
                    (places[term[6]][0], bytes(memory[
                        places[term[6]][1]:places[term[6]][1] + module.layouts[places[term[6]][0]][0]
                    ])))
                require(isinstance(subject, bytes), "runtime case subject")
                tag, case = selected_case(subject_type, subject)
                arm = arms[term[13] + tag]
                target = arm[3]
                assigned: dict[int, tuple[int, int | bytes]] = {}
                payload_base = module.sum_payload_offsets[types[subject_type][4]]
                for argument_id, parameter_id in zip(
                    span(arm[4], arm[5], len(arm_args), "runtime case arguments"),
                    span(blocks[target][5], arm[5], len(block_params), "runtime case params"),
                ):
                    argument = arm_args[argument_id]
                    if argument[1] == 1:
                        value = values[argument[2]]
                    else:
                        payload_id = argument[2]
                        field_type = payloads[payload_id][3]
                        offset = payload_base + module.payload_offsets[payload_id]
                        if types[field_type][1] in (1, 2, 3):
                            size = module.layouts[field_type][0]
                            scalar = int.from_bytes(subject[offset:offset + size], "little")
                            require(types[field_type][6] <= scalar <= types[field_type][7],
                                    "runtime bound payload range")
                            value = (field_type, scalar)
                        else:
                            snapshot = bytearray(module.layouts[field_type][0])
                            semantic_copy(snapshot, 0, field_type, subject[offset:])
                            value = (field_type, bytes(snapshot))
                    assigned[block_params[parameter_id][4]] = value
                block_values, block_id = assigned, target

    return run_machine(module.entry, 0, [], 1)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("validate", "run"))
    parser.add_argument("ckir", type=Path)
    args = parser.parse_args()
    module = decode(args.ckir.read_bytes())
    if args.command == "validate":
        print(f"CKIR5 valid: {len(module.tables['sums'])} sums, "
              f"{len(module.tables['cases'])} cases, "
              f"{sum(op[3] == 14 for op in module.tables['operations'])} constructors, "
              f"{sum(term[3] == 5 for term in module.tables['terminators'])} dispatches")
    else:
        result = interpret(module)
        print("library" if result is None else result)


if __name__ == "__main__":
    try:
        main()
    except Ckir5ResourceError as error:
        print(f"checked IR v5 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(252)
    except (Ckir5Error, v4.Ckir4Error, v3.Ckir3Error, v2.CkirError,
            OSError, struct.error) as error:
        print(f"checked IR v5 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(251)
