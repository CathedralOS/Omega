#!/usr/bin/env python3
"""Untrusted independent CKIR1 decoder and closed-entry interpreter."""

from __future__ import annotations

import argparse
import dataclasses
import struct
from pathlib import Path


NO_ID = 0xFFFF_FFFF
HEADER = struct.Struct("<8sHHHH14I")
ROWS = {
    "types": struct.Struct("<IBBHIIII"),
    "records": struct.Struct("<IIIIBBBB"),
    "fields": struct.Struct("<IIII"),
    "machines": struct.Struct("<IIBBHIIIIII"),
    "machine_params": struct.Struct("<IIIII"),
    "blocks": struct.Struct("<IIBBHIIIII"),
    "block_params": struct.Struct("<IIIII"),
    "operations": struct.Struct("<IIIBBHIIIIII"),
    "operands": struct.Struct("<I"),
    "terminators": struct.Struct("<IIIBBHIIIIIII"),
}


class CkirError(ValueError):
    pass


@dataclasses.dataclass(frozen=True)
class Module:
    entry: int
    tables: dict[str, list[tuple[int, ...]]]
    layouts: tuple[tuple[int, int], ...]
    field_offsets: tuple[int, ...]
    value_types: tuple[int, ...]
    place_types: tuple[int, ...]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise CkirError(message)


def span(start: int, count: int, length: int, label: str) -> range:
    require(start <= length and count <= length - start, f"bad {label} span")
    return range(start, start + count)


def decode(contents: bytes) -> Module:
    require(len(contents) >= HEADER.size, "truncated header")
    header = HEADER.unpack_from(contents)
    magic, major, minor, target, flags, *words = header
    require(magic == b"OMGCKIR\0", "bad magic")
    require((major, minor, target) == (1, 0, 1), "bad schema or target")
    require(flags in (0, 1), "bad flags")
    entry, total, *counts = words
    require(total == len(contents), "length mismatch")
    require((entry != NO_ID) == bool(flags & 1), "entry flag mismatch")
    names = list(ROWS)
    require(len(counts) == len(names) + 2, "internal count schema mismatch")
    table_counts = counts[: len(names)]
    ceilings = (8_192, 128, 8_192, 128, 4_096, 2_048, 4_096, 32_768, 131_072, 2_048, 40_960, 32_768)
    require(len(contents) <= 4_194_304, "CKIR byte exhaustion")
    require(all(count <= ceiling for count, ceiling in zip(counts, ceilings)), "CKIR table exhaustion")
    require(counts[4] + counts[6] <= 4_096, "combined parameter exhaustion")
    expected = HEADER.size + sum(ROWS[name].size * count for name, count in zip(names, table_counts))
    require(expected == len(contents), "noncanonical table extent")

    tables: dict[str, list[tuple[int, ...]]] = {}
    cursor = HEADER.size
    for name, count in zip(names, table_counts):
        row = ROWS[name]
        table = [row.unpack_from(contents, cursor + index * row.size) for index in range(count)]
        cursor += count * row.size
        tables[name] = table
    require(cursor == len(contents), "trailing bytes")

    types = tables["types"]
    records = tables["records"]
    fields = tables["fields"]
    machines = tables["machines"]
    machine_params = tables["machine_params"]
    blocks = tables["blocks"]
    block_params = tables["block_params"]
    operations = tables["operations"]
    operands = [row[0] for row in tables["operands"]]
    terminators = tables["terminators"]

    for name in ("types", "records", "fields", "machines", "machine_params", "blocks", "block_params", "operations", "terminators"):
        for index, row in enumerate(tables[name]):
            require(row[0] == index, f"non-dense {name} ID")

    require(len(terminators) == len(blocks), "terminator count")
    seen_type_keys: set[tuple[int, ...]] = set()
    nominal_owner: dict[int, int] = {}
    for type_id, kind, type_flags, reserved, payload0, payload1, low, high in types:
        require(kind in range(1, 6) and reserved == 0 and type_flags <= 1, "bad type row")
        require(kind not in (3, 4) or type_flags == 0, "forbidden trapping flag")
        if kind in (1, 2, 3):
            require(payload0 == payload1 == 0 and low <= high, "bad scalar type")
            require(high <= (255 if kind == 1 else 1 if kind == 3 else 0x7FFF_FFFF), "scalar range")
            if kind == 3:
                require((low, high) == (0, 1), "bool range")
        elif kind == 4:
            require(payload0 < len(records) and payload1 == low == high == 0, "bad nominal type")
            require(payload0 not in nominal_owner, "duplicate nominal owner")
            nominal_owner[payload0] = type_id
        else:
            require(payload0 < len(types) and payload1 <= 65_536 and low == high == 0, "bad array type")
        key = (kind, type_flags, payload0, payload1, low, high)
        require(key not in seen_type_keys, "duplicate interned type")
        seen_type_keys.add(key)

    next_field = 0
    for record_id, nominal, field_start, field_count, record_flags, reserved0, reserved1, reserved2 in records:
        require(
            record_flags <= 1
            and reserved0 == reserved1 == reserved2 == 0
            and nominal < len(types),
            "bad record row",
        )
        require(types[nominal][1] == 4 and types[nominal][4] == record_id, "record nominal")
        require(field_start == next_field and field_count <= 64, "field partition")
        for ordinal, field_id in enumerate(span(field_start, field_count, len(fields), "field")):
            row = fields[field_id]
            require(row == (field_id, record_id, ordinal, row[3]) and row[3] < len(types), "bad field row")
        next_field += field_count
    require(next_field == len(fields) and len(nominal_owner) == len(records), "field/nominal partition")

    visiting: set[int] = set()
    layout_cache: dict[int, tuple[int, int]] = {}
    field_offsets = [0] * len(fields)

    def align(value: int, alignment: int) -> int:
        return (value + alignment - 1) // alignment * alignment

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
        elif kind == 5:
            element_size, element_alignment = type_layout(payload0)
            result = (align(element_size, element_alignment) * payload1, element_alignment)
        else:
            record = records[payload0]
            cursor = 0
            record_alignment = 1
            for field_id in span(record[2], record[3], len(fields), "layout field"):
                field_size, field_alignment = type_layout(fields[field_id][3])
                cursor = align(cursor, field_alignment)
                field_offsets[field_id] = cursor
                cursor += field_size
                record_alignment = max(record_alignment, field_alignment)
            result = (align(cursor, record_alignment), record_alignment)
        visiting.remove(type_id)
        require(result[0] <= 0x7FFF_FFFF, "layout overflow")
        layout_cache[type_id] = result
        return result

    layouts = tuple(type_layout(index) for index in range(len(types)))

    copy_cache: dict[int, bool] = {}

    def copyable(type_id: int) -> bool:
        if type_id in copy_cache:
            return copy_cache[type_id]
        kind, payload0 = types[type_id][1], types[type_id][4]
        if kind in (1, 2, 3):
            result = True
        elif kind == 5:
            result = copyable(payload0)
        else:
            record = records[payload0]
            result = bool(record[4] & 1) and all(
                copyable(fields[field_id][3])
                for field_id in span(record[2], record[3], len(fields), "copy fields")
            )
        copy_cache[type_id] = result
        return result

    for record in records:
        if record[4] & 1:
            require(copyable(record[1]), "invalid [copy] record")

    next_machine_param = next_block = 0
    for row in machines:
        machine_id, owner, access, machine_flags, reserved, result, param_start, param_count, block_start, block_count, entry_block = row
        require(owner < len(records) and access in (1, 2) and machine_flags == reserved == 0, "bad machine")
        require(result == NO_ID or result < len(types) and types[result][1] in (1, 2, 3), "machine result")
        require(param_start == next_machine_param and param_count <= 7, "machine parameter partition")
        require(block_start == next_block and 1 <= block_count <= 128 and entry_block == block_start, "block partition")
        for ordinal, parameter_id in enumerate(span(param_start, param_count, len(machine_params), "machine parameter")):
            p = machine_params[parameter_id]
            require(p[:4] == (parameter_id, machine_id, ordinal, p[3]) and p[3] < len(types) and p[4] == parameter_id, "machine parameter")
            require(types[p[3]][1] in (1, 2, 3) or copyable(p[3]), "noncopyable structural machine parameter")
        next_machine_param += param_count
        next_block += block_count
    require(next_machine_param == len(machine_params) and next_block == len(blocks), "machine partitions")

    next_block_param = next_operation = 0
    for block_id, owner, access, block_flags, reserved, param_start, param_count, op_start, op_count, terminator in blocks:
        require(owner < len(machines) and access in (1, 2) and access <= machines[owner][2], "bad block")
        require(block_flags == reserved == 0 and terminator == block_id, "block flags/terminator")
        require(param_start == next_block_param and param_count <= 7, "block parameter partition")
        require(op_start == next_operation, "operation partition")
        require(block_id in span(machines[owner][8], machines[owner][9], len(blocks), "owner blocks"), "block owner")
        if block_id == machines[owner][10]:
            require(access == machines[owner][2] and param_count == 0, "entry block signature")
        for ordinal, parameter_id in enumerate(span(param_start, param_count, len(block_params), "block parameter")):
            p = block_params[parameter_id]
            require(p[:4] == (parameter_id, block_id, ordinal, p[3]) and p[3] < len(types), "block parameter")
            require(p[4] == len(machine_params) + parameter_id, "block parameter value")
            require(types[p[3]][1] in (1, 2, 3) or copyable(p[3]), "noncopyable structural block parameter")
        next_block_param += param_count
        next_operation += op_count
    require(next_block_param == len(block_params) and next_operation == len(operations), "block partitions")

    value_types = [p[3] for p in machine_params] + [p[3] for p in block_params]
    value_machines = [p[1] for p in machine_params] + [blocks[p[1]][1] for p in block_params]
    value_blocks = [NO_ID for _ in machine_params] + [p[1] for p in block_params]
    value_operations = [NO_ID] * len(value_types)
    place_types: list[int] = []
    place_mutable: list[bool] = []
    place_blocks: list[int] = []
    place_operations: list[int] = []

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
        op_id, owner, block, opcode, result_kind, op_flags, result_id, result_type, operand_start, operand_count, imm0, imm1 = operation
        require(op_flags == 0 and block < len(blocks) and owner == blocks[block][1], "operation owner")
        require(op_id in span(blocks[block][7], blocks[block][8], len(operations), "block operations"), "operation block")
        require(operand_start == next_operand, "operation operands partition")
        op_values = operands[operand_start:operand_start + operand_count]
        require(len(op_values) == operand_count, "operation operands")
        next_operand += operand_count
        require(opcode in range(1, 10), "opcode")
        expected_kind = 0 if opcode in (6, 7) else 2 if opcode in (2, 3, 4) else 1
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
        expected_operands = {1: 0, 2: 0, 3: 1, 4: 2, 5: 1, 6: 2, 7: 2, 8: 2, 9: 2}[opcode]
        require(operand_count == expected_operands, "operation arity")
        if opcode == 1:
            require(imm1 == 0 and types[result_type][1] in (1, 2, 3), "const type")
            require(types[result_type][6] <= imm0 <= types[result_type][7], "const range")
        elif opcode == 2:
            require(imm0 == imm1 == 0 and result_type == records[machines[owner][1]][1] and types[result_type][1] == 4, "self place")
            place_mutable[result_id] = blocks[block][2] == 2
        elif opcode == 3:
            require(imm0 < len(fields) and imm1 == 0 and visible_place(op_values[0], block, op_id), "field place")
            require(place_types[op_values[0]] == records[fields[imm0][1]][1] and result_type == fields[imm0][3], "field type")
            place_mutable[result_id] = place_mutable[op_values[0]]
        elif opcode == 4:
            require(imm0 == imm1 == 0 and visible_place(op_values[0], block, op_id) and visible_value(op_values[1], owner, block, op_id), "index refs")
            base_type = place_types[op_values[0]]
            require(types[base_type][1] == 5 and result_type == types[base_type][4], "index type")
            require(types[value_types[op_values[1]]][1] in (1, 2), "index scalar")
            place_mutable[result_id] = place_mutable[op_values[0]]
        elif opcode == 5:
            require(imm0 == imm1 == 0 and visible_place(op_values[0], block, op_id), "load ref")
            require(result_type == place_types[op_values[0]] and types[result_type][1] in (1, 2, 3), "load type")
        elif opcode == 6:
            require(imm0 == imm1 == 0 and visible_place(op_values[0], block, op_id) and visible_value(op_values[1], owner, block, op_id), "store refs")
            require(types[place_types[op_values[0]]][1] == types[value_types[op_values[1]]][1] and types[place_types[op_values[0]]][1] in (1, 2, 3), "store type")
            require(place_mutable[op_values[0]], "shared place store")
        elif opcode == 7:
            require(imm0 in (1, 2) and imm1 == 0 and visible_place(op_values[0], block, op_id), "copy refs")
            require(
                visible_value(op_values[1], owner, block, op_id)
                if imm0 == 1
                else visible_place(op_values[1], block, op_id),
                "copy source",
            )
            source_type = value_types[op_values[1]] if imm0 == 1 else place_types[op_values[1]]
            require(place_types[op_values[0]] == source_type and types[source_type][1] in (4, 5), "copy type")
            require(copyable(source_type), "copy of noncopyable type")
            require(place_mutable[op_values[0]], "shared place copy")
        elif opcode == 8:
            require(imm0 == imm1 == 0 and all(visible_value(value, owner, block, op_id) for value in op_values), "add refs")
            require(types[value_types[op_values[0]]][1] == types[value_types[op_values[1]]][1] == types[result_type][1] and types[result_type][1] in (1, 2), "add type")
        else:
            require(imm0 == imm1 == 0 and all(visible_value(value, owner, block, op_id) for value in op_values), "less refs")
            require(types[value_types[op_values[0]]][1] == types[value_types[op_values[1]]][1] and types[value_types[op_values[0]]][1] in (1, 2), "less type")
            require(types[result_type][1] == 3, "less result")
    require(len(value_types) == counts[-2] and len(place_types) == counts[-1], "reconstructed result counts")

    for term_id, owner, block, kind, term_flags, reserved, value, target0, start0, count0, target1, start1, count1 in terminators:
        require(
            block < len(blocks)
            and owner == blocks[block][1]
            and block == term_id
            and term_flags == reserved == 0,
            "terminator owner",
        )
        require(kind in range(1, 5), "terminator kind")
        block_end = blocks[block][7] + blocks[block][8]
        require(start0 == next_operand, "target-0 operand partition")
        next_operand += count0
        require(start1 == next_operand, "target-1 operand partition")
        next_operand += count1
        for target, start, count in ((target0, start0, count0), (target1, start1, count1)):
            if target == NO_ID:
                require(count == 0, "arguments without target")
            else:
                require(target < len(blocks) and blocks[target][1] == owner and target != machines[owner][10], "bad edge target")
                require(count == blocks[target][6], "edge arity")
                for argument, parameter_id in zip(operands[start:start + count], span(blocks[target][5], count, len(block_params), "edge params")):
                    require(visible_value(argument, owner, block, block_end), "edge argument visibility")
                    argument_type = value_types[argument]
                    parameter_type = block_params[parameter_id][3]
                    require(
                        argument_type == parameter_type
                        if types[parameter_type][1] in (4, 5)
                        else types[argument_type][1] == types[parameter_type][1],
                        "edge argument type",
                    )
        result_type = machines[owner][5]
        if kind == 1:
            require(value == target1 == NO_ID and target0 != NO_ID, "jump shape")
        elif kind == 2:
            require(visible_value(value, owner, block, block_end) and types[value_types[value]][1] == 3 and target0 != NO_ID and target1 != NO_ID, "branch shape")
        elif kind == 3:
            require(value == target0 == target1 == NO_ID and result_type == NO_ID, "Unit return")
        else:
            require(target0 == target1 == NO_ID and visible_value(value, owner, block, block_end), "value return")
            require(types[value_types[value]][1] == types[result_type][1], "return carrier")
    require(next_operand == len(operands), "unused operand words")
    candidates = [
        machine_id
        for machine_id, machine in enumerate(machines)
        if machine[7] == 0
        and machine[5] != NO_ID
        and types[machine[5]][1] in (1, 2, 3)
    ]
    require(len(candidates) <= 1, "ambiguous conformance roots")
    require(entry == (candidates[0] if candidates else NO_ID), "root cardinality/header mismatch")
    if entry != NO_ID:
        machine = machines[entry]
        require(machine[2] in (1, 2) and machine[7] == 0 and machine[5] != NO_ID, "bad entry signature")
        owner_type = records[machine[1]][1]
        require(layouts[owner_type][0] <= 131_072, "entry layout ceiling")
        require(_zero_establishes(owner_type, types, records, fields), "entry owner not zero-established")
    return Module(entry, tables, layouts, tuple(field_offsets), tuple(value_types), tuple(place_types))


def _zero_establishes(type_id, types, records, fields, active=None) -> bool:
    active = set() if active is None else active
    if type_id in active:
        return False
    active.add(type_id)
    _, kind, _, _, payload0, _, low, high = types[type_id]
    if kind in (1, 2, 3):
        result = low <= 0 <= high
    elif kind == 5:
        result = _zero_establishes(payload0, types, records, fields, active)
    else:
        record = records[payload0]
        result = all(_zero_establishes(fields[index][3], types, records, fields, active) for index in span(record[2], record[3], len(fields), "zero fields"))
    active.remove(type_id)
    return result


def interpret(module: Module, step_limit: int = 1_000_000) -> int | None:
    if module.entry == NO_ID:
        return None
    t = module.tables
    types, records, fields = t["types"], t["records"], t["fields"]
    machines, blocks = t["machines"], t["blocks"]
    block_params, operations, operands, terminators = t["block_params"], t["operations"], [row[0] for row in t["operands"]], t["terminators"]
    machine = machines[module.entry]
    owner_type = records[machine[1]][1]
    memory = bytearray(module.layouts[owner_type][0])
    parameters: dict[int, tuple[int, int | bytes]] = {}
    block_id = machine[10]
    steps = 0

    def scalar_leaves(type_id: int, base: int = 0):
        kind, payload0, payload1 = types[type_id][1], types[type_id][4], types[type_id][5]
        if kind in (1, 2, 3):
            yield base, module.layouts[type_id][0]
        elif kind == 5:
            stride = module.layouts[payload0][0]
            for index in range(payload1):
                yield from scalar_leaves(payload0, base + index * stride)
        else:
            record = records[payload0]
            for field_id in span(record[2], record[3], len(fields), "copy fields"):
                yield from scalar_leaves(
                    fields[field_id][3],
                    base + module.field_offsets[field_id],
                )

    def semantic_copy(type_id: int, destination: int, source: int) -> None:
        leaves = list(scalar_leaves(type_id))
        snapshots = [bytes(memory[source + offset:source + offset + size]) for offset, size in leaves]
        require(
            all(len(payload) == size for payload, (_, size) in zip(snapshots, leaves)),
            "runtime copy source extent",
        )
        for payload, (offset, size) in zip(snapshots, leaves):
            memory[destination + offset:destination + offset + size] = payload

    while True:
        steps += 1
        require(steps <= step_limit, "interpreter step exhaustion")
        block = blocks[block_id]
        values = dict(parameters)
        places: dict[int, tuple[int, int]] = {}
        for op_id in span(block[7], block[8], len(operations), "runtime operations"):
            op = operations[op_id]
            opcode, result_id, result_type = op[3], op[6], op[7]
            args = operands[op[8]:op[8] + op[9]]
            if opcode == 1:
                values[result_id] = (result_type, op[10])
            elif opcode == 2:
                places[result_id] = (result_type, 0)
            elif opcode == 3:
                base_type, base = places[args[0]]
                require(base_type == records[fields[op[10]][1]][1], "runtime field base")
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
                values[result_id] = (result_type, int.from_bytes(memory[address:address + size], "little"))
            elif opcode == 6:
                place_type, address = places[args[0]]
                value = int(values[args[1]][1])
                require(types[place_type][6] <= value <= types[place_type][7], "runtime store trap")
                memory[address:address + module.layouts[place_type][0]] = value.to_bytes(module.layouts[place_type][0], "little")
            elif opcode == 7:
                destination_type, destination = places[args[0]]
                if op[10] == 2:
                    source_type, source = places[args[1]]
                    require(source_type == destination_type, "runtime copy type")
                    semantic_copy(destination_type, destination, source)
                else:
                    source_type, payload = values[args[1]]
                    require(source_type == destination_type and isinstance(payload, bytes), "runtime structural value")
                    temporary = len(memory)
                    memory.extend(payload)
                    semantic_copy(destination_type, destination, temporary)
                    del memory[temporary:]
            elif opcode == 8:
                left, right = int(values[args[0]][1]), int(values[args[1]][1])
                value = left + right
                require(value <= (255 if types[result_type][1] == 1 else 0xFFFF_FFFF), "runtime add carrier trap")
                require(types[result_type][6] <= value <= types[result_type][7], "runtime add range trap")
                values[result_id] = (result_type, value)
            elif opcode == 9:
                values[result_id] = (result_type, int(int(values[args[0]][1]) < int(values[args[1]][1])))
            else:
                raise CkirError("runtime opcode")
        term = terminators[block_id]
        kind = term[3]
        if kind in (1, 2):
            take_first = kind == 1 or bool(int(values[term[6]][1]))
            target = term[7] if take_first else term[10]
            start = term[8] if take_first else term[11]
            count = term[9] if take_first else term[12]
            assigned: dict[int, tuple[int, int | bytes]] = {}
            for argument, parameter_id in zip(operands[start:start + count], span(blocks[target][5], count, len(block_params), "runtime edge")):
                parameter_type = block_params[parameter_id][3]
                value = values[argument]
                if types[parameter_type][1] in (1, 2, 3):
                    magnitude = int(value[1])
                    require(types[parameter_type][6] <= magnitude <= types[parameter_type][7], "runtime edge range trap")
                assigned[block_params[parameter_id][4]] = (parameter_type, value[1])
            parameters = assigned
            block_id = target
        elif kind == 3:
            return 0
        else:
            result = int(values[term[6]][1])
            result_type = machine[5]
            require(types[result_type][6] <= result <= types[result_type][7], "runtime return range trap")
            return result


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("validate", "run"))
    parser.add_argument("ckir", type=Path)
    arguments = parser.parse_args()
    module = decode(arguments.ckir.read_bytes())
    if arguments.command == "validate":
        print(
            f"CKIR1 valid: {len(module.tables['types'])} types, "
            f"{len(module.tables['machines'])} machines, entry={module.entry}"
        )
    else:
        result = interpret(module)
        print("library" if result is None else result)


if __name__ == "__main__":
    try:
        main()
    except (CkirError, OSError) as error:
        raise SystemExit(f"checked IR reference: {error}")
