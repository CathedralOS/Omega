#!/usr/bin/env python3
"""Independent CKIR4 decoder and bounded interpreter.

CKIR4 retains CKIR3's tables and adds opcode 13, ConstructRecord.  The
declaration/layout portion is checked through a declaration-only CKIR2
projection; the original CKIR4 control, value, place, constant, and constructor
relations are then reconstructed here without trusting a producer artifact.
"""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

import checked_ir_v2_reference as v2
import checked_ir_v3_reference as v3


NO_ID = 0xFFFF_FFFF
HEADER = v3.HEADER
ROWS = v3.ROWS
TABLE_ORDER = v3.TABLE_ORDER
COUNT_NAMES = v3.COUNT_NAMES


class Ckir4Error(v3.Ckir3Error):
    pass


class Ckir4ResourceError(Ckir4Error):
    """A validated public CKIR4 extent selects status 252."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise Ckir4Error(message)


def span(start: int, count: int, length: int, label: str) -> range:
    require(start <= length and count <= length - start, f"bad {label} span")
    return range(start, start + count)


def _declaration_projection(
    entry: int,
    flags: int,
    tables: dict[str, list[tuple[int, ...]]],
) -> v2.Module:
    """Use the frozen checker for types, records, fields, and machine signatures."""
    types = tables["types"]
    machine_params = tables["machine_params"]
    machines: list[tuple[int, ...]] = []
    blocks: list[tuple[int, ...]] = []
    operations: list[tuple[int, ...]] = []
    terminators: list[tuple[int, ...]] = []
    next_value = len(machine_params)
    for machine_id, machine in enumerate(tables["machines"]):
        result_type = machine[5]
        op_start = len(operations)
        if (
            result_type != NO_ID
            and result_type < len(types)
            and types[result_type][1] in (1, 2, 3)
        ):
            operations.append(
                (len(operations), machine_id, machine_id, 1, 1, 0,
                 next_value, result_type, 0, 0, types[result_type][6], 0)
            )
            terminator = (machine_id, machine_id, machine_id, 4, 0, 0,
                          next_value, NO_ID, 0, 0, NO_ID, 0, 0)
            next_value += 1
        else:
            terminator = (machine_id, machine_id, machine_id, 3, 0, 0,
                          NO_ID, NO_ID, 0, 0, NO_ID, 0, 0)
        blocks.append(
            (machine_id, machine_id, machine[2], 0, 0, 0, 0,
             op_start, len(operations) - op_start, machine_id)
        )
        terminators.append(terminator)
        machines.append((*machine[:8], machine_id, 1, machine_id))

    projected = {
        "types": tables["types"],
        "records": tables["records"],
        "fields": tables["fields"],
        "machines": machines,
        "machine_params": machine_params,
        "blocks": blocks,
        "block_params": [],
        "operations": operations,
        "operands": [],
        "terminators": terminators,
    }
    names = list(v2.ROWS)
    payload = b"".join(
        v2.ROWS[name].pack(*row)
        for name in names
        for row in projected[name]
    )
    return v2.decode(
        v2.HEADER.pack(
            b"OMGCKIR\0", 2, 0, 1, flags, entry,
            v2.HEADER.size + len(payload),
            *(len(projected[name]) for name in names),
            next_value, 0,
        ) + payload
    )


def _copyable(
    type_id: int,
    tables: dict[str, list[tuple[int, ...]]],
    active: set[int] | None = None,
) -> bool:
    types, records, fields = tables["types"], tables["records"], tables["fields"]
    active = set() if active is None else active
    require(type_id not in active, "recursive copyability")
    active.add(type_id)
    kind, payload0 = types[type_id][1], types[type_id][4]
    if kind in (1, 2, 3):
        result = True
    elif kind == 5:
        result = _copyable(payload0, tables, active)
    else:
        record = records[payload0]
        result = bool(record[4] & 1) and all(
            _copyable(fields[field_id][3], tables, active)
            for field_id in span(record[2], record[3], len(fields), "copy fields")
        )
    active.remove(type_id)
    return result


def _validate_constant_graph(
    tables: dict[str, list[tuple[int, ...]]],
) -> tuple[list[int], list[tuple[int, ...]]]:
    types, records, fields = tables["types"], tables["records"], tables["fields"]
    nodes = tables["constants"]
    children = [row[0] for row in tables["constant_children"]]
    heights: list[int] = []
    keys: list[tuple[int, ...]] = []
    next_child = 0
    for index, node in enumerate(nodes):
        node_id, type_id, child_start, child_count, scalar, reserved = node
        require(node_id == index and type_id < len(types) and reserved == 0,
                "constant node identity")
        require(child_start == next_child, "constant child partition")
        node_children = children[child_start:child_start + child_count]
        require(len(node_children) == child_count and all(child < index for child in node_children),
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
                    for field_id in span(record[2], record[3], len(fields), "constant fields")
                ]
                require(child_count <= 4, "record constant child exhaustion")
            else:
                expected = [types[type_id][4]] * types[type_id][5]
                require(child_count <= 1_024, "array constant child exhaustion")
            require(child_count == len(expected), "structural constant arity")
            require(all(nodes[child][1] == wanted for child, wanted in zip(node_children, expected)),
                    "constant child type")
            height = 1 + max((heights[child] for child in node_children), default=-1)
            key = (height, type_id, child_count, *node_children)
        require(not keys or keys[-1] < key, "constant canonical order")
        heights.append(height)
        keys.append(key)
        next_child += child_count
    require(next_child == len(children), "unused constant child")
    return children, nodes


def decode(contents: bytes) -> v2.Module:
    require(len(contents) >= HEADER.size, "truncated CKIR4 header")
    magic, major, minor, target, flags, entry, total, *raw_counts = HEADER.unpack_from(contents)
    require(magic == b"OMGCKIR\0" and (major, minor, target) == (4, 0, 1),
            "bad CKIR4 schema or target")
    require(flags in (0, 1) and (entry != NO_ID) == bool(flags & 1), "bad entry flags")
    require(total == len(contents) and len(contents) <= 2_522_192,
            "CKIR4 length or byte exhaustion")
    require(len(raw_counts) == len(COUNT_NAMES), "internal CKIR4 count schema")
    counts = dict(zip(COUNT_NAMES, raw_counts))
    ceilings = {
        "types": 8_192, "records": 128, "fields": 8_192, "machines": 128,
        "machine_params": 896, "blocks": 2_048, "block_params": 4_096,
        "operations": 32_768, "operands": 94_208, "terminators": 2_048,
        "values": 36_864, "places": 32_768, "constants": 8_192,
        "constant_children": 16_384,
    }
    require(all(counts[name] <= ceiling for name, ceiling in ceilings.items()),
            "CKIR4 table exhaustion")
    require(counts["machine_params"] + counts["block_params"] <= 4_096,
            "combined parameter exhaustion")
    expected_length = HEADER.size + sum(ROWS[name].size * counts[name] for name in TABLE_ORDER)
    require(expected_length == len(contents), "noncanonical CKIR4 table extent")

    tables: dict[str, list[tuple[int, ...]]] = {}
    cursor = HEADER.size
    for name in TABLE_ORDER:
        row = ROWS[name]
        tables[name] = [
            row.unpack_from(contents, cursor + index * row.size)
            for index in range(counts[name])
        ]
        cursor += counts[name] * row.size
    require(cursor == len(contents), "CKIR4 trailing bytes")

    declaration = _declaration_projection(entry, flags, tables)
    types, records, fields = tables["types"], tables["records"], tables["fields"]
    machines = tables["machines"]
    machine_params = tables["machine_params"]
    blocks, block_params = tables["blocks"], tables["block_params"]
    operations = tables["operations"]
    operands = [row[0] for row in tables["operands"]]
    terminators = tables["terminators"]
    children, nodes = _validate_constant_graph(tables)

    for name in ("blocks", "block_params", "operations", "terminators"):
        for index, row in enumerate(tables[name]):
            require(row[0] == index, f"non-dense {name} ID")
    require(len(terminators) == len(blocks), "terminator count")

    next_machine_param = next_block = 0
    for machine in machines:
        machine_id = machine[0]
        require(machine[6] == next_machine_param, "machine parameter partition")
        require(machine[8] == next_block and 1 <= machine[9] <= 128 and machine[10] == machine[8],
                "machine block partition")
        next_machine_param += machine[7]
        next_block += machine[9]
    require(next_machine_param == len(machine_params) and next_block == len(blocks),
            "machine partitions")

    next_block_param = next_operation = 0
    for block in blocks:
        block_id, owner, access, block_flags, reserved = block[:5]
        require(owner < len(machines) and access in (1, 2) and access <= machines[owner][2],
                "bad block")
        require(block_flags == reserved == 0 and block[9] == block_id, "block flags/terminator")
        require(block[5] == next_block_param and block[6] <= 7, "block parameter partition")
        require(block[7] == next_operation, "operation partition")
        require(block_id in span(machines[owner][8], machines[owner][9], len(blocks),
                                 "owner blocks"), "block owner")
        if block_id == machines[owner][10]:
            require(access == machines[owner][2] and block[6] == 0,
                    "entry block signature")
        for ordinal, parameter_id in enumerate(
            span(block[5], block[6], len(block_params), "block parameter")
        ):
            parameter = block_params[parameter_id]
            require(parameter[:4] == (parameter_id, block_id, ordinal, parameter[3])
                    and parameter[3] < len(types), "block parameter")
            require(parameter[4] == len(machine_params) + parameter_id,
                    "block parameter value")
            require(types[parameter[3]][1] in (1, 2, 3) or _copyable(parameter[3], tables),
                    "noncopyable structural block parameter")
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
        return (
            value_id < len(value_types)
            and value_machines[value_id] == owner
            and (
                value_blocks[value_id] == NO_ID
                or value_blocks[value_id] == block
                and (value_operations[value_id] == NO_ID or value_operations[value_id] < operation)
            )
        )

    def visible_place(place_id: int, block: int, operation: int) -> bool:
        return (
            place_id < len(place_types)
            and place_blocks[place_id] == block
            and place_operations[place_id] < operation
        )

    next_operand = 0
    for operation in operations:
        (op_id, owner, block, opcode, result_kind, op_flags, result_id,
         result_type, operand_start, operand_count, imm0, imm1) = operation
        require(op_flags == 0 and block < len(blocks) and owner == blocks[block][1],
                "operation owner")
        require(op_id in span(blocks[block][7], blocks[block][8], len(operations),
                              "block operations"), "operation block")
        require(operand_start == next_operand, "operation operands partition")
        op_values = operands[operand_start:operand_start + operand_count]
        require(len(op_values) == operand_count, "operation operands")
        next_operand += operand_count
        require(opcode in range(1, 14), "opcode")
        if opcode == 10:
            require(imm0 < len(machines), "call target")
            expected_kind = 0 if machines[imm0][5] == NO_ID else 1
        elif opcode == 13:
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

        expected_operands = (
            1 + machines[imm0][7] if opcode == 10
            else {1: 0, 2: 0, 3: 1, 4: 2, 5: 1, 6: 2, 7: 2,
                  8: 2, 9: 2, 11: 1, 12: 2}.get(opcode)
        )
        if opcode != 13:
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
            require(
                visible_value(op_values[1], owner, block, op_id)
                if imm0 == 1 else visible_place(op_values[1], block, op_id),
                "copy source",
            )
            source_type = value_types[op_values[1]] if imm0 == 1 else place_types[op_values[1]]
            require(place_types[op_values[0]] == source_type and types[source_type][1] in (4, 5),
                    "copy type")
            require(_copyable(source_type, tables), "copy of noncopyable type")
            require(place_mutable[op_values[0]], "shared place copy")
        elif opcode == 8:
            require(imm0 == imm1 == 0
                    and all(visible_value(value, owner, block, op_id) for value in op_values),
                    "add refs")
            require(types[value_types[op_values[0]]][1]
                    == types[value_types[op_values[1]]][1]
                    == types[result_type][1] and types[result_type][1] in (1, 2),
                    "add type")
        elif opcode in (9, 12):
            require(imm0 == imm1 == 0
                    and all(visible_value(value, owner, block, op_id) for value in op_values),
                    "comparison refs")
            require(types[value_types[op_values[0]]][1]
                    == types[value_types[op_values[1]]][1]
                    and types[value_types[op_values[0]]][1] in (1, 2),
                    "comparison type")
            require(types[result_type][1] == 3, "comparison result")
        elif opcode == 10:
            callee = machines[imm0]
            require(imm1 == 0 and visible_place(op_values[0], block, op_id), "call receiver")
            require(place_types[op_values[0]] == records[callee[1]][1], "call receiver type")
            require(callee[2] == 1 or place_mutable[op_values[0]],
                    "mutable call through shared place")
            for argument, parameter_id in zip(
                op_values[1:], span(callee[6], callee[7], len(machine_params), "call parameters")
            ):
                require(visible_value(argument, owner, block, op_id), "call argument visibility")
                require(value_types[argument] == machine_params[parameter_id][3],
                        "call argument type")
            if callee[5] != NO_ID:
                require(result_type == callee[5] and types[result_type][1] in (1, 2, 3),
                        "call result type")
            call_graph[owner].add(imm0)
        elif opcode == 11:
            require(imm1 == 0 and len(op_values) == 1
                    and visible_place(op_values[0], block, op_id), "CopyAggregateConst refs")
            destination = op_values[0]
            require(place_mutable[destination], "CopyAggregateConst place")
            require(imm0 < len(nodes) and nodes[imm0][1] == place_types[destination],
                    "CopyAggregateConst root type")
            require(types[nodes[imm0][1]][1] in (4, 5)
                    and _copyable(nodes[imm0][1], tables), "CopyAggregateConst root")
            roots.append(imm0)
        else:
            require(imm0 == imm1 == 0 and types[result_type][1] == 4,
                    "ConstructRecord result")
            record = records[types[result_type][4]]
            require(bool(record[4] & 1) and _copyable(result_type, tables),
                    "ConstructRecord copyability")
            require(operand_count == record[3], "ConstructRecord arity")
            for argument, field_id in zip(
                op_values, span(record[2], record[3], len(fields), "constructor fields")
            ):
                require(visible_value(argument, owner, block, op_id),
                        "ConstructRecord operand visibility")
                actual, wanted = value_types[argument], fields[field_id][3]
                if types[wanted][1] in (1, 2, 3):
                    require(types[actual][1] == types[wanted][1]
                            and types[actual][6] >= types[wanted][6]
                            and types[actual][7] <= types[wanted][7],
                            "ConstructRecord scalar field")
                else:
                    require(actual == wanted, "ConstructRecord structural field")
            if record[3] > 4:
                raise Ckir4ResourceError("ConstructRecord field exhaustion")
            constructor_results.add(result_id)

    require(len(value_types) == counts["values"] and len(place_types) == counts["places"],
            "reconstructed result counts")

    for term in terminators:
        (term_id, owner, block, kind, term_flags, reserved, value,
         target0, start0, count0, target1, start1, count1) = term
        require(block < len(blocks) and owner == blocks[block][1] and block == term_id
                and term_flags == reserved == 0, "terminator owner")
        require(kind in range(1, 5), "terminator kind")
        block_end = blocks[block][7] + blocks[block][8]
        require(start0 == next_operand, "target-0 operand partition")
        next_operand += count0
        require(start1 == next_operand, "target-1 operand partition")
        next_operand += count1
        for target, start, count in ((target0, start0, count0), (target1, start1, count1)):
            if target == NO_ID:
                require(count == 0, "arguments without target")
                continue
            require(target < len(blocks) and blocks[target][1] == owner
                    and target != machines[owner][10], "bad edge target")
            require(count == blocks[target][6], "edge arity")
            for argument, parameter_id in zip(
                operands[start:start + count],
                span(blocks[target][5], count, len(block_params), "edge params"),
            ):
                require(visible_value(argument, owner, block, block_end),
                        "edge argument visibility")
                require(argument not in constructor_results,
                        "direct ConstructRecord edge argument")
                actual, wanted = value_types[argument], block_params[parameter_id][3]
                require(actual == wanted if types[wanted][1] in (4, 5)
                        else types[actual][1] == types[wanted][1],
                        "edge argument type")
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
    require(next_operand == len(operands), "unused operand words")

    require((not nodes) == (not roots), "constant/root presence")
    reachable_nodes: set[int] = set()
    pending_nodes = list(set(roots))
    while pending_nodes:
        node = pending_nodes.pop()
        if node in reachable_nodes:
            continue
        reachable_nodes.add(node)
        row = nodes[node]
        pending_nodes.extend(children[row[2]:row[2] + row[3]])
    require(reachable_nodes == set(range(len(nodes))), "unreachable constant node")

    indegree = [0] * len(machines)
    for callees in call_graph:
        for callee in callees:
            indegree[callee] += 1
    pending = [machine_id for machine_id, degree in enumerate(indegree) if degree == 0]
    removed = 0
    while pending:
        caller = min(pending)
        pending.remove(caller)
        removed += 1
        for callee in sorted(call_graph[caller]):
            indegree[callee] -= 1
            if indegree[callee] == 0:
                pending.append(callee)
    require(removed == len(machines), "cyclic machine calls")

    return v2.Module(
        entry, tables, declaration.layouts, declaration.field_offsets,
        tuple(value_types), tuple(place_types),
    )


def materialize_constant(module: v2.Module, root: int) -> bytes:
    return v3.materialize_constant(module, root)


def constant_image(module: v2.Module) -> tuple[bytes, dict[int, int]]:
    return v3.constant_image(module)


def interpret(module: v2.Module, step_limit: int = 65_536, frame_limit: int = 64) -> int | None:
    if module.entry == NO_ID:
        return None
    tables = module.tables
    types, records, fields = tables["types"], tables["records"], tables["fields"]
    machines, blocks = tables["machines"], tables["blocks"]
    block_params, operations = tables["block_params"], tables["operations"]
    operands = [row[0] for row in tables["operands"]]
    terminators = tables["terminators"]
    owner_type = records[machines[module.entry][1]][1]
    memory = bytearray(module.layouts[owner_type][0])
    steps = 0

    def scalar_leaves(type_id: int, base: int = 0):
        kind, payload0, payload1 = types[type_id][1], types[type_id][4], types[type_id][5]
        if kind in (1, 2, 3):
            yield base, module.layouts[type_id][0]
        elif kind == 4:
            record = records[payload0]
            for field_id in span(record[2], record[3], len(fields), "runtime copy fields"):
                yield from scalar_leaves(fields[field_id][3], base + module.field_offsets[field_id])
        else:
            stride = module.layouts[payload0][0]
            for index in range(payload1):
                yield from scalar_leaves(payload0, base + index * stride)

    def semantic_copy(
        output: bytearray, destination: int, type_id: int, payload: bytes,
    ) -> None:
        for offset, size in scalar_leaves(type_id):
            require(offset + size <= len(payload), "runtime structural source extent")
            output[destination + offset:destination + offset + size] = payload[offset:offset + size]

    def run_machine(
        machine_id: int,
        receiver: int,
        arguments: list[tuple[int, int | bytes]],
        depth: int,
    ) -> int | None:
        nonlocal steps
        require(depth <= frame_limit, "active machine-frame exhaustion")
        machine = machines[machine_id]
        machine_values: dict[int, tuple[int, int | bytes]] = {}
        require(len(arguments) == machine[7], "runtime call arity")
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
                operation = operations[op_id]
                opcode, result_id, result_type = operation[3], operation[6], operation[7]
                args = operands[operation[8]:operation[8] + operation[9]]
                if opcode == 1:
                    values[result_id] = (result_type, operation[10])
                elif opcode == 2:
                    places[result_id] = (result_type, receiver)
                elif opcode == 3:
                    _, base = places[args[0]]
                    places[result_id] = (result_type, base + module.field_offsets[operation[10]])
                elif opcode == 4:
                    base_type, base = places[args[0]]
                    index = int(values[args[1]][1])
                    element_type, length = types[base_type][4], types[base_type][5]
                    require(index < length, "runtime index trap")
                    places[result_id] = (result_type, base + index * module.layouts[element_type][0])
                elif opcode == 5:
                    _, address = places[args[0]]
                    size = module.layouts[result_type][0]
                    values[result_id] = (
                        result_type,
                        int.from_bytes(memory[address:address + size], "little"),
                    )
                elif opcode == 6:
                    place_type, address = places[args[0]]
                    value = int(values[args[1]][1])
                    require(types[place_type][6] <= value <= types[place_type][7],
                            "runtime store range")
                    size = module.layouts[place_type][0]
                    memory[address:address + size] = value.to_bytes(size, "little")
                elif opcode == 7:
                    destination_type, destination = places[args[0]]
                    if operation[10] == 2:
                        source_type, source = places[args[1]]
                        payload = bytes(memory[source:source + module.layouts[source_type][0]])
                    else:
                        source_type, payload = values[args[1]]
                        require(isinstance(payload, bytes), "runtime structural value")
                    require(source_type == destination_type, "runtime copy type")
                    semantic_copy(memory, destination, destination_type, payload)
                elif opcode == 8:
                    value = int(values[args[0]][1]) + int(values[args[1]][1])
                    require(types[result_type][6] <= value <= types[result_type][7],
                            "runtime add range")
                    values[result_id] = (result_type, value)
                elif opcode in (9, 12):
                    left, right = int(values[args[0]][1]), int(values[args[1]][1])
                    values[result_id] = (
                        result_type,
                        int(left < right if opcode == 9 else left <= right),
                    )
                elif opcode == 10:
                    _, callee_receiver = places[args[0]]
                    result = run_machine(
                        operation[10], callee_receiver,
                        [values[value] for value in args[1:]], depth + 1,
                    )
                    if operation[4] == 1:
                        require(result is not None, "runtime missing call result")
                        values[result_id] = (result_type, result)
                elif opcode == 11:
                    destination_type, destination = places[args[0]]
                    require(destination_type == tables["constants"][operation[10]][1],
                            "runtime constant root type")
                    semantic_copy(
                        memory, destination, destination_type,
                        materialize_constant(module, operation[10]),
                    )
                elif opcode == 13:
                    record = records[types[result_type][4]]
                    payload = bytearray(module.layouts[result_type][0])
                    for argument, field_id in zip(
                        args, span(record[2], record[3], len(fields), "runtime constructor fields")
                    ):
                        field_type = fields[field_id][3]
                        actual_type, value = values[argument]
                        offset = module.field_offsets[field_id]
                        if types[field_type][1] in (1, 2, 3):
                            scalar = int(value)
                            require(types[field_type][6] <= scalar <= types[field_type][7],
                                    "runtime constructor range")
                            size = module.layouts[field_type][0]
                            payload[offset:offset + size] = scalar.to_bytes(size, "little")
                        else:
                            require(actual_type == field_type and isinstance(value, bytes),
                                    "runtime constructor structural value")
                            semantic_copy(payload, offset, field_type, value)
                    values[result_id] = (result_type, bytes(payload))
                else:
                    raise Ckir4Error("runtime opcode")
            term = terminators[block_id]
            if term[3] in (1, 2):
                first = term[3] == 1 or bool(int(values[term[6]][1]))
                target, start, count = (
                    (term[7], term[8], term[9])
                    if first else (term[10], term[11], term[12])
                )
                assigned: dict[int, tuple[int, int | bytes]] = {}
                for argument, parameter_id in zip(
                    operands[start:start + count],
                    span(blocks[target][5], count, len(block_params), "runtime edge"),
                ):
                    assigned[block_params[parameter_id][4]] = values[argument]
                block_values, block_id = assigned, target
            elif term[3] == 3:
                return None
            else:
                return int(values[term[6]][1])

    return run_machine(module.entry, 0, [], 1)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("validate", "run"))
    parser.add_argument("ckir", type=Path)
    args = parser.parse_args()
    module = decode(args.ckir.read_bytes())
    if args.command == "validate":
        image, roots = constant_image(module)
        constructors = sum(operation[3] == 13 for operation in module.tables["operations"])
        print(
            f"CKIR4 valid: {len(module.tables['types'])} types, "
            f"{constructors} constructors, {len(roots)} constant roots, "
            f"{len(image)} image bytes"
        )
    else:
        result = interpret(module)
        print("library" if result is None else result)


if __name__ == "__main__":
    try:
        main()
    except Ckir4ResourceError as error:
        print(f"checked IR v4 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(252)
    except (Ckir4Error, v3.Ckir3Error, v2.CkirError, OSError, struct.error) as error:
        print(f"checked IR v4 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(251)
