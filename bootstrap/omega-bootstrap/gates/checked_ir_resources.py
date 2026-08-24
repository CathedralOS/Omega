#!/usr/bin/env python3
"""Generate canonical CKIR1 resource fixtures without using the reference decoder."""

from __future__ import annotations

import argparse
import dataclasses
import os
import struct
from pathlib import Path


NO_ID = 0xFFFF_FFFF
HEADER = struct.Struct("<8sHHHH14I")
ROWS = (
    ("types", struct.Struct("<IBBHIIII")),
    ("records", struct.Struct("<IIIIBBBB")),
    ("fields", struct.Struct("<IIII")),
    ("machines", struct.Struct("<IIBBHIIIIII")),
    ("machine_params", struct.Struct("<IIIII")),
    ("blocks", struct.Struct("<IIBBHIIIII")),
    ("block_params", struct.Struct("<IIIII")),
    ("operations", struct.Struct("<IIIBBHIIIIII")),
    ("operands", struct.Struct("<I")),
    ("terminators", struct.Struct("<IIIBBHIIIIIII")),
)

TIGHT_COUNTS = {
    "encoded_bytes": 2_260_040,
    "types": 8_192,
    "records": 128,
    "fields": 8_192,
    "machines": 128,
    "machine_params": 896,
    "blocks": 2_048,
    "block_params": 4_096,
    "combined_params": 4_096,
    "operations": 32_768,
    "operands": 94_208,
    "values": 36_864,
    "places": 32_768,
}


class FixtureError(ValueError):
    """The generator constructed a noncanonical fixture."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise FixtureError(message)


def carrier_kind(types: list[tuple[int, ...]], type_id: int) -> int:
    require(0 <= type_id < len(types), "type ID out of range")
    return types[type_id][1]


@dataclasses.dataclass
class Module:
    entry: int = NO_ID
    types: list[tuple[int, ...]] = dataclasses.field(default_factory=list)
    records: list[tuple[int, ...]] = dataclasses.field(default_factory=list)
    fields: list[tuple[int, ...]] = dataclasses.field(default_factory=list)
    machines: list[tuple[int, ...]] = dataclasses.field(default_factory=list)
    machine_params: list[tuple[int, ...]] = dataclasses.field(default_factory=list)
    blocks: list[tuple[int, ...]] = dataclasses.field(default_factory=list)
    block_params: list[tuple[int, ...]] = dataclasses.field(default_factory=list)
    operations: list[tuple[int, ...]] = dataclasses.field(default_factory=list)
    operands: list[tuple[int, ...]] = dataclasses.field(default_factory=list)
    terminators: list[tuple[int, ...]] = dataclasses.field(default_factory=list)
    value_count: int = 0
    place_count: int = 0

    def tables(self) -> dict[str, list[tuple[int, ...]]]:
        return {name: getattr(self, name) for name, _ in ROWS}

    def counts(self) -> tuple[int, ...]:
        tables = self.tables()
        return tuple(len(tables[name]) for name, _ in ROWS) + (
            self.value_count,
            self.place_count,
        )

    def encoded_length(self) -> int:
        tables = self.tables()
        return HEADER.size + sum(len(tables[name]) * row.size for name, row in ROWS)

    def row_offset(self, table: str, index: int) -> int:
        cursor = HEADER.size
        tables = self.tables()
        for name, row in ROWS:
            if name == table:
                require(0 <= index < len(tables[name]), f"{table} row offset")
                return cursor + index * row.size
            cursor += len(tables[name]) * row.size
        raise FixtureError(f"unknown table {table}")

    def _layout(self, type_id: int, active: set[int], cache: dict[int, tuple[int, int]]) -> tuple[int, int]:
        if type_id in cache:
            return cache[type_id]
        require(type_id not in active, "recursive fixture layout")
        active.add(type_id)
        row = self.types[type_id]
        kind, payload0, payload1 = row[1], row[4], row[5]
        if kind in (1, 3):
            result = (1, 1)
        elif kind == 2:
            result = (4, 4)
        elif kind == 5:
            element_size, element_alignment = self._layout(payload0, active, cache)
            result = (element_size * payload1, element_alignment)
        else:
            record = self.records[payload0]
            cursor, alignment = 0, 1
            for field_id in range(record[2], record[2] + record[3]):
                field_size, field_alignment = self._layout(self.fields[field_id][3], active, cache)
                cursor += (-cursor) % field_alignment
                cursor += field_size
                alignment = max(alignment, field_alignment)
            cursor += (-cursor) % alignment
            result = (cursor, alignment)
        active.remove(type_id)
        cache[type_id] = result
        return result

    def layout(self, type_id: int) -> tuple[int, int]:
        return self._layout(type_id, set(), {})

    def validate(self) -> None:
        tables = self.tables()
        for name, row_struct in ROWS:
            for index, row in enumerate(tables[name]):
                try:
                    row_struct.pack(*row)
                except (struct.error, TypeError) as error:
                    raise FixtureError(f"unencodable {name} row {index}: {error}") from error
                if name != "operands":
                    require(row[0] == index, f"non-dense {name} ID")

        require(len(self.terminators) == len(self.blocks), "terminator/block count mismatch")
        require(self.entry == NO_ID or 0 <= self.entry < len(self.machines), "entry ID")

        seen_types: set[tuple[int, ...]] = set()
        nominal_types: dict[int, int] = {}
        for type_id, kind, flags, reserved, payload0, payload1, low, high in self.types:
            require(kind in (1, 2, 3, 4, 5), "type kind")
            require(flags in (0, 1) and reserved == 0, "type flags")
            require(kind not in (3, 4) or flags == 0, "type trapping flag")
            if kind in (1, 2, 3):
                require(payload0 == payload1 == 0 and low <= high, "scalar type")
                limit = 255 if kind == 1 else 1 if kind == 3 else 0x7FFF_FFFF
                require(high <= limit, "scalar range")
                if kind == 3:
                    require((low, high) == (0, 1), "bool range")
            elif kind == 4:
                require(payload0 < len(self.records) and payload1 == low == high == 0, "nominal type")
                require(payload0 not in nominal_types, "duplicate nominal type")
                nominal_types[payload0] = type_id
            else:
                require(payload0 < len(self.types) and low == high == 0, "array type")
            key = (kind, flags, payload0, payload1, low, high)
            require(key not in seen_types, "duplicate interned type")
            seen_types.add(key)

        next_field = 0
        for record_id, nominal, field_start, field_count, flags, r0, r1, r2 in self.records:
            require(flags in (0, 1) and r0 == r1 == r2 == 0, "record flags")
            require(nominal_types.get(record_id) == nominal, "record nominal relation")
            require(field_start == next_field and field_count <= 64, "field partition")
            require(field_count <= len(self.fields) - field_start, "field span")
            for ordinal, field_id in enumerate(range(field_start, field_start + field_count)):
                field = self.fields[field_id]
                require(field[:3] == (field_id, record_id, ordinal), "field owner/ordinal")
                require(field[3] < len(self.types), "field type")
            next_field += field_count
        require(next_field == len(self.fields), "field table partition")
        require(len(nominal_types) == len(self.records), "nominal coverage")

        next_machine_param = 0
        next_block = 0
        block_owner = [NO_ID] * len(self.blocks)
        for machine_id, owner, access, flags, reserved, result, param_start, param_count, block_start, block_count, entry_block in self.machines:
            require(owner < len(self.records) and access in (1, 2), "machine owner/access")
            require(flags == reserved == 0, "machine reserved fields")
            require(result == NO_ID or carrier_kind(self.types, result) in (1, 2, 3), "machine result")
            require(param_start == next_machine_param and param_count <= 7, "machine parameter partition")
            require(block_start == next_block and 1 <= block_count <= 128, "machine block partition")
            require(entry_block == block_start, "machine entry block")
            for ordinal, parameter_id in enumerate(range(param_start, param_start + param_count)):
                parameter = self.machine_params[parameter_id]
                require(parameter == (parameter_id, machine_id, ordinal, parameter[3], parameter_id), "machine parameter row")
                require(parameter[3] < len(self.types), "machine parameter type")
            for block_id in range(block_start, block_start + block_count):
                require(block_id < len(self.blocks), "machine block span")
                block_owner[block_id] = machine_id
            next_machine_param += param_count
            next_block += block_count
        require(next_machine_param == len(self.machine_params), "machine parameter coverage")
        require(next_block == len(self.blocks), "machine block coverage")

        next_block_param = 0
        next_operation = 0
        for block_id, owner, access, flags, reserved, param_start, param_count, op_start, op_count, terminator in self.blocks:
            require(owner == block_owner[block_id] and access in (1, 2), "block owner/access")
            require(access <= self.machines[owner][2], "block receiver widening")
            require(flags == reserved == 0 and terminator == block_id, "block reserved/terminator")
            require(param_start == next_block_param and param_count <= 7, "block parameter partition")
            require(op_start == next_operation, "operation partition")
            require(param_count <= len(self.block_params) - param_start, "block parameter span")
            require(op_count <= len(self.operations) - op_start, "operation span")
            if block_id == self.machines[owner][10]:
                require(access == self.machines[owner][2] and param_count == 0, "entry block signature")
            for ordinal, parameter_id in enumerate(range(param_start, param_start + param_count)):
                parameter = self.block_params[parameter_id]
                require(parameter == (parameter_id, block_id, ordinal, parameter[3], len(self.machine_params) + parameter_id), "block parameter row")
                require(parameter[3] < len(self.types), "block parameter type")
            next_block_param += param_count
            next_operation += op_count
        require(next_block_param == len(self.block_params), "block parameter coverage")
        require(next_operation == len(self.operations), "operation coverage")

        value_types = [row[3] for row in self.machine_params] + [row[3] for row in self.block_params]
        value_machines = [row[1] for row in self.machine_params] + [self.blocks[row[1]][1] for row in self.block_params]
        value_blocks: list[int | None] = [None] * len(self.machine_params) + [row[1] for row in self.block_params]
        value_operations: list[int | None] = [None] * len(value_types)
        place_types: list[int] = []
        place_blocks: list[int] = []
        place_operations: list[int] = []

        def value_visible(value_id: int, machine: int, block: int, operation: int) -> bool:
            return (
                0 <= value_id < len(value_types)
                and value_machines[value_id] == machine
                and (
                    value_blocks[value_id] is None
                    or value_blocks[value_id] == block
                    and (value_operations[value_id] is None or value_operations[value_id] < operation)
                )
            )

        def place_visible(place_id: int, block: int, operation: int) -> bool:
            return (
                0 <= place_id < len(place_types)
                and place_blocks[place_id] == block
                and place_operations[place_id] < operation
            )

        def exact_or_carrier_compatible(left: int, right: int) -> bool:
            left_kind = carrier_kind(self.types, left)
            right_kind = carrier_kind(self.types, right)
            if left_kind in (1, 2, 3) or right_kind in (1, 2, 3):
                return left_kind == right_kind
            return left == right

        def copyable(type_id: int, active: set[int]) -> bool:
            if type_id in active:
                return False
            kind = carrier_kind(self.types, type_id)
            if kind in (1, 2, 3):
                return True
            if kind == 5:
                return copyable(self.types[type_id][4], active | {type_id})
            record_id = self.types[type_id][4]
            record = self.records[record_id]
            if record[4] != 1:
                return False
            return all(
                copyable(self.fields[field_id][3], active | {type_id})
                for field_id in range(record[2], record[2] + record[3])
            )

        next_operand = 0
        for operation in self.operations:
            op_id, owner, block, opcode, result_kind, flags, result_id, result_type, operand_start, operand_count, imm0, imm1 = operation
            require(block < len(self.blocks) and owner == self.blocks[block][1], "operation owner")
            require(op_id in range(self.blocks[block][7], self.blocks[block][7] + self.blocks[block][8]), "operation block")
            require(flags == 0 and operand_start == next_operand, "operation flags/operand partition")
            require(operand_count <= len(self.operands) - operand_start, "operation operand span")
            arguments = [self.operands[index][0] for index in range(operand_start, operand_start + operand_count)]
            next_operand += operand_count
            if opcode == 1:
                require(result_kind == 1 and operand_count == 0 and imm1 == 0, "Const shape")
                require(result_id == len(value_types) and carrier_kind(self.types, result_type) in (1, 2, 3), "Const result")
                require(self.types[result_type][6] <= imm0 <= self.types[result_type][7], "Const range")
            elif opcode == 2:
                require(result_kind == 2 and operand_count == 0 and imm0 == imm1 == 0, "SelfPlace shape")
                owner_type = self.records[self.machines[owner][1]][1]
                require(result_id == len(place_types) and result_type == owner_type, "SelfPlace result")
            elif opcode in (8, 9):
                require(result_kind == 1 and operand_count == 2 and imm0 == imm1 == 0, "binary shape")
                require(all(value_visible(value, owner, block, op_id) for value in arguments), "binary visibility")
                left_kind = carrier_kind(self.types, value_types[arguments[0]])
                right_kind = carrier_kind(self.types, value_types[arguments[1]])
                require(left_kind == right_kind and left_kind in (1, 2), "binary operand type")
                expected_kind = left_kind if opcode == 8 else 3
                require(result_id == len(value_types) and carrier_kind(self.types, result_type) == expected_kind, "binary result")
            elif opcode == 7:
                require(
                    result_kind == 0 and result_id == result_type == NO_ID
                    and operand_count == 2 and imm0 in (1, 2) and imm1 == 0,
                    "Copy shape",
                )
                destination, source = arguments
                require(place_visible(destination, block, op_id), "Copy destination visibility")
                destination_type = place_types[destination]
                require(copyable(destination_type, set()), "Copy destination type")
                if imm0 == 1:
                    require(value_visible(source, owner, block, op_id), "Copy value visibility")
                    require(value_types[source] == destination_type, "Copy value type")
                else:
                    require(place_visible(source, block, op_id), "Copy place visibility")
                    require(place_types[source] == destination_type, "Copy place type")
            else:
                raise FixtureError(f"fixture generator does not emit opcode {opcode}")

            if result_kind == 1:
                value_types.append(result_type)
                value_machines.append(owner)
                value_blocks.append(block)
                value_operations.append(op_id)
            elif result_kind == 2:
                place_types.append(result_type)
                place_blocks.append(block)
                place_operations.append(op_id)
            elif result_kind != 0:
                raise FixtureError("fixture operation unexpectedly has no result")

        require(len(value_types) == self.value_count, "value count reconstruction")
        require(len(place_types) == self.place_count, "place count reconstruction")

        for term_id, owner, block, kind, flags, reserved, value, target0, start0, count0, target1, start1, count1 in self.terminators:
            require(block == term_id and owner == self.blocks[block][1], "terminator owner")
            require(flags == reserved == 0 and kind in (1, 2, 3, 4), "terminator kind/flags")
            require(start0 == next_operand and count0 <= len(self.operands) - start0, "target-0 partition")
            next_operand += count0
            require(start1 == next_operand and count1 <= len(self.operands) - start1, "target-1 partition")
            next_operand += count1
            block_end = self.blocks[block][7] + self.blocks[block][8]
            if kind in (1, 2):
                if kind == 1:
                    require(value == target1 == NO_ID and count1 == 0, "Jump shape")
                    edges = ((target0, start0, count0),)
                else:
                    require(value_visible(value, owner, block, block_end), "branch value visibility")
                    require(carrier_kind(self.types, value_types[value]) == 3, "branch value type")
                    edges = ((target0, start0, count0), (target1, start1, count1))
                for target, start, count in edges:
                    require(target < len(self.blocks) and self.blocks[target][1] == owner, "branch target owner")
                    require(target != self.machines[owner][10], "branch targets entry")
                    require(count == self.blocks[target][6], "edge arity")
                    for ordinal in range(count):
                        argument = self.operands[start + ordinal][0]
                        require(value_visible(argument, owner, block, block_end), "edge value visibility")
                        parameter = self.block_params[self.blocks[target][5] + ordinal]
                        require(exact_or_carrier_compatible(value_types[argument], parameter[3]), "edge type")
            elif kind == 3:
                require(value == target0 == target1 == NO_ID and count0 == count1 == 0, "ReturnUnit shape")
                require(self.machines[owner][5] == NO_ID, "ReturnUnit machine result")
            else:
                require(target0 == target1 == NO_ID and count0 == count1 == 0, "ReturnValue targets")
                require(value_visible(value, owner, block, block_end), "ReturnValue visibility")
                result_type = self.machines[owner][5]
                require(result_type != NO_ID, "ReturnValue machine result")
                require(carrier_kind(self.types, value_types[value]) == carrier_kind(self.types, result_type), "ReturnValue carrier")
        require(next_operand == len(self.operands), "operand table partition")

        candidates = [
            machine_id
            for machine_id, machine in enumerate(self.machines)
            if machine[7] == 0
            and machine[5] != NO_ID
            and carrier_kind(self.types, machine[5]) in (1, 2, 3)
        ]
        require(len(candidates) <= 1, "ambiguous fixture root")
        require(self.entry == (candidates[0] if candidates else NO_ID), "fixture root/header relation")

    def encode(self) -> bytes:
        self.validate()
        tables = self.tables()
        payload_parts: list[bytes] = []
        for name, row_struct in ROWS:
            payload_parts.extend(row_struct.pack(*row) for row in tables[name])
        payload = b"".join(payload_parts)
        total = HEADER.size + len(payload)
        require(total == self.encoded_length(), "encoded length accounting")
        flags = 0 if self.entry == NO_ID else 1
        header = HEADER.pack(
            b"OMGCKIR\0", 1, 0, 1, flags,
            self.entry, total, *self.counts(),
        )
        result = header + payload
        require(len(result) == total, "encoded byte count")
        return result


def scalar_types_with_nominals(record_count: int, total_count: int, include_bool: bool) -> list[tuple[int, ...]]:
    minimum = 1 + int(include_bool) + record_count
    require(total_count >= minimum, "type inventory too small")
    types: list[tuple[int, ...]] = [(0, 1, 0, 0, 0, 0, 0, 255)]
    if include_bool:
        types.append((1, 3, 0, 0, 0, 0, 0, 1))
    extra_count = total_count - record_count - len(types)
    for index in range(extra_count):
        type_id = len(types)
        types.append((type_id, 2, 0, 0, 0, 0, index, index))
    for record_id in range(record_count):
        type_id = len(types)
        types.append((type_id, 4, 0, 0, record_id, 0, 0, 0))
    require(len(types) == total_count, "type inventory count")
    return types


def build_maximal() -> Module:
    module = Module()
    module.types = scalar_types_with_nominals(128, 8_192, include_bool=True)
    nominal_start = 8_192 - 128
    module.records = [(record, nominal_start + record, record * 64, 64, 0, 0, 0, 0) for record in range(128)]
    module.fields = [(field, field // 64, field % 64, 0) for field in range(8_192)]

    for machine in range(128):
        module.machines.append((machine, machine, 1, 0, 0, NO_ID, machine * 7, 7, machine * 16, 16, machine * 16))
        for ordinal in range(7):
            parameter_id = machine * 7 + ordinal
            type_id = 1 if ordinal == 6 else 0
            module.machine_params.append((parameter_id, machine, ordinal, type_id, parameter_id))

    block_param_cursor = 0
    operation_cursor = 0
    for block in range(2_048):
        local = block % 16
        param_count = 7 if local in (1, 2, 3) else 4 if local == 4 else 0
        module.blocks.append((block, block // 16, 1, 0, 0, block_param_cursor, param_count, operation_cursor, 16, block))
        for ordinal in range(param_count):
            parameter_id = block_param_cursor + ordinal
            type_id = 1 if ordinal == 6 else 0
            module.block_params.append((parameter_id, block, ordinal, type_id, 896 + parameter_id))
        block_param_cursor += param_count
        operation_cursor += 16

    for operation in range(32_768):
        block = operation // 16
        machine = block // 16
        operand_start = operation * 2
        module.operations.append((operation, machine, block, 8, 1, 0, 4_096 + operation, 0, operand_start, 2, 0, 0))
        module.operands.extend(((machine * 7,), (machine * 7 + 1,)))

    term_operand_start = len(module.operands)
    for block in range(2_048):
        machine = block // 16
        target = machine * 16 + 1
        start0 = term_operand_start + block * 14
        start1 = start0 + 7
        arguments = tuple((machine * 7 + ordinal,) for ordinal in range(7))
        module.operands.extend(arguments)
        module.operands.extend(arguments)
        module.terminators.append((block, machine, block, 2, 0, 0, machine * 7 + 6, target, start0, 7, target, start1, 7))

    module.value_count = 36_864
    module.place_count = 0
    encoded = module.encode()
    expected = {
        "types": 8_192, "records": 128, "fields": 8_192,
        "machines": 128, "machine_params": 896, "blocks": 2_048,
        "block_params": 3_200, "operations": 32_768,
        "operands": 94_208, "terminators": 2_048,
    }
    tables = module.tables()
    require(all(len(tables[name]) == count for name, count in expected.items()), "maximal table counts")
    require(len(module.machine_params) + len(module.block_params) == 4_096, "maximal combined parameters")
    require(module.value_count == 36_864 and module.place_count == 0, "maximal result counts")
    require(len(encoded) == 2_260_040, "maximal encoded length")
    return module


def build_block_params_exact() -> Module:
    module = Module()
    module.types = scalar_types_with_nominals(1, 2, include_bool=False)
    module.records = [(0, 1, 0, 0, 0, 0, 0, 0)]
    block_param_cursor = 0
    for machine in range(5):
        block_start = machine * 128
        module.machines.append((machine, 0, 1, 0, 0, NO_ID, 0, 0, block_start, 128, block_start))
        for local in range(128):
            block = block_start + local
            param_count = 0 if local == 0 else min(7, 4_096 - block_param_cursor)
            module.blocks.append((block, machine, 1, 0, 0, block_param_cursor, param_count, 0, 0, block))
            for ordinal in range(param_count):
                parameter_id = block_param_cursor + ordinal
                module.block_params.append((parameter_id, block, ordinal, 0, parameter_id))
            block_param_cursor += param_count
            module.terminators.append((block, machine, block, 3, 0, 0, NO_ID, NO_ID, 0, 0, NO_ID, 0, 0))
    module.value_count = 4_096
    require(block_param_cursor == 4_096, "block parameter exact count")
    return module


def build_places_exact() -> Module:
    module = Module()
    module.types = scalar_types_with_nominals(1, 2, include_bool=False)
    module.records = [(0, 1, 0, 0, 0, 0, 0, 0)]
    module.machines = [(0, 0, 1, 0, 0, NO_ID, 0, 0, 0, 1, 0)]
    module.blocks = [(0, 0, 1, 0, 0, 0, 0, 0, 32_768, 0)]
    module.operations = [(operation, 0, 0, 2, 2, 0, operation, 1, 0, 0, 0, 0) for operation in range(32_768)]
    module.terminators = [(0, 0, 0, 3, 0, 0, NO_ID, NO_ID, 0, 0, NO_ID, 0, 0)]
    module.place_count = 32_768
    return module


def build_frame(place_count: int) -> Module:
    require(place_count in (32_766, 32_767), "frame fixture place count")
    module = Module(entry=0)
    module.types = scalar_types_with_nominals(1, 2, include_bool=False)
    module.records = [(0, 1, 0, 0, 0, 0, 0, 0)]
    module.machines = [(0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0)]
    module.blocks = [(0, 0, 1, 0, 0, 0, 0, 0, place_count + 1, 0)]
    module.operations.append((0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 70, 0))
    for place in range(place_count):
        operation = place + 1
        module.operations.append((operation, 0, 0, 2, 2, 0, place, 1, 0, 0, 0, 0))
    module.terminators = [(0, 0, 0, 4, 0, 0, 0, NO_ID, 0, 0, NO_ID, 0, 0)]
    module.value_count = 1
    module.place_count = place_count
    frame_before_final_alignment = 16 + place_count * 8
    frame = frame_before_final_alignment + (-frame_before_final_alignment) % 16
    expected = 262_144 if place_count == 32_766 else 262_160
    require(frame == expected, "frame byte accounting")
    return module


def build_text(extra_const: bool) -> Module:
    module = Module(entry=0)
    module.types = scalar_types_with_nominals(1, 3, include_bool=True)
    module.records = [(0, 2, 0, 0, 0, 0, 0, 0)]
    module.machines = [(0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0)]
    operation_count = 32_762 + int(extra_const)
    module.blocks = [(0, 0, 1, 0, 0, 0, 0, 0, operation_count, 0)]

    const_count = 3 if extra_const else 2
    for operation in range(const_count):
        module.operations.append((operation, 0, 0, 1, 1, 0, operation, 0, 0, 0, operation + 1, 0))

    operand_cursor = 0
    add_count_before_return = 11_919
    for _ in range(add_count_before_return):
        operation = len(module.operations)
        module.operations.append((operation, 0, 0, 8, 1, 0, operation, 0, operand_cursor, 2, 0, 0))
        module.operands.extend(((0,), (1,)))
        operand_cursor += 2
    for _ in range(20_840):
        operation = len(module.operations)
        module.operations.append((operation, 0, 0, 9, 1, 0, operation, 1, operand_cursor, 2, 0, 0))
        module.operands.extend(((0,), (1,)))
        operand_cursor += 2
    final_add = len(module.operations)
    module.operations.append((final_add, 0, 0, 8, 1, 0, final_add, 0, operand_cursor, 2, 0, 0))
    module.operands.extend(((0,), (1,)))
    operand_cursor += 2
    module.terminators = [(0, 0, 0, 4, 0, 0, final_add, NO_ID, operand_cursor, 0, NO_ID, operand_cursor, 0)]
    module.value_count = len(module.operations)

    # CKIR1 section 7.1 templates: shim 26, entry prologue 18, Const 11,
    # Add 46, Less 24, and scalar ReturnValue 30 bytes.
    text_bytes = 26 + 18 + const_count * 11 + 11_920 * 46 + 20_840 * 24 + 30
    expected = 1_048_587 if extra_const else 1_048_576
    require(text_bytes == expected, "text byte accounting")
    require(len(module.operations) == operation_count and operand_cursor == 65_520, "text operation accounting")
    return module


def build_layout_over() -> Module:
    module = Module(entry=0)
    module.types = [
        (0, 1, 0, 0, 0, 0, 0, 255),
        (1, 5, 1, 0, 0, 65_536, 0, 0),
        (2, 4, 0, 0, 0, 0, 0, 0),
    ]
    module.records = [(0, 2, 0, 3, 0, 0, 0, 0)]
    module.fields = [(0, 0, 0, 1), (1, 0, 1, 1), (2, 0, 2, 0)]
    module.machines = [(0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0)]
    module.blocks = [(0, 0, 1, 0, 0, 0, 0, 0, 1, 0)]
    module.operations = [(0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0)]
    module.terminators = [(0, 0, 0, 4, 0, 0, 0, NO_ID, 0, 0, NO_ID, 0, 0)]
    module.value_count = 1
    require(module.layout(2)[0] == 131_073, "layout-over owner size")
    return module


def build_array_over() -> Module:
    module = Module()
    module.types = [
        (0, 1, 0, 0, 0, 0, 0, 255),
        (1, 5, 1, 0, 0, 65_537, 0, 0),
    ]
    return module


def build_structural_jump_control() -> Module:
    """Exercise valid Jump/ReturnUnit and structural value transfer/Copy."""
    module = Module()
    module.types = [
        (0, 1, 0, 0, 0, 0, 0, 255),
        (1, 4, 0, 0, 0, 0, 0, 0),
    ]
    module.records = [(0, 1, 0, 1, 1, 0, 0, 0)]
    module.fields = [(0, 0, 0, 0)]
    module.machines = [(0, 0, 2, 0, 0, NO_ID, 0, 1, 0, 2, 0)]
    module.machine_params = [(0, 0, 0, 1, 0)]
    module.blocks = [
        (0, 0, 2, 0, 0, 0, 0, 0, 0, 0),
        (1, 0, 2, 0, 0, 0, 1, 0, 2, 1),
    ]
    module.block_params = [(0, 1, 0, 1, 1)]
    module.operations = [
        (0, 0, 1, 2, 2, 0, 0, 1, 0, 0, 0, 0),
        (1, 0, 1, 7, 0, 0, NO_ID, NO_ID, 0, 2, 1, 0),
    ]
    module.operands = [(0,), (0,), (0,)]
    module.terminators = [
        (0, 0, 0, 1, 0, 0, NO_ID, 1, 2, 1, NO_ID, 3, 0),
        (1, 0, 1, 3, 0, 0, NO_ID, NO_ID, 3, 0, NO_ID, 3, 0),
    ]
    module.value_count = 2
    module.place_count = 1
    return module


def write_atomic(path: Path, contents: bytes) -> None:
    temporary = path.with_name(f".{path.name}.tmp-{os.getpid()}")
    try:
        with temporary.open("xb") as output:
            output.write(contents)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
    finally:
        if temporary.exists():
            temporary.unlink()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("OUT_DIR", type=Path)
    arguments = parser.parse_args()

    rows = [
        ("maximal.ckir", 0, True, "empty", False, "all tight table caps except standalone block-parameter/place caps"),
        ("wire-over.ckir", 252, False, "empty", False, "one byte beyond the 2260040-byte wire ceiling"),
        ("block-params-exact.ckir", 0, True, "empty", False, "4096 block parameters and combined parameters"),
        ("places-exact.ckir", 0, True, "empty", False, "32768 place results in a library module"),
        ("frame-exact.ckir", 0, True, "elf", False, "selected-machine frame is exactly 262144 bytes"),
        ("frame-over.ckir", 252, True, "empty", True, "structurally valid selected-machine frame is 262160 bytes"),
        ("text-exact.ckir", 0, True, "elf", False, "selected-machine text is exactly 1048576 bytes"),
        ("text-over.ckir", 252, True, "empty", True, "one additional Const makes text 1048587 bytes"),
        ("layout-over.ckir", 252, False, "empty", True, "selected owner layout is 131073 bytes"),
        ("array-over.ckir", 252, False, "empty", True, "fixed-array length is 65537"),
        ("structural-jump-control.ckir", 0, True, "empty", True, "valid Jump/ReturnUnit, structural edge argument, and Copy-from-value"),
        ("structural-shared-copy.ckir", 251, False, "empty", True, "Copy-from-value rejects in a non-entry shared block"),
    ]
    modules = {
        "maximal.ckir": build_maximal(),
        "block-params-exact.ckir": build_block_params_exact(),
        "places-exact.ckir": build_places_exact(),
        "frame-exact.ckir": build_frame(32_766),
        "frame-over.ckir": build_frame(32_767),
        "text-exact.ckir": build_text(False),
        "text-over.ckir": build_text(True),
        "layout-over.ckir": build_layout_over(),
        "array-over.ckir": build_array_over(),
        "structural-jump-control.ckir": build_structural_jump_control(),
    }
    special = {"wire-over.ckir", "structural-shared-copy.ckir"}
    require(set(modules) | special == {row[0] for row in rows}, "manifest/module inventory")
    encoded = {name: module.encode() for name, module in modules.items()}
    encoded["wire-over.ckir"] = encoded["maximal.ckir"] + b"\0"
    require(len(encoded["wire-over.ckir"]) == 2_260_041, "wire-over byte count")
    shared_copy = bytearray(encoded["structural-jump-control.ckir"])
    shared_copy[modules["structural-jump-control.ckir"].row_offset("blocks", 1) + 8] = 1
    encoded["structural-shared-copy.ckir"] = bytes(shared_copy)

    out = arguments.OUT_DIR
    if out.exists():
        require(out.is_dir(), "OUT_DIR exists and is not a directory")
    else:
        out.mkdir(parents=True)
    for name, _, _, _, _, _ in rows:
        write_atomic(out / name, encoded[name])
    manifest = "name\texpected_backend_status\texpected_reference_valid\texpected_output\tself_representative\tnote\n" + "".join(
        f"{name}\t{status}\t{'true' if reference_valid else 'false'}\t{expected_output}\t{'true' if self_representative else 'false'}\t{note}\n"
        for name, status, reference_valid, expected_output, self_representative, note in rows
    )
    write_atomic(out / "manifest.tsv", manifest.encode("utf-8"))


if __name__ == "__main__":
    try:
        main()
    except (FixtureError, OSError, struct.error) as error:
        raise SystemExit(f"checked IR resources: {error}")
