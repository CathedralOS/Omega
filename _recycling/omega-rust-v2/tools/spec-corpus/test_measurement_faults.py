"""Delete one recorded comparison at each layer while every native test name stays green."""
import os
from pathlib import Path
import unittest
from unittest.mock import patch

from check_layer_readiness import collect
import reviewed_comparisons


def inspect(log, witness):
    report = {"suites": {}, "cases": {}, "expected": {}, "cohort_sources": {}, "obligations": {},
              "package_fixtures": [], "measurement_errors": [], "last_layer": "L5"}
    tests, _ = collect(log, witness.suite, report)
    report["suites"][witness.suite] = {"tests": tests}
    row = {"rule": witness.rule}
    with patch.object(reviewed_comparisons, "registrations", return_value=[witness]):
        reviewed_comparisons.apply(report, {witness.owner: [row]})
    return row, report["measurement_errors"]


class MeasurementFaults(unittest.TestCase):
    def verify_changed_log(self, log, witness):
        marker = f"OMEGA_REVIEWED_COMPARISON\t{witness.comparison}\t"
        self.assertEqual(sum(line.startswith(marker) for line in log.splitlines()), 1)
        baseline, errors = inspect(log, witness)
        self.assertTrue(baseline.get(witness.side), errors)
        changed = "\n".join(line for line in log.splitlines() if not line.startswith(marker))
        row, changed_errors = inspect(changed, witness)
        self.assertFalse(row.get(witness.side))
        self.assertEqual(changed_errors, [*errors, f"missing reviewed comparison: {witness.comparison} "
                         f"({witness.suite} {witness.target}::{witness.test}, test ok)"])

    def measure(self, directory=None):
        for stage in ("L0", "L1", "L2", "L3", "L4", "L5"):
            w = next(r for r in reviewed_comparisons.registrations() if r.suite == stage and not r.comparison.startswith("fixture/"))
            log = (f"Running {w.target} (target/test)\ntest {w.test} ... ok\n"
                   f"---- {w.test} stdout ----\nOMEGA_REVIEWED_COMPARISON\t{w.comparison}\tassert_eq!(value, expected)\n"
                   "test result: ok. 1 passed; 0 failed; 0 ignored\n")
            with self.subTest(stage=stage):
                if directory:
                    log = (Path(directory) / f"{stage}.log").read_text(encoding="utf-8")
                self.verify_changed_log(log, w)

    def test_green_names_cannot_replace_a_comparison_in_any_suite(self):
        self.measure()

    @unittest.skipUnless(os.environ.get("OMEGA_FAULT_EVIDENCE"), "set OMEGA_FAULT_EVIDENCE to replay a full readiness run")
    def test_removing_real_comparisons(self):
        self.measure(os.environ["OMEGA_FAULT_EVIDENCE"])


if __name__ == "__main__":
    unittest.main()
