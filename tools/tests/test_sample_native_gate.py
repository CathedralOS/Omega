"""The sample gate edits decisions, never the package review's byte framing."""

import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    "sample_native_gate", Path(__file__).resolve().parents[1] / "sample_native_gate.py"
)
gate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gate)


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


if __name__ == "__main__":
    unittest.main()
