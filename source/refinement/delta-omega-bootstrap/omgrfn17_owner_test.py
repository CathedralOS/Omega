#!/usr/bin/env python3
"""Fast responsibility-local tests for the fresh OMGRFN17 Python family."""

from __future__ import annotations

import struct
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from omgrfn17_ckir import decode, producer_decode
from omgrfn17_frame import HEADER
from omgrfn17_profiles import load_fixture, profiles, source, witness
from omgrfn17_source import check_witness_relation, parse_selected_source

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
GATES = REPO / "source/on-ramp/omega-bootstrap/gates"
sys.path[:0] = [str(GATES), str(REPO / "source/on-ramp/omega-bootstrap/compiler")]
import shared_byte_view_resolution_fixture as shared  # noqa: E402


class Owners(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixture = load_fixture()
        cls.frames = profiles()
        cls.frame = cls.frames["one-byte"]
        split = HEADER.unpack_from(cls.frame)
        start = HEADER.size
        cls.omgcomp = cls.frame[start:start + split[3]]; start += split[3]
        cls.witness = cls.frame[start:start + split[4]]; start += split[4]
        cls.ckir = cls.frame[start:start + split[5]]

    def owner(self, name: str, frame: bytes | None = None) -> subprocess.CompletedProcess:
        return subprocess.run([sys.executable, str(HERE / f"omgrfn17-{name}.py")],
                              input=self.frame if frame is None else frame,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=5)

    def test_all_responsibilities_accept_same_frame(self):
        for profile, frame in self.frames.items():
            for name in ("r1", "r2", "r3", "r4-lowering", "r4-source-result",
                         "r5-structure", "r5-result", "r5-elf"):
                with self.subTest(profile=profile, name=name):
                    observed = self.owner(name, frame)
                    self.assertEqual(observed.returncode, 0, observed.stderr)
                    self.assertEqual(observed.stdout, b"")

    def test_frame_and_witness_local_mutations(self):
        wrong_magic = b"OMGRFNX\0" + self.frame[8:]
        self.assertEqual(self.owner("r1", wrong_magic).returncode, 251)
        selected_source = source("F")
        changed = bytearray(self.witness)
        # Selected machine declaration now spans the owner's name.
        declaration_table = 84 + 36
        struct.pack_into("<II", changed, declaration_table + 28 + 16,
                         selected_source.index(b"Probe"), 5)
        with self.assertRaisesRegex(ValueError, "selected machine"):
            check_witness_relation(self.omgcomp, bytes(changed))

    def test_source_direct_binder_and_result_mutations(self):
        self.assertEqual(parse_selected_source(self.omgcomp).result, 70)
        selected_source = source("F")
        wrong = selected_source.replace(b"head: u8", b"head:u32 ")
        with self.assertRaisesRegex(ValueError, "head/tail target types"):
            parse_selected_source(shared.encode(wrong))
        frame = bytearray(self.frame)
        struct.pack_into("<I", frame, 32, 71)
        struct.pack_into("<I", frame, 36, 71)
        self.assertEqual(self.owner("r4-source-result", bytes(frame)).returncode, 251)
        self.assertEqual(self.owner("r5-result", bytes(frame)).returncode, 251)

    def test_ckir_and_elf_local_mutations(self):
        bad_elf = self.frame[:-1] + bytes([self.frame[-1] ^ 1])
        self.assertEqual(self.owner("r5-elf", bad_elf).returncode, 251)
        with tempfile.TemporaryDirectory() as directory:
            self.fixture.emit(Path(directory))
            malformed = (Path(directory) / "pass-through-reordered.ckir15").read_bytes()
        start = HEADER.size + len(self.omgcomp) + len(self.witness)
        end = start + len(self.ckir)
        changed = self.frame[:start] + malformed + self.frame[end:]
        self.assertEqual(self.owner("r3", changed).returncode, 251)
        self.assertEqual(self.owner("r5-structure", changed).returncode, 251)

    def test_runtime_origin_and_optional_arithmetic_remain_independent(self):
        runtime = self.fixture.encode(self.fixture.runtime_parameter_tables(), entry=0xFFFF_FFFF)
        arithmetic = self.fixture.encode(self.fixture.arithmetic_composition_tables(), values=31)
        self.assertIsNotNone(producer_decode(runtime))
        self.assertIsNotNone(decode(runtime))
        self.assertIsNotNone(producer_decode(arithmetic))
        self.assertIsNotNone(decode(arithmetic))


if __name__ == "__main__":
    unittest.main()
