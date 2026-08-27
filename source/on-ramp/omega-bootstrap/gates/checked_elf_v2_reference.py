#!/usr/bin/env python3
"""Independent CKIR2-to-ELF reconstruction and exact artifact checker."""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

import checked_ir_v2_reference


NO_ID = 0xFFFF_FFFF
PAGE = 4096
IMAGE_BASE = 0x400000
ELF_HEADER = struct.Struct("<16sHHIQQQIHHHHHH")
PROGRAM_HEADER = struct.Struct("<IIQQQQQQ")


class ArtifactError(ValueError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ArtifactError(message)


def align(value: int, alignment: int) -> int:
    return (value + alignment - 1) // alignment * alignment


class TextEmitter:
    def __init__(self, block_offsets: dict[int, int] | None = None) -> None:
        self.code = bytearray()
        self.block_offsets = block_offsets

    @property
    def cursor(self) -> int:
        return len(self.code)

    def raw(self, contents: bytes) -> None:
        self.code.extend(contents)

    def byte(self, value: int) -> None:
        self.code.append(value)

    def u32(self, value: int) -> None:
        require(0 <= value <= 0xFFFF_FFFF, "unsigned instruction word")
        self.code.extend(struct.pack("<I", value))

    def s32(self, value: int) -> None:
        require(-(1 << 31) <= value < (1 << 31), "signed instruction word")
        self.code.extend(struct.pack("<i", value))

    def rel32(self, target: int | None) -> None:
        if target is None:
            self.u32(0)
        else:
            self.s32(target - (self.cursor + 4))


class Reconstructor:
    def __init__(self, module: checked_ir_v2_reference.Module) -> None:
        self.module = module
        self.tables = module.tables
        self.types = self.tables["types"]
        self.records = self.tables["records"]
        self.fields = self.tables["fields"]
        self.machines = self.tables["machines"]
        self.blocks = self.tables["blocks"]
        self.block_params = self.tables["block_params"]
        self.operations = self.tables["operations"]
        self.operands = [row[0] for row in self.tables["operands"]]
        self.terminators = self.tables["terminators"]
        self.value_machines, self.value_blocks, self.place_blocks = self._scopes()
        self.value_slots: dict[int, int] = {}
        self.place_slots: dict[int, int] = {}
        self.scratch_base = 0
        self.frame_size = 0
        self.frame_sizes: dict[int, int] = {}
        self.scratch_bases: dict[int, int] = {}
        self.reachable: set[int] = set()
        self.current_machine = NO_ID
        self.trap_offset = 0

    def _scopes(self) -> tuple[list[int], list[int], list[int]]:
        machine_params = self.tables["machine_params"]
        block_params = self.tables["block_params"]
        value_machines = [row[1] for row in machine_params]
        value_blocks = [NO_ID] * len(machine_params)
        value_machines.extend(self.blocks[row[1]][1] for row in block_params)
        value_blocks.extend(row[1] for row in block_params)
        place_blocks: list[int] = []
        for operation in self.operations:
            if operation[4] == 1:
                value_machines.append(operation[1])
                value_blocks.append(operation[2])
            elif operation[4] == 2:
                place_blocks.append(operation[2])
        require(len(value_machines) == len(self.module.value_types), "value scope reconstruction")
        require(len(place_blocks) == len(self.module.place_types), "place scope reconstruction")
        return value_machines, value_blocks, place_blocks

    def _machine_blocks(self, machine_id: int) -> range:
        machine = self.machines[machine_id]
        return range(machine[8], machine[8] + machine[9])

    def assign_frame(self) -> None:
        graph = [set() for _ in self.machines]
        for operation in self.operations:
            if operation[3] == 10:
                graph[operation[1]].add(operation[10])
        queue = [self.module.entry]
        self.reachable = {self.module.entry}
        while queue:
            caller = queue.pop(0)
            for callee in sorted(graph[caller]):
                if callee not in self.reachable:
                    self.reachable.add(callee)
                    queue.append(callee)

        for machine_id in sorted(self.reachable):
            cursor = 8  # receiver address
            for value_id, type_id in enumerate(self.module.value_types):
                if self.value_machines[value_id] != machine_id:
                    continue
                width = 4 if self.types[type_id][1] <= 3 else 8
                cursor = align(cursor, width) + width
                require(cursor <= 262_144, "value frame exhaustion")
                self.value_slots[value_id] = cursor
            for place_id, block_id in enumerate(self.place_blocks):
                if self.blocks[block_id][1] != machine_id:
                    continue
                cursor = align(cursor, 8) + 8
                require(cursor <= 262_144, "place frame exhaustion")
                self.place_slots[place_id] = cursor
            scratch_count = max(
                [max(self.terminators[block_id][9], self.terminators[block_id][12]) for block_id in self._machine_blocks(machine_id)]
                + [operation[9] - 1 for operation in self.operations if operation[1] == machine_id and operation[3] == 10]
                + [0]
            )
            cursor = align(cursor, 8)
            self.scratch_bases[machine_id] = cursor
            cursor += scratch_count * 8
            frame_size = align(cursor, 16)
            require(frame_size <= 262_144, "frame exhaustion")
            self.frame_sizes[machine_id] = frame_size

        indegree = [0] * len(self.machines)
        for caller, callees in enumerate(graph):
            for callee in callees:
                indegree[callee] += 1
        topo: list[int] = []
        pending = [machine_id for machine_id, degree in enumerate(indegree) if degree == 0]
        while pending:
            caller = min(pending)
            pending.remove(caller)
            topo.append(caller)
            for callee in sorted(graph[caller]):
                indegree[callee] -= 1
                if indegree[callee] == 0:
                    pending.append(callee)
        require(len(topo) == len(self.machines), "cyclic call graph")
        live = {self.module.entry: self.frame_sizes[self.module.entry] + 16}
        require(live[self.module.entry] <= 262_144, "live stack exhaustion")
        for caller in topo:
            if caller not in live:
                continue
            for callee in graph[caller]:
                cost = live[caller] + self.frame_sizes[callee] + 16
                require(cost <= 262_144, "live stack exhaustion")
                live[callee] = max(live.get(callee, 0), cost)
        self.frame_size = self.frame_sizes[self.module.entry]
        self.scratch_base = self.scratch_bases[self.module.entry]

    def value_slot(self, value_id: int) -> int:
        require(value_id in self.value_slots, "value outside selected frame")
        return self.value_slots[value_id]

    def place_slot(self, place_id: int) -> int:
        require(place_id in self.place_slots, "place outside selected frame")
        return self.place_slots[place_id]

    def load_value(self, emitter: TextEmitter, value_id: int) -> None:
        emitter.raw(b"\x8b\x85")
        emitter.s32(-self.value_slot(value_id))

    def store_value(self, emitter: TextEmitter, value_id: int) -> None:
        emitter.raw(b"\x89\x85")
        emitter.s32(-self.value_slot(value_id))

    def load_place(self, emitter: TextEmitter, place_id: int) -> None:
        emitter.raw(b"\x48\x8b\x85")
        emitter.s32(-self.place_slot(place_id))

    def store_place(self, emitter: TextEmitter, place_id: int) -> None:
        emitter.raw(b"\x48\x89\x85")
        emitter.s32(-self.place_slot(place_id))

    def trap_jump(self, emitter: TextEmitter, opcode: int) -> None:
        emitter.raw(bytes((0x0F, opcode)))
        emitter.rel32(self.trap_offset)

    def range_check(self, emitter: TextEmitter, type_id: int) -> None:
        type_row = self.types[type_id]
        emitter.byte(0x3D)
        emitter.u32(type_row[6])
        self.trap_jump(emitter, 0x82)
        emitter.byte(0x3D)
        emitter.u32(type_row[7])
        self.trap_jump(emitter, 0x87)

    def scalar_leaves(self, type_id: int, base: int = 0):
        type_row = self.types[type_id]
        kind, payload0, payload1 = type_row[1], type_row[4], type_row[5]
        if kind <= 3:
            yield type_id, base
        elif kind == 4:
            record = self.records[payload0]
            for field_id in range(record[2], record[2] + record[3]):
                yield from self.scalar_leaves(
                    self.fields[field_id][3],
                    base + self.module.field_offsets[field_id],
                )
        else:
            stride = self.module.layouts[payload0][0]
            for index in range(payload1):
                yield from self.scalar_leaves(payload0, base + index * stride)

    def emit_copy(self, emitter: TextEmitter, type_id: int) -> None:
        for leaf_type, offset in self.scalar_leaves(type_id):
            if self.types[leaf_type][1] == 2:
                emitter.raw(b"\x41\x8b\x83")
                emitter.u32(offset)
                emitter.raw(b"\x41\x89\x82")
                emitter.u32(offset)
            else:
                emitter.raw(b"\x41\x0f\xb6\x83")
                emitter.u32(offset)
                emitter.raw(b"\x41\x88\x82")
                emitter.u32(offset)

    def emit_operation(self, emitter: TextEmitter, operation: tuple[int, ...]) -> None:
        opcode, result_id, result_type = operation[3], operation[6], operation[7]
        arguments = self.operands[operation[8] : operation[8] + operation[9]]
        imm0 = operation[10]
        if opcode == 1:
            emitter.byte(0xB8)
            emitter.u32(imm0)
            self.store_value(emitter, result_id)
        elif opcode == 2:
            emitter.raw(b"\x48\x8b\x85")
            emitter.s32(-8)
            self.store_place(emitter, result_id)
        elif opcode == 3:
            self.load_place(emitter, arguments[0])
            emitter.raw(b"\x48\x05")
            emitter.u32(self.module.field_offsets[imm0])
            self.store_place(emitter, result_id)
        elif opcode == 4:
            base_type = self.module.place_types[arguments[0]]
            element_type = self.types[base_type][4]
            self.load_place(emitter, arguments[0])
            emitter.raw(b"\x49\x89\xc2")
            self.load_value(emitter, arguments[1])
            emitter.raw(b"\x89\xc1\x81\xf9")
            emitter.u32(self.types[base_type][5])
            self.trap_jump(emitter, 0x83)
            emitter.raw(b"\x48\x69\xc9")
            emitter.u32(self.module.layouts[element_type][0])
            emitter.raw(b"\x49\x01\xca\x4c\x89\xd0")
            self.store_place(emitter, result_id)
        elif opcode == 5:
            self.load_place(emitter, arguments[0])
            emitter.raw(b"\x8b\x00" if self.types[result_type][1] == 2 else b"\x0f\xb6\x00")
            self.store_value(emitter, result_id)
        elif opcode == 6:
            destination_type = self.module.place_types[arguments[0]]
            self.load_place(emitter, arguments[0])
            emitter.raw(b"\x49\x89\xc2")
            self.load_value(emitter, arguments[1])
            self.range_check(emitter, destination_type)
            emitter.raw(b"\x41\x89\x02" if self.types[destination_type][1] == 2 else b"\x41\x88\x02")
        elif opcode == 7:
            self.load_place(emitter, arguments[0])
            emitter.raw(b"\x49\x89\xc2")
            if imm0 == 1:
                emitter.raw(b"\x4c\x8b\x9d")
                emitter.s32(-self.value_slot(arguments[1]))
                source_type = self.module.value_types[arguments[1]]
            else:
                emitter.raw(b"\x4c\x8b\x9d")
                emitter.s32(-self.place_slot(arguments[1]))
                source_type = self.module.place_types[arguments[1]]
            self.emit_copy(emitter, source_type)
        elif opcode == 8:
            self.load_value(emitter, arguments[0])
            emitter.raw(b"\x03\x85")
            emitter.s32(-self.value_slot(arguments[1]))
            self.trap_jump(emitter, 0x82)
            self.range_check(emitter, result_type)
            self.store_value(emitter, result_id)
        elif opcode == 9:
            self.load_value(emitter, arguments[0])
            emitter.raw(b"\x3b\x85")
            emitter.s32(-self.value_slot(arguments[1]))
            emitter.raw(b"\x0f\x92\xc0\x0f\xb6\xc0")
            self.store_value(emitter, result_id)
        elif opcode == 10:
            call_arguments = arguments[1:]
            for ordinal, value_id in enumerate(call_arguments):
                displacement = -(self.scratch_base + (len(call_arguments) - ordinal) * 8)
                if self.types[self.module.value_types[value_id]][1] <= 3:
                    self.load_value(emitter, value_id)
                    emitter.raw(b"\x89\x85")
                    emitter.s32(displacement)
                else:
                    emitter.raw(b"\x48\x8b\x85")
                    emitter.s32(-self.value_slot(value_id))
                    emitter.raw(b"\x48\x89\x85")
                    emitter.s32(displacement)
            self.load_place(emitter, arguments[0])
            emitter.raw(b"\x48\x89\xc7")
            if call_arguments:
                emitter.raw(b"\x48\x8d\xb5")
                emitter.s32(-(self.scratch_base + len(call_arguments) * 8))
            else:
                emitter.raw(b"\x31\xf6")
            emitter.byte(0xE8)
            target = None if emitter.block_offsets is None else emitter.block_offsets[self.machines[imm0][10]]
            emitter.rel32(target)
            if operation[4] == 1:
                self.store_value(emitter, result_id)
        else:  # decode() already rejects this; retain a fail-closed local guard.
            raise ArtifactError("unsupported operation")

    def edge_size(self, start: int, count: int) -> int:
        size = 5
        for value_id in self.operands[start : start + count]:
            size += 46 if self.types[self.module.value_types[value_id]][1] <= 3 else 28
        return size

    def emit_edge(self, emitter: TextEmitter, start: int, count: int, target: int) -> None:
        arguments = self.operands[start : start + count]
        for ordinal, value_id in enumerate(arguments):
            scratch = self.scratch_base + (ordinal + 1) * 8
            if self.types[self.module.value_types[value_id]][1] <= 3:
                self.load_value(emitter, value_id)
                emitter.raw(b"\x89\x85")
                emitter.s32(-scratch)
            else:
                emitter.raw(b"\x48\x8b\x85")
                emitter.s32(-self.value_slot(value_id))
                emitter.raw(b"\x48\x89\x85")
                emitter.s32(-scratch)
        parameter_start = self.blocks[target][5]
        for ordinal in range(count):
            value_id = self.block_params[parameter_start + ordinal][4]
            scratch = self.scratch_base + (ordinal + 1) * 8
            if self.types[self.module.value_types[value_id]][1] <= 3:
                emitter.raw(b"\x8b\x85")
                emitter.s32(-scratch)
                self.range_check(emitter, self.module.value_types[value_id])
                self.store_value(emitter, value_id)
            else:
                emitter.raw(b"\x48\x8b\x85")
                emitter.s32(-scratch)
                emitter.raw(b"\x48\x89\x85")
                emitter.s32(-self.value_slot(value_id))
        emitter.byte(0xE9)
        block_target = None if emitter.block_offsets is None else emitter.block_offsets[target]
        emitter.rel32(block_target)

    def emit_terminator(self, emitter: TextEmitter, block_id: int) -> None:
        term = self.terminators[block_id]
        kind = term[3]
        if kind == 1:
            self.emit_edge(emitter, term[8], term[9], term[7])
        elif kind == 2:
            self.load_value(emitter, term[6])
            emitter.raw(b"\x85\xc0\x0f\x84")
            skip = emitter.cursor + 4 + self.edge_size(term[8], term[9])
            emitter.rel32(skip)
            self.emit_edge(emitter, term[8], term[9], term[7])
            self.emit_edge(emitter, term[11], term[12], term[10])
        elif kind == 3:
            emitter.raw(b"\xc9\xc3")
        elif kind == 4:
            self.load_value(emitter, term[6])
            self.range_check(emitter, self.machines[self.current_machine][5])
            emitter.raw(b"\xc9\xc3")
        else:
            raise ArtifactError("unsupported terminator")

    def emit_block(self, emitter: TextEmitter, block_id: int) -> None:
        self.current_machine = self.blocks[block_id][1]
        machine = self.machines[self.current_machine]
        self.frame_size = self.frame_sizes[self.current_machine]
        self.scratch_base = self.scratch_bases[self.current_machine]
        if block_id == machine[10]:
            emitter.raw(b"\x55\x48\x89\xe5\x48\x81\xec")
            emitter.u32(self.frame_size)
            emitter.raw(b"\x48\x89\xbd")
            emitter.s32(-8)
            for ordinal, parameter_id in enumerate(range(machine[6], machine[6] + machine[7])):
                parameter = self.tables["machine_params"][parameter_id]
                value_id, type_id = parameter[4], parameter[3]
                if self.types[type_id][1] <= 3:
                    emitter.raw(b"\x8b\x86")
                    emitter.s32(ordinal * 8)
                    self.range_check(emitter, type_id)
                    self.store_value(emitter, value_id)
                else:
                    emitter.raw(b"\x48\x8b\x86")
                    emitter.s32(ordinal * 8)
                    emitter.raw(b"\x48\x89\x85")
                    emitter.s32(-self.value_slot(value_id))
        block = self.blocks[block_id]
        for operation_id in range(block[7], block[7] + block[8]):
            self.emit_operation(emitter, self.operations[operation_id])
        self.emit_terminator(emitter, block_id)

    def emit_text(self, rx_size: int | None, block_offsets: dict[int, int] | None) -> tuple[bytes, dict[int, int]]:
        emitter = TextEmitter(block_offsets)
        emitter.raw(b"\x48\x8d\x3d")
        emitter.rel32(None if rx_size is None else rx_size - PAGE)
        emitter.byte(0xE8)
        root = None if block_offsets is None else block_offsets[self.machines[self.module.entry][10]]
        emitter.rel32(root)
        emitter.raw(b"\x0f\xb6\xf8\xb8")
        emitter.u32(231)
        emitter.raw(b"\x0f\x05\x0f\x0b")
        self.trap_offset = emitter.cursor
        emitter.raw(b"\x0f\x0b")
        discovered: dict[int, int] = {}
        for machine_id in sorted(self.reachable):
            for block_id in self._machine_blocks(machine_id):
                discovered[block_id] = emitter.cursor
                self.emit_block(emitter, block_id)
        return bytes(emitter.code), discovered

    def reconstruct(self) -> bytes:
        if self.module.entry == NO_ID:
            return b""
        self.assign_frame()
        owner_type = self.records[self.machines[self.module.entry][1]][1]
        owner_size = self.module.layouts[owner_type][0]
        require(owner_size <= 131_072, "entry owner layout exhaustion")
        first_text, block_offsets = self.emit_text(None, None)
        require(len(first_text) <= 1_048_576, "text exhaustion")
        rx_size = align(PAGE + len(first_text), PAGE)
        text, rediscovered = self.emit_text(rx_size, block_offsets)
        require(len(text) == len(first_text), "unstable instruction sizing")
        require(rediscovered == block_offsets, "unstable block offsets")
        bss_size = align(max(owner_size, 1), PAGE)
        ident = b"\x7fELF\x02\x01\x01" + bytes(9)
        image = bytearray(
            ELF_HEADER.pack(
                ident, 2, 62, 1, IMAGE_BASE + PAGE, ELF_HEADER.size, 0, 0,
                ELF_HEADER.size, PROGRAM_HEADER.size, 2, 0, 0, 0,
            )
        )
        image.extend(PROGRAM_HEADER.pack(1, 5, 0, IMAGE_BASE, IMAGE_BASE, rx_size, rx_size, PAGE))
        image.extend(
            PROGRAM_HEADER.pack(
                1, 6, rx_size, IMAGE_BASE + rx_size, IMAGE_BASE + rx_size,
                0, bss_size, PAGE,
            )
        )
        require(len(image) == 176, "ELF envelope size")
        image.extend(bytes(PAGE - len(image)))
        image.extend(text)
        image.extend(bytes(rx_size - len(image)))
        return bytes(image)


def reconstruct(ckir: bytes) -> bytes:
    return Reconstructor(checked_ir_v2_reference.decode(ckir)).reconstruct()


def mismatch(expected: bytes, actual: bytes) -> str | None:
    if len(expected) != len(actual):
        return f"length {len(actual)}, expected {len(expected)}"
    for offset, (wanted, got) in enumerate(zip(expected, actual)):
        if wanted != got:
            return f"byte {offset}: {got:#04x}, expected {wanted:#04x}"
    return None


def check(ckir_path: Path, elf_path: Path) -> tuple[bytes, bytes]:
    expected = reconstruct(ckir_path.read_bytes())
    actual = elf_path.read_bytes()
    problem = mismatch(expected, actual)
    require(problem is None, f"artifact mismatch at {problem}")
    return expected, actual


def mutation_sweep(expected: bytes, actual: bytes) -> None:
    require(expected == actual, "mutation sweep requires canonical artifact")
    require(mismatch(expected, actual[:-1]) is not None, "truncation control accepted")
    require(mismatch(expected, actual + b"\0") is not None, "trailing-byte control accepted")
    for offset in range(len(actual)):
        mutated = bytearray(actual)
        mutated[offset] ^= 1
        require(mismatch(expected, mutated) is not None, f"mutation at {offset} accepted")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("check", "mutation-sweep"))
    parser.add_argument("ckir", type=Path)
    parser.add_argument("elf", type=Path)
    arguments = parser.parse_args()
    expected, actual = check(arguments.ckir, arguments.elf)
    if arguments.command == "mutation-sweep":
        mutation_sweep(expected, actual)
    print(
        f"CKIR2 ELF reconstructed: {len(actual)} bytes"
        + (f", {len(actual)} byte mutations rejected" if arguments.command == "mutation-sweep" else "")
    )


if __name__ == "__main__":
    try:
        main()
    except (ArtifactError, checked_ir_v2_reference.CkirError, OSError) as error:
        raise SystemExit(f"checked ELF reference: {error}")
