#!/usr/bin/env python3
"""Pin the typed-to-one-sorted encoder's clause inventory and diagnostics.

Runs locally only; no non-standard Python packages:

    python3 tools/tests/test_matching_logic_sort_encoding.py -v
"""

import importlib.util
from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[2]
TOOL = ROOT / "tools" / "matching-logic-sort-encoding" / "sort_encoding.py"
CASES = ROOT / "tools" / "matching-logic-sort-encoding" / "cases"


def load_tool():
    specification = importlib.util.spec_from_file_location(
        "sort_encoding_under_test", TOOL
    )
    module = importlib.util.module_from_spec(specification)
    previous = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        specification.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


encoding = load_tool()


def report_for(name):
    case = encoding.load_case(CASES / name)
    return encoding.report(case, "test")


def rules(result):
    return [row["rule"] for row in result["diagnostics"]]


class ReferenceCaseTest(unittest.TestCase):
    def test_reference_case_is_clean(self):
        result = report_for("reference.json")
        self.assertEqual(result["diagnostics"], [])

    def test_reference_emits_refined_integer_family(self):
        result = report_for("reference.json")
        patterns = [row["clause"] for row in result["admissions"]]
        self.assertIn("Nat(x) == Integer(x) & 0 <= x", patterns)
        self.assertIn(
            "Int64(x) == Integer(x) & -2^63 <= x & x < 2^63", patterns)
        self.assertIn("U3(x) == Integer(x) & 0 <= x & x <= 3", patterns)

    def test_reference_emits_slice_pointee_quantifier(self):
        result = report_for("reference.json")
        patterns = [row["clause"] for row in result["admissions"]]
        self.assertTrue(any(
            "exists b l. x = <b, l>" in pattern
            and "forall i. (0 <= i & i < l) -> UInt8(b + i)" in pattern
            for pattern in patterns))

    def test_reference_emits_sum_disjointness_and_loans(self):
        result = report_for("reference.json")
        kinds = {(row["kind"], row["clause"]) for row in result["admissions"]}
        self.assertIn(
            ("disjointness",
             "Tag_MaybeI64_Some(x) -> not Tag_MaybeI64_None(x)"), kinds)
        self.assertIn(
            ("disjointness",
             "Loan_mut(x) over region r -> no other live Loan over r"),
            kinds)
        self.assertIn(("lineage", "ChildOf(x, y)"), kinds)

    def test_evidence_record_fields(self):
        result = report_for("reference.json")
        self.assertEqual(result["schema"], "omega-sort-encoding-record/1")
        self.assertEqual(
            result["fragment"],
            "one-sorted finitary basic matching logic, no fixpoint symbols")
        for field in ("subject", "target_capsule", "observation_profile",
                      "bridge_graph", "admissions"):
            self.assertIn(field, result)


class NegativeCaseTest(unittest.TestCase):
    def test_each_negative_case_flags_its_named_rule(self):
        expectations = {
            "missing_definedness.json": {"missing-definedness-precondition"},
            "uninhabited_membership.json": {"vacuous-membership"},
            "widening_revision.json": {"revision-not-refinement"},
            "exclusive_loan_conflict.json": {"exclusive-loan-duplicated"},
            "non_injective_pair.json": {"pair-constructor-not-injective"},
            "sum_payload_unknown.json": {"unknown-payload-membership"},
            "uncertified_fixpoint.json": {"unguarded-fixpoint"},
        }
        for name, expected in expectations.items():
            with self.subTest(case=name):
                self.assertTrue(expected <= set(rules(report_for(name))))

    def test_lower_bound_widening_is_also_rejected(self):
        case = encoding.load_case(CASES / "widening_revision.json")
        case["revisions"][0]["tightened_lower"] = -1
        result = encoding.report(case, "test")
        self.assertIn("revision-not-refinement", rules(result))

    def test_shared_loans_may_duplicate(self):
        case = encoding.load_case(CASES / "exclusive_loan_conflict.json")
        for loan in case["loans"]:
            loan["kind"] = "shared"
        case["inhabitants"]["Loan_shared"] = 2
        result = encoding.report(case, "test")
        self.assertNotIn("exclusive-loan-duplicated", rules(result))


if __name__ == "__main__":
    unittest.main()
