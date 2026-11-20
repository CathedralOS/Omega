"""Check mixed route handoffs and the distinction between observations and refusals."""
import argparse
from itertools import product
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch

from route import MODES, Program, Route, observation_kind


class RouteTests(unittest.TestCase):
    def test_matrix_reuses_only_identical_producer_invocations_and_runs_each_guest(self):
        with tempfile.TemporaryDirectory() as temp, patch("route.invoke", return_value=b"output") as invoke:
            root = Path(temp)
            shared = {}

            def cell(name, image, source=b"compiler", payload=b"input", guest=False):
                work = root / name
                work.mkdir()
                executable, subject = work / "host", work / "source"
                executable.write_bytes(image)
                subject.write_bytes(source)
                route = Route(argparse.Namespace(seconds=1), work, shared)
                route.run("result", "run", Program(executable).bind(subject), lambda: payload, guest=guest)
                route.steps[0][1]()
                return route.artifacts["result"]

            original = cell("one", b"host")
            copy = cell("two", b"host")
            self.assertEqual(copy["reused_from"], "one/result")
            for field in ("sha256", "bytes", "producer_sha256", "request_sha256"):
                self.assertEqual(copy[field], original[field])
            cell("other-host", b"other")
            cell("other-source", b"host", source=b"compile", payload=b"rinput")
            cell("other-input", b"host", payload=b"input2")
            for name in ("guest-one", "guest-two"):
                self.assertNotIn("reused_from", cell(name, b"host", guest=True))
            self.assertEqual(invoke.call_count, 6)
            (root / "one/result").write_bytes(b"damaged")
            with self.assertRaisesRegex(ValueError, "shared producer output changed"):
                cell("damaged", b"host")

    def test_each_plan_executes_the_selected_epsilon_target(self):
        for gamma, delta, epsilon in product(MODES, repeat=3):
            with self.subTest(gamma=gamma, delta=delta, epsilon=epsilon):
                args = argparse.Namespace(gamma=gamma, delta=delta, epsilon=epsilon,
                                          gamma_build=None, profile="conformance", seconds=1, delta_interpreter="direct")
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

    def test_delta_can_interpret_the_translators_actual_gamma_output(self):
        args = argparse.Namespace(gamma="native", delta="interpret", epsilon="interpret",
                                  gamma_build=None, profile="conformance", seconds=1, delta_interpreter="gamma")
        route = Route(args, Path("/unused"))
        route.select()
        stages = [name for name, _ in route.steps]
        self.assertTrue(any("translate evaluator.delta to Gamma" in name for name in stages))
        self.assertIn("[reference -> evaluator.gamma]", stages[-1])
        self.assertNotIn("assemble the direct Delta interpreter", stages)

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
