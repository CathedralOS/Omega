#!/usr/bin/env python3
"""Independent CKIR17 checked-adapter event reference.

The decoder deliberately owns the CKIR17 selected relation.  It imports only
the frozen inherited row encodings; it does not call an older CKIR verdict and
it never resolves a BoundaryEvent to a host operation.
"""

from __future__ import annotations

import argparse
import dataclasses
import json
import struct
import sys
from pathlib import Path

import checked_ir_v15_reference as v15


NO_ID = 0xFFFF_FFFF
FREE = 1
STATIC_ATTACHED = 2

HEADER = struct.Struct("<8sHHHH25I")
ROWS = dict(v15.ROWS)
ROWS.update({
    "services": struct.Struct("<6I"),
    "machine_reaches": struct.Struct("<3I"),
    "rankings": struct.Struct("<5I"),
    "boundary_targets": struct.Struct("<9I"),
})
TABLE_ORDER = tuple(v15.TABLE_ORDER) + (
    "services", "machine_reaches", "rankings", "boundary_targets",
)
COUNT_NAMES = TABLE_ORDER + ("values", "places")

EXPECTED_COUNTS = {
    "types": 6,
    "records": 1,
    "fields": 0,
    "sums": 0,
    "cases": 0,
    "case_payloads": 0,
    "machines": 3,
    "machine_params": 7,
    "blocks": 9,
    "block_params": 14,
    "constants": 0,
    "constant_children": 0,
    "operations": 15,
    "operands": 38,
    "terminators": 9,
    "case_arms": 0,
    "case_arm_args": 0,
    "services": 1,
    "machine_reaches": 3,
    "rankings": 1,
    "boundary_targets": 1,
    "values": 32,
    "places": 0,
}

CEILINGS = {
    "types": 8_192, "records": 128, "fields": 8_192,
    "sums": 128, "cases": 4_096, "case_payloads": 4_096,
    "machines": 128, "machine_params": 896, "blocks": 2_048,
    "block_params": 4_096, "constants": 8_192,
    "constant_children": 16_384, "operations": 32_768,
    "operands": 94_208, "terminators": 2_048, "case_arms": 4_096,
    "case_arm_args": 94_208, "services": 128,
    "machine_reaches": 4_096, "rankings": 128,
    "boundary_targets": 4_096, "values": 36_864, "places": 32_768,
}
BYTE_CEILING = 2_654_288
TRACE_CEILING = 65_536
STEP_CEILING = 262_144
FRAME_CEILING = 64


class Ckir17Error(Exception):
    pass


class Ckir17ResourceError(Ckir17Error):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise Ckir17Error(message)


def bounded_span(start: int, count: int, length: int, label: str) -> range:
    require(start <= length and count <= length - start, f"bad {label} span")
    return range(start, start + count)


def signed_word(word: int) -> int:
    return word if word < 0x8000_0000 else word - 0x1_0000_0000


@dataclasses.dataclass(frozen=True)
class Module:
    tables: dict[str, tuple[tuple[int, ...], ...]]
    value_types: tuple[int, ...]
    value_owners: tuple[int, ...]
    value_blocks: tuple[int, ...]
    value_operations: tuple[int, ...]
    call_graph: tuple[frozenset[int], ...]


def _read_tables(contents: bytes) -> tuple[dict[str, list[tuple[int, ...]]], dict[str, int]]:
    require(len(contents) >= HEADER.size, "truncated CKIR17 header")
    fields = HEADER.unpack_from(contents)
    magic, major, minor, target, flags, entry, total, *raw_counts = fields
    require(
        (magic, major, minor, target, flags, entry)
        == (b"OMGCKIR\0", 17, 0, 1, 0, NO_ID),
        "bad CKIR17 identity or library flags",
    )
    require(total == len(contents), "CKIR17 length mismatch")
    if len(contents) > BYTE_CEILING:
        raise Ckir17ResourceError("CKIR17 byte exhaustion")
    require(len(raw_counts) == len(COUNT_NAMES), "internal CKIR17 count schema")
    counts = dict(zip(COUNT_NAMES, raw_counts))
    if any(counts[name] > CEILINGS[name] for name in COUNT_NAMES):
        raise Ckir17ResourceError("CKIR17 table exhaustion")
    require(counts == EXPECTED_COUNTS, "noncanonical selected CKIR17 counts")
    expected = HEADER.size + sum(ROWS[name].size * counts[name] for name in TABLE_ORDER)
    require(expected == len(contents), "noncanonical CKIR17 table extent")

    cursor = HEADER.size
    tables: dict[str, list[tuple[int, ...]]] = {}
    for name in TABLE_ORDER:
        row = ROWS[name]
        tables[name] = [
            row.unpack_from(contents, cursor + row.size * index)
            for index in range(counts[name])
        ]
        cursor += row.size * counts[name]
    require(cursor == len(contents), "CKIR17 trailing bytes")
    return tables, counts


def _strong_components(edges: list[set[int]]) -> list[set[int]]:
    index = 0
    stack: list[int] = []
    on_stack: set[int] = set()
    indices: dict[int, int] = {}
    low: dict[int, int] = {}
    result: list[set[int]] = []

    def visit(node: int) -> None:
        nonlocal index
        indices[node] = low[node] = index
        index += 1
        stack.append(node)
        on_stack.add(node)
        for target in sorted(edges[node]):
            if target not in indices:
                visit(target)
                low[node] = min(low[node], low[target])
            elif target in on_stack:
                low[node] = min(low[node], indices[target])
        if low[node] == indices[node]:
            component: set[int] = set()
            while True:
                member = stack.pop()
                on_stack.remove(member)
                component.add(member)
                if member == node:
                    break
            result.append(component)

    for node in range(len(edges)):
        if node not in indices:
            visit(node)
    return result


def decode(contents: bytes) -> Module:
    tables, counts = _read_tables(contents)
    types = tables["types"]
    records = tables["records"]
    machines = tables["machines"]
    machine_params = tables["machine_params"]
    blocks = tables["blocks"]
    block_params = tables["block_params"]
    operations = tables["operations"]
    operands = [row[0] for row in tables["operands"]]
    terminators = tables["terminators"]
    services = tables["services"]
    reaches = tables["machine_reaches"]
    rankings = tables["rankings"]
    targets = tables["boundary_targets"]

    dense_tables = (
        "types", "records", "machines", "machine_params", "blocks",
        "block_params", "operations", "terminators", "services",
        "machine_reaches", "rankings", "boundary_targets",
    )
    for name in dense_tables:
        for index, row in enumerate(tables[name]):
            require(row[0] == index, f"non-dense {name} ID")

    expected_types = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 3, 0, 0, 0, 0, 0, 1),
        (2, 1, 0, 0, 0, 0, 0, 255),
        (3, 7, 0, 0, 2, 0, 0, 0),
        (4, 9, 0, 0, 0, 0, 0x8000_0000, 0x7FFF_FFFF),
        (5, 10, 0, 0, 0, 0, 0, 0),
    ]
    require(types == expected_types, "selected CKIR17 type profile")
    require(signed_word(types[4][6]) == -(1 << 31)
            and signed_word(types[4][7]) == (1 << 31) - 1,
            "signed i32 endpoint interpretation")
    require(records == [(0, 0, 0, 0, 0, 0, 0, 0)],
            "selected provider record")
    require(all(not tables[name] for name in (
        "fields", "sums", "cases", "case_payloads", "constants",
        "constant_children", "case_arms", "case_arm_args",
    )), "excluded CKIR17 inherited table")

    require(services == [(0, 0, 0, 0, 1, 0)], "selected Console service")
    require(targets == [(0, 0, 4, 4, 4, 0, 4, NO_ID, 2)],
            "selected write_byte boundary target")
    require(reaches == [(0, 0, 0), (1, 1, 0), (2, 2, 0)],
            "exact machine reaches")
    require(rankings == [(0, 0, 1, 1, 1)],
            "exact SliceLength ranking")

    require(machines == [
        (0, NO_ID, 0, FREE, 0, NO_ID, 0, 3, 0, 7, 0),
        (1, 0, 0, STATIC_ATTACHED, 0, NO_ID, 3, 2, 7, 1, 7),
        (2, 0, 0, STATIC_ATTACHED, 0, NO_ID, 5, 2, 8, 1, 8),
    ], "selected free/static-attached machines")

    next_param = 0
    value_types: list[int] = []
    value_owners: list[int] = []
    value_blocks: list[int] = []
    value_operations: list[int] = []
    for machine_id, machine in enumerate(machines):
        owner, access, flags = machine[1], machine[2], machine[3]
        require(machine[4] == 0 and machine[5] == NO_ID, "machine reserved/result")
        if flags == FREE:
            require(owner == NO_ID and access == 0, "free-machine receiver shape")
        elif flags == STATIC_ATTACHED:
            require(owner < len(records) and access == 0,
                    "static-attached receiver shape")
        else:
            require(flags == 0 and owner < len(records) and access in (1, 2),
                    "attached receiver shape")
        require(machine[6] == next_param, "machine-parameter partition")
        for ordinal, parameter_id in enumerate(bounded_span(
                machine[6], machine[7], len(machine_params), "machine params")):
            parameter = machine_params[parameter_id]
            require(parameter == (parameter_id, machine_id, ordinal,
                                   parameter[3], parameter_id),
                    "machine parameter identity")
            require(parameter[3] in (1, 2, 3, 4, 5), "machine parameter type")
            value_types.append(parameter[3])
            value_owners.append(machine_id)
            value_blocks.append(NO_ID)
            value_operations.append(NO_ID)
        next_param += machine[7]
    require(next_param == len(machine_params), "machine parameter EOF")
    require([row[3] for row in machine_params] == [5, 3, 1, 5, 3, 5, 3],
            "selected machine parameter signature")

    next_block_param = next_operation = 0
    for block_id, block in enumerate(blocks):
        owner, access, block_flags, reserved = block[1:5]
        require(owner < len(machines) and reserved == 0, "block owner/reserved")
        require(access == machines[owner][2], "block receiver access")
        require(block_flags in (0, 1), "block flags")
        require(block[5] == next_block_param, "block-parameter partition")
        for ordinal, parameter_id in enumerate(bounded_span(
                block[5], block[6], len(block_params), "block params")):
            parameter = block_params[parameter_id]
            require(parameter == (parameter_id, block_id, ordinal,
                                   parameter[3], len(machine_params) + parameter_id),
                    "block parameter identity")
            require(parameter[3] in (1, 2, 3, 5), "block parameter type")
            value_types.append(parameter[3])
            value_owners.append(owner)
            value_blocks.append(block_id)
            value_operations.append(NO_ID)
        next_block_param += block[6]
        require(block[7] == next_operation and block[9] == block_id,
                "block operation/terminator partition")
        next_operation += block[8]
    require(next_block_param == len(block_params)
            and next_operation == len(operations), "block table EOF")
    require([row[3] for row in block_params]
            == [5, 3, 1, 5, 2, 3, 1, 5, 3, 1, 5, 1, 5, 2],
            "selected block parameter signatures")
    require([index for index, block in enumerate(blocks) if block[3] == 1]
            == [1, 3], "exact two synthetic true blocks")

    def visible(value: int, owner: int, block: int, operation: int) -> bool:
        return value < len(value_types) and value_owners[value] == owner and (
            value_blocks[value] == NO_ID or value_blocks[value] == block and (
                value_operations[value] == NO_ID
                or value_operations[value] < operation
            )
        )

    next_operand = 0
    call_graph = [set() for _ in machines]
    producer: dict[int, tuple[int, int]] = {}
    opcode_counts: dict[int, int] = {}
    boundary_operations: list[int] = []
    for operation in operations:
        (op_id, owner, block, opcode, result_kind, op_flags, result_id,
         result_type, operand_start, operand_count, imm0, imm1) = operation
        require(owner == blocks[block][1]
                and op_id in bounded_span(blocks[block][7], blocks[block][8],
                                          len(operations), "block operations")
                and op_flags == 0, "operation owner/flags")
        require(operand_start == next_operand, "operation operand partition")
        op_values = operands[operand_start:operand_start + operand_count]
        require(len(op_values) == operand_count, "operation operand extent")
        next_operand += operand_count
        require(opcode in (1, 23, 24, 25, 28, 29, 30), "CKIR17 opcode")
        opcode_counts[opcode] = opcode_counts.get(opcode, 0) + 1
        expected_kind = 0 if opcode in (28, 29) else 1
        require(result_kind == expected_kind, "operation result kind")
        if result_kind == 0:
            require(result_id == result_type == NO_ID, "spurious operation result")
        else:
            require(result_id == len(value_types) and result_type < len(types),
                    "dense operation value")
            value_types.append(result_type)
            value_owners.append(owner)
            value_blocks.append(block)
            value_operations.append(op_id)
            producer[result_id] = (op_id, opcode)

        expected_arity = {1: 0, 23: 1, 24: 1, 25: 1, 29: 2, 30: 1}.get(opcode)
        if opcode == 28:
            require(imm0 < len(machines), "receiverless call target")
            expected_arity = machines[imm0][7]
        require(operand_count == expected_arity, "operation arity")
        require(all(visible(value, owner, block, op_id) for value in op_values),
                "operation operand visibility")

        if opcode == 1:
            require(imm1 == 0 and result_type in (1, 2), "selected scalar Const")
            low, high = types[result_type][6:8]
            require(low <= imm0 <= high, "Const range")
        elif opcode in (23, 24, 25):
            source_type = value_types[op_values[0]]
            require(source_type == 3 and imm0 == imm1 == 0,
                    "runtime byte-view operand")
            wanted = 1 if opcode == 23 else 2 if opcode == 24 else 3
            require(result_type == wanted, "runtime byte-view result")
            if opcode in (24, 25):
                require(blocks[block][3] == 1, "partial view outside true block")
        elif opcode == 28:
            callee = machines[imm0]
            require(callee[3] in (FREE, STATIC_ATTACHED) and imm1 == 0,
                    "ReceiverlessCall target class")
            for value, parameter_id in zip(op_values,
                    bounded_span(callee[6], callee[7], len(machine_params),
                                 "callee parameters")):
                require(value_types[value] == machine_params[parameter_id][3],
                        "ReceiverlessCall argument type")
            call_graph[owner].add(imm0)
        elif opcode == 29:
            require(imm0 == 0 and imm1 == 0, "BoundaryEvent target")
            target = targets[imm0]
            require(value_types[op_values[0]] == 5
                    and types[5][4] == target[1]
                    and value_types[op_values[1]] == target[6],
                    "BoundaryEvent service/signature")
            require(producer.get(op_values[1], (NO_ID, NO_ID))[1] == 30,
                    "BoundaryEvent requires exact widen")
            boundary_operations.append(op_id)
        else:
            require(opcode == 30 and imm0 == imm1 == 0
                    and value_types[op_values[0]] == 2 and result_type == 4,
                    "exact U8ToI32")

    require(next_operand == 18, "operation operand endpoint")
    require(opcode_counts == {1: 3, 23: 2, 24: 2, 25: 2,
                              28: 2, 29: 2, 30: 2},
            "selected operation family")
    require(boundary_operations == [4, 10], "ordered helper requirement calls")
    require(len(value_types) == counts["values"] and counts["places"] == 0,
            "reconstructed value/place counts")
    require(all(type_id != 5 for type_id in value_types[len(machine_params)
                                                        + len(block_params):]),
            "opaque service construction")

    block_edges = [set() for _ in blocks]
    edge_rows: list[tuple[int, int, tuple[int, ...]]] = []
    next_term_operand = next_operand
    for term in terminators:
        (term_id, owner, block, kind, term_flags, reserved, value,
         target0, start0, count0, target1, start1, count1,
         arm_start, arm_count) = term
        require(term_id == block and owner == blocks[block][1]
                and term_flags == reserved == arm_count == 0,
                "terminator identity/flags")
        require(arm_start == 0, "excluded case-arm relation")
        require(start0 == next_term_operand, "target-0 operand partition")
        next_term_operand += count0
        require(start1 == next_term_operand, "target-1 operand partition")
        next_term_operand += count1
        for target, start, count in ((target0, start0, count0),
                                     (target1, start1, count1)):
            if target == NO_ID:
                require(count == 0, "arguments without target")
                continue
            require(target < len(blocks) and blocks[target][1] == owner
                    and target != machines[owner][10]
                    and count == blocks[target][6], "edge target/arity")
            args = tuple(operands[start:start + count])
            for argument, parameter_id in zip(args, bounded_span(
                    blocks[target][5], count, len(block_params), "edge params")):
                require(visible(argument, owner, block,
                                blocks[block][7] + blocks[block][8]),
                        "edge argument visibility")
                require(value_types[argument] == block_params[parameter_id][3],
                        "edge argument type")
            block_edges[block].add(target)
            edge_rows.append((block, target, args))
        if kind == 1:
            require(value == target1 == NO_ID and target0 != NO_ID,
                    "Jump shape")
        elif kind == 2:
            require(visible(value, owner, block,
                            blocks[block][7] + blocks[block][8])
                    and value_types[value] == 1
                    and target0 != NO_ID and target1 != NO_ID,
                    "Branch shape")
        elif kind == 3:
            require(value == target0 == target1 == NO_ID,
                    "ReturnUnit shape")
        else:
            require(False, "selected terminator kind")
    require(next_term_operand == len(operands), "terminator operand EOF")

    # Every synthetic block is selected by a true SliceNonEmpty predecessor,
    # owns exactly Head then Tail over the same incoming view, and commits the
    # tail on its outgoing edge.
    predecessors: list[list[tuple[int, int, tuple[int, ...]]]] = [
        [] for _ in blocks
    ]
    for source, target, args in edge_rows:
        predecessors[target].append((source, 0 if terminators[source][7] == target else 1,
                                     args))
    for block_id in (1, 3):
        block = blocks[block_id]
        require(len(predecessors[block_id]) == 1
                and predecessors[block_id][0][1] == 0,
                "synthetic true predecessor")
        source = predecessors[block_id][0][0]
        guard = terminators[source][6]
        guard_op_id, guard_opcode = producer.get(guard, (NO_ID, NO_ID))
        require(guard_opcode == 23 and value_operations[guard] == guard_op_id,
                "synthetic nonempty guard")
        op_ids = list(bounded_span(block[7], block[8], len(operations),
                                   "synthetic operations"))
        require([operations[index][3] for index in op_ids] == [24, 25],
                "synthetic head/tail pair")
        head, tail = operations[op_ids[0]], operations[op_ids[1]]
        require(operands[head[8]] == operands[tail[8]],
                "synthetic shared view source")
        require(terminators[block_id][3] == 1, "synthetic jump")
        outgoing_args = tuple(operands[
            terminators[block_id][8]:terminators[block_id][8]
            + terminators[block_id][9]
        ])
        require(head[6] in outgoing_args and tail[6] in outgoing_args,
                "synthetic head/tail custody")

    # Ranking is reconstructed over the helper CFG. The guard-to-synthetic
    # edge may carry the original view; every cyclic edge leaving a synthetic
    # true block must carry its exact Tail result into each view parameter.
    ranking = rankings[0]
    ranked_machine, ranked_ordinal = ranking[1], ranking[2]
    ranked_parameter = machine_params[machines[ranked_machine][6] + ranked_ordinal]
    require(ranked_parameter[3] == 3, "ranking subject is runtime view")
    components = _strong_components(block_edges)
    cyclic_components = [component for component in components
                         if len(component) > 1
                         or any(node in block_edges[node] for node in component)]
    recurrent_descents = 0
    for component in cyclic_components:
        require(all(blocks[node][1] == ranked_machine for node in component),
                "ranking component owner")
        for source, target, args in edge_rows:
            if source not in component or target not in component \
                    or blocks[source][3] != 1:
                continue
            for ordinal, parameter_id in enumerate(bounded_span(
                    blocks[target][5], blocks[target][6], len(block_params),
                    "ranked target params")):
                if block_params[parameter_id][3] != 3:
                    continue
                argument = args[ordinal]
                op_id, opcode = producer.get(argument, (NO_ID, NO_ID))
                require(opcode == 25 and operations[op_id][2] == source,
                        "recurrent view must be exact local tail")
                recurrent_descents += 1
    require(recurrent_descents == 1, "exact strict recurrent descent")

    # Reach closure is a ceiling check, independent of selected-candidate
    # dispatch. BoundaryEvent contributes a direct service use; calls inherit
    # the callee's service set.
    reach_sets = [set() for _ in machines]
    for _, machine, service in reaches:
        require(machine < len(machines) and service < len(services),
                "reach identity")
        require(service not in reach_sets[machine], "duplicate reach")
        reach_sets[machine].add(service)
    for op_id in boundary_operations:
        operation = operations[op_id]
        service = targets[operation[10]][1]
        require(service in reach_sets[operation[1]], "direct service outside reach")
    for caller, callees in enumerate(call_graph):
        for callee in callees:
            require(reach_sets[callee] <= reach_sets[caller],
                    "callee reach outside caller ceiling")

    indegree = [0] * len(machines)
    for callees in call_graph:
        for callee in callees:
            indegree[callee] += 1
    ready = [index for index, degree in enumerate(indegree) if degree == 0]
    removed = 0
    while ready:
        caller = min(ready)
        ready.remove(caller)
        removed += 1
        for callee in sorted(call_graph[caller]):
            indegree[callee] -= 1
            if indegree[callee] == 0:
                ready.append(callee)
    require(removed == len(machines), "cyclic receiverless calls")

    return Module(
        {name: tuple(rows) for name, rows in tables.items()},
        tuple(value_types), tuple(value_owners), tuple(value_blocks),
        tuple(value_operations), tuple(frozenset(edges) for edges in call_graph),
    )


def invoke(module: Module, adapter: str | int, data: bytes,
           *, step_limit: int = STEP_CEILING,
           trace_limit: int = TRACE_CEILING) -> tuple[int, ...]:
    """Execute one static adapter against an injected abstract event sink."""
    if len(data) > trace_limit:
        raise Ckir17ResourceError("CKIR17 runtime view exhaustion")
    machine_id = {"write": 1, "write_line": 2}.get(adapter, adapter)
    require(machine_id in (1, 2), "selected adapter invocation")
    tables = module.tables
    types = tables["types"]
    machines = tables["machines"]
    machine_params = tables["machine_params"]
    blocks = tables["blocks"]
    block_params = tables["block_params"]
    operations = tables["operations"]
    operands = [row[0] for row in tables["operands"]]
    terminators = tables["terminators"]
    targets = tables["boundary_targets"]
    events: list[int] = []
    steps = 0
    service_token = object()

    def check_value(type_id: int, value: object) -> None:
        kind = types[type_id][1]
        if kind == 1:
            require(isinstance(value, int) and 0 <= value <= 255,
                    "runtime u8 range")
        elif kind == 3:
            require(value in (0, 1), "runtime bool range")
        elif kind == 7:
            require(isinstance(value, bytes), "runtime shared view")
        elif kind == 9:
            require(isinstance(value, int) and -(1 << 31) <= value < (1 << 31),
                    "runtime signed i32 range")
        elif kind == 10:
            require(value is service_token, "runtime service custody")
        else:
            require(False, "runtime selected type")

    def run_machine(selected: int, arguments: list[tuple[int, object]], depth: int) -> None:
        nonlocal steps
        if depth > FRAME_CEILING:
            raise Ckir17ResourceError("CKIR17 active-frame exhaustion")
        machine = machines[selected]
        require(machine[3] in (FREE, STATIC_ATTACHED),
                "runtime receiverless machine")
        require(len(arguments) == machine[7], "runtime call arity")
        machine_values: dict[int, tuple[int, object]] = {}
        for ordinal, argument in enumerate(arguments):
            parameter = machine_params[machine[6] + ordinal]
            require(argument[0] == parameter[3], "runtime argument type")
            check_value(parameter[3], argument[1])
            machine_values[parameter[4]] = argument
        block_values: dict[int, tuple[int, object]] = {}
        block_id = machine[10]
        while True:
            steps += 1
            if steps > step_limit:
                raise Ckir17ResourceError("CKIR17 dynamic step exhaustion")
            block = blocks[block_id]
            values = dict(machine_values)
            values.update(block_values)
            for op_id in bounded_span(block[7], block[8], len(operations),
                                      "runtime operations"):
                op = operations[op_id]
                opcode, result_id, result_type = op[3], op[6], op[7]
                args = [values[operands[index]] for index in bounded_span(
                    op[8], op[9], len(operands), "runtime operands")]
                if opcode == 1:
                    values[result_id] = (result_type, op[10])
                elif opcode == 23:
                    values[result_id] = (result_type, int(len(args[0][1]) != 0))
                elif opcode == 24:
                    view = args[0][1]
                    require(isinstance(view, bytes) and bool(view),
                            "runtime SliceHead on empty view")
                    values[result_id] = (result_type, view[0])
                elif opcode == 25:
                    view = args[0][1]
                    require(isinstance(view, bytes) and bool(view),
                            "runtime SliceTailOne on empty view")
                    values[result_id] = (result_type, view[1:])
                elif opcode == 28:
                    run_machine(op[10], args, depth + 1)
                elif opcode == 29:
                    require(args[0][1] is service_token and op[10] < len(targets),
                            "runtime BoundaryEvent service")
                    byte = args[1][1]
                    require(isinstance(byte, int) and 0 <= byte <= 255,
                            "runtime BoundaryEvent byte")
                    if len(events) >= trace_limit:
                        raise Ckir17ResourceError("CKIR17 event trace exhaustion")
                    events.append(byte)
                else:
                    require(opcode == 30, "runtime opcode")
                    values[result_id] = (result_type, int(args[0][1]))

            term = terminators[block_id]
            if term[3] == 3:
                return
            if term[3] == 1:
                target, start, count = term[7], term[8], term[9]
            else:
                require(term[3] == 2, "runtime terminator")
                branch = int(values[term[6]][1])
                target, start, count = ((term[7], term[8], term[9])
                                        if branch else (term[10], term[11], term[12]))
            staged = [values[operands[index]] for index in bounded_span(
                start, count, len(operands), "runtime edge arguments")]
            block_values = {}
            for ordinal, argument in enumerate(staged):
                parameter = block_params[blocks[target][5] + ordinal]
                require(argument[0] == parameter[3], "runtime edge type")
                check_value(parameter[3], argument[1])
                block_values[parameter[4]] = argument
            block_id = target

    run_machine(int(machine_id), [(5, service_token), (3, bytes(data))], 1)
    return tuple(events)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("validate", "run"))
    parser.add_argument("ckir", type=Path)
    parser.add_argument("--adapter", choices=("write", "write_line"), default="write")
    parser.add_argument("--hex", default="")
    parser.add_argument("--input-file", type=Path)
    args = parser.parse_args()
    module = decode(args.ckir.read_bytes())
    if args.command == "validate":
        print("CKIR17 valid: free helper, two static adapters, ranked Console events")
        return
    require(not (args.hex and args.input_file is not None),
            "choose one runtime input")
    data = args.input_file.read_bytes() if args.input_file is not None \
        else bytes.fromhex(args.hex)
    print(json.dumps(invoke(module, args.adapter, data), separators=(",", ":")))


if __name__ == "__main__":
    try:
        main()
    except Ckir17ResourceError as error:
        print(f"checked IR v17 reference: {error}", file=sys.stderr)
        raise SystemExit(252)
    except (Ckir17Error, OSError, ValueError, struct.error) as error:
        print(f"checked IR v17 reference: {error}", file=sys.stderr)
        raise SystemExit(251)
