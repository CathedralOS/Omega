#!/usr/bin/env python3
"""Generate independently checked canonical CKIR3 resource fixtures.

This is untrusted gate plumbing.  It constructs semantic table rows directly,
checks their canonical partitions, layouts, constant DAG, operation roots, and
published counts, and only then serializes CKIR3 bytes.  CKIR1 fixtures remain
frozen and are neither imported nor modified.
"""

from __future__ import annotations

import argparse
import dataclasses
import os
import struct
from pathlib import Path


NO_ID = 0xFFFF_FFFF
HEADER = struct.Struct("<8sHHHH16I")
ROWS = (
    ("types", struct.Struct("<IBBHIIII")),
    ("records", struct.Struct("<IIIIBBBB")),
    ("fields", struct.Struct("<IIII")),
    ("machines", struct.Struct("<IIBBHIIIIII")),
    ("machine_params", struct.Struct("<IIIII")),
    ("blocks", struct.Struct("<IIBBHIIIII")),
    ("block_params", struct.Struct("<IIIII")),
    ("constants", struct.Struct("<IIIIII")),
    ("constant_children", struct.Struct("<I")),
    ("operations", struct.Struct("<IIIBBHIIIIII")),
    ("operands", struct.Struct("<I")),
    ("terminators", struct.Struct("<IIIBBHIIIIIII")),
)
CAPS = {
    "types": 8_192, "records": 128, "fields": 8_192,
    "machines": 128, "machine_params": 896, "blocks": 2_048,
    "block_params": 4_096, "operations": 32_768,
    "operands": 94_208, "constants": 8_192,
    "constant_children": 16_384,
}


class FixtureError(ValueError):
    """The generator attempted to publish a noncanonical CKIR3 module."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise FixtureError(message)


def align(value: int, amount: int) -> int:
    return value + (-value) % amount


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
    constants: list[tuple[int, ...]] = dataclasses.field(default_factory=list)
    constant_children: list[tuple[int, ...]] = dataclasses.field(default_factory=list)
    operations: list[tuple[int, ...]] = dataclasses.field(default_factory=list)
    operands: list[tuple[int, ...]] = dataclasses.field(default_factory=list)
    terminators: list[tuple[int, ...]] = dataclasses.field(default_factory=list)
    value_count: int = 0
    place_count: int = 0

    def tables(self) -> dict[str, list[tuple[int, ...]]]:
        return {name: getattr(self, name) for name, _ in ROWS}

    def counts(self) -> tuple[int, ...]:
        return (
            len(self.types), len(self.records), len(self.fields),
            len(self.machines), len(self.machine_params), len(self.blocks),
            len(self.block_params), len(self.operations), len(self.operands),
            len(self.terminators), self.value_count, self.place_count,
            len(self.constants), len(self.constant_children),
        )

    def encoded_length(self) -> int:
        tables = self.tables()
        return HEADER.size + sum(len(tables[name]) * row.size for name, row in ROWS)

    def _layout(self, type_id: int, active: set[int], cache: dict[int, tuple[int, int]]) -> tuple[int, int]:
        if type_id in cache:
            return cache[type_id]
        require(type_id not in active, "recursive type layout")
        active.add(type_id)
        row = self.types[type_id]
        kind, payload0, payload1 = row[1], row[4], row[5]
        if kind in (1, 3):
            result = (1, 1)
        elif kind == 2:
            result = (4, 4)
        elif kind == 5:
            size, alignment = self._layout(payload0, active, cache)
            result = (size * payload1, alignment)
        else:
            record = self.records[payload0]
            cursor, alignment = 0, 1
            for field_id in range(record[2], record[2] + record[3]):
                size, field_alignment = self._layout(self.fields[field_id][3], active, cache)
                cursor = align(cursor, field_alignment) + size
                alignment = max(alignment, field_alignment)
            result = (align(cursor, alignment), alignment)
        active.remove(type_id)
        cache[type_id] = result
        return result

    def layout(self, type_id: int) -> tuple[int, int]:
        return self._layout(type_id, set(), {})

    def constant_facts(self) -> tuple[list[int], set[int], int]:
        heights: list[int] = []
        keys: list[tuple[int, ...]] = []
        roots = {row[10] for row in self.operations if row[3] == 11}
        child_cursor = 0
        for index, row in enumerate(self.constants):
            node_id, type_id, start, count, scalar, reserved = row
            require(node_id == index and 0 <= type_id < len(self.types) and reserved == 0,
                    "constant identity/type/reserved")
            require(start == child_cursor and count <= len(self.constant_children) - start,
                    "constant child partition")
            children = [item[0] for item in self.constant_children[start:start + count]]
            kind = self.types[type_id][1]
            if kind in (1, 2, 3):
                require(count == 0 and self.types[type_id][6] <= scalar <= self.types[type_id][7],
                        "scalar constant shape/range")
                height = 0
                key = (height, type_id, scalar)
            else:
                require(scalar == 0 and all(0 <= child < index for child in children),
                        "structural constant scalar/edge")
                if kind == 4:
                    record = self.records[self.types[type_id][4]]
                    wanted = [self.fields[field][3] for field in range(record[2], record[2] + record[3])]
                    require(count <= 4, "record constant child cap")
                else:
                    wanted = [self.types[type_id][4]] * self.types[type_id][5]
                    require(count <= 1_024, "array constant child cap")
                require(len(children) == len(wanted), "structural constant arity")
                require(all(self.constants[child][1] == expected for child, expected in zip(children, wanted)),
                        "structural constant child type")
                height = 1 + max((heights[child] for child in children), default=-1)
                key = (height, type_id, count, *children)
            require(not keys or keys[-1] < key, "constant canonical ordering/duplicate")
            heights.append(height)
            keys.append(key)
            child_cursor += count
        require(child_cursor == len(self.constant_children), "unused constant children")
        require((not self.constants) == (not roots), "constant/root presence")
        require(all(root < len(self.constants) and self.types[self.constants[root][1]][1] in (4, 5)
                    for root in roots), "structural constant roots")
        reachable: set[int] = set()
        pending = list(roots)
        while pending:
            node = pending.pop()
            if node in reachable:
                continue
            reachable.add(node)
            start, count = self.constants[node][2:4]
            pending.extend(item[0] for item in self.constant_children[start:start + count])
        require(len(reachable) == len(self.constants), "unreachable constant node")
        image = 0
        for root in sorted(roots):
            type_id = self.constants[root][1]
            size, alignment = self.layout(type_id)
            image = align(image, alignment) + max(size, 1)
        return heights, roots, image

    def validate(self, *, allow: frozenset[str] = frozenset()) -> None:
        tables = self.tables()
        for name, row_type in ROWS:
            for index, row in enumerate(tables[name]):
                try:
                    row_type.pack(*row)
                except (struct.error, TypeError) as error:
                    raise FixtureError(f"unencodable {name} row {index}: {error}") from error
                if name not in ("operands", "constant_children"):
                    require(row[0] == index, f"non-dense {name} ID")
            if name in CAPS and name not in allow:
                require(len(tables[name]) <= CAPS[name], f"{name} resource cap")
        require(len(self.machine_params) + len(self.block_params) <= 4_096,
                "combined parameter cap")
        require(len(self.terminators) == len(self.blocks), "terminator/block relation")
        require(self.value_count <= 36_864 and self.place_count <= 32_768,
                "value/place cap")

        seen_types: set[tuple[int, ...]] = set()
        nominal: dict[int, int] = {}
        for type_id, kind, flags, reserved, payload0, payload1, low, high in self.types:
            require(kind in (1, 2, 3, 4, 5) and flags in (0, 1) and reserved == 0,
                    "type kind/flags/reserved")
            if kind in (1, 2, 3):
                require(payload0 == payload1 == 0 and low <= high, "scalar type shape")
                limit = 255 if kind == 1 else 1 if kind == 3 else 0x7FFF_FFFF
                require(high <= limit and (kind != 3 or (low, high) == (0, 1)), "scalar type range")
                require(flags == 0, "scalar flags")
            elif kind == 4:
                require(flags == 0 and payload0 < len(self.records) and payload1 == low == high == 0,
                        "nominal type shape")
                require(payload0 not in nominal, "duplicate nominal record")
                nominal[payload0] = type_id
            else:
                require(payload0 < len(self.types) and 0 <= payload1 <= 65_536 and low == high == 0,
                        "array type shape")
            key = (kind, flags, payload0, payload1, low, high)
            require(key not in seen_types, "duplicate interned type")
            seen_types.add(key)

        field_cursor = 0
        for record_id, nominal_type, start, count, flags, r0, r1, r2 in self.records:
            require(nominal.get(record_id) == nominal_type and flags in (0, 1) and r0 == r1 == r2 == 0,
                    "record nominal/flags")
            require(start == field_cursor and count <= 64 and count <= len(self.fields) - start,
                    "record field partition")
            for ordinal, field_id in enumerate(range(start, start + count)):
                require(self.fields[field_id][:3] == (field_id, record_id, ordinal), "field owner/ordinal")
                require(self.fields[field_id][3] < len(self.types), "field type")
            field_cursor += count
        require(field_cursor == len(self.fields) and len(nominal) == len(self.records),
                "field/nominal coverage")
        for type_id in range(len(self.types)):
            self.layout(type_id)

        machine_param_cursor = block_cursor = 0
        block_owner = [NO_ID] * len(self.blocks)
        for machine in self.machines:
            mid, owner, access, flags, reserved, result, pstart, pcount, bstart, bcount, entry = machine
            require(owner < len(self.records) and access in (1, 2) and flags == reserved == 0,
                    "machine owner/access/reserved")
            require(result == NO_ID or result < len(self.types) and self.types[result][1] in (1, 2, 3),
                    "machine scalar result")
            require(pstart == machine_param_cursor and pcount <= 7, "machine parameter partition")
            require(bstart == block_cursor and 1 <= bcount <= 128 and entry == bstart,
                    "machine block partition")
            for ordinal, pid in enumerate(range(pstart, pstart + pcount)):
                require(self.machine_params[pid][:3] == (pid, mid, ordinal), "machine parameter identity")
                require(self.machine_params[pid][3] < len(self.types), "machine parameter type")
            for block in range(bstart, bstart + bcount):
                require(block < len(self.blocks), "machine block extent")
                block_owner[block] = mid
            machine_param_cursor += pcount
            block_cursor += bcount
        require(machine_param_cursor == len(self.machine_params) and block_cursor == len(self.blocks),
                "machine partition coverage")

        block_param_cursor = operation_cursor = 0
        for block in self.blocks:
            bid, owner, access, flags, reserved, pstart, pcount, ostart, ocount, term = block
            require(owner == block_owner[bid] and access in (1, 2) and access <= self.machines[owner][2],
                    "block owner/access")
            require(flags == reserved == 0 and term == bid, "block reserved/terminator")
            require(pstart == block_param_cursor and pcount <= 7, "block parameter partition")
            require(ostart == operation_cursor and ocount <= len(self.operations) - ostart,
                    "operation partition")
            require(bid != self.machines[owner][10] or pcount == 0 and access == self.machines[owner][2],
                    "entry block signature")
            for ordinal, pid in enumerate(range(pstart, pstart + pcount)):
                require(self.block_params[pid][:3] == (pid, bid, ordinal), "block parameter identity")
                require(self.block_params[pid][3] < len(self.types), "block parameter type")
            block_param_cursor += pcount
            operation_cursor += ocount
        require(block_param_cursor == len(self.block_params) and operation_cursor == len(self.operations),
                "block partition coverage")

        next_value = len(self.machine_params) + len(self.block_params)
        next_place = operand_cursor = 0
        value_types = [row[3] for row in self.machine_params] + [row[3] for row in self.block_params]
        place_types: list[int] = []
        place_mutable: list[bool] = []
        place_block: list[int] = []
        roots: list[int] = []
        call_edges: set[tuple[int, int]] = set()
        for op in self.operations:
            oid, machine, block, opcode, result_kind, flags, result_id, result_type, start, count, imm0, imm1 = op
            require(machine == self.blocks[block][1] and self.blocks[block][7] <= oid < self.blocks[block][7] + self.blocks[block][8],
                    "operation owner")
            require(flags == 0 and start == operand_cursor and count <= len(self.operands) - start,
                    "operation flags/operand partition")
            args = [item[0] for item in self.operands[start:start + count]]
            if result_kind == 0:
                require(result_id == result_type == NO_ID, "no-result operation shape")
            elif result_kind == 1:
                require(result_id == next_value and result_type < len(self.types), "value result order/type")
                value_types.append(result_type)
                next_value += 1
            elif result_kind == 2:
                require(result_id == next_place and result_type < len(self.types), "place result order/type")
                next_place += 1
            else:
                raise FixtureError("operation result kind")
            if opcode == 1:
                require(result_kind == 1 and count == 0 and imm1 == 0 and self.types[result_type][1] <= 3,
                        "Const shape")
                require(self.types[result_type][6] <= imm0 <= self.types[result_type][7], "Const range")
            elif opcode == 2:
                owner_type = self.records[self.machines[machine][1]][1]
                require(result_kind == 2 and result_type == owner_type and count == 0 and imm0 == imm1 == 0,
                        "SelfPlace shape")
                place_types.append(result_type); place_mutable.append(self.blocks[block][2] == 2); place_block.append(block)
            elif opcode == 3:
                require(result_kind == 2 and count == 1 and args[0] < len(place_types) and imm0 < len(self.fields) and imm1 == 0,
                        "FieldPlace shape")
                field = self.fields[imm0]
                require(place_types[args[0]] == self.records[field[1]][1] and result_type == field[3],
                        "FieldPlace type")
                place_types.append(result_type); place_mutable.append(place_mutable[args[0]]); place_block.append(block)
            elif opcode in (8, 9):
                require(result_kind == 1 and count == 2 and all(arg < len(value_types) for arg in args)
                        and imm0 == imm1 == 0, "binary operation shape")
            elif opcode == 10:
                require(1 <= count <= 8 and args[0] < len(place_types) and imm0 < len(self.machines) and imm1 == 0,
                        "Call shape")
                target = self.machines[imm0]
                require(target[1] == self.machines[machine][1] and count == target[7] + 1,
                        "same-owner Call signature")
                call_edges.add((machine, imm0))
            elif opcode == 11:
                require(result_kind == 0 and count == 1 and args[0] < len(place_types) and place_mutable[args[0]]
                        and place_block[args[0]] == block and imm0 < len(self.constants) and imm1 == 0,
                        "CopyAggregateConst shape/place")
                require(self.constants[imm0][1] == place_types[args[0]], "CopyAggregateConst root type")
                roots.append(imm0)
            else:
                raise FixtureError(f"unsupported generated opcode {opcode}")
            operand_cursor += count
        require(next_value == self.value_count and next_place == self.place_count,
                "value/place reconstruction")

        for term in self.terminators:
            tid, machine, block, kind, flags, reserved, value, target0, start0, count0, target1, start1, count1 = term
            require((machine, block, flags, reserved) == (self.blocks[tid][1], tid, 0, 0),
                    "terminator owner/reserved")
            require(start0 == operand_cursor and count0 <= len(self.operands) - start0,
                    "terminator operand span 0")
            operand_cursor += count0
            require(start1 == operand_cursor and count1 <= len(self.operands) - start1,
                    "terminator operand span 1")
            operand_cursor += count1
            if kind == 2:
                require(value < len(value_types) and target0 < len(self.blocks) and target1 < len(self.blocks),
                        "Branch shape")
            elif kind == 3:
                require(value == target0 == target1 == NO_ID and count0 == count1 == 0,
                        "ReturnUnit shape")
            elif kind == 4:
                require(value < len(value_types) and target0 == target1 == NO_ID and count0 == count1 == 0,
                        "ReturnValue shape")
            else:
                raise FixtureError("unsupported generated terminator")
        require(operand_cursor == len(self.operands), "operand coverage")

        self.constant_facts()
        require(set(roots) == {row[10] for row in self.operations if row[3] == 11},
                "constant root reconstruction")
        active = [1] * len(self.machines)
        indegree = [0] * len(self.machines)
        for source, target in call_edges:
            indegree[target] += 1
        removed = 0
        while True:
            found = next((i for i, present in enumerate(active) if present and indegree[i] == 0), None)
            if found is None:
                break
            active[found] = 0; removed += 1
            for source, target in call_edges:
                if source == found:
                    indegree[target] -= 1
        require(removed == len(self.machines), "cyclic machine call graph")
        require(self.entry == NO_ID or 0 <= self.entry < len(self.machines), "entry ID")
        if self.entry != NO_ID:
            entry = self.machines[self.entry]
            require(entry[7] == 0 and entry[5] != NO_ID and self.types[entry[5]][1] <= 3,
                    "entry signature")

    def encode(self, *, allow: frozenset[str] = frozenset()) -> bytes:
        self.validate(allow=allow)
        payload = b"".join(row_type.pack(*row) for name, row_type in ROWS for row in getattr(self, name))
        total = HEADER.size + len(payload)
        if "encoded_bytes" not in allow:
            require(total <= 2_522_192, "encoded CKIR3 cap")
        flags = int(self.entry != NO_ID)
        result = HEADER.pack(b"OMGCKIR\0", 3, 0, 1, flags, self.entry, total, *self.counts()) + payload
        require(len(result) == total, "encoded length")
        return result


def u8(type_id: int, low: int = 0, high: int = 255) -> tuple[int, ...]:
    return (type_id, 1, 0, 0, 0, 0, low, high)


def u32(type_id: int, low: int = 0, high: int = 0x7FFF_FFFF) -> tuple[int, ...]:
    return (type_id, 2, 0, 0, 0, 0, low, high)


def boolean(type_id: int) -> tuple[int, ...]:
    return (type_id, 3, 0, 0, 0, 0, 0, 1)


def array(type_id: int, element: int, count: int) -> tuple[int, ...]:
    return (type_id, 5, 1, 0, element, count, 0, 0)


def nominal(type_id: int, record: int) -> tuple[int, ...]:
    return (type_id, 4, 0, 0, record, 0, 0, 0)


def append_node(module: Module, type_id: int, children: list[int] | tuple[int, ...] = (), scalar: int = 0) -> int:
    node = len(module.constants)
    start = len(module.constant_children)
    module.constants.append((node, type_id, start, len(children), scalar, 0))
    module.constant_children.extend((child,) for child in children)
    return node


def install_machine(module: Module, owner: int, roots: list[tuple[int, int]], *, selected: bool = True) -> None:
    """Install roots into owner fields; ``roots`` contains (field, node)."""
    machine = len(module.machines)
    block = len(module.blocks)
    op_start = len(module.operations)
    value = module.value_count
    place = module.place_count
    module.machines.append((machine, owner, 2, 0, 0, 0 if selected else NO_ID, 0, 0, block, 1, block))
    self_op = len(module.operations)
    module.operations.append((self_op, machine, block, 2, 2, 0, place, module.records[owner][1],
                              len(module.operands), 0, 0, 0))
    base = place
    place += 1
    for field, root in roots:
        field_op = len(module.operations)
        module.operands.append((base,))
        module.operations.append((field_op, machine, block, 3, 2, 0, place, module.fields[field][3],
                                  len(module.operands) - 1, 1, field, 0))
        destination = place
        place += 1
        copy_op = len(module.operations)
        module.operands.append((destination,))
        module.operations.append((copy_op, machine, block, 11, 0, 0, NO_ID, NO_ID,
                                  len(module.operands) - 1, 1, root, 0))
    if selected:
        const = len(module.operations)
        module.operations.append((const, machine, block, 1, 1, 0, value, 0, len(module.operands), 0, 70, 0))
        value += 1
        term = (block, machine, block, 4, 0, 0, value - 1, NO_ID,
                len(module.operands), 0, NO_ID, len(module.operands), 0)
    else:
        term = (block, machine, block, 3, 0, 0, NO_ID, NO_ID,
                len(module.operands), 0, NO_ID, len(module.operands), 0)
    module.blocks.append((block, machine, 2, 0, 0, 0, 0, op_start,
                          len(module.operations) - op_start, block))
    module.terminators.append(term)
    module.value_count = value
    module.place_count = place


def build_nodes(over: bool) -> Module:
    module = Module(entry=0)
    chain_count = 8 if over else 7
    nominal_id = 4 + chain_count
    module.types = [u32(0), array(1, 0, 1), array(2, 1, 1_024), array(3, 1, 1_018)]
    for index in range(chain_count):
        module.types.append(array(4 + index, 2 if index == 0 else 3 + index, 1))
    module.types.extend((nominal(nominal_id, 0), nominal(nominal_id + 1, 1)))
    module.records = [(0, nominal_id, 0, 0, 0, 0, 0, 0),
                      (1, nominal_id + 1, 0, 4, 1, 0, 0, 0)]
    module.fields = [(0, 1, 0, nominal_id - 1), (1, 1, 1, 2), (2, 1, 2, 2), (3, 1, 3, 3)]
    scalars = [append_node(module, 0, scalar=index) for index in range(4_090)]
    wrappers = [append_node(module, 1, [scalar]) for scalar in scalars]
    groups = [append_node(module, 2, wrappers[start:start + 1_024]) for start in (0, 1_024, 2_048)]
    groups.append(append_node(module, 3, wrappers[3_072:]))
    top = groups[0]
    for index in range(chain_count):
        top = append_node(module, 4 + index, [top])
    root = append_node(module, nominal_id + 1, [top, groups[1], groups[2], groups[3]])
    module.machines = [
        (0, 0, 2, 0, 0, 0, 0, 0, 0, 1, 0),
        (1, 1, 2, 0, 0, NO_ID, 0, 0, 1, 1, 1),
    ]
    module.blocks = [
        (0, 0, 2, 0, 0, 0, 0, 0, 1, 0),
        (1, 1, 2, 0, 0, 0, 0, 1, 2, 1),
    ]
    module.operations = [
        (0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 70, 0),
        (1, 1, 1, 2, 2, 0, 0, nominal_id + 1, 0, 0, 0, 0),
        (2, 1, 1, 11, 0, 0, NO_ID, NO_ID, 0, 1, root, 0),
    ]
    module.operands = [(0,)]
    module.terminators = [
        (0, 0, 0, 4, 0, 0, 0, NO_ID, 1, 0, NO_ID, 1, 0),
        (1, 1, 1, 3, 0, 0, NO_ID, NO_ID, 1, 0, NO_ID, 1, 0),
    ]
    module.value_count = 1
    module.place_count = 1
    require((len(module.constants), len(module.constant_children)) == ((8_193, 8_192) if over else (8_192, 8_191)),
            "node fixture counts")
    return module


def build_children(over: bool) -> Module:
    count = 17 if over else 16
    module = Module(entry=0)
    module.types = [u32(0, 0, 255)] + [u32(index, 0, index) for index in range(1, count)]
    module.types += [array(count + index, index, 1 if over and index == 16 else 1_024) for index in range(count)]
    owner_type = 2 * count
    module.types.append(nominal(owner_type, 0))
    module.records = [(0, owner_type, 0, count, 1, 0, 0, 0)]
    module.fields = [(index, 0, index, count + index) for index in range(count)]
    scalars = [append_node(module, index, scalar=index) for index in range(count)]
    roots = [append_node(module, count + index,
                         [scalars[index]] * (1 if over and index == 16 else 1_024)) for index in range(count)]
    install_machine(module, 0, list(enumerate(roots)), selected=True)
    expected = (34, 16_385) if over else (32, 16_384)
    require((len(module.constants), len(module.constant_children)) == expected, "child fixture counts")
    return module


def build_nodes_children() -> Module:
    module = Module(entry=0)
    module.types = [u32(0), array(1, 0, 3), array(2, 0, 4), array(3, 1, 1_024),
                    array(4, 1, 1_000), array(5, 1, 9), array(6, 2, 12), nominal(7, 0)]
    module.records = [(0, 7, 0, 6, 1, 0, 0, 0)]
    module.fields = [(index, 0, index, kind) for index, kind in enumerate((3, 3, 3, 4, 5, 6))]
    scalars = [append_node(module, 0, scalar=index) for index in range(4_093)]
    leaves3 = [append_node(module, 1, [scalar] * 3) for scalar in scalars[:4_081]]
    leaves4 = [append_node(module, 2, [scalar] * 4) for scalar in scalars[4_081:]]
    roots = [append_node(module, 3, leaves3[start:start + 1_024]) for start in (0, 1_024, 2_048)]
    roots.append(append_node(module, 4, leaves3[3_072:4_072]))
    roots.append(append_node(module, 5, leaves3[4_072:]))
    roots.append(append_node(module, 6, leaves4))
    install_machine(module, 0, list(enumerate(roots)), selected=True)
    require((len(module.constants), len(module.constant_children)) == (8_192, 16_384),
            "simultaneous node/child counts")
    return module


def build_image(over: bool) -> Module:
    module = Module(entry=0)
    module.types = [u8(0), array(1, 0, 1_024), array(2, 1, 64), nominal(3, 0), nominal(4, 1)]
    module.records = [(0, 3, 0, 1, 1, 0, 0, 0), (1, 4, 1, 2 if over else 1, 1, 0, 0, 0)]
    module.fields = [(0, 0, 0, 2), (1, 1, 0, 2)]
    if over:
        module.fields.append((2, 1, 1, 0))
    scalar = append_node(module, 0, scalar=1)
    inner = append_node(module, 1, [scalar] * 1_024)
    outer = append_node(module, 2, [inner] * 64)
    root_a = append_node(module, 3, [outer])
    root_b = append_node(module, 4, [outer, scalar] if over else [outer])
    # Direct SelfPlace copies make each distinct nominal constant one image root.
    for owner, root, selected in ((0, root_a, True), (1, root_b, False)):
        machine = len(module.machines); block = len(module.blocks); op_start = len(module.operations)
        module.machines.append((machine, owner, 2, 0, 0, 0 if selected else NO_ID, 0, 0, block, 1, block))
        place = module.place_count
        module.operations.append((len(module.operations), machine, block, 2, 2, 0, place,
                                  module.records[owner][1], len(module.operands), 0, 0, 0))
        module.place_count += 1
        module.operands.append((place,))
        module.operations.append((len(module.operations), machine, block, 11, 0, 0, NO_ID, NO_ID,
                                  len(module.operands) - 1, 1, root, 0))
        if selected:
            value = module.value_count
            module.operations.append((len(module.operations), machine, block, 1, 1, 0, value, 0,
                                      len(module.operands), 0, 70, 0))
            module.value_count += 1
            term = (block, machine, block, 4, 0, 0, value, NO_ID, len(module.operands), 0,
                    NO_ID, len(module.operands), 0)
        else:
            term = (block, machine, block, 3, 0, 0, NO_ID, NO_ID, len(module.operands), 0,
                    NO_ID, len(module.operands), 0)
        module.blocks.append((block, machine, 2, 0, 0, 0, 0, op_start,
                              len(module.operations) - op_start, block))
        module.terminators.append(term)
    operand_end = len(module.operands)
    module.terminators = [
        row[:8] + (operand_end, 0, NO_ID, operand_end, 0)
        for row in module.terminators
    ]
    _, _, image = module.constant_facts()
    require(image == 131_073 if over else image == 131_072, "isolated image boundary")
    require(module.layout(3)[0] == 65_536, "selected owner isolation")
    return module


def base_scalar_program() -> Module:
    module = Module(entry=0)
    module.types = [u8(0), nominal(1, 0)]
    module.records = [(0, 1, 0, 0, 0, 0, 0, 0)]
    module.machines = [(0, 0, 2, 0, 0, 0, 0, 0, 0, 1, 0)]
    return module


def build_frame(over: bool) -> Module:
    places = 32_765 if over else 32_764
    module = base_scalar_program()
    module.blocks = [(0, 0, 2, 0, 0, 0, 0, 0, places + 1, 0)]
    module.operations.append((0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 70, 0))
    for place in range(places):
        op = len(module.operations)
        module.operations.append((op, 0, 0, 2, 2, 0, place, 1, 0, 0, 0, 0))
    module.terminators = [(0, 0, 0, 4, 0, 0, 0, NO_ID, 0, 0, NO_ID, 0, 0)]
    module.value_count = 1; module.place_count = places
    frame = align(16 + places * 8, 16)
    require(frame == (262_144 if over else 262_128), "selected frame boundary")
    return module


def append_text_body(module: Module, const_count: int, add_count: int, less_count: int,
                     *, self_copy_root: int | None = None) -> None:
    operand_cursor = len(module.operands)
    if self_copy_root is not None:
        place = module.place_count
        module.operations.append((len(module.operations), 0, 0, 2, 2, 0, place,
                                  module.records[0][1], operand_cursor, 0, 0, 0))
        module.place_count += 1
        module.operands.append((place,)); operand_cursor += 1
        module.operations.append((len(module.operations), 0, 0, 11, 0, 0, NO_ID, NO_ID,
                                  operand_cursor - 1, 1, self_copy_root, 0))
    for index in range(const_count):
        value = module.value_count
        module.operations.append((len(module.operations), 0, 0, 1, 1, 0, value, 0,
                                  operand_cursor, 0, index + 1, 0))
        module.value_count += 1
    for _ in range(add_count - 1):
        value = module.value_count
        module.operands.extend(((0,), (1,)))
        module.operations.append((len(module.operations), 0, 0, 8, 1, 0, value, 0,
                                  operand_cursor, 2, 0, 0))
        operand_cursor += 2; module.value_count += 1
    for _ in range(less_count):
        value = module.value_count
        module.operands.extend(((0,), (1,)))
        module.operations.append((len(module.operations), 0, 0, 9, 1, 0, value, 1,
                                  operand_cursor, 2, 0, 0))
        operand_cursor += 2; module.value_count += 1
    value = module.value_count
    module.operands.extend(((0,), (1,)))
    module.operations.append((len(module.operations), 0, 0, 8, 1, 0, value, 0,
                              operand_cursor, 2, 0, 0))
    operand_cursor += 2; module.value_count += 1
    module.blocks = [(0, 0, 2, 0, 0, 0, 0, 0, len(module.operations), 0)]
    module.terminators = [(0, 0, 0, 4, 0, 0, value, NO_ID, operand_cursor, 0,
                           NO_ID, operand_cursor, 0)]


def build_text(over: bool) -> Module:
    module = Module(entry=0)
    module.types = [u8(0), boolean(1), nominal(2, 0)]
    module.records = [(0, 2, 0, 0, 0, 0, 0, 0)]
    module.machines = [(0, 0, 2, 0, 0, 0, 0, 0, 0, 1, 0)]
    append_text_body(module, 3 if over else 2, 11_920, 20_840)
    text = 26 + 18 + (3 if over else 2) * 11 + 11_920 * 46 + 20_840 * 24 + 30
    require(text == (1_048_587 if over else 1_048_576), "text boundary")
    return module


def build_elf_exact() -> Module:
    module = Module(entry=0)
    module.types = [u8(0), boolean(1), array(2, 0, 1_024), array(3, 2, 128), nominal(4, 0)]
    module.records = [(0, 4, 0, 1, 1, 0, 0, 0)]
    module.fields = [(0, 0, 0, 3)]
    module.machines = [(0, 0, 2, 0, 0, 0, 0, 0, 0, 1, 0)]
    scalar = append_node(module, 0, scalar=1)
    inner = append_node(module, 2, [scalar] * 1_024)
    outer = append_node(module, 3, [inner] * 128)
    root = append_node(module, 4, [outer])
    append_text_body(module, 7, 11_920, 20_836, self_copy_root=root)
    text = 26 + 18 + 7 * 11 + 11_920 * 46 + 20_836 * 24 + 14 + 27 + 30
    require(text == 1_048_576, "exact ELF text")
    _, _, image = module.constant_facts()
    require(image == 131_072 and align(4_096 + text, 4_096) + image == 1_183_744,
            "exact ELF extent")
    return module


def build_wire_exact() -> Module:
    module = Module()
    nominal_start = 8_192 - 128
    module.types = [u8(0), boolean(1), u32(2), array(3, 2, 3), array(4, 3, 820), array(5, 4, 5)]
    for type_id in range(6, nominal_start):
        module.types.append(u32(type_id, type_id, type_id))
    module.types.extend(nominal(nominal_start + record, record) for record in range(128))
    module.records = [(record, nominal_start + record, record * 64, 64, 1 if record == 0 else 0, 0, 0, 0)
                      for record in range(128)]
    module.fields = [(field, field // 64, field % 64, 5 if field == 0 else 0) for field in range(8_192)]

    mparam = 0; block = 0; bparam = 0; operation = 0
    for machine in range(128):
        pcount = 5 if machine == 1 else 7
        pstart = mparam
        module.machines.append((machine, 0, 2, 0, 0, 0 if machine == 1 else NO_ID,
                                pstart, pcount, block, 16, block))
        for ordinal in range(pcount):
            ptype = 1 if ordinal == 4 and machine in (0, 1) or ordinal == pcount - 1 and machine not in (0, 1) else 0
            module.machine_params.append((mparam, machine, ordinal, ptype, mparam))
            mparam += 1
        for local in range(16):
            count = 7 if local in (1, 2, 3) else 6 if machine == 0 and local == 4 else 4 if local == 4 else 0
            module.blocks.append((block, machine, 2, 0, 0, bparam, count, operation, 16, block))
            for ordinal in range(count):
                module.block_params.append((bparam, block, ordinal, 1 if ordinal == count - 1 else 0,
                                            len(module.machine_params) + bparam))
                bparam += 1
            block += 1; operation += 16
    require((mparam, bparam) == (894, 3_202), "wire parameter maxima")
    module.block_params = [row[:4] + (mparam + row[0],) for row in module.block_params]

    scalars = [append_node(module, 2, scalar=index) for index in range(4_093)]
    leaves = [append_node(module, 3, [scalar] * 3) for scalar in scalars]
    groups: list[int] = []
    for index in range(5):
        children = leaves[index * 820:(index + 1) * 820]
        if index == 4:
            children += [leaves[-1]] * (820 - len(children))
        groups.append(append_node(module, 4, children))
    root = append_node(module, 5, groups)

    next_value = len(module.machine_params) + len(module.block_params)
    next_place = 0; operand_cursor = 0
    for oid in range(32_768):
        machine = oid // 256; block_id = oid // 16
        if oid == 0:
            module.operations.append((oid, machine, block_id, 2, 2, 0, next_place, nominal_start,
                                      operand_cursor, 0, 0, 0)); next_place += 1
        elif oid == 1:
            module.operands.append((0,)); module.operations.append((oid, machine, block_id, 3, 2, 0,
                next_place, 5, operand_cursor, 1, 0, 0)); operand_cursor += 1; next_place += 1
        elif oid == 2:
            module.operands.append((1,)); module.operations.append((oid, machine, block_id, 11, 0, 0,
                NO_ID, NO_ID, operand_cursor, 1, root, 0)); operand_cursor += 1
        elif oid == 3:
            args = [0, 0, 1, 2, 3, 4]
            module.operands.extend((arg,) for arg in args)
            module.operations.append((oid, machine, block_id, 10, 1, 0, next_value, 0,
                                      operand_cursor, 6, 1, 0))
            operand_cursor += 6; next_value += 1
        else:
            start = sum(row[7] for row in module.machines[:machine])
            module.operands.extend(((start,), (start + 1,)))
            module.operations.append((oid, machine, block_id, 8, 1, 0, next_value, 0,
                                      operand_cursor, 2, 0, 0))
            operand_cursor += 2; next_value += 1
    require(operand_cursor == 65_536, "wire operation operands")

    for block_id in range(2_048):
        machine = block_id // 16
        pstart = module.machines[machine][6]
        pcount = module.machines[machine][7]
        condition = pstart + (4 if machine in (0, 1) else pcount - 1)
        target = machine * 16 + 1
        args = [pstart] * 6 + [condition]
        start0 = operand_cursor
        module.operands.extend((arg,) for arg in args); operand_cursor += 7
        start1 = operand_cursor
        module.operands.extend((arg,) for arg in args); operand_cursor += 7
        module.terminators.append((block_id, machine, block_id, 2, 0, 0, condition,
                                   target, start0, 7, target, start1, 7))
    module.value_count = next_value; module.place_count = next_place
    require(operand_cursor == 94_208, "wire total operands")
    require((len(module.constants), len(module.constant_children)) == (8_192, 16_384),
            "wire constant maxima")
    require(module.encoded_length() == 2_522_192, "wire exact encoded length")
    return module


def write_atomic(path: Path, contents: bytes) -> None:
    temporary = path.with_name(f".{path.name}.tmp-{os.getpid()}")
    try:
        with temporary.open("xb") as output:
            output.write(contents); output.flush(); os.fsync(output.fileno())
        os.replace(temporary, path)
    finally:
        if temporary.exists():
            temporary.unlink()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("OUT_DIR", type=Path)
    args = parser.parse_args()
    builders = {
        "nodes-exact.ckir3": (build_nodes(False), frozenset()),
        "nodes-over.ckir3": (build_nodes(True), frozenset({"constants"})),
        "children-exact.ckir3": (build_children(False), frozenset()),
        "children-over.ckir3": (build_children(True), frozenset({"constant_children"})),
        "nodes-children-exact.ckir3": (build_nodes_children(), frozenset()),
        "image-exact.ckir3": (build_image(False), frozenset()),
        "image-over.ckir3": (build_image(True), frozenset()),
        "frame-greatest.ckir3": (build_frame(False), frozenset()),
        "frame-next.ckir3": (build_frame(True), frozenset()),
        "text-exact.ckir3": (build_text(False), frozenset()),
        "text-over.ckir3": (build_text(True), frozenset()),
        "elf-exact.ckir3": (build_elf_exact(), frozenset()),
        "wire-exact.ckir3": (build_wire_exact(), frozenset()),
    }
    encoded = {name: module.encode(allow=allow) for name, (module, allow) in builders.items()}
    encoded["wire-over.ckir3"] = encoded["wire-exact.ckir3"] + b"\0"
    require(len(encoded["wire-over.ckir3"]) == 2_522_193, "wire adjacent length")
    expected_lengths = {
        "nodes-exact.ckir3": 230_216, "nodes-over.ckir3": 230_268,
        "children-exact.ckir3": 69_052, "children-over.ckir3": 69_256,
        "nodes-children-exact.ckir3": 263_252,
        "image-exact.ckir3": 5_184, "image-over.ckir3": 5_204,
        "frame-greatest.ckir3": 1_310_860, "frame-next.ckir3": 1_310_900,
        "text-exact.ckir3": 1_572_844, "text-over.ckir3": 1_572_884,
        "elf-exact.ckir3": 1_577_708, "wire-exact.ckir3": 2_522_192,
        "wire-over.ckir3": 2_522_193,
    }
    require({name: len(data) for name, data in encoded.items()} == expected_lengths,
            "fixture encoded-length inventory")
    rows = (
        ("nodes-exact.ckir3", 0, True, "elf", False, "8192 canonical nodes, 8191 child words"),
        ("nodes-over.ckir3", 252, False, "empty", False, "8193 canonical-under-relaxed-cap nodes"),
        ("children-exact.ckir3", 0, True, "elf", False, "16384 child words, 65536-byte owner"),
        ("children-over.ckir3", 252, False, "empty", True, "16385 canonical-under-relaxed-cap child words"),
        ("nodes-children-exact.ckir3", 0, True, "elf", False, "8192 nodes and 16384 child words simultaneously"),
        ("image-exact.ckir3", 0, True, "elf:139264", True, "131072 image; selected owner independently 65536"),
        ("image-over.ckir3", 252, False, "empty", False, "131073 image; selected owner remains 65536"),
        ("frame-greatest.ckir3", 0, True, "elf", False, "greatest selected frame 262128; live stack 262144"),
        ("frame-next.ckir3", 252, True, "empty", True, "next realizable selected frame 262144; backend live stack over"),
        ("text-exact.ckir3", 0, True, "elf:1052672", True, "exact 1048576-byte text"),
        ("text-over.ckir3", 252, True, "empty", False, "structurally valid 1048587-byte backend text"),
        ("elf-exact.ckir3", 0, True, "elf:1183744", False, "simultaneous exact RX and R maxima"),
        ("wire-exact.ckir3", 0, True, "empty", True, "exact 2522192-byte canonical library CKIR3"),
        ("wire-over.ckir3", 252, False, "empty", True, "one trailing byte beyond encoded cap"),
    )
    out = args.OUT_DIR
    out.mkdir(parents=True, exist_ok=True)
    for name, _, _, _, _, _ in rows:
        write_atomic(out / name, encoded[name])
    manifest = (
        "name\texpected_backend_status\treference_valid\texpected_output\t"
        "self_representative\tencoded_bytes\tnote\n" + "".join(
        f"{name}\t{status}\t{'true' if reference_valid else 'false'}\t{output}\t"
        f"{'true' if self_representative else 'false'}\t{len(encoded[name])}\t{note}\n"
        for name, status, reference_valid, output, self_representative, note in rows
        )
    )
    write_atomic(out / "manifest.tsv", manifest.encode("utf-8"))
    print(f"CKIR3 resources: {len(rows)} fixtures; exact wire {len(encoded['wire-exact.ckir3'])} bytes")


if __name__ == "__main__":
    try:
        main()
    except (FixtureError, OSError, struct.error) as error:
        raise SystemExit(f"checked IR v3 resources: {error}")
