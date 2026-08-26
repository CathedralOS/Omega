#!/usr/bin/env python3
"""Independent CKIR14-to-ELF exact reconstruction for OMGRFN16."""

from __future__ import annotations

import sys
from pathlib import Path

from omgrfn16_ckir import IR14, V5
from omgrfn16_frame import RefinementError, RefinementResourceError, require


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
GATES = REPO / "bootstrap/omega-bootstrap/gates"
sys.path.insert(0, str(GATES))

import checked_elf_v2_reference as elf2  # noqa: E402
import checked_elf_v3_reference as elf3  # noqa: E402
import checked_elf_v4_reference as elf4  # noqa: E402


NO_ID = 0xFFFF_FFFF
PAGE = 4096
IMAGE_BASE = 0x400000


class Reconstructor(elf4.Reconstructor):
    def __init__(self, module) -> None:
        super().__init__(module)
        self.object_offsets: dict[int, int] = {}
        self.constant_targets: dict[int, int] | None = None

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
                    self.reachable.add(callee); queue.append(callee)

        for machine_id in sorted(self.reachable):
            cursor = 8
            for value_id, type_id in enumerate(self.module.value_types):
                if self.value_machines[value_id] != machine_id:
                    continue
                width = 4 if self.types[type_id][1] <= 3 else 8
                cursor = elf3.align(cursor, width) + width
                require(cursor <= 262_144, "value frame exhaustion")
                self.value_slots[value_id] = cursor
            for place_id, block_id in enumerate(self.place_blocks):
                if self.blocks[block_id][1] != machine_id:
                    continue
                cursor = elf3.align(cursor, 8) + 8
                require(cursor <= 262_144, "place frame exhaustion")
                self.place_slots[place_id] = cursor
            for operation in self.operations:
                if operation[1] != machine_id or operation[3] not in (13, 14, 22, 25):
                    continue
                value_id, type_id = operation[6], operation[7]
                size, alignment = self.module.layouts[type_id]
                cursor = elf3.align(cursor + max(size, 1), alignment)
                require(cursor <= 262_144, "operation object frame exhaustion")
                self.object_offsets[value_id] = cursor
                if operation[3] == 13:
                    self.constructor_offsets[value_id] = cursor
            scratch_count = max(
                [max(self.terminators[block_id][9], self.terminators[block_id][12])
                 for block_id in self._machine_blocks(machine_id)]
                + [operation[9] - 1 for operation in self.operations
                   if operation[1] == machine_id and operation[3] == 10]
                + [0]
            )
            cursor = elf3.align(cursor, 8)
            self.scratch_bases[machine_id] = cursor
            cursor += scratch_count * 8
            frame_size = elf3.align(cursor, 16)
            require(frame_size <= 262_144, "frame exhaustion")
            self.frame_sizes[machine_id] = frame_size

        indegree = [0] * len(self.machines)
        for caller, callees in enumerate(graph):
            for callee in callees:
                indegree[callee] += 1
        pending = [index for index, degree in enumerate(indegree) if degree == 0]
        topo: list[int] = []
        while pending:
            caller = min(pending); pending.remove(caller); topo.append(caller)
            for callee in sorted(graph[caller]):
                indegree[callee] -= 1
                if indegree[callee] == 0:
                    pending.append(callee)
        require(len(topo) == len(self.machines), "cyclic call graph")
        live = {self.module.entry: self.frame_sizes[self.module.entry] + 16}
        for caller in topo:
            if caller not in live:
                continue
            require(live[caller] <= 262_144, "live stack exhaustion")
            for callee in graph[caller]:
                live[callee] = max(live.get(callee, 0),
                                   live[caller] + self.frame_sizes[callee] + 16)
        require(all(value <= 262_144 for value in live.values()), "live stack exhaustion")
        self.frame_size = self.frame_sizes[self.module.entry]
        self.scratch_base = self.scratch_bases[self.module.entry]

    def emit_operation(self, emitter: elf2.TextEmitter, operation: tuple[int, ...]) -> None:
        opcode = operation[3]
        args = self.operands[operation[8]:operation[8] + operation[9]]
        result = operation[6]
        if opcode == 15:
            self.load_value(emitter, args[0]); emitter.raw(b"\x83\xf0\x01")
            self.store_value(emitter, result)
        elif opcode in (16, 17):
            self.load_value(emitter, args[0])
            emitter.raw(b"\x23\x85" if opcode == 16 else b"\x0b\x85")
            emitter.s32(-self.value_slot(args[1])); self.store_value(emitter, result)
        elif opcode in (18, 19, 20):
            self.load_value(emitter, args[0]); emitter.raw(b"\x3b\x85")
            emitter.s32(-self.value_slot(args[1]))
            emitter.raw({18: b"\x0f\x94\xc0", 19: b"\x0f\x97\xc0",
                         20: b"\x0f\x93\xc0"}[opcode])
            emitter.raw(b"\x0f\xb6\xc0"); self.store_value(emitter, result)
        elif opcode == 21:
            self.load_value(emitter, args[0]); emitter.raw(b"\x0f\xb6\xc0")
            self.store_value(emitter, result)
        elif opcode in (26, 27):
            self.load_value(emitter, args[0])
            if opcode == 26:
                emitter.raw(b"\x2b\x85"); emitter.s32(-self.value_slot(args[1]))
                self.trap_jump(emitter, 0x82)
            else:
                emitter.raw(b"\xf7\xa5"); emitter.s32(-self.value_slot(args[1]))
                emitter.raw(b"\x85\xd2"); self.trap_jump(emitter, 0x85)
            self.range_check(emitter, operation[7]); self.store_value(emitter, result)
        elif opcode == 22:
            target = (None if self.constant_targets is None
                      else self.constant_targets[operation[10]])
            emitter.raw(b"\x4c\x8d\x9d"); emitter.s32(-self.object_offsets[result])
            emitter.raw(b"\x48\x8d\x05"); emitter.rel32(target)
            emitter.raw(b"\x49\x89\x03\xb8"); emitter.u32(self.tables["constants"][operation[10]][3])
            emitter.raw(b"\x49\x89\x43\x08")
            emitter.raw(b"\x4c\x89\x9d"); emitter.s32(-self.value_slot(result))
        elif opcode == 23:
            emitter.raw(b"\x4c\x8b\x9d"); emitter.s32(-self.value_slot(args[0]))
            emitter.raw(b"\x49\x83\x7b\x08\x00\x0f\x95\xc0\x0f\xb6\xc0")
            self.store_value(emitter, result)
        elif opcode == 24:
            emitter.raw(b"\x4c\x8b\x9d"); emitter.s32(-self.value_slot(args[0]))
            emitter.raw(b"\x49\x83\x7b\x08\x00"); self.trap_jump(emitter, 0x84)
            emitter.raw(b"\x49\x8b\x03\x0f\xb6\x00"); self.store_value(emitter, result)
        elif opcode == 25:
            emitter.raw(b"\x4c\x8b\x9d"); emitter.s32(-self.value_slot(args[0]))
            emitter.raw(b"\x4c\x8d\x95"); emitter.s32(-self.object_offsets[result])
            emitter.raw(b"\x49\x83\x7b\x08\x00"); self.trap_jump(emitter, 0x84)
            emitter.raw(b"\x49\x8b\x03\x48\x83\xc0\x01\x49\x89\x02")
            emitter.raw(b"\x49\x8b\x43\x08\x48\x83\xe8\x01\x49\x89\x42\x08")
            emitter.raw(b"\x4c\x89\x95"); emitter.s32(-self.value_slot(result))
        else:
            super().emit_operation(emitter, operation)

    def emit_text(self, bss_offset, block_offsets, constant_targets):
        self.constant_targets = constant_targets
        return super().emit_text(bss_offset, block_offsets, constant_targets)

    def constant_image(self) -> tuple[bytes, dict[int, int]]:
        roots = sorted(
            {operation[10] for operation in self.operations if operation[3] in (11, 22)}
        )
        image = bytearray(); offsets: dict[int, int] = {}
        constants = self.tables["constants"]
        children = self.tables["constant_children"]
        for root_id in roots:
            root = constants[root_id]
            type_id = root[1]
            alignment = self.module.layouts[type_id][1]
            aligned = elf3.align(len(image), alignment)
            image.extend(bytes(aligned - len(image))); offsets[root_id] = len(image)
            if self.types[type_id][1] == 7:
                for child_id in range(root[2], root[2] + root[3]):
                    child = constants[children[child_id][0]]
                    image.append(child[4])
            else:
                image.extend(V5.v4.materialize_constant(self.module, root_id) or b"\0")
        require(len(image) <= 131_072, "constant image exhaustion")
        return bytes(image), offsets

    def reconstruct(self) -> bytes:
        require(self.module.entry != NO_ID, "OMGRFN16 requires entry artifact")
        self.assign_frame()
        owner_type = self.records[self.machines[self.module.entry][1]][1]
        owner_size = self.module.layouts[owner_type][0]
        require(owner_size <= 131_072, "entry owner layout exhaustion")
        constant_image, root_offsets = self.constant_image()
        first_text, block_offsets = self.emit_text(None, None, None)
        require(len(first_text) <= 1_048_576, "text exhaustion")
        rx_size = elf3.align(PAGE + len(first_text), PAGE)
        ro_size = elf3.align(len(constant_image), PAGE) if root_offsets else 0
        bss_offset = rx_size + ro_size
        targets = {root: rx_size + offset - PAGE for root, offset in root_offsets.items()}
        text, rediscovered = self.emit_text(bss_offset, block_offsets, targets)
        require(len(text) == len(first_text) and rediscovered == block_offsets,
                "stable exact text reconstruction")
        bss_size = elf3.align(max(owner_size, 1), PAGE)
        program_count = 3 if root_offsets else 2
        ident = b"\x7fELF\x02\x01\x01" + bytes(9)
        image = bytearray(elf2.ELF_HEADER.pack(
            ident, 2, 62, 1, IMAGE_BASE + PAGE, elf2.ELF_HEADER.size, 0, 0,
            elf2.ELF_HEADER.size, elf2.PROGRAM_HEADER.size, program_count, 0, 0, 0,
        ))
        image.extend(elf2.PROGRAM_HEADER.pack(
            1, 5, 0, IMAGE_BASE, IMAGE_BASE, rx_size, rx_size, PAGE,
        ))
        if root_offsets:
            image.extend(elf2.PROGRAM_HEADER.pack(
                1, 4, rx_size, IMAGE_BASE + rx_size, IMAGE_BASE + rx_size,
                ro_size, ro_size, PAGE,
            ))
        image.extend(elf2.PROGRAM_HEADER.pack(
            1, 6, bss_offset, IMAGE_BASE + bss_offset, IMAGE_BASE + bss_offset,
            0, bss_size, PAGE,
        ))
        image.extend(bytes(PAGE - len(image))); image.extend(text)
        image.extend(bytes(rx_size - len(image)))
        if root_offsets:
            image.extend(constant_image); image.extend(bytes(ro_size - len(constant_image)))
        if len(image) > 1_183_744:
            raise RefinementResourceError("ELF byte exhaustion")
        return bytes(image)


def reconstruct(ckir: bytes) -> bytes:
    try:
        return Reconstructor(IR14.decode(ckir)).reconstruct()
    except V5.Ckir5ResourceError:
        raise
    except RefinementResourceError:
        raise
    except Exception as error:
        raise RefinementError(f"exact artifact reconstruction: {error}") from error
