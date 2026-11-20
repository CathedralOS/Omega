"""Check mixed route handoffs and the distinction between observations and refusals."""
import argparse
import json
from hashlib import sha256
from itertools import product
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from compiled import add_invocation_budget, execute, reference
from independent.epsilon.composition import linked_program
from independent.epsilon.enum_test import byte_class_case, byte_class_wrong_base_case
from route import MODES, ROOT, Program, Route, check_observation, execute_route, observation_kind, prior_invocations


class RouteTests(unittest.TestCase):
    def test_matrix_invocation_budget_is_opt_in(self):
        parser = argparse.ArgumentParser(exit_on_error=False)
        add_invocation_budget(parser)
        self.assertIsNone(parser.parse_args([]).seconds)
        self.assertEqual(parser.parse_args(["--seconds", "12"]).seconds, 12)
        for value in ("0", "-1", "invalid"):
            with self.subTest(value=value), self.assertRaises(argparse.ArgumentError):
                parser.parse_args(["--seconds", value])

    def test_shared_guest_and_reference_pass_the_selected_budget_to_the_process(self):
        finished = subprocess.CompletedProcess(["evaluator"], 0, b"*", b"")
        with patch("compiled.subprocess.run", return_value=finished) as run:
            for observe in (execute, reference):
                self.assertEqual(observe("evaluator", b"source"), (0, b"*"))
                self.assertIsNone(run.call_args.kwargs["timeout"])
                self.assertEqual(observe("evaluator", b"source", timeout=12), (0, b"*"))
                self.assertEqual(run.call_args.kwargs["timeout"], 12)

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
                route.run("result", "run", Program(executable).bind(subject), lambda: payload, terminal=guest)
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

    def test_interrupted_row_preserves_only_finished_intermediates_for_replay(self):
        args = argparse.Namespace(gamma="native", delta="native", epsilon="native",
                                  profile="conformance", seconds=None)
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "old-matrix" / "native-native-native"
            r = Route(args, source)
            r.host = Program(Path(sys.executable))
            script = b'print("stage")'

            def check_start():
                checkpoint = json.loads((source / "result.json").read_text())
                self.assertEqual(checkpoint["stages"], [{"stage": "first", "completed": False}])
                self.assertEqual(prior_invocations([source.parent]), {})

            r.stage("first", check_start)
            r.run("module.out", "build", r.host, lambda: script)

            # A killed process cannot replace its last durable intermediate receipt.
            def interrupt():
                checkpoint = json.loads((source / "result.json").read_text())
                self.assertEqual(checkpoint["result"], "running")
                self.assertEqual(checkpoint["stages"][-1], {"stage": "interrupted", "completed": False})
                self.assertTrue(all(stage["completed"] for stage in checkpoint["stages"][:-1]))
                self.assertEqual(list(checkpoint["artifacts"]), ["module.out"])
                raise KeyboardInterrupt

            r.stage("interrupted", interrupt)
            with self.assertRaises(KeyboardInterrupt):
                execute_route(r, b"source", b"", None, verbose=False)

            cache = prior_invocations([source.parent])
            work = root / "other" / "fresh"
            work.mkdir(parents=True)
            replay = Route(args, work, cache)
            replay.run("module.out", "build", r.host, lambda: script)
            replay.run("guest.eeout", "run guest", r.host, lambda: script, terminal=True)
            with patch("route.invoke", return_value=b"fresh") as invoke:
                for _, action in replay.steps:
                    action()
                invoke.assert_called_once()
            self.assertEqual((work / "module.out").read_bytes(), b"stage\n")
            self.assertEqual((work / "guest.eeout").read_bytes(), b"fresh")
            self.assertEqual((work.parent / replay.artifacts["module.out"]["reused_from"]).resolve(),
                             source / "module.out")
            # Historic receipts can contain terminal executions with exactly the same input.
            (work / "result.json").write_text(json.dumps({"artifacts": replay.artifacts}))
            self.assertEqual(prior_invocations([source, work]), cache)

    def test_whole_constructor_requires_the_emitted_omega_tape_to_exit_42(self):
        # Call main, halt, then return the unsigned byte literal from main.
        tape = bytes.fromhex("13 0b00000000000000 00 00 01 00 2a00000000000000 14")
        cases = {"returns-42": bytes(5) + tape, "returns-41": bytes(5) + tape.replace(b"*", b")"),
                 "no-tape": bytes(5), "constructor-refused": bytes.fromhex("0002000000")}
        with tempfile.TemporaryDirectory() as temp:
            for name, output in cases.items():
                with self.subTest(name=name):
                    work = Path(temp) / name
                    args = argparse.Namespace(gamma="native", delta="native", epsilon="native",
                                              profile="construction", seconds=None, case="omega-constructor")
                    route = Route(args, work)
                    route.host = Program(Path(sys.executable))
                    route.file("guest.eeout", "Omega constructor output", lambda: output)
                    route.stage("check the emitted Omega program", route.check_omega_program)
                    status, report = execute_route(route, b"compiler", b"input", None, verbose=False)
                    if name == "returns-42":
                        self.assertEqual((status, report["result"]), (0, "matched"))
                        self.assertEqual(report["artifacts"]["omega-program.tape"],
                                         {"bytes": len(tape), "sha256": sha256(tape).hexdigest(), "guest_exit": 42})
                    else:
                        self.assertEqual((status, report["result"]), (1, "failed"))
                        self.assertNotIn("omega-program.tape", report["artifacts"])

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

    def test_epsilon_compiler_can_feed_a_delta_interpreter_on_each_gamma_host(self):
        for gamma, delta, interpreter in product(MODES, MODES, ("direct", "gamma")):
            with self.subTest(gamma=gamma, delta=delta, interpreter=interpreter):
                args = argparse.Namespace(gamma=gamma, delta=delta, epsilon="interpret", gamma_build=None,
                                          profile="construction", delta_interpreter=interpreter,
                                          epsilon_interpreter="delta")
                route = Route(args, Path("/unused"))
                route.select()
                stages = [name for name, _ in route.steps]
                self.assertTrue(any(stage.endswith(" -> module.out") for stage in stages))
                self.assertTrue(any(stage.endswith(" -> subject.delta") for stage in stages))
                if interpreter == "direct":
                    self.assertEqual(stages.count("assemble the direct Delta interpreter"), 1)
                    self.assertIn(" -> subject.delta] -> guest.eeout", stages[-1])
                else:
                    self.assertIn("[reference -> subject.gamma] -> guest.eeout", stages[-1])

                refusal = Route(args, Path("/unused"), expect_rejection=True)
                refusal.select()
                self.assertTrue(refusal.steps[-1][0].endswith(" -> compiler.response"))

    def test_interpreting_generated_delta_stops_at_an_epsilon_compiler_refusal(self):
        args = argparse.Namespace(epsilon="interpret", epsilon_interpreter="delta")
        with tempfile.TemporaryDirectory() as temp:
            route = Route(args, Path(temp), expect_rejection=True)
            response = route.work / "compiler.response"
            response.write_bytes(b"rejected 18 2032")
            report = {}
            check_observation(route, struct.pack("<BBI", 2, 18, 2032), report)
            self.assertEqual(report["result"], "expected-rejection")
            self.assertFalse(report["guest_executed"])
            response.write_bytes(b"rejected 18 2033")
            with self.assertRaises(ValueError):
                check_observation(route, struct.pack("<BBI", 2, 18, 2032), {})

    def test_wrong_base_changes_one_argument_and_stops_at_the_frontend(self):
        original, payload, _ = byte_class_case(ROOT)
        source, stdin, expected = byte_class_wrong_base_case(ROOT)
        call = b'omega_digit_in_base(value, selected)'
        self.assertEqual(source, original.replace(call, b'omega_digit_in_base(value, value)'))
        self.assertEqual(stdin, payload)
        self.assertEqual(expected, struct.pack('<BBI', 2, 18, original.index(call)))
        for mode in MODES:
            args = argparse.Namespace(gamma="native", delta="native", epsilon=mode,
                                      gamma_build=None, profile="conformance", seconds=1)
            route = Route(args, Path("/unused"), expect_rejection=True)
            route.select()
            stages = [name for name, _ in route.steps]
            artifact = "guest.eeout" if mode == "interpret" else "compiler.response"
            self.assertTrue(stages[-1].endswith(" -> " + artifact))
            self.assertFalse(any("subject.delta" in stage for stage in stages))

    def test_rejection_matches_only_the_exact_successful_frontend_response(self):
        expected = struct.pack('<BBI', 2, 18, 123)
        for mode, output, exit_code, result in (
            ("interpret", expected, 0, "matched"),
            ("native", b"rejected 18 123", 0, "expected-rejection"),
            ("native", b"rejected 18 123", 1, "failed"),
            ("native", b"rejected 18 124", 0, "failed"),
            ("native", b"rejected 17 123", 0, "failed"),
            ("native", b"rejected 18 123\n", 0, "failed"),
            ("native", b"unsupported 123", 0, "failed"),
            ("native", b"compiled-storage\n(def example () Int 42)", 0, "failed"),
            ("interpret", struct.pack('<BBI', 2, 18, 124), 0, "failed"),
            ("interpret", expected + b'X', 0, "failed"),
        ):
            with self.subTest(mode=mode, output=output, exit_code=exit_code), tempfile.TemporaryDirectory() as temp:
                args = argparse.Namespace(gamma="native", delta="native", epsilon=mode,
                                          profile="conformance", seconds=30)
                work = Path(temp) / "row"
                route = Route(args, work, expect_rejection=True)
                route.host = Program(Path(sys.executable))
                artifact = "guest.eeout" if mode == "interpret" else "compiler.response"
                # A standalone producer lets the normal invocation check its status and stdout.
                script = f'import sys\nsys.stdout.buffer.write({output!r})\nsys.exit({exit_code})'.encode()
                route.run(artifact, "run test producer", route.host, lambda: script, terminal=True)
                status, report = execute_route(route, b"subject", b"", expected, verbose=False)
                self.assertEqual((status, report["result"]), (int(result == "failed"), result))
                if exit_code == 0:
                    self.assertEqual((work / artifact).read_bytes(), output)
                    receipt = report["artifacts"][artifact]
                    self.assertEqual(receipt["sha256"], sha256(output).hexdigest())
                    self.assertEqual(receipt["producer_sha256"], sha256(Path(sys.executable).read_bytes()).hexdigest())
                    self.assertEqual(receipt["request_sha256"], sha256(script).hexdigest())
                if status == 0:
                    self.assertEqual((report["rejection_code"], report["source_offset"]), (18, 123))
                    self.assertFalse(report["guest_executed"])

    def test_custom_oracle_selects_the_terminal_stage_before_construction(self):
        with tempfile.TemporaryDirectory() as temp:
            oracle = Path(temp) / "expected.eeout"
            command = [sys.executable, str(ROOT / "tests/bootstrap/route.py"), "--plan",
                       "--gamma", "native", "--delta", "native", "--epsilon", "alpha",
                       "--expect-observation", str(oracle)]
            oracle.write_bytes(struct.pack('<BBI', 2, 18, 123))
            result = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(result.returncode, 2)
            self.assertIn("--expect-observation requires --source", result.stderr)

            command.extend(["--source", str(Path(temp) / "subject.epsilon")])
            for observation, artifact in [(oracle.read_bytes(), "compiler.response"),
                                          (struct.pack('<Bi', 0, 42), "guest.eeout"),
                                          (bytes((1, 5)), "guest.eeout")]:
                oracle.write_bytes(observation)
                result = subprocess.run(command, capture_output=True, text=True, check=True)
                self.assertTrue(result.stdout.rstrip().endswith(" -> " + artifact))

            oracle.write_bytes(bytes((2, 18)))
            result = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(result.returncode, 2)
            self.assertIn("invalid Epsilon observation", result.stderr)

    def test_custom_refusal_and_timed_out_named_rejection_remain_failures(self):
        args = argparse.Namespace(gamma="native", delta="native", epsilon="alpha",
                                  profile="conformance", seconds=1)
        with tempfile.TemporaryDirectory() as temp:
            custom = Route(args, Path(temp) / "custom")
            custom.stage("attach runtime", lambda: linked_program(b"rejected 18 123"))
            status, report = execute_route(custom, b"subject", b"", None, verbose=False)
            self.assertEqual((status, report["result"]), (1, "compiler-refusal"))

            negative = Route(args, Path(temp) / "timeout", expect_rejection=True)
            negative.run("compiler.response", "check source", Program(Path(sys.executable)), bytes, terminal=True)
            with patch("route.invoke", side_effect=subprocess.TimeoutExpired("producer", 1)):
                status, report = execute_route(negative, b"subject", b"", b"\2\x12" + bytes(4), verbose=False)
            self.assertEqual((status, report["result"]), (124, "timeout"))
            self.assertNotIn("compiler.response", report["artifacts"])

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
