#!/usr/bin/env python3
"""Independent CKIR4-to-ELF reconstruction and exact artifact checker."""

from __future__ import annotations

import argparse
from pathlib import Path

import checked_elf_v2_reference as elf2
import checked_elf_v3_reference as elf3
import checked_ir_v4_reference as ir4


NO_ID = 0xFFFF_FFFF


class ArtifactError(elf3.ArtifactError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ArtifactError(message)


class Reconstructor(elf3.Reconstructor):
    def __init__(self, module) -> None:
        super().__init__(module)
        self.constructor_offsets: dict[int, int] = {}

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

            constructor_values = sorted(
                operation[6]
                for operation in self.operations
                if operation[1] == machine_id and operation[3] == 13
            )
            for value_id in constructor_values:
                type_id = self.module.value_types[value_id]
                size, alignment = self.module.layouts[type_id]
                extent = max(size, 1)
                cursor = elf3.align(cursor + extent, alignment)
                require(cursor <= 262_144, "constructor object frame exhaustion")
                self.constructor_offsets[value_id] = cursor

            scratch_count = max(
                [
                    max(self.terminators[block_id][9], self.terminators[block_id][12])
                    for block_id in self._machine_blocks(machine_id)
                ]
                + [
                    operation[9] - 1
                    for operation in self.operations
                    if operation[1] == machine_id and operation[3] == 10
                ]
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

    def _emit_leaf_copy(
        self,
        emitter: elf2.TextEmitter,
        type_id: int,
        source_base: int,
        destination_base: int,
    ) -> None:
        for leaf_type, relative in self.scalar_leaves(type_id):
            source = source_base + relative
            destination = destination_base + relative
            if self.types[leaf_type][1] == 2:
                emitter.raw(b"\x41\x8b\x83")
                emitter.u32(source)
                emitter.raw(b"\x41\x89\x82")
                emitter.u32(destination)
            else:
                emitter.raw(b"\x41\x0f\xb6\x83")
                emitter.u32(source)
                emitter.raw(b"\x41\x88\x82")
                emitter.u32(destination)

    def emit_operation(self, emitter: elf2.TextEmitter, operation: tuple[int, ...]) -> None:
        if operation[3] != 13:
            super().emit_operation(emitter, operation)
            return
        result_id, result_type = operation[6], operation[7]
        arguments = self.operands[operation[8]:operation[8] + operation[9]]
        object_offset = self.constructor_offsets[result_id]
        emitter.raw(b"\x4c\x8d\x95")
        emitter.s32(-object_offset)
        record = self.records[self.types[result_type][4]]
        for argument, field_id in zip(
            arguments, range(record[2], record[2] + record[3])
        ):
            field_type = self.fields[field_id][3]
            destination = self.module.field_offsets[field_id]
            if self.types[field_type][1] <= 3:
                self.load_value(emitter, argument)
                self.range_check(emitter, field_type)
                emitter.raw(
                    b"\x41\x89\x82"
                    if self.types[field_type][1] == 2
                    else b"\x41\x88\x82"
                )
                emitter.u32(destination)
            else:
                emitter.raw(b"\x4c\x8b\x9d")
                emitter.s32(-self.value_slot(argument))
                self._emit_leaf_copy(emitter, field_type, 0, destination)
        emitter.raw(b"\x4c\x89\x95")
        emitter.s32(-self.value_slot(result_id))


def reconstruct(ckir: bytes) -> bytes:
    return Reconstructor(ir4.decode(ckir)).reconstruct()


def mismatch(expected: bytes, actual: bytes) -> str | None:
    return elf3.mismatch(expected, actual)


def check(ckir_path: Path, elf_path: Path) -> tuple[bytes, bytes]:
    expected = reconstruct(ckir_path.read_bytes())
    actual = elf_path.read_bytes()
    problem = mismatch(expected, actual)
    require(problem is None, f"artifact mismatch at {problem}")
    return expected, actual


def mutation_sweep(expected: bytes, actual: bytes) -> None:
    elf3.mutation_sweep(expected, actual)


def main() -> None:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    emit = subparsers.add_parser("emit")
    emit.add_argument("ckir", type=Path)
    emit.add_argument("elf", type=Path)
    for command in ("check", "mutation-sweep"):
        item = subparsers.add_parser(command)
        item.add_argument("ckir", type=Path)
        item.add_argument("elf", type=Path)
    args = parser.parse_args()
    if args.command == "emit":
        artifact = reconstruct(args.ckir.read_bytes())
        args.elf.write_bytes(artifact)
        print(f"CKIR4 ELF emitted: {len(artifact)} bytes")
        return
    expected, actual = check(args.ckir, args.elf)
    if args.command == "mutation-sweep":
        mutation_sweep(expected, actual)
    print(
        f"CKIR4 ELF reconstructed: {len(actual)} bytes"
        + (f", {len(actual)} byte mutations rejected"
           if args.command == "mutation-sweep" else "")
    )


if __name__ == "__main__":
    try:
        main()
    except ir4.Ckir4ResourceError as error:
        print(f"checked ELF v4 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(252)
    except (ArtifactError, elf3.ArtifactError, ir4.Ckir4Error, OSError) as error:
        print(f"checked ELF v4 reference: {error}", file=__import__("sys").stderr)
        raise SystemExit(251)
