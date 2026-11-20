"""Check mixed route handoffs and the distinction between observations and refusals."""
import argparse
from itertools import product
from pathlib import Path
import struct
import tempfile
import unittest

from route import MODES, Program, Route, observation_kind


class RouteTests(unittest.TestCase):
    def test_each_plan_executes_the_selected_epsilon_target(self):
        for gamma, delta, epsilon in product(MODES, repeat=3):
            with self.subTest(gamma=gamma, delta=delta, epsilon=epsilon):
                args = argparse.Namespace(gamma=gamma, delta=delta, epsilon=epsilon,
                                          gamma_build=None, profile="conformance", seconds=1)
                route = Route(args, Path("/unused"))
                route.select()
                stages = [name for name, _ in route.steps]
                if epsilon == "interpret":
                    self.assertIn("interpret subject.epsilon", stages[-1])
                else:
                    target = "subject.vm" if epsilon == "alpha" else "subject.elf"
                    self.assertIn(f"[{target}]", stages[-1])
                    self.assertTrue(any("attach the runtime" in name for name in stages))
                self.assertEqual("assemble the direct Delta interpreter" in stages, delta == "interpret")
                self.assertEqual("rebuild Beta and the Gamma interpreter on Alpha" in stages,
                                 gamma == "interpret")

    def test_interpreters_consume_only_their_own_source(self):
        with tempfile.TemporaryDirectory() as temp:
            outer, inner = (Path(temp) / name for name in ("outer", "inner"))
            outer.write_bytes(b"outer")
            inner.write_bytes(b"\x00in")
            guest = b"\x00\xff"
            invocation = Program(Path("vm")).bind(outer).bind(inner)
            self.assertEqual(invocation.input(guest),
                             struct.pack("<I", 5) + b"outer" + struct.pack("<I", 3) + b"\x00in" + guest)

    def test_only_canonical_epsilon_observations_count_as_completed(self):
        self.assertEqual(observation_kind(b"\x00\xff\xff\xff\xffexit"), "exit")
        self.assertEqual(observation_kind(b"\x01\x09prefix"), "trap")
        self.assertEqual(observation_kind(b"\x02\x1a\x03\x00\x00\x00"), "reject")
        for invalid in (b"", b"\x00\x00", b"\x01", b"\x01\x0a", b"\x02\x00" + bytes(4),
                        b"\x02\x08" + bytes(5), b"\xffEEOUT\x01\x00" + bytes(32)):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                observation_kind(invalid)


if __name__ == "__main__":
    unittest.main()
