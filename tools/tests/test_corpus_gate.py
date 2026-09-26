"""Native fixture oracles cannot be repinned into a passing golden."""

from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import corpus_gate


class NativeExitExpectations(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.fixture = self.root / "pass" / "arithmetic" / "conversion"
        self.fixture.mkdir(parents=True)
        self.record = {"fixture": "pass/arithmetic/conversion", "tier": "pass",
                       "status": "built", "facts": {"exit": "70"}}

    def failures(self, expectation, facts, target="macos_arm64", status="built"):
        (self.fixture / "expected_native_exit.txt").write_text(expectation)
        self.record.update(facts=facts, status=status)
        return corpus_gate.native_exit_failures([self.record], self.root, target)

    def test_explicit_exit_is_checked_without_a_golden(self):
        self.assertEqual(self.failures("70\n", {"exit": "70"}), [])
        self.assertTrue(self.failures("70", {"exit": "7"}))

    def test_trap_requires_the_target_specific_termination(self):
        for target, facts in [
            ("macos_arm64", {"exit": "signal", "signal": "5"}),
            ("linux_arm64", {"exit": "signal", "signal": "5"}),
            ("linux_x86_64", {"exit": "signal", "signal": "4"}),
            ("windows_x86_64", {"exit": str(-1073741795)}),
        ]:
            with self.subTest(target=target):
                self.assertEqual(self.failures("trap", facts, target), [])
                for wrong in ({"exit": "70"}, {"exit": "signal", "signal": "11"},
                              {"exit": str(-1073741819)},  # Windows access violation.
                              {"exit": "timeout"}, {"exit": "unrunnable"}, {}):
                    self.assertTrue(self.failures("trap", wrong, target))

    def test_missing_execution_or_invalid_expectation_fails(self):
        self.assertTrue(self.failures("70", {}, status="checked"))
        self.assertTrue(self.failures("trap", {}, status="rejected"))
        self.assertTrue(self.failures("trap", {"exit": "signal"}))
        for expectation in ("", "signal", "70\n7", "2147483648"):
            self.assertTrue(self.failures(expectation, {"exit": "70"}))

    def test_unannotated_fixture_preserves_observation_only_behavior(self):
        self.assertEqual(corpus_gate.native_exit_failures(
            [self.record], self.root, "macos_arm64"), [])

    def test_record_refuses_a_failed_oracle_without_overwriting_golden(self):
        fixture = self.root / "tests" / "omega" / "pass" / "arithmetic" / "conversion"
        fixture.mkdir(parents=True)
        (fixture / "expected_native_exit.txt").write_text("trap\n")
        golden = self.root / "golden.txt"
        golden.write_text("retained golden\n")
        self.record.update(millis=1, expected_satisfied=None, diagnostics=[])
        with patch.object(corpus_gate, "ROOT", self.root), \
             patch.object(corpus_gate, "host_target", return_value="macos_arm64"), \
             patch.object(corpus_gate, "run_runner", return_value=[self.record]), \
             patch.dict(corpus_gate.os.environ, {}, clear=True), \
             patch.object(sys, "argv", ["corpus_gate", "--native", "--record",
                                       "--runner", "unused", "--golden", str(golden)]):
            self.assertEqual(corpus_gate.main(), 1)
        self.assertEqual(golden.read_text(), "retained golden\n")


if __name__ == "__main__":
    unittest.main()
