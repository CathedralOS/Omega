#!/usr/bin/env python3
"""Independent exact CKIR16 conservative ELF reconstruction for OMGRFN18."""

from __future__ import annotations

import sys
from pathlib import Path

from omgrfn16_elf_reference import Reconstructor as InheritedReconstructor
from omgrfn18_ckir import V5, decode
from omgrfn18_frame import RefinementError, RefinementResourceError, require
from omgrfn18_u64 import bounds

HERE = Path(__file__).resolve().parent
GATES = HERE.parents[3] / "source/on-ramp/omega-bootstrap/gates"
sys.path.insert(0, str(GATES))
import checked_elf_v2_reference as elf2  # noqa: E402


class Reconstructor(InheritedReconstructor):
    """Width-aware CKIR16 specialization of the frozen conservative engine.

    Header/layout orchestration is reused, but every kind-8 byte template is
    reconstructed here from semantic CKIR fields.  The production backend is
    neither loaded nor executed.
    """

    def _u64_value(self, value_id: int) -> bool:
        return self.types[self.module.value_types[value_id]][1] == 8

    def load_value(self, emitter: elf2.TextEmitter, value_id: int) -> None:
        emitter.raw(b"\x48\x8b\x85" if self._u64_value(value_id) else b"\x8b\x85")
        emitter.s32(-self.value_slot(value_id))

    def store_value(self, emitter: elf2.TextEmitter, value_id: int) -> None:
        emitter.raw(b"\x48\x89\x85" if self._u64_value(value_id) else b"\x89\x85")
        emitter.s32(-self.value_slot(value_id))

    def range_check(self, emitter: elf2.TextEmitter, type_id: int) -> None:
        if self.types[type_id][1] != 8:
            super().range_check(emitter, type_id)
            return
        low, high = bounds(self.types[type_id])
        for endpoint, condition in ((low, 0x82), (high, 0x87)):
            emitter.raw(b"\x49\xb9")
            emitter.raw(endpoint.lo.to_bytes(4, "little") + endpoint.hi.to_bytes(4, "little"))
            emitter.raw(b"\x4c\x39\xc8")
            self.trap_jump(emitter, condition)

    def scalar_leaves(self, type_id: int, base: int = 0):
        kind, payload0, payload1 = (self.types[type_id][1],
                                    self.types[type_id][4], self.types[type_id][5])
        if kind in (1, 2, 3, 8):
            yield type_id, base
        elif kind == 4:
            record = self.records[payload0]
            for field_id in range(record[2], record[2] + record[3]):
                yield from self.scalar_leaves(
                    self.fields[field_id][3], base + self.module.field_offsets[field_id]
                )
        elif kind == 5:
            stride = self.module.layouts[payload0][0]
            for index in range(payload1):
                yield from self.scalar_leaves(payload0, base + index * stride)
        else:
            yield from super().scalar_leaves(type_id, base)

    def _emit_leaf_copy(self, emitter: elf2.TextEmitter, type_id: int,
                        source_base: int, destination_base: int) -> None:
        for leaf_type, relative in self.scalar_leaves(type_id):
            source, destination = source_base + relative, destination_base + relative
            kind = self.types[leaf_type][1]
            if kind == 8:
                emitter.raw(b"\x49\x8b\x83"); emitter.u32(source)
                emitter.raw(b"\x49\x89\x82"); emitter.u32(destination)
            elif kind == 2:
                emitter.raw(b"\x41\x8b\x83"); emitter.u32(source)
                emitter.raw(b"\x41\x89\x82"); emitter.u32(destination)
            else:
                emitter.raw(b"\x41\x0f\xb6\x83"); emitter.u32(source)
                emitter.raw(b"\x41\x88\x82"); emitter.u32(destination)

    def _emit_type_copy(self, emitter: elf2.TextEmitter, type_id: int,
                        source_base: int, destination_base: int) -> None:
        if self.types[type_id][1] == 8:
            self._emit_leaf_copy(emitter, type_id, source_base, destination_base)
        else:
            super()._emit_type_copy(emitter, type_id, source_base, destination_base)

    def emit_operation(self, emitter: elf2.TextEmitter,
                       operation: tuple[int, ...]) -> None:
        opcode, result, result_type = operation[3], operation[6], operation[7]
        args = self.operands[operation[8]:operation[8] + operation[9]]
        if opcode == 1 and self.types[result_type][1] == 8:
            emitter.raw(b"\x48\xb8")
            emitter.raw(operation[10].to_bytes(4, "little")
                        + operation[11].to_bytes(4, "little"))
            self.store_value(emitter, result)
        elif opcode == 5 and self.types[result_type][1] == 8:
            self.load_place(emitter, args[0]); emitter.raw(b"\x48\x8b\x00")
            self.store_value(emitter, result)
        elif opcode == 6 and self.types[self.module.place_types[args[0]]][1] == 8:
            type_id = self.module.place_types[args[0]]
            self.load_place(emitter, args[0]); emitter.raw(b"\x49\x89\xc2")
            self.load_value(emitter, args[1]); self.range_check(emitter, type_id)
            emitter.raw(b"\x49\x89\x02")
        elif opcode == 9 and self._u64_value(args[0]):
            self.load_value(emitter, args[0]); emitter.raw(b"\x48\x3b\x85")
            emitter.s32(-self.value_slot(args[1]))
            emitter.raw(b"\x0f\x92\xc0\x0f\xb6\xc0")
            self.store_value(emitter, result)
        elif opcode == 10:
            call_args = args[1:]
            for ordinal, value_id in enumerate(call_args):
                displacement = -(self.scratch_base + (len(call_args) - ordinal) * 8)
                if self.types[self.module.value_types[value_id]][1] in (1, 2, 3, 8):
                    self.load_value(emitter, value_id)
                    emitter.raw(b"\x48\x89\x85" if self._u64_value(value_id)
                                else b"\x89\x85")
                    emitter.s32(displacement)
                else:
                    emitter.raw(b"\x48\x8b\x85"); emitter.s32(-self.value_slot(value_id))
                    emitter.raw(b"\x48\x89\x85"); emitter.s32(displacement)
            self.load_place(emitter, args[0]); emitter.raw(b"\x48\x89\xc7")
            if call_args:
                emitter.raw(b"\x48\x8d\xb5")
                emitter.s32(-(self.scratch_base + len(call_args) * 8))
            else:
                emitter.raw(b"\x31\xf6")
            emitter.byte(0xE8)
            target = None if emitter.block_offsets is None else \
                emitter.block_offsets[self.machines[operation[10]][10]]
            emitter.rel32(target)
            if operation[4] == 1:
                self.store_value(emitter, result)
        elif opcode == 13:
            emitter.raw(b"\x4c\x8d\x95")
            emitter.s32(-self.object_offsets[result])
            record = self.records[self.types[result_type][4]]
            for argument, field_id in zip(
                args, range(record[2], record[2] + record[3])
            ):
                field_type = self.fields[field_id][3]
                destination = self.module.field_offsets[field_id]
                kind = self.types[field_type][1]
                if kind in (1, 2, 3, 8):
                    self.load_value(emitter, argument); self.range_check(emitter, field_type)
                    if kind == 8:
                        emitter.raw(b"\x49\x89\x82")
                    elif kind == 2:
                        emitter.raw(b"\x41\x89\x82")
                    else:
                        emitter.raw(b"\x41\x88\x82")
                    emitter.u32(destination)
                else:
                    emitter.raw(b"\x4c\x8b\x9d"); emitter.s32(-self.value_slot(argument))
                    self._emit_leaf_copy(emitter, field_type, 0, destination)
            emitter.raw(b"\x4c\x89\x95"); emitter.s32(-self.value_slot(result))
        else:
            super().emit_operation(emitter, operation)

    def edge_size(self, start: int, count: int) -> int:
        size = 5
        for value_id in self.operands[start:start + count]:
            kind = self.types[self.module.value_types[value_id]][1]
            size += 66 if kind == 8 else 46 if kind <= 3 else 28
        return size

    def emit_edge(self, emitter: elf2.TextEmitter, start: int, count: int,
                  target: int, *, emit_jump: bool = True) -> None:
        arguments = self.operands[start:start + count]
        for ordinal, value_id in enumerate(arguments):
            scratch = self.scratch_base + (ordinal + 1) * 8
            kind = self.types[self.module.value_types[value_id]][1]
            if kind in (1, 2, 3, 8):
                self.load_value(emitter, value_id)
                emitter.raw(b"\x48\x89\x85" if kind == 8 else b"\x89\x85")
                emitter.s32(-scratch)
            else:
                emitter.raw(b"\x48\x8b\x85"); emitter.s32(-self.value_slot(value_id))
                emitter.raw(b"\x48\x89\x85"); emitter.s32(-scratch)
        parameter_start = self.blocks[target][5] if target < len(self.blocks) else 0
        for ordinal in range(count):
            value_id = self.block_params[parameter_start + ordinal][4]
            type_id = self.module.value_types[value_id]
            scratch = self.scratch_base + (ordinal + 1) * 8
            if self.types[type_id][1] in (1, 2, 3, 8):
                emitter.raw(b"\x48\x8b\x85" if self.types[type_id][1] == 8
                            else b"\x8b\x85")
                emitter.s32(-scratch)
                self.range_check(emitter, type_id); self.store_value(emitter, value_id)
            else:
                emitter.raw(b"\x48\x8b\x85"); emitter.s32(-scratch)
                emitter.raw(b"\x48\x89\x85"); emitter.s32(-self.value_slot(value_id))
        if emit_jump:
            emitter.byte(0xE9)
            destination = None if emitter.block_offsets is None else emitter.block_offsets[target]
            emitter.rel32(destination)

    def emit_block(self, emitter: elf2.TextEmitter, block_id: int) -> None:
        self.current_machine = self.blocks[block_id][1]
        machine = self.machines[self.current_machine]
        self.frame_size = self.frame_sizes[self.current_machine]
        self.scratch_base = self.scratch_bases[self.current_machine]
        if block_id == machine[10]:
            emitter.raw(b"\x55\x48\x89\xe5\x48\x81\xec"); emitter.u32(self.frame_size)
            emitter.raw(b"\x48\x89\xbd"); emitter.s32(-8)
            for ordinal, parameter_id in enumerate(range(machine[6], machine[6] + machine[7])):
                parameter = self.tables["machine_params"][parameter_id]
                value_id, type_id = parameter[4], parameter[3]
                kind = self.types[type_id][1]
                if kind in (1, 2, 3, 8):
                    emitter.raw(b"\x48\x8b\x86" if kind == 8 else b"\x8b\x86")
                    emitter.s32(ordinal * 8)
                    self.range_check(emitter, type_id); self.store_value(emitter, value_id)
                else:
                    emitter.raw(b"\x48\x8b\x86"); emitter.s32(ordinal * 8)
                    emitter.raw(b"\x48\x89\x85"); emitter.s32(-self.value_slot(value_id))
        block = self.blocks[block_id]
        for operation_id in range(block[7], block[7] + block[8]):
            self.emit_operation(emitter, self.operations[operation_id])
        self.emit_terminator(emitter, block_id)


def reconstruct(contents: bytes) -> bytes:
    try:
        return Reconstructor(decode(contents)).reconstruct()
    except V5.Ckir5ResourceError:
        raise
    except RefinementResourceError:
        raise
    except Exception as error:
        raise RefinementError(f"exact CKIR16 ELF reconstruction: {error}") from error
