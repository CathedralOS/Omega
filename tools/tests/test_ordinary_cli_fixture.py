"""Kept-artifact diagnostics preserve execution and source-custody verdicts."""

import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

sys.path[:0] = [str(Path(__file__).resolve().parents[1]),
                str(Path(__file__).resolve().parents[1] / "ci")]
import ordinary_cli_fixture as cli


class ProjectSelectionTests(unittest.TestCase):
    def test_run_fixture_keeps_its_directory_and_authored_engine_exits(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fixture = "run/domains/captured_destination"
            relative = Path("tests/omega") / fixture
            project = root / relative
            project.mkdir(parents=True)
            (project / "main.omg").write_text("machine main() {}\n")
            (project / "build.omg").write_text("ProgramEntry\n")
            (project / "expected_native_exit.txt").write_text("70\n")
            interpreter = project / "expected_interpreter_exit.txt"
            interpreter.write_text("70\n")
            self.assertEqual(cli.relative_project(root, fixture), relative)
            self.assertEqual(cli.expected_exit(root, fixture, "linux_x86_64"), 70)
            interpreter.write_text("0\n")
            with self.assertRaisesRegex(ValueError, "matching native and interpreter"):
                cli.expected_exit(root, fixture, "linux_x86_64")
            interpreter.write_text("70\n")
            (project / "expected_stdout.txt").write_text("required output\n")
            with self.assertRaisesRegex(ValueError, "cannot omit authored stream"):
                cli.expected_exit(root, fixture, "linux_x86_64")

    def test_runtime_selection_still_refuses_fail_and_noncanonical_paths(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for selector in ("fail/domains/example", "run/domains/example/../other",
                             "/run/domains/example", "run\\domains\\example"):
                with self.subTest(selector=selector), self.assertRaises(ValueError):
                    cli.relative_project(root, selector)
            with self.assertRaisesRegex(ValueError, "lacks its own tracked directory/build"):
                cli.relative_project(root, "run/domains/missing")


class KeptArtifactDiagnosticsTests(unittest.TestCase):
    def observe(self, *, diagnostics=True, runtime_exit=0, inspection=0, mutate=False,
                keep_artifact=True):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            relative = Path("tests/omega/pass/example/exit")
            project = root / relative
            project.mkdir(parents=True)
            (project / "main.omg").write_text("// Expected exit: 0\nmachine main() {}\n")
            (project / "build.omg").write_text("machine build() {}\n")
            (project / "expected_native_exit.txt").write_text("0\n")
            paths = [str(relative / leaf) for leaf in
                     ("main.omg", "build.omg", "expected_native_exit.txt")]
            kept = root / "kept"
            kept.mkdir()
            if keep_artifact:
                (kept / "program.oterm").write_bytes(b"retained artifact")
            output = root / "observation"
            producer = Mock(cli=Path("/verified/omega"), target="linux_x86_64")
            producer.record.return_value = {"revision": "a" * 40}
            calls = []

            def run(arguments, **kwargs):
                calls.append(arguments)
                selected = output / "workspace" / relative
                if arguments[1] == "update":
                    (selected / "omega.lock").write_text("ordinary accepted lock")
                    return Mock(returncode=0)
                if arguments[1] == "run":
                    terminal = "interp exit: 0" if runtime_exit == 0 else "Terminal interpretation FAILED"
                    kwargs["stderr"].write(
                        f"native exit 0\n{terminal}\nbuild dir kept: {kept}\n".encode())
                    return Mock(returncode=runtime_exit)
                self.assertEqual(arguments[:3], [producer.cli, "inspect-terminal", "--artifact"])
                self.assertEqual(arguments[3], output / "products/program.oterm")
                self.assertEqual(kwargs["timeout"], 240)
                kwargs["stdout"].write(b"operation id=operation:79 kind=Example\n")
                if mutate:
                    (selected / "main.omg").write_text("changed source")
                if inspection == "timeout":
                    raise subprocess.TimeoutExpired(arguments, 240)
                return Mock(returncode=inspection)

            environment = {"OMEGA_COMPILER_CHECK_SITES": "1"} if diagnostics else {}
            with patch.dict(os.environ, environment, clear=True), \
                    patch.object(cli.subprocess, "check_output", return_value="\n".join(paths)), \
                    patch.object(cli.subprocess, "run", side_effect=run):
                row = cli.observe(root, producer, "pass/example/exit", output)
            inspection_output = output / "diagnostic-terminal-inspection.stdout"
            text = inspection_output.read_bytes() if inspection_output.exists() else None
            self.assertTrue(producer.verify.called)
            return row, calls, text

    def test_diagnostics_are_off_by_default(self):
        row, calls, text = self.observe(diagnostics=False)
        self.assertTrue(row["passed"])
        self.assertNotIn("diagnostic_inspection", row)
        self.assertEqual([call[1] for call in calls], ["update", "run"])
        self.assertIsNone(text)

    def test_inspection_refusal_or_timeout_does_not_reject_a_successful_run(self):
        for result in (0, 1, "timeout"):
            with self.subTest(inspection=result):
                row, calls, text = self.observe(inspection=result)
                self.assertTrue(row["passed"])
                self.assertEqual(row["native_exit"], row["terminal_exit"])
                self.assertEqual([call[1] for call in calls], ["update", "run", "inspect-terminal"])
                self.assertIn(b"operation:79", text)
                diagnostic = row["diagnostic_inspection"]
                if result == "timeout":
                    self.assertEqual(diagnostic["status"], "failed")
                    self.assertIn("exceeded its 240s allowance", diagnostic["error"])
                else:
                    self.assertEqual(diagnostic["exit_code"], result)

    def test_inspection_does_not_mask_original_runtime_refusal(self):
        row, _, text = self.observe(runtime_exit=201)
        self.assertFalse(row["passed"])
        self.assertIn("wrapper 201", row["error"])
        self.assertEqual(row["diagnostic_inspection"]["exit_code"], 0)
        self.assertIn(b"operation:79", text)

    def test_diagnostic_source_change_still_refuses_custody(self):
        row, _, _ = self.observe(mutate=True)
        self.assertFalse(row["passed"])
        self.assertIn("ordinary CLI fixture input changed", row["error"])

    def test_missing_artifact_keeps_original_missing_product_refusal(self):
        row, calls, _ = self.observe(keep_artifact=False)
        self.assertFalse(row["passed"])
        self.assertEqual(row["diagnostic_inspection"]["status"], "missing")
        self.assertEqual([call[1] for call in calls], ["update", "run"])
        self.assertIn("lost its retained Terminal product", row["error"])


if __name__ == "__main__":
    unittest.main()
