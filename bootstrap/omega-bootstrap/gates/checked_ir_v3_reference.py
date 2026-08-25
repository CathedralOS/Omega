#!/usr/bin/env python3
"""Independent CKIR3 decoder and bounded interpreter.

Inherited CKIR2 rows are checked by translating only the versioned additions
away and invoking the independent CKIR2 checker.  Constant-graph custody,
CopyAggregateConst, and LessEqual are checked and interpreted here.
"""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

import checked_ir_v2_reference as v2


NO_ID = 0xFFFF_FFFF
HEADER = struct.Struct("<8sHHHH16I")
CONSTANT = struct.Struct("<IIIIII")
WORD = struct.Struct("<I")
TABLE_ORDER = (
    "types", "records", "fields", "machines", "machine_params", "blocks",
    "block_params", "constants", "constant_children", "operations",
    "operands", "terminators",
)
ROWS = dict(v2.ROWS, constants=CONSTANT, constant_children=WORD)
COUNT_NAMES = (
    "types", "records", "fields", "machines", "machine_params", "blocks",
    "block_params", "operations", "operands", "terminators", "values",
    "places", "constants", "constant_children",
)


class Ckir3Error(v2.CkirError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise Ckir3Error(message)


def span(start: int, count: int, length: int, label: str) -> range:
    require(start <= length and count <= length - start, f"bad {label} span")
    return range(start, start + count)


def _encode_v2(
    entry: int,
    flags: int,
    counts: dict[str, int],
    tables: dict[str, list[tuple[int, ...]]],
) -> bytes:
    operations = tables["operations"]
    operands = [row[0] for row in tables["operands"]]
    blocks: list[tuple[int, ...]] = []
    rewritten_operations: list[tuple[int, ...]] = []
    rewritten_operands: list[tuple[int]] = []

    for block in tables["blocks"]:
        op_start = len(rewritten_operations)
        for old_id in span(block[7], block[8], len(operations), "operation"):
            operation = operations[old_id]
            opcode = operation[3]
            arguments = operands[operation[8]:operation[8] + operation[9]]
            require(len(arguments) == operation[9], "operation operand extent")
            if opcode == 11:
                continue
            new_id = len(rewritten_operations)
            operand_start = len(rewritten_operands)
            rewritten_operands.extend((argument,) for argument in arguments)
            rewritten_operations.append(
                (new_id, *operation[1:3], 9 if opcode == 12 else opcode,
                 *operation[4:8], operand_start, operation[9], *operation[10:])
            )
        blocks.append((*block[:7], op_start, len(rewritten_operations) - op_start, block[9]))

    terminators: list[tuple[int, ...]] = []
    for term in tables["terminators"]:
        starts: list[int] = []
        for start, count in ((term[8], term[9]), (term[11], term[12])):
            arguments = operands[start:start + count]
            require(len(arguments) == count, "terminator operand extent")
            starts.append(len(rewritten_operands))
            rewritten_operands.extend((argument,) for argument in arguments)
        terminators.append((*term[:8], starts[0], term[9], term[10], starts[1], term[12]))

    v2_tables = dict(tables)
    v2_tables["blocks"] = blocks
    v2_tables["operations"] = rewritten_operations
    v2_tables["operands"] = rewritten_operands
    v2_tables["terminators"] = terminators
    names = list(v2.ROWS)
    table_counts = [len(v2_tables[name]) for name in names]
    payload = b"".join(
        v2.ROWS[name].pack(*row)
        for name in names
        for row in v2_tables[name]
    )
    return v2.HEADER.pack(
        b"OMGCKIR\0", 2, 0, 1, flags, entry, v2.HEADER.size + len(payload),
        *table_counts, counts["values"], counts["places"],
    ) + payload


def _copyable(type_id: int, tables: dict[str, list[tuple[int, ...]]], active: set[int] | None = None) -> bool:
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


def decode(contents: bytes) -> v2.Module:
    require(len(contents) >= HEADER.size, "truncated CKIR3 header")
    unpacked = HEADER.unpack_from(contents)
    magic, major, minor, target, flags, entry, total, *raw_counts = unpacked
    require(magic == b"OMGCKIR\0" and (major, minor, target) == (3, 0, 1), "bad CKIR3 schema or target")
    require(flags in (0, 1) and (entry != NO_ID) == bool(flags & 1), "bad entry flags")
    require(total == len(contents) and len(contents) <= 2_522_192, "CKIR3 length or byte exhaustion")
    require(len(raw_counts) == len(COUNT_NAMES), "internal CKIR3 count schema")
    counts = dict(zip(COUNT_NAMES, raw_counts))
    ceilings = {
        "types": 8_192, "records": 128, "fields": 8_192, "machines": 128,
        "machine_params": 896, "blocks": 2_048, "block_params": 4_096,
        "operations": 32_768, "operands": 94_208, "terminators": 2_048,
        "values": 36_864, "places": 32_768, "constants": 8_192,
        "constant_children": 16_384,
    }
    require(all(counts[name] <= ceiling for name, ceiling in ceilings.items()), "CKIR3 table exhaustion")
    expected = HEADER.size + sum(ROWS[name].size * counts[name] for name in TABLE_ORDER)
    require(expected == len(contents), "noncanonical CKIR3 table extent")

    tables: dict[str, list[tuple[int, ...]]] = {}
    cursor = HEADER.size
    for name in TABLE_ORDER:
        row = ROWS[name]
        tables[name] = [row.unpack_from(contents, cursor + index * row.size) for index in range(counts[name])]
        cursor += counts[name] * row.size
    require(cursor == len(contents), "CKIR3 trailing bytes")

    base = v2.decode(_encode_v2(entry, flags, counts, tables))
    require(tuple(base.value_types) and len(base.value_types) == counts["values"] or counts["values"] == 0, "value reconstruction")
    require(len(base.place_types) == counts["places"], "place reconstruction")

    types, records, fields = tables["types"], tables["records"], tables["fields"]
    nodes = tables["constants"]
    children = [row[0] for row in tables["constant_children"]]
    heights: list[int] = []
    keys: list[tuple[int, ...]] = []
    next_child = 0
    for index, node in enumerate(nodes):
        node_id, type_id, child_start, child_count, scalar, reserved = node
        require(node_id == index and type_id < len(types) and reserved == 0, "constant node identity")
        require(child_start == next_child, "constant child partition")
        node_children = children[child_start:child_start + child_count]
        require(len(node_children) == child_count and all(child < index for child in node_children), "constant child order")
        kind = types[type_id][1]
        if kind in (1, 2, 3):
            require(child_count == 0 and types[type_id][6] <= scalar <= types[type_id][7], "scalar constant")
            height = 0
            key = (height, type_id, scalar)
        else:
            require(scalar == 0, "structural constant scalar")
            if kind == 4:
                record = records[types[type_id][4]]
                expected_types = [fields[field_id][3] for field_id in span(record[2], record[3], len(fields), "constant fields")]
                require(child_count <= 4, "record constant child exhaustion")
            else:
                expected_types = [types[type_id][4]] * types[type_id][5]
                require(child_count <= 1_024, "array constant child exhaustion")
            require(child_count == len(expected_types), "structural constant arity")
            require(all(nodes[child][1] == wanted for child, wanted in zip(node_children, expected_types)), "constant child type")
            height = 1 + max((heights[child] for child in node_children), default=-1)
            key = (height, type_id, child_count, *node_children)
        require(not keys or keys[-1] < key, "constant canonical order")
        heights.append(height)
        keys.append(key)
        next_child += child_count
    require(next_child == len(children), "unused constant child")

    place_mutable: list[bool] = []
    place_blocks: list[int] = []
    place_operations: list[int] = []
    roots: list[int] = []
    operands = [row[0] for row in tables["operands"]]
    for operation in tables["operations"]:
        op_id, owner, block, opcode, result_kind, op_flags, result_id, result_type, start, count, imm0, imm1 = operation
        arguments = operands[start:start + count]
        if result_kind == 2:
            require(result_id == len(place_mutable), "place result order")
            if opcode == 2:
                mutable = tables["blocks"][block][2] == 2
            elif opcode in (3, 4):
                require(arguments and arguments[0] < len(place_mutable), "place base")
                mutable = place_mutable[arguments[0]]
            else:
                raise Ckir3Error("unexpected place producer")
            place_mutable.append(mutable)
            place_blocks.append(block)
            place_operations.append(op_id)
        if opcode == 11:
            require(op_flags == 0, "CopyAggregateConst flags")
            require(result_kind == 0 and result_id == result_type == NO_ID and count == 1 and imm1 == 0, "CopyAggregateConst shape")
            require(len(arguments) == 1 and arguments[0] < len(base.place_types), "CopyAggregateConst destination")
            destination = arguments[0]
            require(place_blocks[destination] == block and place_operations[destination] < op_id and place_mutable[destination], "CopyAggregateConst place")
            require(imm0 < len(nodes) and nodes[imm0][1] == base.place_types[destination], "CopyAggregateConst root type")
            require(types[nodes[imm0][1]][1] in (4, 5) and _copyable(nodes[imm0][1], tables), "CopyAggregateConst root")
            roots.append(imm0)
    require(len(place_mutable) == len(base.place_types), "place metadata reconstruction")
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

    tables["machine_params"] = tables.pop("machine_params")
    tables["block_params"] = tables.pop("block_params")
    return v2.Module(entry, tables, base.layouts, base.field_offsets, base.value_types, base.place_types)


def materialize_constant(module: v2.Module, root: int) -> bytes:
    tables = module.tables
    types, records, fields = tables["types"], tables["records"], tables["fields"]
    nodes = tables["constants"]
    children = [row[0] for row in tables["constant_children"]]

    def fill(node_id: int, output: bytearray, base: int) -> None:
        _, type_id, start, count, scalar, _ = nodes[node_id]
        kind, payload0, payload1 = types[type_id][1], types[type_id][4], types[type_id][5]
        if kind in (1, 2, 3):
            size = module.layouts[type_id][0]
            output[base:base + size] = scalar.to_bytes(size, "little")
        elif kind == 4:
            record = records[payload0]
            for child, field_id in zip(children[start:start + count], span(record[2], record[3], len(fields), "materialize fields")):
                fill(child, output, base + module.field_offsets[field_id])
        else:
            stride = module.layouts[payload0][0]
            for ordinal, child in enumerate(children[start:start + count]):
                fill(child, output, base + ordinal * stride)

    type_id = nodes[root][1]
    output = bytearray(module.layouts[type_id][0])
    fill(root, output, 0)
    return bytes(output)


def constant_image(module: v2.Module) -> tuple[bytes, dict[int, int]]:
    roots = sorted({operation[10] for operation in module.tables["operations"] if operation[3] == 11})
    image = bytearray()
    offsets: dict[int, int] = {}
    for root in roots:
        type_id = module.tables["constants"][root][1]
        alignment = module.layouts[type_id][1]
        aligned = (len(image) + alignment - 1) // alignment * alignment
        image.extend(bytes(aligned - len(image)))
        offsets[root] = len(image)
        value = materialize_constant(module, root)
        image.extend(value if value else b"\0")
    require(len(image) <= 131_072, "constant image exhaustion")
    return bytes(image), offsets


def interpret(module: v2.Module, step_limit: int = 65_536, frame_limit: int = 16) -> int | None:
    if module.entry == NO_ID:
        return None
    t = module.tables
    types, records, fields = t["types"], t["records"], t["fields"]
    machines, blocks = t["machines"], t["blocks"]
    block_params, operations = t["block_params"], t["operations"]
    operands = [row[0] for row in t["operands"]]
    terminators = t["terminators"]
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

    def semantic_copy(type_id: int, destination: int, payload: bytes) -> None:
        for offset, size in scalar_leaves(type_id):
            require(offset + size <= len(payload), "runtime structural source extent")
            memory[destination + offset:destination + offset + size] = payload[offset:offset + size]

    def run_machine(machine_id: int, receiver: int, arguments: list[tuple[int, int | bytes]], depth: int) -> int | None:
        nonlocal steps
        require(depth <= frame_limit, "active machine-frame exhaustion")
        machine = machines[machine_id]
        machine_values: dict[int, tuple[int, int | bytes]] = {}
        require(len(arguments) == machine[7], "runtime call arity")
        for ordinal, value in enumerate(arguments):
            parameter = t["machine_params"][machine[6] + ordinal]
            machine_values[parameter[4]] = value
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
                    values[result_id] = (result_type, int.from_bytes(memory[address:address + size], "little"))
                elif opcode == 6:
                    place_type, address = places[args[0]]
                    value = int(values[args[1]][1])
                    require(types[place_type][6] <= value <= types[place_type][7], "runtime store range")
                    size = module.layouts[place_type][0]
                    memory[address:address + size] = value.to_bytes(size, "little")
                elif opcode == 7:
                    destination_type, destination = places[args[0]]
                    if op[10] == 2:
                        source_type, source = places[args[1]]
                        require(source_type == destination_type, "runtime copy type")
                        payload = bytes(memory[source:source + module.layouts[source_type][0]])
                    else:
                        source_type, payload = values[args[1]]
                        require(source_type == destination_type and isinstance(payload, bytes), "runtime structural value")
                    semantic_copy(destination_type, destination, payload)
                elif opcode == 8:
                    value = int(values[args[0]][1]) + int(values[args[1]][1])
                    require(types[result_type][6] <= value <= types[result_type][7], "runtime add range")
                    values[result_id] = (result_type, value)
                elif opcode in (9, 12):
                    left, right = int(values[args[0]][1]), int(values[args[1]][1])
                    values[result_id] = (result_type, int(left < right if opcode == 9 else left <= right))
                elif opcode == 10:
                    _, callee_receiver = places[args[0]]
                    result = run_machine(op[10], callee_receiver, [values[value] for value in args[1:]], depth + 1)
                    if op[4] == 1:
                        require(result is not None, "runtime missing call result")
                        values[result_id] = (result_type, result)
                elif opcode == 11:
                    destination_type, destination = places[args[0]]
                    require(destination_type == t["constants"][op[10]][1], "runtime constant root type")
                    semantic_copy(destination_type, destination, materialize_constant(module, op[10]))
                else:
                    raise Ckir3Error("runtime opcode")
            term = terminators[block_id]
            if term[3] in (1, 2):
                first = term[3] == 1 or bool(int(values[term[6]][1]))
                target, start, count = (term[7], term[8], term[9]) if first else (term[10], term[11], term[12])
                assigned: dict[int, tuple[int, int | bytes]] = {}
                for argument, parameter_id in zip(operands[start:start + count], span(blocks[target][5], count, len(block_params), "runtime edge")):
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
        print(f"CKIR3 valid: {len(module.tables['types'])} types, {len(module.tables['constants'])} constants, {len(roots)} roots, {len(image)} image bytes")
    else:
        result = interpret(module)
        print("library" if result is None else result)


if __name__ == "__main__":
    try:
        main()
    except (Ckir3Error, v2.CkirError, OSError, struct.error) as error:
        raise SystemExit(f"checked IR v3 reference: {error}")
