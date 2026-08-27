#!/usr/bin/env python3
"""Independent exact CKIR20 conservative ELF reconstruction for OMGRFN23."""

from __future__ import annotations

import omgrfn16_elf_reference as legacy
from omgrfn21_elf import Reconstructor as FixedBufferReconstructor
from omgrfn23_ckir import NO_ID, V5, arguments, meaning_decode
from omgrfn23_frame import RefinementError, RefinementResourceError, require


class Reconstructor(FixedBufferReconstructor):
    """Add checked full-u64 indexing for both selected record arrays.

    All scalar, structural Copy, sum, call, edge, and ELF rules remain owned by
    the older audited reconstruction.  This class owns the only CKIR20 address
    delta: deriving either selected record stride and checking the multiply and
    address addition before publishing a place.
    """

    def emit_operation(self, emitter, operation: tuple[int, ...]) -> None:
        opcode, result, result_type = operation[3], operation[6], operation[7]
        args = arguments(self.module, operation)
        if opcode == 4 and len(args) == 2 and self._u64_value(args[1]):
            base_type = self.module.place_types[args[0]]
            element_type = self.types[base_type][4]
            require(self.types[base_type][1] == 5
                    and element_type == result_type
                    and self.types[result_type][1] in (4, 6),
                    "selected u64 IndexPlace exact aggregate array")
            length = self.types[base_type][5]
            stride = self.module.layouts[element_type][0]
            require(length == 16_384 and stride in (40, 56),
                    "selected observation/token length and stride")
            self.load_place(emitter, args[0]); emitter.raw(b"\x49\x89\xc2")
            self.load_value(emitter, args[1]); emitter.raw(b"\x49\xb9")
            emitter.raw(length.to_bytes(8, "little"))
            emitter.raw(b"\x4c\x39\xc8")
            self.trap_jump(emitter, 0x83)  # unsigned JAE
            emitter.raw(b"\x48\x69\xc0"); emitter.u32(stride)
            self.trap_jump(emitter, 0x80)  # signed JO
            emitter.raw(b"\x49\x01\xc2")
            self.trap_jump(emitter, 0x82)  # unsigned JB
            emitter.raw(b"\x4c\x89\xd0")
            self.store_place(emitter, result)
        else:
            super().emit_operation(emitter, operation)

    def reconstruct(self) -> bytes:
        require(self.module.entry != NO_ID, "OMGRFN23 requires entry artifact")
        self.assign_frame()
        owner_type = self.records[self.machines[self.module.entry][1]][1]
        owner_size = self.module.layouts[owner_type][0]
        require(owner_size <= 2 * 1024 * 1024,
                "entry owner layout exhaustion")
        constant_image, root_offsets = self.constant_image()
        first_text, block_offsets = self.emit_text(None, None, None)
        require(len(first_text) <= 1_048_576, "text exhaustion")
        rx_size = legacy.elf3.align(legacy.PAGE + len(first_text), legacy.PAGE)
        ro_size = (legacy.elf3.align(len(constant_image), legacy.PAGE)
                   if root_offsets else 0)
        bss_offset = rx_size + ro_size
        targets = {root: rx_size + offset - legacy.PAGE
                   for root, offset in root_offsets.items()}
        text, rediscovered = self.emit_text(bss_offset, block_offsets, targets)
        require(len(text) == len(first_text) and rediscovered == block_offsets,
                "stable exact text reconstruction")
        bss_size = legacy.elf3.align(max(owner_size, 1), legacy.PAGE)
        program_count = 3 if root_offsets else 2
        ident = b"\x7fELF\x02\x01\x01" + bytes(9)
        image = bytearray(legacy.elf2.ELF_HEADER.pack(
            ident, 2, 62, 1, legacy.IMAGE_BASE + legacy.PAGE,
            legacy.elf2.ELF_HEADER.size, 0, 0, legacy.elf2.ELF_HEADER.size,
            legacy.elf2.PROGRAM_HEADER.size, program_count, 0, 0, 0,
        ))
        image.extend(legacy.elf2.PROGRAM_HEADER.pack(
            1, 5, 0, legacy.IMAGE_BASE, legacy.IMAGE_BASE,
            rx_size, rx_size, legacy.PAGE,
        ))
        if root_offsets:
            image.extend(legacy.elf2.PROGRAM_HEADER.pack(
                1, 4, rx_size, legacy.IMAGE_BASE + rx_size,
                legacy.IMAGE_BASE + rx_size, ro_size, ro_size, legacy.PAGE,
            ))
        image.extend(legacy.elf2.PROGRAM_HEADER.pack(
            1, 6, bss_offset, legacy.IMAGE_BASE + bss_offset,
            legacy.IMAGE_BASE + bss_offset, 0, bss_size, legacy.PAGE,
        ))
        image.extend(bytes(legacy.PAGE - len(image))); image.extend(text)
        image.extend(bytes(rx_size - len(image)))
        if root_offsets:
            image.extend(constant_image)
            image.extend(bytes(ro_size - len(constant_image)))
        if len(image) > 1_183_744:
            raise RefinementResourceError("ELF byte exhaustion")
        return bytes(image)


def reconstruct(contents: bytes) -> bytes:
    try:
        return Reconstructor(meaning_decode(contents)).reconstruct()
    except V5.Ckir5ResourceError:
        raise
    except RefinementResourceError:
        raise
    except Exception as error:
        raise RefinementError(f"exact CKIR20 ELF reconstruction: {error}") from error
