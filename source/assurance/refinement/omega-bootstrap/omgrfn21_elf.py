#!/usr/bin/env python3
"""Independent exact CKIR18 conservative ELF reconstruction for OMGRFN21."""

from __future__ import annotations

from omgrfn18_elf import Reconstructor as U64Reconstructor
from omgrfn21_ckir import V5, arguments, decode
from omgrfn21_frame import RefinementError, RefinementResourceError, require


class Reconstructor(U64Reconstructor):
    """CKIR18 specialization reusing the audited OMGRFN18 qword owner.

    OMGRFN18 continues to own ordinary kind-8 constants, loads, stores, Less,
    calls, edge transport, range checks, layouts, and ELF orchestration.  This
    subclass adds only CKIR18's selected qword IndexPlace and Add templates.
    The production backend is neither imported nor executed.
    """

    def emit_operation(self, emitter, operation: tuple[int, ...]) -> None:
        opcode, result, result_type = operation[3], operation[6], operation[7]
        args = arguments(self.module, operation)
        if opcode == 4 and self._u64_value(args[1]):
            base_type = self.module.place_types[args[0]]
            require(self.types[base_type][1] == 5
                    and self.types[base_type][4] == result_type
                    and self.types[result_type][1] == 1,
                    "selected u64 IndexPlace exact-byte array")
            # Preserve the base in R10, compare the complete unsigned index
            # against an imm64 length, then add only after JAE cannot fire.
            self.load_place(emitter, args[0]); emitter.raw(b"\x49\x89\xc2")
            self.load_value(emitter, args[1]); emitter.raw(b"\x49\xb9")
            emitter.raw(self.types[base_type][5].to_bytes(8, "little"))
            emitter.raw(b"\x4c\x39\xc8")
            self.trap_jump(emitter, 0x83)
            emitter.raw(b"\x49\x01\xc2\x4c\x89\xd0")
            self.store_place(emitter, result)
        elif opcode == 8 and self.types[result_type][1] == 8:
            self.load_value(emitter, args[0]); emitter.raw(b"\x48\x03\x85")
            emitter.s32(-self.value_slot(args[1]))
            self.trap_jump(emitter, 0x82)
            self.range_check(emitter, result_type)
            self.store_value(emitter, result)
        else:
            super().emit_operation(emitter, operation)


def reconstruct(contents: bytes) -> bytes:
    try:
        return Reconstructor(decode(contents)).reconstruct()
    except V5.Ckir5ResourceError:
        raise
    except RefinementResourceError:
        raise
    except Exception as error:
        raise RefinementError(f"exact CKIR18 ELF reconstruction: {error}") from error
