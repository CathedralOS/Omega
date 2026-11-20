"""Regression checks for measurements that could otherwise give a false green."""
import unittest
from unittest.mock import patch
from pathlib import Path
from tempfile import TemporaryDirectory
import frontend_diagnostics
import handoff_evidence
import reviewed_comparisons

from check_frontend_readiness import PRODUCT_CASES, context_results
from check_layer_readiness import case_blocks_readiness, collect, compare_fragment_runs, failure_excerpt, incomplete_cohorts, readiness_exit
from test_l5_records import SAMPLE_LINES, contracts, libtest, sample, spec


class LayerReportTests(unittest.TestCase):
    def test_a_refusal_cannot_verify_a_promised_psi_output(self):
        section = {"capabilities": ["The output retains its operand."], "asserted": ["bad-input"]}
        self.report["lowering_evidence"] = {"contract": [section], "assertions": {"bad-input": {"role": "refusal"}}}
        self.assertEqual(readiness_exit(self.report), 1)
        self.report["lowering_evidence"]["assertions"]["operand"] = {"role": "producer"}
        section["asserted"].append("operand")
        self.assertEqual(readiness_exit(self.report), 0)

    def test_matching_counts_cannot_hide_a_different_fragment_stop_or_source_location(self):
        frontend = {"same": {"status": "refused", "stage": "L2", "package": "example", "file": "example.omg",
                             "line": 2, "byte_column": 3, "byte_start": 9, "byte_end": 10, "reason": "unknown name"},
                    "replaced": {"status": "prepared"}}
        case = {"cohort": "spec", "stage": "L2", "detail": "example.omg:2:4: unknown name (bytes 10..11)", "status": "recorded refusal"}
        self.report["frontend_diagnostics"] = {"fragments": frontend}
        self.report["cases"] = {"spec/same": case, "spec/replacement": case}
        agreement = compare_fragment_runs(self.report)
        self.assertEqual(set(agreement["differences"]), {"same", "replaced", "replacement"})
        self.assertEqual(agreement["compared"], 1)
        case["detail"] = "example.omg:2:3: unknown name (bytes 9..10)"
        self.assertNotIn("same", compare_fragment_runs(self.report)["differences"])
        case["stage"] = "L3"
        self.assertIn("same", compare_fragment_runs(self.report)["differences"])

    def setUp(self):
        self.report = {"cases": {}, "expected": {}, "obligations": {}, "package_fixtures": [], "measurement_errors": [], "suites": {}}

    def test_independent_runs_preserve_the_full_reason_and_selected_source(self):
        label = "chapter.md:17"
        reason = "lookup %250A failed%0D%0Acalled from%09main"
        stop = f"OMEGA_FRONTEND_STOP\t{label}\tL3\texample\texample.omg\t4\t5\t9\t13\tπ%0D%0A\t{reason}"
        case = f"OMEGA_CASE\tspec\t{label}\tL3\trecorded refusal\texample.omg:4:5: {reason} (bytes 9..13)"
        frontend_diagnostics.collect(self.report, stop)
        earlier = self.report["frontend_diagnostics"]["fragments"][label]
        self.assertEqual(earlier["occurrence"], "π\r\n")
        self.assertEqual(earlier["reason"], "lookup %0A failed\r\ncalled from\tmain")
        collect(spec([case], status="ok"), "L5", self.report)
        self.assertEqual(compare_fragment_runs(self.report)["differences"], {})
        self.report["cases"].clear()
        collect(spec([case.replace("%0D%0A", "  ")], status="ok"), "L5", self.report)
        self.assertIn(label, compare_fragment_runs(self.report)["differences"])
        frontend_diagnostics.collect(self.report, stop.replace("9\t13", "9\t12"))
        self.assertNotIn(label, self.report["frontend_diagnostics"]["fragments"])

    def test_same_typing_refusal_after_static_evaluation_has_different_reach(self):
        label = "chapter.md:72"
        marker = f"OMEGA_FRONTEND_STOP\t{label}\tL3-resumed\texample\texample.omg\t1\t1\t0\t1\tx\twrong type"
        frontend_diagnostics.collect(self.report, marker)
        frontend = self.report["frontend_diagnostics"]["fragments"][label]
        self.assertEqual(frontend["later_layers_not_reached"], ["L5"])
        case = f"OMEGA_CASE\tspec\t{label}\tL3\trecorded refusal\texample.omg:1:1: wrong type (bytes 0..1)"
        collect(spec([case], status="ok"), "L5", self.report)
        initial = self.report["cases"]["spec/" + label]
        self.assertEqual(initial["later_layers_not_reached"], ["L4", "L5"])
        difference = compare_fragment_runs(self.report)["differences"][label]
        self.assertIn("L3 after static evaluation", difference)
        self.assertIn("L3 initial typing", difference)
        self.report["cases"].clear()
        collect(spec([case.replace("\tL3\t", "\tL3-resumed\t")], status="ok"), "L5", self.report)
        self.assertEqual(compare_fragment_runs(self.report)["differences"], {})

    def test_foreign_source_offsets_must_agree_even_without_source_text(self):
        label = "chapter.md:90"
        frontend_diagnostics.collect(self.report, f"OMEGA_FRONTEND_STOP\t{label}\tL2\tdep\texample.omg\t\t\t7\t8\t\tmissing name")
        actual = "dep/example.omg: missing name (bytes 1..2)"
        case = {"cohort": "spec", "stage": "L2", "status": "recorded refusal", "detail": actual}
        self.report["cases"]["spec/" + label] = case
        self.assertIn(actual, compare_fragment_runs(self.report)["differences"][label])
        case["detail"] = actual.replace("1..2", "7..8")
        self.assertEqual(compare_fragment_runs(self.report)["differences"], {})

    def test_passing_sample_must_match_pinned_bytes_even_at_the_same_position(self):
        altered = [line.replace("\tMain::main\t", "\tMain::lost\t")
                   if line.startswith("OMEGA_SAMPLE_REFUSAL\tcross-width\t") else line for line in SAMPLE_LINES]
        collect(sample(altered), "L5", self.report)
        self.assertEqual(self.report["measurement_errors"], [
            "L5: sample/cross-width compiler refusal differs from its pinned source bytes or diagnostic"])

    def test_unicode_line_separator_inside_a_refusal_remains_part_of_the_reason(self):
        original = "a data carrier cannot own a machine"
        reason = original + "\u2028next occurrence"
        changed = [line.replace(original, reason) if "\tcross-width\t" in line else line for line in SAMPLE_LINES]
        collect(sample(changed, status="FAILED"), "L5", self.report)
        self.assertEqual(self.report["measurement_errors"], [])
        self.assertEqual(self.report["cases"]["sample/cross-width"]["compiler_refusal"]["reason"], reason)

    def test_frontend_reports_wrong_stage_and_missing_witness_without_losing_other_locations(self):
        for stage in frontend_diagnostics.PRODUCTS:
            self.report["suites"][stage] = {"tests": []}
        frontend_diagnostics.collect(self.report,
            "OMEGA_FRONTEND_COUNT\t1\nOMEGA_FRONTEND\tL4\twrong_array_identity_after_evaluation\tprepare\ttyper\tcase.omg\t2\t3\t9\t10\tgood-twin-prepares,lower-repeats-stop\twrong type\n"
            "OMEGA_FRONTEND_STOP\tchapter.md:30\tL2\texample\texample.omg\t2\t3\t9\t10\tx\tunknown x\n")
        observed = self.report["frontend_diagnostics"]["fragments"]["chapter.md:30"]
        self.assertEqual((observed["stage"], observed["spec_line"], observed["byte_column"], observed["occurrence"]), ("L2", 32, 3, "x"))
        self.assertIn("missing verified CLI diagnostic: wrong_array_identity_after_evaluation", self.report["measurement_errors"])
        self.assertFalse(self.report["frontend_diagnostics"]["products"]["L3"]["verified"])

    def test_lowering_diagnostic_requires_its_own_successful_cli_test_and_stage_controls(self):
        name = "attributed_alias_refused_by_lowering"
        line = f"OMEGA_FRONTEND\tL5\t{name}\tlower\tlowering\tcase.omg\t2\t21\t30\t33\tgood-twin-lowers,bad-input-prepares\tunsupported alias"
        def probe(test, status):
            log = f"Running tests/frontend_diagnostics.rs (target/test)\ntest {test} ... {status}\n---- {test} stdout ----\n{line}"
            return list(frontend_diagnostics.witness_lines(self.report, log, True))
        self.assertEqual(probe(name, "ok"), [line])
        self.assertEqual(probe("other", "ok"), [])
        self.assertEqual(probe(name, "FAILED"), [])
        self.report["last_layer"] = "L5"
        frontend_diagnostics.collect(self.report, line.replace("bad-input-prepares", "bad-input-fails"))
        self.assertNotIn(name, self.report["frontend_diagnostics"]["cli"])
        self.report["last_layer"] = "L4"
        self.assertNotIn(name, frontend_diagnostics.active_cases(self.report))

    def test_frontend_product_credit_follows_the_assertion_in_its_passing_owner(self):
        # Parse the captured layer stdout first, just as the measurement runner does.
        # Even a failed comparison after a first success must revoke the product credit.
        for stage, (target, name, _) in frontend_diagnostics.PRODUCTS.items():
            label = "line-comments/crlf-product" if stage == "L0" else f"product/{stage.lower()}"
            success = f"OMEGA_REVIEWED_COMPARISON\t{label}\tassert_eq!(actual, expected)"
            failure = success.replace("OMEGA_REVIEWED", "OMEGA_FAILED")
            binary = f"tests\\{target}.rs"
            for fault, status, owned, other in [
                (None, "ok", [success], []),
                ("missing", "ok", [], []),
                ("swapped owner", "ok", [], [success]),
                ("duplicate", "ok", [success, success], []),
                ("failed test", "FAILED", [success], []),
                ("failed comparison", "ok", [success, failure], []),
            ]:
                with self.subTest(stage=stage, fault=fault):
                    report = {"last_layer": "L5", "measurement_errors": [], "cases": {}, "suites": {}}
                    output = libtest(binary, (name, status, owned), ("other", "ok", other))
                    tests, _ = collect(output, stage, report)
                    report["suites"][stage] = {"tests": tests}
                    frontend_diagnostics.collect(report, "")
                    witness = report["frontend_diagnostics"]["products"][stage]
                    self.assertEqual(witness["comparison"], label)
                    self.assertEqual(witness["verified"], fault is None)
                    self.assertEqual(any(f"{stage} product witness" in e for e in report["measurement_errors"]), fault is not None)

    def test_before_lowering_does_not_require_the_l5_product_comparison(self):
        self.report["last_layer"] = "L4"
        frontend_diagnostics.collect(self.report, "")
        self.assertEqual(set(self.report["frontend_diagnostics"]["products"]), {"L0", "L1", "L2", "L3", "L4"})
        self.assertFalse(any("L5 product witness" in e for e in self.report["measurement_errors"]))

    def test_complete_fragment_count_cannot_certify_a_failed_comparison(self):
        for stage in frontend_diagnostics.PRODUCTS:
            self.report["suites"][stage] = {"tests": []}
        self.report["suites"]["L4"]["tests"].append({
            "target": "tests/spec_corpus.rs", "test": "every_spec_fragment_has_a_preparation_outcome", "status": "FAILED"})
        frontend_diagnostics.collect(self.report, "OMEGA_FRONTEND_COUNT\t1\nOMEGA_FRONTEND_PREPARED\tchapter.md:30")
        self.assertFalse(self.report["frontend_diagnostics"]["fragment_expectations_verified"])
        self.assertTrue(any("expectations or source ranges unverified" in error for error in self.report["measurement_errors"]))

    def test_conflicting_observations_do_not_credit_either_outcome_for_that_fragment(self):
        done = "OMEGA_FRONTEND_PREPARED\tchapter.md:30"
        stop = "OMEGA_FRONTEND_STOP\tchapter.md:30\tL2\texample\texample.omg\t1\t1\t0\t1\tx\tunknown x"
        for records in ((done, stop), (stop, done), (stop, stop.replace("0\t1\tx", "0\t2\tx")), (stop, stop)):
            frontend_diagnostics.collect(self.report, "\n".join(("OMEGA_FRONTEND_COUNT\t1", *records)))
            self.assertNotIn("chapter.md:30", self.report["frontend_diagnostics"]["fragments"])
            self.assertTrue(self.report["measurement_errors"])

    def test_equal_value_cannot_hide_the_wrong_declaration_or_a_missing_assertion(self):
        for twin, test in handoff_evidence.IDENTITY_TESTS.items():
            for stage in handoff_evidence.DOCUMENTS:
                actual = "FIRST" if stage == "L3" else twin
                handoff_evidence.record(self.report, "L4", "tests/constant_identity_handoff.rs", ("", test),
                                        f"OMEGA_CONSTANT_IDENTITY\t{stage}\t{twin}\t{actual}")
        # A green test name cannot replace the comparison that ran inside it.
        test = handoff_evidence.ASSERTIONS["field-recipe/true"][2]
        self.report["suites"]["L4"] = {"tests": [{"target": "tests/frozen_handoff.rs", "test": test, "status": "ok"}], "exit": 0}
        self.report["suites"]["L4"]["tests"].extend(
            {"target": "tests/constant_identity_handoff.rs", "test": name, "status": "ok" if twin == "FIRST" else "FAILED"}
            for twin, name in handoff_evidence.IDENTITY_TESTS.items())
        Path("target").mkdir(exist_ok=True)
        with TemporaryDirectory(dir="target") as temp:
            handoff_evidence.finish(self.report, Path(temp))
        errors = self.report["measurement_errors"]
        mismatch = self.report["cases"]["handoff/constant identity/L3/OTHER"]
        self.assertEqual(mismatch["detail"], "COPY = OTHER: expected OTHER, observed 'FIRST'")
        self.assertTrue(any("field recipe under true returns false" in e for e in errors))
        rows = self.report["handoff_rules"]
        self.assertFalse(next(r for r in rows["L2"] if r["rule"] == "Namespaces and imports")["consumer"])
        self.assertFalse(next(r for r in rows["L4"] if r["rule"].startswith("Named reads connect"))["direct"])
        self.assertEqual(mismatch["later_layers_not_reached"], ["L4"])
        self.assertTrue(any("unexpected identity observation after L3" in e for e in errors))
        self.assertEqual(readiness_exit(self.report), 1)

    def test_a_new_rule_enters_the_inventory_without_a_test_mapping(self):
        rows = handoff_evidence.document_rules("L0", "## Rules\n\n| R18 | New promise | Source | — | — |\n")
        self.assertEqual([(row["rule"], row["description"], row["fixtures"]) for row in rows], [("R18", "New promise", [])])
        with self.assertRaises(ValueError):
            handoff_evidence.document_rules("L0", "## Other rules\n")

    def test_green_suites_cannot_pass_with_an_unmeasured_handoff_inside_the_requested_scope(self):
        row = {"rule": "product", "line": 7, "direct": ["exact product"], "consumer": [], "refusal_only_rule": False}
        outlines = {"L4": [row]}
        self.assertEqual(handoff_evidence.unverified_contracts(outlines, "L4"), [])
        self.report["unverified_contracts"] = handoff_evidence.unverified_contracts(outlines, "L5")
        self.assertEqual(self.report["unverified_contracts"][0]["missing"], ["consumer"])
        self.assertEqual(readiness_exit(self.report), 1)

    def test_reviewed_comparison_needs_a_receipt_and_a_passing_test_in_scope(self):
        witness = next(w for w in reviewed_comparisons.registrations() if w.suite == "L5" and w.side == "consumer")
        current = (witness.target, witness.test)
        line = f"OMEGA_REVIEWED_COMPARISON\t{witness.comparison}\tassert_eq!(actual, expected)"
        reviewed_comparisons.sources()  # Validate the complete registry before selecting one row.
        with patch.object(reviewed_comparisons, "registrations", return_value=[witness]):
            for limit, status, emitting_test, credit, missing in [
                ("L5", "ok", current, True, False),
                ("L5", "ok", None, False, True),
                ("L5", "ok", (witness.target, "other"), False, True),
                ("L5", "FAILED", current, False, False),
                ("L4", None, None, False, False),
            ]:
                with self.subTest(limit=limit, status=status, emitting_test=emitting_test):
                    report = {"measurement_errors": [], "last_layer": limit, "suites": {}}
                    if status:
                        report["suites"][witness.suite] = {"tests": [{"target": witness.target, "test": witness.test, "status": status}]}
                    if emitting_test:
                        reviewed_comparisons.record(report, witness.suite, witness.target, emitting_test, line)
                    row = {"rule": witness.rule}
                    reviewed_comparisons.apply(report, {witness.owner: [row]})
                    self.assertEqual(bool(row.get("consumer")), credit)
                    self.assertEqual(bool(report["measurement_errors"]), missing)
                    self.assertEqual(bool(row.get("outside_run")), limit == "L4")
                    self.assertEqual(bool(row.get("failed_tests")), status == "FAILED")

    def test_first_success_cannot_hide_a_later_failure_at_the_same_comparison(self):
        witness = next(w for w in reviewed_comparisons.registrations() if w.side == "consumer")
        expr = "assert_eq!(actual, expected)"
        logs = (f"Running {witness.target} (target/test)\ntest {witness.test} ... FAILED\n"
                f"---- {witness.test} stdout ----\nOMEGA_REVIEWED_COMPARISON\t{witness.comparison}\t{expr}\n"
                "thread 'example' panicked at file.rs:10:4:\nassertion failed\n  left: 2\n right: 1\n"
                "note: run with `RUST_BACKTRACE=1`\n"
                f"OMEGA_FAILED_COMPARISON\t{witness.comparison}\t{expr}\n"
                "test result: FAILED. 0 passed; 1 failed; 0 ignored\n")
        tests, _ = collect(logs, witness.suite, self.report)
        self.report["last_layer"] = "L5"
        self.report["suites"][witness.suite] = {"tests": tests, "exit": 101}
        row = {"rule": witness.rule}
        with patch.object(reviewed_comparisons, "registrations", return_value=[witness]):
            reviewed_comparisons.apply(self.report, {witness.owner: [row]})
        self.assertFalse(row.get("consumer"))
        self.assertIn(witness.comparison, row["failed_checks"][0])
        self.assertEqual(failure_excerpt(tests[0]["diagnostic"])[-2:], ["  left: 2", " right: 1"])
        self.assertEqual(readiness_exit(self.report), 1)

    def test_fixture_evidence_must_come_from_its_comparison(self):
        line = "OMEGA_GOLDEN_MATCH\tlayers/01_tokens-to-syntax-trees/fixtures/accept/right_shift.omg"
        handoff_evidence.record(self.report, "L1", "tests/golden_fixtures.rs", ("", "some_other_test"), line)
        self.assertFalse(self.report["handoff_observations"]["goldens"])
        self.assertTrue(self.report["measurement_errors"])

    def test_other_passing_fixtures_cannot_replace_the_reviewed_refusal(self):
        witness = next(w for w in reviewed_comparisons.registrations() if w.comparison == "fixture/reject/codepoint_escape.omg")
        report = {"last_layer": "L4", "measurement_errors": [], "handoff_observations": {"goldens": {
            "L0/reject/newline_in_quoted.omg": witness.test}},
            "suites": {"L0": {"tests": [{"target": witness.target, "test": witness.test, "status": "ok"}]}}}
        row = {"rule": witness.rule}
        with patch.object(reviewed_comparisons, "registrations", return_value=[witness]):
            reviewed_comparisons.apply(report, {"L0": [row]})
        self.assertFalse(row.get("refusals"))
        self.assertIn("fixture/reject/codepoint_escape.omg", report["measurement_errors"][0])

    def test_repeated_fixture_invocations_retain_both_inputs_and_outcomes(self):
        name = "a_different_selector_never_repairs_the_extracted_element"
        refusal = "refused: element restoration needs the identical selector value of the extracted element at view.omg"
        out = libtest("tests\\view_loans.rs", (name, "ok", [
            f"OMEGA_FIXTURE\t{name}\tview.omg\tBorrow=2,Move=1,VerifyLibraryImplementation=1\tproduced Psi",
            f"OMEGA_FIXTURE\t{name}\tview.omg\tBorrow=2,Move=2,RestoreBorrowed=1,VerifyLibraryImplementation=1\t{refusal}"]))
        collect(out, "L5", self.report)
        self.assertEqual(self.report["measurement_errors"], [])
        self.assertEqual([row["obligations"]["Move"] for row in self.report["package_fixtures"]], [1, 2])
        self.assertEqual(self.report["package_fixtures"][1]["outcome"], refusal)
        self.assertEqual({(row["target"], row["test"]) for row in self.report["package_fixtures"]}, {("tests\\view_loans.rs", name)})

    def test_matching_a_known_limitation_still_blocks_readiness(self):
        tests, counts = collect(contracts(), "L5", self.report)
        self.assertEqual(counts["passed"], 2)
        self.assertEqual([test["status"] for test in tests], ["ok", "ok"])
        self.assertEqual(self.report["measurement_errors"], [])
        self.assertEqual(self.report["cases"]["handoff/call in a contract predicate"]["status"], "open gap")
        case = self.report["cases"]["handoff/call in a contract predicate"]
        self.assertEqual(case["fixture_locations"], [self.report["package_fixtures"][0]["location"]])
        self.assertEqual(case["fixture_locations"][0]["occurrence"], "predicate()")
        self.assertEqual(readiness_exit(self.report), 1)

    def test_expected_rejection_and_standalone_fragment_have_different_meanings_from_a_blocked_sample(self):
        self.report["cases"] = {
            "negative": {"cohort": "sample", "status": "intended rejection verified"},
            "fragment": {"cohort": "spec", "status": "missing context"},
        }
        self.assertEqual(readiness_exit(self.report), 0)
        self.report["cases"]["package"] = {"cohort": "sample", "status": "missing context"}
        self.assertEqual(readiness_exit(self.report), 1)

    def test_missing_case_and_duplicate_row_are_reported(self):
        row = "OMEGA_CASE\tspec\tchapter.md:10\tL2\trecorded refusal\texample.omg:1:1: unresolved name Build (bytes 0..5)"
        collect(spec(["OMEGA_INVENTORY\tMove,Borrow", "OMEGA_EXPECTED\tspec\t2", row, row,
                      "OMEGA_OBLIGATION\tchapter.md:20\tMove\t3"]), "L5", self.report)
        self.assertIn("duplicate result for spec/chapter.md:10", self.report["measurement_errors"])
        incomplete = list(incomplete_cohorts(self.report))
        self.assertIn("spec: observed 1 results, expected 2", incomplete)
        self.assertIn("spec/chapter.md:20: obligation counts without a case result", incomplete)
        self.assertEqual(self.report["obligations"], {"Move": {"chapter.md:20": 3}, "Borrow": {}})

    def test_a_summary_without_the_names_is_incomplete(self):
        collect("test other ... ok\ntest result: FAILED. 1 passed; 1 failed; 0 ignored\n", "L1", self.report)
        self.assertTrue(self.report["measurement_errors"])
        self.assertEqual(readiness_exit(self.report), 1)

    def test_unrecognized_status_cannot_pass(self):
        line = "OMEGA_CASE\tspec\tchapter.md:10\tL5\tunexpected acceptance\tL5 lowered field"
        collect(spec(["OMEGA_INVENTORY\tMove", "OMEGA_EXPECTED\tspec\t1", line], status="ok"), "L5", self.report)
        self.assertEqual(self.report["measurement_errors"], [f"L5: malformed case result {line}"])
        self.assertEqual(self.report["cases"], {})
        self.assertEqual(readiness_exit(self.report), 1)

    def test_context_verdict_requires_every_independent_witness_in_its_own_section(self):
        label = "language/some_packages.md:41"
        good, bad = ["  PASS " + witness for witness in PRODUCT_CASES[label]]
        verdict = "PASS " + label
        log = [label, good, bad, verdict]
        self.assertEqual(context_results(label, log), (True, [good.strip(), bad.strip()], None))
        for fault in ([label, good, verdict], [label, good, good, verdict], [label, good, verdict, bad],
                      [*log, "FAIL " + label], [label, good, bad.replace("PASS", "FAIL"), verdict],
                      [label, good, "language/reflection.md:77", bad, verdict]):
            with self.subTest(fault=fault):
                passed, _, error = context_results(label, fault)
                self.assertFalse(passed)
                self.assertIsNotNone(error)
        bad = bad.replace("PASS", "FAIL") + " — L2 unresolved name"
        self.assertEqual(context_results(label, [label, good, bad, "FAIL " + label]),
                         (False, [good.strip(), bad.strip()], None))

    def test_a_lowered_example_with_a_wrong_or_missing_snapshot_is_a_blocking_case(self):
        case = {"cohort": "spec", "status": "executable product inspected"}
        for verdict in ["mismatched expectation", "absent expectation", "unverified"]:
            self.assertTrue(case_blocks_readiness({**case, "expectation_check": {"verdict": verdict}}))
        self.assertFalse(case_blocks_readiness({**case, "expectation_check": {"verdict": "passed"}}))

    def test_each_failed_assertion_keeps_its_own_source_diagnostic(self):
        out = libtest("tests\\fields.rs", ("before", "FAILED", []), ("after", "FAILED", []))
        reasons = ["L2\tfile:1:2\tunknown name", "L5\tfile:2:1\twrong owner"]
        for reason in reasons:
            out = out.replace("assertion failed: failures.is_empty()", reason, 1)
        out += spec(["OMEGA_CASE\tspec\tchapter.md:10\tL2\trecorded refusal\texample.omg:1:1: unresolved name Build (bytes 0..5)"], status="ok")
        tests, counts = collect(out, "L5", self.report)
        self.assertEqual(counts["failed"], 2)
        self.assertEqual([failure_excerpt(test["diagnostic"])[1:] for test in tests[:2]], [[reason] for reason in reasons])
        self.assertTrue(all(" panicked at " in "\n".join(test["diagnostic"]) for test in tests[:2]))
        self.assertEqual(self.report["cases"]["spec/chapter.md:10"]["later_layers_not_reached"], ["L3", "L4", "L5"])
        self.assertEqual(self.report["cases"]["spec/chapter.md:10"]["test"], "spec_fragments_have_reviewed_lowering_outcomes")


if __name__ == "__main__":
    unittest.main()
