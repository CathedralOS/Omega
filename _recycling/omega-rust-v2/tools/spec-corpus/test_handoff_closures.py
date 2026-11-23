"""Exercise closure decisions using libtest results and captured L5 assertion records."""
from io import StringIO
import unittest
from unittest.mock import Mock, patch

import handoff_closures as closures
import lowering_evidence as lowering
from check_layer_readiness import case_blocks_readiness

HEADER = "case\tscope\tchecks\n"
CASE = "example gap"
KEY = "handoff/" + CASE
JOIN = "psi-output/guard-join"
CALL = "psi-output/call-operands"
EXPR = "assert_eq!(joins[0], joins[1])"
CHECKS = {label: lowering.ASSERTIONS[label] for label in [JOIN, CALL]}


def capture(label, expr):
    check = CHECKS[label]
    return f"Running {check.target} (binary)\n---- {check.test} stdout ----\nOMEGA_L5_ASSERT\t{label}\t{expr}\n"


def measure(*, statuses=("ok",), join=capture(JOIN, EXPR), scope="product", cases=None, refusals=(), registry_error=None):
    tests = [{"target": CHECKS[label].target, "test": CHECKS[label].test, "status": status}
             for label, outcomes in [(JOIN, statuses), (CALL, ["ok"])] for status in outcomes]
    report = {"last_layer": "L5", "suites": {"L5": {"tests": tests}}, "measurement_errors": [], "cases": dict(cases or {})}
    with patch.object(lowering, "ASSERTIONS", CHECKS):
        errors = report["measurement_errors"]
        receipts, _ = lowering.observe(join + capture(CALL, "assert_eq!(call, expected)"), errors)
        states = lowering.credit_assertions(receipts, lowering.test_results(report), errors)
        report["lowering_evidence"] = {"assertions": {label: dict(check._asdict(), state=states[label], comparison=receipts.get(label))
                                                     for label, check in CHECKS.items()}}
        with patch.object(closures, "REGISTRY") as registry, patch.object(closures.refusal_evidence, "registrations",
                                                                                       return_value=refusals, side_effect=registry_error):
            registry.open.return_value = StringIO(f"{HEADER}{CASE}\t{scope}\t{JOIN}|{CALL}\n")
            closures.apply(report)
    return report


class HandoffClosureTests(unittest.TestCase):
    def test_all_compared_expressions_and_their_tests_stay_with_the_case(self):
        for scope in closures.STATUS:
            with self.subTest(scope=scope):
                report = measure(scope=scope)
                case = report["cases"][KEY]
                self.assertEqual(report["measurement_errors"], [])
                self.assertEqual(case["status"], closures.STATUS[scope])
                self.assertFalse(case_blocks_readiness(case))
                self.assertEqual(set(case["output_receipts"]), {JOIN, CALL})
                self.assertEqual(case["output_receipts"][JOIN]["comparison"], EXPR)
                text = "\n".join(closures.summary(report))
                for value in [KEY, JOIN, CALL, EXPR, CHECKS[JOIN].target, CHECKS[JOIN].test]:
                    self.assertIn(value, text)
                self.assertEqual("judgement pending" in text, scope == "checker-input")

    def test_a_green_sibling_never_substitutes_for_a_missing_or_invalid_check(self):
        good = capture(JOIN, EXPR)
        for statuses, log in [((), good), (("FAILED",), good), (("ok", "ok"), good), (("ok",), ""),
                              (("ok",), good + good), (("ok",), good.replace("\t" + EXPR, "")),
                              (("ok",), good.replace(EXPR, "assert_eq!(joins[0],")),
                              (("ok",), good.replace(CHECKS[JOIN].test, "wrong")),
                              (("ok",), good.replace("OMEGA_L5_ASSERT", "---- other stderr ----\nOMEGA_L5_ASSERT"))]:
            with self.subTest(statuses=statuses, log=log):
                report = measure(statuses=statuses, join=log)
                case = report["cases"][KEY]
                self.assertTrue(case_blocks_readiness(case))
                self.assertEqual(case["status"], "inspection failed")
                self.assertEqual(set(case["output_receipts"]), {CALL})
                self.assertTrue(any(KEY in error and JOIN in error for error in report["measurement_errors"]))

    def test_no_closure_over_an_open_case_or_an_unobserved_refusal_registration(self):
        open_case = {"status": "open gap"}
        for cases, refusals in [({KEY: open_case}, []), ({}, [Mock(gap_labels=[CASE])])]:
            report = measure(cases=cases, refusals=refusals)
            self.assertEqual(report["cases"], cases)
            self.assertTrue(any("conflicts" in error for error in report["measurement_errors"]))
            self.assertNotIn("output_receipts", open_case)

    def test_an_invalid_refusal_registry_credits_no_closure(self):
        report = measure(registry_error=ValueError("duplicate gap label: example gap"))
        case = report["cases"][KEY]
        self.assertEqual(case["status"], "inspection failed")
        self.assertTrue(case_blocks_readiness(case))
        self.assertIn("duplicate gap label", case["detail"])
        self.assertEqual(len(report["measurement_errors"]), 1, report["measurement_errors"])
        self.assertIn("closures not credited", report["measurement_errors"][0])
        self.assertEqual(closures.summary(report), [])

    def test_registry_rejects_duplicate_cases_conflicting_scopes_and_non_product_labels(self):
        row = f"{CASE}\tproduct\t{JOIN}\n"
        negative = "psi-output/test-refusal"
        with patch.dict(lowering.ASSERTIONS, {negative: CHECKS[JOIN]._replace(role="refusal")}):
            for body in [row + row, row + row.replace("product", "checker-input"), row.replace("product", "other"),
                         row.replace(JOIN, JOIN + "|" + JOIN), row.replace(JOIN, "psi-output/unknown"),
                         row.replace(JOIN, negative), row.replace(JOIN, ""), row.replace(JOIN, "obligation-output/zero-value")]:
                with self.subTest(body=body), self.assertRaises(ValueError):
                    closures.parse(StringIO(HEADER + body))

    def test_no_registry_rows_or_before_l5_produce_no_closures(self):
        self.assertEqual(closures.parse(StringIO(HEADER)), {})
        report = {"last_layer": "L4"}
        closures.apply(report)
        self.assertEqual(report, {"last_layer": "L4"})
