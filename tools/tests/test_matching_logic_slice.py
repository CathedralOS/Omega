#!/usr/bin/env python3
"""Pin the matching-logic vertical slice checker's verdicts and diagnostics.

Runs locally only; no non-standard Python packages:

    python3 tools/tests/test_matching_logic_slice.py -v
"""

import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
TOOL = ROOT / "tools" / "matching-logic-slice" / "slice_checker.py"
CASES = ROOT / "tools" / "matching-logic-slice" / "cases"


def load_tool():
    specification = importlib.util.spec_from_file_location(
        "slice_checker_under_test", TOOL
    )
    module = importlib.util.module_from_spec(specification)
    previous = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        specification.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


checker = load_tool()


def verdict_for(name):
    return checker.check_case(CASES / name)


class ReferenceCaseTest(unittest.TestCase):
    def test_reference_case_accepts(self):
        result = verdict_for("reference.json")
        self.assertEqual(result["verdict"], "accept", result)
        self.assertEqual(result["expect"], "accept")

    def test_reference_records_axiom_admissions(self):
        result = verdict_for("reference.json")
        admissions = result.get("admissions", [])
        for name in ("step_s0_s1", "counter_step", "counter_s0_nat",
                     "nat_succ"):
            self.assertIn(f"axiom:{name}", admissions)

    def test_reference_goal_is_reconstructed(self):
        case = checker.load_case(CASES / "reference.json")
        goal = checker.build_goal(case)
        result = verdict_for("reference.json")
        self.assertEqual(result["goal"], goal)
        self.assertEqual(goal[0], "exists")


class NegativeCaseTest(unittest.TestCase):
    def expect_reject(self, name, rule):
        result = verdict_for(name)
        self.assertEqual(result["verdict"], "reject", result)
        diagnostic = result.get("diagnostic") or {}
        self.assertEqual(diagnostic.get("rule"), rule, result)

    def test_weaker_goal_rejects(self):
        self.expect_reject("weaker_goal.json", "goal")

    def test_undeclared_axiom_rejects(self):
        self.expect_reject("undeclared_axiom.json", "axiom")

    def test_undefined_witness_rejects(self):
        self.expect_reject("undefined_witness.json", "defined_closed")

    def test_quantifier_escape_rejects(self):
        self.expect_reject("quantifier_escape.json", "exists_elim")

    def test_eq_subst_side_rejects(self):
        self.expect_reject("eq_subst_side.json", "eq_subst")

    def test_membership_gap_rejects(self):
        self.expect_reject("membership_gap.json", "membership_intro")

    def test_transition_reversed_rejects(self):
        self.expect_reject("transition_reversed.json", "exists_intro")


class RecordTest(unittest.TestCase):
    def test_record_reports_no_divergence(self):
        record = checker.build_record(ROOT)
        self.assertEqual(record["schema"],
                         "omega-matching-logic-slice-record/1")
        self.assertEqual(record["summary"]["divergences"], [])
        self.assertGreaterEqual(record["summary"]["total"], 2)

    def test_check_cli_exit_codes(self):
        # Exit 0 whenever the verdict matches the case's expectation —
        # a negative case rejecting is the discipline holding, not a failure.
        accept = checker.main(["check", str(CASES / "reference.json")])
        reject = checker.main(["check", str(CASES / "weaker_goal.json")])
        self.assertEqual(accept, 0)
        self.assertEqual(reject, 0)

    def test_check_cli_nonzero_on_divergence(self):
        case = json.loads((CASES / "weaker_goal.json").read_text())
        case["expect"] = "accept"
        with tempfile.NamedTemporaryFile(
                "w", suffix=".json", delete=False) as handle:
            json.dump(case, handle)
            path = handle.name
        try:
            self.assertEqual(checker.main(["check", path]), 1)
        finally:
            Path(path).unlink()


if __name__ == "__main__":
    unittest.main()
