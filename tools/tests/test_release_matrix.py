#!/usr/bin/env python3
"""Guard the release-matrix runner: its gate table must stay verbatim-equal
to the completion contract's command blocks, every command a gate runs must
resolve to a real tool vector, and a failed/limited/timed-out command must
leave its gate open — never a pass.

Runs locally only; no GitHub access or non-standard Python packages:

    python3 tools/tests/test_release_matrix.py -v
"""

import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
TOOL = ROOT / "tools" / "release_matrix.py"
CONTRACT = ROOT / "wiki" / "drafts" / "reference" / "rust_compiler_completion.md"


def load_tool():
    specification = importlib.util.spec_from_file_location(
        "release_matrix_under_test", TOOL)
    module = importlib.util.module_from_spec(specification)
    previous = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        specification.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


release_matrix = load_tool()

GATE_ORDER = ["RC-REPOSITORY", "RC-SOURCE-SEMANTICS", "RC-PCC-REPLAY",
              "RC-PORTABLE-PSI", "RC-BUILD-AND-PACKAGES", "RC-NATIVE-MATRIX",
              "RC-DIAGNOSTICS", "RC-REPRESENTATIVE-PROGRAMS"]


def contract_text():
    # Shell quotes are invocation syntax, not argv content: the runner stores
    # argument vectors, so compare against the contract with quotes removed.
    return " ".join(CONTRACT.read_text(encoding="utf-8").split()
                    ).replace("'", "")


class GateTable(unittest.TestCase):
    def test_eight_named_gates_in_contract_order(self):
        self.assertEqual(GATE_ORDER, list(release_matrix.GATES))

    def test_every_gate_command_appears_in_the_contract(self):
        normalized = contract_text()
        for name, gate in release_matrix.GATES.items():
            for argv in gate["commands"]:
                joined = " ".join(argv)
                if argv[:3] == [release_matrix.MBX, "test", "--doc"]:
                    # The contract composes this leg: "also run the same
                    # package selection with `mbx test --doc`."
                    self.assertIn("mbx test --doc", normalized)
                    self.assertEqual(
                        [part for i, part in enumerate(gate["commands"][0])
                         if i and gate["commands"][0][i - 1] == "-p"],
                        [part for i, part in enumerate(argv)
                         if i and argv[i - 1] == "-p"],
                        "{} doc-test leg must keep the same package "
                        "selection".format(name))
                    continue
                self.assertIn(
                    joined, normalized,
                    "{} command drifted from the contract: {}".format(
                        name, joined))

    def test_every_command_is_an_argument_vector(self):
        for name, gate in release_matrix.GATES.items():
            for argv in gate["commands"]:
                self.assertIsInstance(argv, list)
                self.assertTrue(argv, "{} empty command".format(name))
                self.assertTrue(all(isinstance(part, str) for part in argv))

    def test_four_required_platform_rows(self):
        self.assertEqual(
            ["linux_x86_64", "linux_arm64", "macos_arm64", "windows_x86_64"],
            [row["product"] for row in release_matrix.PLATFORM_ROWS.values()])


class CommandResolution(unittest.TestCase):
    def test_mbx_commands_resolve_to_the_selected_runner(self):
        self.assertEqual(
            ["cargo", "nextest", "run"],
            release_matrix.resolve(
                [release_matrix.MBX, "nextest", "run"], "cargo"))

    def test_cargo_commands_are_never_rewritten_to_mbx(self):
        self.assertEqual(
            ["cargo", "fmt", "--all", "--", "--check"],
            release_matrix.resolve(
                [release_matrix.CARGO, "fmt", "--all", "--", "--check"],
                "mbx"))

    def test_mbx_command_is_unsupported_without_a_runner(self):
        self.assertFalse(
            release_matrix.command_supported(
                [release_matrix.MBX, "check"], None))


class HostRowSelection(unittest.TestCase):
    def assert_host_row(self, system, machine, expected_product):
        with mock.patch.object(release_matrix.platform, "system",
                               return_value=system), \
             mock.patch.object(release_matrix.platform, "machine",
                               return_value=machine):
            row = release_matrix.host_row()
        self.assertEqual(expected_product,
                         None if row is None else row["product"])

    def test_each_required_host_maps_to_its_product(self):
        cases = [("Linux", "x86_64", "linux_x86_64"),
                 ("Linux", "aarch64", "linux_arm64"),
                 ("Darwin", "arm64", "macos_arm64"),
                 ("Windows", "AMD64", "windows_x86_64")]
        for system, machine, product in cases:
            with self.subTest(host=(system, machine)):
                self.assert_host_row(system, machine, product)

    def test_unlisted_host_closes_no_row(self):
        self.assert_host_row("FreeBSD", "amd64", None)


class GateExecution(unittest.TestCase):
    def plan(self, commands):
        return {"commands": [
            {"argv": argv, "supported": supported}
            for argv, supported in commands]}

    def run_gate(self, plan, limit=None, timeout=None):
        with tempfile.TemporaryDirectory() as scratch:
            return release_matrix.run_gate(
                ROOT, "RC-TEST", plan, Path(scratch), timeout, limit)

    def test_all_zero_exits_pass(self):
        verdict, results = self.run_gate(self.plan([
            ([sys.executable, "-c", ""], True),
            ([sys.executable, "-c", "print('ok')"], True),
        ]))
        self.assertEqual("pass", verdict)
        self.assertEqual([0, 0], [result["exit"] for result in results])
        for result in results:
            self.assertIn("log", result)
            self.assertNotIn("skip", result)

    def test_nonzero_exit_opens_the_gate(self):
        verdict, results = self.run_gate(self.plan([
            ([sys.executable, "-c", "import sys; sys.exit(3)"], True),
        ]))
        self.assertEqual("open", verdict)
        self.assertEqual(3, results[0]["exit"])
        self.assertNotIn("skip", results[0])

    def test_unsupported_command_opens_the_gate(self):
        verdict, results = self.run_gate(self.plan([
            (["definitely-not-a-real-runner"], False),
        ]))
        self.assertEqual("open", verdict)
        self.assertEqual("runner or cargo-nextest unavailable",
                         results[0]["skip"])

    def test_limit_marks_remaining_commands_beyond_limit(self):
        verdict, results = self.run_gate(self.plan([
            ([sys.executable, "-c", ""], True),
            ([sys.executable, "-c", ""], True),
        ]), limit=1)
        self.assertEqual("open", verdict)
        self.assertEqual(0, results[0]["exit"])
        self.assertEqual("beyond --limit", results[1]["skip"])

    def test_timeout_opens_the_gate(self):
        verdict, results = self.run_gate(self.plan([
            ([sys.executable, "-c", "import time; time.sleep(30)"], True),
        ]), timeout=0.5)
        self.assertEqual("open", verdict)
        self.assertIsNone(results[0]["exit"])
        self.assertEqual("timeout", results[0]["skip"])


class PlanMode(unittest.TestCase):
    def invoke(self, *arguments):
        return subprocess.run(
            [sys.executable, str(TOOL), *arguments],
            cwd=ROOT, check=False, capture_output=True, text=True)

    def test_plan_lists_every_gate_and_the_host_row(self):
        result = self.invoke("--plan")
        self.assertEqual(0, result.returncode, result.stderr)
        record = json.loads(result.stdout)
        self.assertEqual(GATE_ORDER, list(record["gates"]))
        self.assertEqual(GATE_ORDER, record["selected_gates"])
        self.assertIn("platform_row", record)
        self.assertIn("toolchain", record)
        self.assertIn("commit", record)

    def test_gate_selection_restricts_the_plan(self):
        result = self.invoke("--plan", "--gate", "RC-PORTABLE-PSI")
        self.assertEqual(0, result.returncode, result.stderr)
        record = json.loads(result.stdout)
        self.assertEqual(["RC-PORTABLE-PSI"], record["selected_gates"])

    def test_unknown_gate_refuses_before_any_work(self):
        result = self.invoke("--plan", "--gate", "RC-NOT-A-GATE")
        self.assertEqual(2, result.returncode)
        self.assertIn("unknown gates", result.stderr)


if __name__ == "__main__":
    unittest.main()
