"""A stale review file must not turn a failed cache-gate update green."""
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import Mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from cache_gate import Gate


class ReviewSettlementTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.project = Path(temporary.name)
        self.review = self.project / "build/package-manager/review-windows_x86_64.txt"
        self.review.parent.mkdir(parents=True)
        self.review.write_text("decision test pending\n", encoding="utf-8")
        self.gate = Gate(Path("unused-compiler"), self.project, "windows_x86_64")

    def test_failed_update_cannot_use_old_review(self):
        self.gate.command = Mock(return_value=(1, "error: cannot write review: unavailable\n"))
        code, _ = self.gate.prepare(self.project, self.project / "cache", "failed")
        self.assertEqual(code, 1)
        self.gate.command.assert_called_once()
        self.assertIn("pending", self.review.read_text(encoding="utf-8"))

    def test_missing_reported_review_cannot_fall_back_to_old_review(self):
        missing = self.project / "missing/review-windows_x86_64.txt"
        self.gate.command = Mock(return_value=(1, f"review: {missing}\n"))
        with self.assertRaisesRegex(AssertionError, "missing review"):
            self.gate.prepare(self.project, self.project / "cache", "missing")
        self.gate.command.assert_called_once()
        self.assertIn("pending", self.review.read_text(encoding="utf-8"))

    def test_emitted_review_resumes_and_preserves_failure(self):
        self.gate.command = Mock(side_effect=[(1, f"review: {self.review}\n"), (1, "resume refused\n")])
        code, text = self.gate.prepare(self.project, self.project / "cache", "emitted")
        self.assertEqual(code, 1)
        self.assertIn("resume refused", text)
        self.assertEqual(self.gate.command.call_count, 2)
        self.assertEqual(self.review.read_text(encoding="utf-8"), "decision test accept\n")


if __name__ == "__main__":
    unittest.main()
