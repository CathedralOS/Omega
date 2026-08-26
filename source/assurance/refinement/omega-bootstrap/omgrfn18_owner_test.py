#!/usr/bin/env python3
"""Fast responsibility-local controls for the modular OMGRFN18 family."""

from __future__ import annotations

import copy
import struct
import subprocess
import sys
import types
import unittest
from pathlib import Path

from omgrfn18_bundle import pack
from omgrfn18_ckir import decode, interpret, producer_decode
from omgrfn18_elf import Reconstructor, reconstruct
from omgrfn18_frame import HEADER, MAX_FRAME, split
from omgrfn18_profiles import ckir, ckir_tables, definitions, encode_ckir, profiles, source, witness
from omgrfn18_source import check_witness_relation, parse_selected_source
from omgrfn18_u64 import U64

HERE = Path(__file__).resolve().parent
OWNERS = ("r1", "r2", "r3", "r4-lowering", "r4-source-result",
          "r5-structure", "r5-result", "r5-elf")


class Owners(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.frames = profiles()
        cls.frame = cls.frames["borrow"]
        cls.parts = split(cls.frame)

    def owner(self, name: str, frame: bytes | None = None) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, str(HERE / f"omgrfn18-{name}.py")],
            input=self.frame if frame is None else frame,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=5,
        )

    def repack(self, *, omgcomp: bytes | None = None, witness_: bytes | None = None,
               ckir_: bytes | None = None, elf: bytes | None = None,
               result: int | None = None) -> bytes:
        return pack(
            self.parts.omgcomp if omgcomp is None else omgcomp,
            self.parts.witness if witness_ is None else witness_,
            self.parts.ckir if ckir_ is None else ckir_,
            self.parts.elf if elf is None else elf,
            self.parts.result if result is None else result,
        )

    def test_all_responsibilities_accept_all_unsigned_boundaries(self) -> None:
        for profile, frame in self.frames.items():
            for owner in OWNERS:
                with self.subTest(profile=profile, owner=owner):
                    observed = self.owner(owner, frame)
                    self.assertEqual(observed.returncode, 0, observed.stderr)
                    self.assertEqual(observed.stdout, b"")

    def test_exact_two_word_order_and_predecessor_borrow(self) -> None:
        self.assertTrue(U64(0xFFFF_FFFF, 1).less(U64(0, 2)))
        self.assertEqual(U64(0, 2).predecessor(), U64(0xFFFF_FFFF, 1))
        self.assertFalse(U64(0, 0x8000_0000).less(U64(0xFFFF_FFFF, 0x7FFF_FFFF)))

    def test_source_purity_and_transport_shape(self) -> None:
        from omgrfn18_profiles import shared

        authored = source(*definitions()["borrow"])
        effectful = authored.replace(b"self.stored < 8589934592",
                                     b"self.echo(self.stored) < 8589934592")
        with self.assertRaisesRegex(ValueError, "direct pure u64 Less"):
            parse_selected_source(shared.encode(effectful))
        omitted_call = authored.replace(b"self.echo(value)", b"value           ")
        with self.assertRaisesRegex(ValueError, "Call result to storage"):
            parse_selected_source(shared.encode(omitted_call))

    def test_frame_and_result_controls_are_local(self) -> None:
        self.assertEqual(self.owner("r1", b"OMGRFNX\0" + self.frame[8:]).returncode, 251)
        changed = bytearray(self.frame); struct.pack_into("<I", changed, 16, 0x0008_0001)
        self.assertEqual(self.owner("r1", bytes(changed)).returncode, 252)
        self.assertEqual(self.owner("r1", self.frame + bytes(MAX_FRAME + 1)).returncode, 252)
        wrong = self.repack(result=71)
        self.assertEqual(self.owner("r4-source-result", wrong).returncode, 251)
        self.assertEqual(self.owner("r5-result", wrong).returncode, 251)

        exhausted = bytearray(self.parts.ckir)
        struct.pack_into("<I", exhausted, 24, 8_193)
        resource_frame = self.repack(ckir_=bytes(exhausted))
        self.assertEqual(self.owner("r3", resource_frame).returncode, 252)
        self.assertEqual(self.owner("r5-structure", resource_frame).returncode, 252)

    def test_witness_kind_policy_and_endpoint_words(self) -> None:
        raw = bytearray(self.parts.witness)
        needle = struct.pack("<IBBHIIII", 3, 10, 0, 0, 0, 0, 0xFFFF_FFFF, 1)
        at = raw.find(needle)
        self.assertGreaterEqual(at, 0)
        raw[at + 5] = 1
        self.assertEqual(self.owner("r2", self.repack(witness_=bytes(raw))).returncode, 251)

        raw = bytearray(self.parts.witness); at = raw.find(needle)
        struct.pack_into("<I", raw, at + 16, 0xFFFF_FFFE)
        # Still a valid interval, but no longer the authored constrained type.
        changed = self.repack(witness_=bytes(raw))
        with self.assertRaisesRegex(ValueError, "authored u64 declaration"):
            check_witness_relation(self.parts.omgcomp, bytes(raw))
        self.assertEqual(self.owner("r2", changed).returncode, 251)
        self.assertEqual(self.owner("r4-lowering", changed).returncode, 0)

    def test_ckir_limb_opcode_order_and_fact_controls(self) -> None:
        stored, ceiling = definitions()["borrow"]
        base = ckir_tables(stored, ceiling)

        changed = copy.deepcopy(base)
        changed["operations"][5] = changed["operations"][5][:7] + (4,) + \
            changed["operations"][5][8:]
        malformed = encode_ckir(changed)
        self.assertEqual(self.owner("r3", self.repack(ckir_=malformed)).returncode, 251)

        changed = copy.deepcopy(base)
        changed["operations"][5] = tuple(changed["operations"][5][:11]) + (3,)
        limb_drift = encode_ckir(changed)
        self.assertIsNotNone(producer_decode(limb_drift))
        self.assertEqual(self.owner("r4-lowering", self.repack(ckir_=limb_drift)).returncode, 251)

        changed = copy.deepcopy(base)
        changed["operands"][4], changed["operands"][5] = \
            changed["operands"][5], changed["operands"][4]
        reordered = encode_ckir(changed)
        self.assertIsNotNone(producer_decode(reordered))
        self.assertEqual(self.owner("r4-lowering", self.repack(ckir_=reordered)).returncode, 251)

        changed = copy.deepcopy(base)
        target = changed["types"][4]
        changed["types"][4] = target[:6] + (0xFFFF_FFFE, target[7])
        off_by_one = encode_ckir(changed)
        self.assertIsNotNone(decode(off_by_one))
        self.assertEqual(self.owner("r4-lowering", self.repack(ckir_=off_by_one)).returncode, 251)

        changed = copy.deepcopy(base)
        term = changed["terminators"][0]
        changed["terminators"][0] = (
            *term[:7], 2, 12, 0, 1, 12, 1, *term[13:]
        )
        false_fact = encode_ckir(changed)
        self.assertIsNotNone(decode(false_fact))
        self.assertEqual(self.owner("r4-lowering", self.repack(ckir_=false_fact)).returncode, 251)

        changed = copy.deepcopy(base); changed["operands"][12] = (2,)
        wrong_identity = encode_ckir(changed)
        self.assertIsNotNone(decode(wrong_identity))
        self.assertEqual(self.owner("r4-lowering", self.repack(ckir_=wrong_identity)).returncode, 251)

    def test_independent_result_and_exact_elf_controls(self) -> None:
        for name, (left, right) in definitions().items():
            module = decode(ckir(left, right))
            self.assertEqual(interpret(module), 70 if left < right else 0, name)
        changed = self.parts.elf[:-1] + bytes([self.parts.elf[-1] ^ 1])
        self.assertEqual(self.owner("r5-elf", self.repack(elf=changed)).returncode, 251)
        artifact = reconstruct(self.parts.ckir)
        self.assertIn(b"\x48\xb8", artifact)       # movabs constant
        self.assertIn(b"\x48\x3b\x85", artifact)  # qword cmp
        self.assertIn(b"\x0f\x92\xc0", artifact)  # unsigned setb
        self.assertIn(b"\x49\xb9", artifact)       # movabs range endpoint

    def test_edge_size_does_not_consult_block_zero_parameter_partition(self) -> None:
        owner = Reconstructor.__new__(Reconstructor)
        owner.operands = [0]
        owner.types = [(0, 8, 0, 0, 0, 0, 0xFFFF_FFFF, 0xFFFF_FFFF)]
        owner.module = types.SimpleNamespace(value_types=(0,))
        owner.blocks = []
        owner.block_params = []
        self.assertEqual(owner.edge_size(0, 1), 71)


if __name__ == "__main__":
    unittest.main()
