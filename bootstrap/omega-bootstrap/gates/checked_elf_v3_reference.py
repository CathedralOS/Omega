#!/usr/bin/env python3
"""Independent CKIR3-to-ELF reconstruction and exact artifact checker."""

from __future__ import annotations

import argparse
from pathlib import Path

import checked_elf_v2_reference as elf2
import checked_ir_v3_reference as ir3


NO_ID = 0xFFFF_FFFF
PAGE = 4096
IMAGE_BASE = 0x400000


class ArtifactError(elf2.ArtifactError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ArtifactError(message)


def align(value: int, alignment: int) -> int:
    return (value + alignment - 1) // alignment * alignment


class Reconstructor(elf2.Reconstructor):
    def __init__(self, module) -> None:
        super().__init__(module)
        self.constant_targets: dict[int, int] | None = None

    def emit_operation(self, emitter: elf2.TextEmitter, operation: tuple[int, ...]) -> None:
        opcode = operation[3]
        if opcode == 11:
            arguments = self.operands[operation[8]:operation[8] + operation[9]]
            root = operation[10]
            destination_type = self.module.place_types[arguments[0]]
            self.load_place(emitter, arguments[0])
            emitter.raw(b"\x49\x89\xc2\x48\x8d\x35")
            target = None if self.constant_targets is None else self.constant_targets[root]
            emitter.rel32(target)
            emitter.raw(b"\x4c\x89\xd7\xb9")
            emitter.u32(self.module.layouts[destination_type][0])
            emitter.raw(b"\xf3\xa4")
        elif opcode == 12:
            arguments = self.operands[operation[8]:operation[8] + operation[9]]
            self.load_value(emitter, arguments[0])
            emitter.raw(b"\x3b\x85")
            emitter.s32(-self.value_slot(arguments[1]))
            emitter.raw(b"\x0f\x96\xc0\x0f\xb6\xc0")
            self.store_value(emitter, operation[6])
        else:
            super().emit_operation(emitter, operation)

    def emit_text(
        self,
        bss_offset: int | None,
        block_offsets: dict[int, int] | None,
        constant_targets: dict[int, int] | None,
    ) -> tuple[bytes, dict[int, int]]:
        self.constant_targets = constant_targets
        emitter = elf2.TextEmitter(block_offsets)
        emitter.raw(b"\x48\x8d\x3d")
        emitter.rel32(None if bss_offset is None else bss_offset - PAGE)
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
        constant_image, root_offsets = ir3.constant_image(self.module)

        first_text, block_offsets = self.emit_text(None, None, None)
        require(len(first_text) <= 1_048_576, "text exhaustion")
        rx_size = align(PAGE + len(first_text), PAGE)
        ro_size = align(len(constant_image), PAGE) if root_offsets else 0
        bss_offset = rx_size + ro_size
        constant_targets = {
            root: rx_size + offset - PAGE
            for root, offset in root_offsets.items()
        }
        text, rediscovered = self.emit_text(bss_offset, block_offsets, constant_targets)
        require(len(text) == len(first_text), "unstable instruction sizing")
        require(rediscovered == block_offsets, "unstable block offsets")
        bss_size = align(max(owner_size, 1), PAGE)
        program_count = 3 if root_offsets else 2
        ident = b"\x7fELF\x02\x01\x01" + bytes(9)
        image = bytearray(
            elf2.ELF_HEADER.pack(
                ident, 2, 62, 1, IMAGE_BASE + PAGE, elf2.ELF_HEADER.size, 0, 0,
                elf2.ELF_HEADER.size, elf2.PROGRAM_HEADER.size, program_count,
                0, 0, 0,
            )
        )
        image.extend(elf2.PROGRAM_HEADER.pack(1, 5, 0, IMAGE_BASE, IMAGE_BASE, rx_size, rx_size, PAGE))
        if root_offsets:
            image.extend(
                elf2.PROGRAM_HEADER.pack(
                    1, 4, rx_size, IMAGE_BASE + rx_size, IMAGE_BASE + rx_size,
                    ro_size, ro_size, PAGE,
                )
            )
        image.extend(
            elf2.PROGRAM_HEADER.pack(
                1, 6, bss_offset, IMAGE_BASE + bss_offset,
                IMAGE_BASE + bss_offset, 0, bss_size, PAGE,
            )
        )
        require(len(image) == elf2.ELF_HEADER.size + program_count * elf2.PROGRAM_HEADER.size, "ELF envelope size")
        image.extend(bytes(PAGE - len(image)))
        image.extend(text)
        image.extend(bytes(rx_size - len(image)))
        if root_offsets:
            image.extend(constant_image)
            image.extend(bytes(ro_size - len(constant_image)))
        require(len(image) <= 1_183_744, "ELF byte exhaustion")
        return bytes(image)


def reconstruct(ckir: bytes) -> bytes:
    return Reconstructor(ir3.decode(ckir)).reconstruct()


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
    require(mismatch(expected, actual[:-1]) is not None, "truncation accepted")
    require(mismatch(expected, actual + b"\0") is not None, "trailing byte accepted")
    for offset in range(len(actual)):
        mutated = bytearray(actual)
        mutated[offset] ^= 1
        require(mismatch(expected, mutated) is not None, f"mutation at {offset} accepted")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("check", "mutation-sweep"))
    parser.add_argument("ckir", type=Path)
    parser.add_argument("elf", type=Path)
    args = parser.parse_args()
    expected, actual = check(args.ckir, args.elf)
    if args.command == "mutation-sweep":
        mutation_sweep(expected, actual)
    print(
        f"CKIR3 ELF reconstructed: {len(actual)} bytes"
        + (f", {len(actual)} byte mutations rejected" if args.command == "mutation-sweep" else "")
    )


if __name__ == "__main__":
    try:
        main()
    except (ArtifactError, elf2.ArtifactError, ir3.Ckir3Error, OSError) as error:
        raise SystemExit(f"checked ELF v3 reference: {error}")
