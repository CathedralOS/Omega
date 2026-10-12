"""The sample gate edits decisions, never the package review's byte framing."""

import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    "sample_native_gate", Path(__file__).resolve().parents[1] / "sample_native_gate.py"
)
gate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gate)

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import sample_projects


class ReviewAcceptanceTests(unittest.TestCase):
    def test_only_pending_decisions_change(self):
        with tempfile.TemporaryDirectory() as directory:
            review = Path(directory) / "build/package-manager/review-windows_x86_64.txt"
            review.parent.mkdir(parents=True)
            original = (
                'omega-package-review 1\npackage "mathematics ∀"\n'
                'finding pending\ndecision row ab12 pending\n'
                'decision row cd34 reject\nend-review\n'
            ).encode("utf-8")
            review.write_bytes(original)
            success = subprocess.CompletedProcess([], 0, "", "")
            with patch.object(gate, "run", return_value=success) as runner:
                reason, _ = gate.accept_review("omega", directory, "windows_x86_64", 10)
            self.assertIsNone(reason)
            self.assertEqual(
                review.read_bytes(),
                original.replace(b"decision row ab12 pending", b"decision row ab12 accept"),
            )
            self.assertEqual(runner.call_count, 2)
            self.assertEqual(runner.call_args.args[0][-1], "--resume")

    def test_resume_failure_is_not_successful_acceptance(self):
        with tempfile.TemporaryDirectory() as directory:
            review = Path(directory) / "build/package-manager/review-windows_x86_64.txt"
            review.parent.mkdir(parents=True)
            review.write_bytes(b"decision row ab12 pending\n")
            results = [
                subprocess.CompletedProcess([], 0, "review ready", ""),
                subprocess.CompletedProcess([], 1, "", "review findings changed"),
            ]
            with patch.object(gate, "run", side_effect=results):
                reason, output = gate.accept_review("omega", directory, "windows_x86_64", 10)
            self.assertEqual(reason, "resume-failed")
            self.assertEqual(output, "review findings changed")


class FixtureOracleTests(unittest.TestCase):
    """`input.txt` beside the entry is the run's stdin; `expected_*` files
    score their channels exactly instead of the header oracle."""

    def project(self, directory, header="// Expected exit: 0.\n"):
        root = Path(directory)
        (root / "main.omg").write_text(header + "module probe;\n", encoding="utf-8")
        (root / "build.omg").write_text(
            'machine build(builder: &mut Build) {\n'
            '    Build::application(builder, "probe");\n'
            '    Build::bind(builder, macos_arm64::ProgramEntry, probe::Main::main);\n'
            '}\n',
            encoding="utf-8",
        )
        return root

    def measure(self, directory, stdout, stderr, exit_code=0):
        """Run measure() with `run` faked: reviews succeed, `omega run`
        returns the supplied program result and records its stdin."""
        calls = []

        def fake_run(command, timeout, stdin=subprocess.DEVNULL):
            calls.append((command, stdin))
            if "run" in command:
                return subprocess.CompletedProcess(command, exit_code, stdout, stderr)
            return subprocess.CompletedProcess(command, 0, "", "")

        root = Path(directory)
        project = sample_projects.Project(
            directory=root, entry=root / "main.omg", role="program")
        with patch.object(gate, "run", side_effect=fake_run), \
                patch.object(gate, "project_at", return_value=project):
            result = gate.measure("omega", directory, "macos_arm64", 10)
        runs = [stdin for command, stdin in calls if "run" in command]
        return result, runs

    def test_input_fixture_feeds_input_and_scores_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = self.project(directory)
            (root / "input.txt").write_bytes(b"A")
            (root / "expected_native_exit.txt").write_text("0\n", encoding="utf-8")
            (root / "expected_stdout.txt").write_text(
                "float equality: equal and unequal\n", encoding="utf-8")
            (root / "expected_stderr.txt").write_text("", encoding="utf-8")
            result, runs = self.measure(
                directory,
                "float equality: equal and unequal\n",
                "native exit 0\n",
            )
            self.assertEqual(result["state"], "pass")
            self.assertEqual(len(runs), 1)
            self.assertIsNot(runs[0], subprocess.DEVNULL)
            self.assertEqual(runs[0].mode, "rb")

    def test_fixture_stdout_mismatch_is_oracle_failed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = self.project(directory)
            (root / "input.txt").write_bytes(b"A")
            (root / "expected_stdout.txt").write_text("wanted\n", encoding="utf-8")
            result, _ = self.measure(directory, "other\n", "native exit 0\n")
            self.assertEqual(result["state"], "oracle-failed")
            self.assertIn("stdout differs", result["detail"])

    def test_program_stderr_is_compared_without_wrapper_lines(self):
        with tempfile.TemporaryDirectory() as directory:
            root = self.project(directory)
            (root / "input.txt").write_bytes(b"A")
            (root / "expected_stderr.txt").write_text("", encoding="utf-8")
            result, _ = self.measure(directory, "", "warning\nnative exit 0\n")
            self.assertEqual(result["state"], "oracle-failed")
            self.assertIn("stderr differs", result["detail"])

    def test_header_oracle_keeps_empty_stdin(self):
        with tempfile.TemporaryDirectory() as directory:
            self.project(
                directory,
                header="// Expected exit: 2.\n// Expected output contains: INVALID\n")
            result, runs = self.measure(directory, "INVALID\n", "native exit 2\n", exit_code=2)
            self.assertEqual(result["state"], "pass")
            self.assertEqual(runs, [subprocess.DEVNULL])


if __name__ == "__main__":
    unittest.main()
