"""Gate coverage for the repository baseline runner."""

import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    "baseline_gate", Path(__file__).resolve().parents[1] / "baseline_gate.py")
gate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gate)


class BaselineCommandTests(unittest.TestCase):
    def test_commands_match_the_documented_baseline(self):
        commands = dict(gate.baseline_commands("mbx"))
        self.assertEqual(
            commands["fmt"],
            ["cargo", "fmt", "--all", "--", "--check"])
        self.assertEqual(
            commands["clippy"],
            ["mbx", "clippy", "--workspace", "--all-targets", "--",
             "-D", "warnings"])
        self.assertEqual(
            commands["architecture"],
            ["mbx", "nextest", "run", "-p", "omega-architecture-test",
             "--all-targets", "--no-fail-fast"])
        self.assertEqual(
            commands["canary-corpus-audit"],
            ["mbx", "nextest", "run", "-p", "compiler", "--test",
             "canary_suite", "--no-fail-fast", "--no-tests", "fail", "-E",
             "test(=surface_and_targets::retired_domain_when_surface_is_absent_from_authored_corpus)"])
        self.assertEqual(
            commands["check"],
            ["mbx", "check", "--workspace", "--all-targets"])
        self.assertEqual(
            commands["libraries"],
            ["mbx", "nextest", "run", "--workspace", "--lib", "--no-fail-fast"])

    def test_gate_names_are_stable_and_ordered(self):
        names = [name for name, _ in gate.baseline_commands("cargo")]
        self.assertEqual(names, ["fmt", "clippy", "architecture",
                                 "canary-corpus-audit", "check", "libraries"])

    def test_runner_prefers_mbx_then_cargo_then_errors(self):
        with patch.object(gate.shutil, "which",
                          side_effect=lambda name: f"/bin/{name}" if name == "mbx" else None):
            self.assertEqual(gate.resolve_runner(), "/bin/mbx")
        with patch.object(gate.shutil, "which",
                          side_effect=lambda name: "/bin/cargo" if name == "cargo" else None):
            self.assertEqual(gate.resolve_runner(), "/bin/cargo")
        with patch.object(gate.shutil, "which", return_value=None):
            with self.assertRaises(ValueError):
                gate.resolve_runner()


class RunGateTests(unittest.TestCase):
    def test_all_gates_run_despite_earlier_failures(self):
        calls = []
        returncodes = [0, 3, 0, 0, 0, 0]

        def fake_run(command, cwd, check):
            calls.append(command)
            return type("Completed", (),
                        {"returncode": returncodes[len(calls) - 1]})()

        with patch.object(gate.subprocess, "run", side_effect=fake_run):
            results = gate.run_gates(Path("/repo"), gate.baseline_commands("cargo"), "linux x86_64")
        self.assertEqual(len(calls), 6)
        self.assertEqual([r[2] for r in results], [0, 3, 0, 0, 0, 0])

    def test_report_is_nonzero_only_when_a_gate_fails(self):
        ok = [("fmt", ["cargo", "fmt"], 0, 1.0)]
        self.assertEqual(gate.report(ok, "linux x86_64", "cargo"), 0)
        bad = ok + [("clippy", ["cargo", "clippy"], 101, 2.0)]
        self.assertEqual(gate.report(bad, "linux x86_64", "cargo"), 1)


if __name__ == "__main__":
    unittest.main()
