#!/usr/bin/env python3
"""Pin the external proof importer's translation gates and diagnostics.

Runs locally only; no non-standard Python packages:

    python3 tools/tests/test_matching_logic_external_proof_import.py -v
"""

import importlib.util
from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[2]
TOOL = (ROOT / "tools" / "matching-logic-external-proof-import"
        / "external_proof_import.py")
CASES = ROOT / "tools" / "matching-logic-external-proof-import" / "cases"


def load_tool():
    specification = importlib.util.spec_from_file_location(
        "external_proof_import_under_test", TOOL
    )
    module = importlib.util.module_from_spec(specification)
    previous = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        specification.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


importer = load_tool()


def report_for(name):
    return importer.check_case(CASES / name, ROOT)


def rule(result):
    return result.get("diagnostic", {}).get("rule")


class ReferenceCaseTest(unittest.TestCase):
    def test_reference_case_accepts(self):
        result = report_for("reference.json")
        self.assertEqual(result["verdict"], "accept")

    def test_reference_rechecks_through_slice_checker(self):
        result = report_for("reference.json")
        self.assertEqual(sorted(result["admissions"]),
                         ["axiom:add_s", "axiom:add_z"])
        self.assertEqual(result["axiom_closure"], ["add_s", "add_z"])

    def test_reference_bridge_and_profile(self):
        result = report_for("reference.json")
        self.assertEqual(result["bridge_graph"]["UI"], "forall_elim")
        self.assertEqual(result["bridge_graph"]["TRANS"], "eq_trans")
        profile = result["observation_profile"]
        self.assertEqual(profile["export_steps"], 10)
        self.assertGreater(profile["certificate_bytes"], 0)

    def test_goal_is_the_declared_statement(self):
        import json
        declared = json.loads(
            (CASES / "reference.json").read_text())["obligation"]["pattern"]
        self.assertEqual(report_for("reference.json")["goal"], declared)


class NegativeCaseTest(unittest.TestCase):
    def test_each_negative_case_flags_its_named_rule(self):
        expectations = {
            "undeclared_axiom.json": "undeclared-axiom",
            "axiom_outside_closure.json": "axiom-outside-closure",
            "closure_overstated.json": "closure-overstated",
            "premise_order.json": "premise-order",
            "fixpoint_import.json": "fragment-escape",
            "classical_rule.json": "classical-rule-import",
            "unsupported_rule.json": "unsupported-external-rule",
            "unknown_rule.json": "unknown-external-rule",
            "malformed_step.json": "malformed-step",
            "duplicate_id.json": "malformed-step",
            "goal_mismatch.json": "goal",
            "recheck_failure.json": "translation-recheck",
        }
        for name, expected in expectations.items():
            with self.subTest(case=name):
                result = report_for(name)
                self.assertEqual(result["verdict"], "reject")
                self.assertEqual(rule(result), expected)

    def test_cli_exit_codes_track_expect(self):
        import subprocess
        tool = ["python3", str(TOOL), "check"]
        positive = subprocess.run(
            tool + [str(CASES / "reference.json")],
            cwd=ROOT, capture_output=True)
        negative = subprocess.run(
            tool + [str(CASES / "classical_rule.json")],
            cwd=ROOT, capture_output=True)
        self.assertEqual(positive.returncode, 0)
        self.assertEqual(negative.returncode, 0)  # expected reject still exits 0

    def test_forward_reference_rejects(self):
        import json
        case = json.loads((CASES / "reference.json").read_text())
        for step in case["export"]["steps"]:
            if step["id"] == "u1":
                step["of"] = "g"
        result = importer.check_case_data(case, "test", ROOT)
        self.assertEqual(result["verdict"], "reject")
        self.assertEqual(rule(result), "premise-order")

    def test_citation_outside_cone_still_counts(self):
        import json
        case = json.loads((CASES / "reference.json").read_text())
        case["export"]["steps"].append(
            {"id": "extra", "rule": "AXIOM", "name": "s_nat"})
        result = importer.check_case_data(case, "test", ROOT)
        self.assertEqual(rule(result), "axiom-outside-closure")


if __name__ == "__main__":
    unittest.main()
