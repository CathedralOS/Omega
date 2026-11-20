"""Regression checks for the L5 contract output report. Run from the V2 workspace."""
from pathlib import Path
import os
import re
from tempfile import TemporaryDirectory
import unittest
from unittest import mock

from check_layer_readiness import collect
import frontend_diagnostics
import lowering_evidence as evidence
from test_l5_records import libtest

PROBE_TARGET, PROBE_TEST = evidence.PROBE


def report(**fields):
    base = {"last_layer": "L5", "measurement_errors": [], "suites": {"L5": {"tests": []}}, "cases": {},
            "expected": {}, "obligations": {}, "package_fixtures": []}
    base.update(fields)
    return base


def obligation_kinds():
    source = (evidence.LAYER / "tests/tree-consumer/src/obligation_report.rs").read_text(encoding="utf-8")
    return set(re.findall(r"([A-Z][A-Za-z]+)\s*[({]", source.split("obligation_names! {")[-1]))


def probe_log(*lines, target=PROBE_TARGET.replace("/", "\\"), test=PROBE_TEST):
    return "\n".join([f"     Running {target} (target\\debug\\deps\\x.exe)", "", "successes:", "",
                      f"---- {test} stdout ----", *lines, "", "successes:", f"    {test}", "",
                      "test result: ok. 1 passed; 0 failed; 0 ignored"])


class ContractInventoryTests(unittest.TestCase):
    def test_a_run_ending_at_l4_records_nothing(self):
        observed = report(last_layer="L4")
        evidence.finish(observed, Path("missing-directory"))
        self.assertNotIn("lowering_evidence", observed)
        self.assertEqual(evidence.summary(observed), [])
        self.assertEqual(observed["measurement_errors"], [])

    def test_sentences_separate_capabilities_limits_and_delegated_judgments(self):
        source = ("# L5 Lowering\n\nL5 lowers packages. The checker owns the verdict.\n\n"
                  "## First handoff\n\nA return carries its value. [Handoff tests](tests/handoff.rs) · [Spec](../../../wiki/x.md) Casts still need lowering.\n\n"
                  "## Operations and declarations\n\nCalls retain callees.\n\n"
                  "## Handoffs and remaining work\n\nAssembly memory effects remain open.\n\n"
                  "## Evidence for sign-off\n\nRun everything.\n")
        rows = evidence.contract_entries(source)
        self.assertEqual([row["section"] for row in rows], list(evidence.CONTRACT_SECTIONS))
        self.assertEqual(rows[0]["delegated"], ["The checker owns the verdict."])
        self.assertEqual((rows[1]["capabilities"], rows[1]["limits"]), (["A return carries its value."], ["Casts still need lowering."]))
        self.assertEqual(rows[1]["targets"], ["tests/handoff.rs"])
        self.assertEqual(rows[3]["limits"], ["Assembly memory effects remain open."])
        self.assertFalse(any("Run everything" in row["text"] for row in rows))
        with self.assertRaises(ValueError):
            evidence.contract_entries("# L5\n\nText.\n\n## First handoff\n\nOnly one section.\n")

    def test_each_registered_check_names_one_contract_paragraph_and_an_existing_test(self):
        rows = evidence.contract_entries((evidence.LAYER / "CONTRACT.md").read_text(encoding="utf-8"))
        kinds = obligation_kinds()
        for label, assertion in evidence.ASSERTIONS.items():
            with self.subTest(label=label):
                self.assertEqual(sum(row["text"].startswith(assertion.entry) for row in rows), 1)
                self.assertIn(assertion.role, {"producer", "refusal"})
                self.assertLessEqual(set(assertion.obligations), kinds, "registered obligation kinds exist")
                self.assertEqual(len(set(assertion.obligations)), len(assertion.obligations))
                source = (evidence.LAYER / assertion.target).read_text(encoding="utf-8")
                body = source.split(f"fn {assertion.test}()", 1)
                self.assertEqual(len(body), 2, "registered test exists")
                test_body = body[1].split("#[test]", 1)[0]
                if label.startswith("psi-output/"):
                    self.assertRegex(test_body, rf'observe_handoff!\(\s*psi "{re.escape(label)}",\s*assert(?:_eq|_ne)?!\(')
                else:
                    self.assertIn(f'observed("{label}")', test_body, "the test prints its own receipt")

    def test_every_typed_obligation_kind_has_one_reconciliation_row(self):
        kinds = obligation_kinds()
        rows = evidence.obligation_rows((evidence.LAYER / "LOCK_EVIDENCE.md").read_text(encoding="utf-8"))
        listed = [kind for row in rows for kind in row["kinds"]]
        self.assertEqual(len(listed), len(set(listed)))
        self.assertEqual(set(listed), kinds)


class ReceiptTests(unittest.TestCase):
    def test_receipts_are_attributed_to_the_test_that_printed_them(self):
        log = ("     Running tests\\structural_contracts.rs (target\\debug\\deps\\s.exe)\n"
               "---- a stdout ----\nOMEGA_L5_ASSERT\tone\n\nsuccesses:\nOMEGA_L5_ASSERT\ttwo\n")
        self.assertEqual(list(evidence.scan(log)), [
            ("tests/structural_contracts.rs", "a", "OMEGA_L5_ASSERT\tone"),
            ("tests/structural_contracts.rs", None, "OMEGA_L5_ASSERT\ttwo")])

    def test_a_receipt_counts_only_from_its_registered_and_passing_test(self):
        label, assertion = next(iter(evidence.ASSERTIONS.items()))
        line = f"OMEGA_L5_ASSERT\t{label}"
        for emitted_by, status, state, has_error in [
            (assertion.test, "ok", "credited", False),
            (None, "ok", "missing", True),
            ("another_test", "ok", "missing", True),
            (assertion.test, "FAILED", "failed", False),
            (assertion.test, None, "missing", True),
        ]:
            with self.subTest(emitted_by=emitted_by, status=status):
                errors = []
                log = f"     Running {assertion.target} (x)\n" + (f"---- {emitted_by} stdout ----\n" if emitted_by else "") + line + "\n"
                receipts, _ = evidence.observe(log, errors)
                tests = {(assertion.target, assertion.test): status} if status else {}
                states = evidence.credit_assertions(receipts, {**tests, **{(a.target, a.test): "ok" for k, a in evidence.ASSERTIONS.items() if k != label}},
                                                    errors)
                others = {k for k in evidence.ASSERTIONS if k != label}
                self.assertEqual(states[label], state)
                self.assertEqual(any(label in error for error in errors), has_error)
                self.assertTrue(all(states[k] == "missing" for k in others))

    def test_psi_output_requires_the_comparison_from_its_registered_test(self):
        label = "psi-output/call-operands"
        check = evidence.ASSERTIONS[label]
        header = f"Running {check.target} (binary)\n---- {check.test} stdout ----\nOMEGA_L5_ASSERT\t{label}"
        expression = "assert_eq!(operation.arguments, expected)"
        for suffix, accepted in [("", False), ("\t()", False), ("\t" + expression, True)]:
            with self.subTest(suffix=suffix):
                errors = []
                receipts, _ = evidence.observe(header + suffix + "\n", errors)
                self.assertEqual(label in receipts, accepted)
                self.assertEqual(bool(errors), not accepted)
                if accepted:
                    self.assertEqual(receipts[label], expression)

    def test_a_passing_linked_file_is_not_an_asserted_claim(self):
        rows = [evidence.entry(1, "First handoff", "A return carries its value. [Tests](tests/handoff.rs)"),
                evidence.entry(2, "First handoff", "Unlinked claim."),
                evidence.entry(3, "First handoff", "Broken claim. [Tests](tests/arithmetic.rs)")]
        errors = []
        tests = {("tests/handoff.rs", "a"): "ok", ("tests/arithmetic.rs", "b"): "ok", ("tests/arithmetic.rs", "c"): "FAILED"}
        evidence.attach(rows, {label: "missing" for label in evidence.ASSERTIONS}, tests, errors)
        self.assertEqual([row["status"] for row in rows], ["linked tests passed, claims unregistered", "unmeasured", "linked test file has failures"])
        self.assertTrue(all("expected one contract entry" in error for error in errors))


class OutputProbeTests(unittest.TestCase):
    INVENTORY = "OMEGA_L5_OUTPUT_INVENTORY\tOperationKind::Call,OperationKind::Constant,Package::machines"

    def measure(self, cases, *rows, status="ok", count=2):
        observed = report(cases=cases, expected={"spec": count})
        errors = observed["measurement_errors"]
        _, probe = evidence.observe(probe_log(self.INVENTORY, f"OMEGA_L5_OUTPUT_COUNT\t{count}", *rows), errors)
        return evidence.measure_outputs(probe, status, observed, errors), errors

    def test_unemitted_forms_stay_unmeasured_and_agree_with_the_corpus(self):
        outputs, errors = self.measure(
            {"spec/a.md:1": {"status": "executable product inspected", "stage": "L5"}, "spec/b.md:2": {"status": "open gap", "stage": "L5"}},
            "OMEGA_L5_OUTPUT\ta.md:1\tproduced\tOperationKind::Constant=2,Package::machines=1",
            "OMEGA_L5_OUTPUT\tb.md:2\tstopped\tL5")
        self.assertEqual(errors, [])
        self.assertEqual(outputs["emitted"]["OperationKind::Constant"], {"occurrences": 2, "examples": ["a.md:1"]})
        self.assertEqual(outputs["emitted"]["OperationKind::Call"]["occurrences"], 0)
        self.assertEqual(outputs["examples"], {"produced": 1, "stopped": 1, "panicked": 0})

    def test_disagreement_unknown_forms_and_a_failed_probe_are_incomplete(self):
        _, errors = self.measure(
            {"spec/a.md:1": {"status": "open gap", "stage": "L5"}, "spec/b.md:2": {"status": "recorded refusal", "stage": "L3"}},
            "OMEGA_L5_OUTPUT\ta.md:1\tproduced\tOperationKind::Jump=1",
            "OMEGA_L5_OUTPUT\tb.md:2\tstopped\tL4")
        self.assertTrue(any("disagree for a.md:1" in error for error in errors))
        self.assertTrue(any("disagree for b.md:2" in error for error in errors))
        self.assertTrue(any("uninventoried form OperationKind::Jump" in error for error in errors))
        outputs, errors = self.measure({}, status="FAILED")
        self.assertFalse(outputs["measured"])
        self.assertTrue(any("Psi output forms unmeasured" in error for error in errors))

    @staticmethod
    def refusal(raw_stage):
        """A recorded spec refusal as the runner stores it after normalizing the emitted stage token."""
        return {"status": "recorded refusal", **frontend_diagnostics.compiler_stop(raw_stage)}

    def test_resumed_typing_is_an_l3_refusal_after_l4_and_its_phase_is_cross_checked(self):
        resumed, initial = self.refusal("L3-resumed"), self.refusal("L3")
        self.assertEqual((resumed["stage"], resumed["after_static_evaluation"]), ("L3", True))
        for probe, case, agree in [("L3-resumed", resumed, True), ("L3", initial, True), ("L3-resumed", initial, False),
                                   ("L3", resumed, False), ("L4", resumed, False), ("L3-resumed", self.refusal("L4"), False)]:
            with self.subTest(probe=probe, case=case):
                outputs, errors = self.measure({"spec/a.md:1": case}, f"OMEGA_L5_OUTPUT\ta.md:1\tstopped\t{probe}", count=1)
                self.assertFalse(any("malformed" in error for error in errors), errors)
                self.assertEqual(outputs["examples"], {"produced": 0, "stopped": 1, "panicked": 0})
                self.assertEqual(errors == [], agree, errors)
                if not agree:
                    self.assertTrue(any(f"disagree for a.md:1: probe stopped {probe}" in error for error in errors))
        _, errors = self.measure({"spec/a.md:1": resumed}, "OMEGA_L5_OUTPUT\ta.md:1\tstopped\tL3", count=1)
        self.assertIn("corpus recorded refusal at L3 after static evaluation", errors[0])

    def test_a_missing_setup_cannot_stand_in_for_the_spec_runner_l4_refusal(self):
        _, errors = self.measure({"spec/a.md:1": self.refusal("L4")}, "OMEGA_L5_OUTPUT\ta.md:1\tstopped\tsetup", count=1)
        self.assertTrue(any("malformed L5 output observation" in error for error in errors), errors)

    def test_forms_from_a_row_the_corpus_did_not_lower_are_not_credited(self):
        rows = ("OMEGA_L5_OUTPUT\ta.md:1\tproduced\tOperationKind::Constant=2",
                "OMEGA_L5_OUTPUT\tb.md:2\tproduced\tOperationKind::Constant=5,Package::machines=1")
        for case in [self.refusal("L3-resumed"), {"status": "open gap", "stage": "L5"},
                     {"status": "inspection failed", "stage": "unattributed"}, None]:
            with self.subTest(case=case):
                cases = {"spec/a.md:1": {"status": "executable product inspected", "stage": "L5"}}
                if case:
                    cases["spec/b.md:2"] = case
                outputs, errors = self.measure(cases, *rows)
                self.assertEqual(outputs["emitted"]["OperationKind::Constant"], {"occurrences": 2, "examples": ["a.md:1"]})
                self.assertEqual(outputs["emitted"]["Package::machines"]["occurrences"], 0)
                self.assertEqual(outputs["examples"]["produced"], 2)
                self.assertEqual(len(errors), 1)
                self.assertIn("disagree for b.md:2", errors[0])

    def test_output_from_another_test_is_rejected(self):
        errors = []
        _, probe = evidence.observe(probe_log(self.INVENTORY, test="other"), errors)
        self.assertIsNone(probe["inventory"])
        self.assertTrue(errors)

    def test_repeated_psi_forms_cannot_override_earlier_measurements(self):
        form = "OperationKind::Constant"
        for line in [f"OMEGA_L5_OUTPUT_INVENTORY\t{form},{form}",
                     f"OMEGA_L5_OUTPUT\ta.md:1\tproduced\t{form}=2,{form}=3",
                     f"OMEGA_L5_OUTPUT\ta.md:1\tproduced\t{form}=2,"]:
            errors = []
            _, probe = evidence.observe(probe_log(line), errors)
            self.assertTrue(errors, line)
            self.assertEqual(probe["examples"], {})
            self.assertIsNone(probe["inventory"])


def result(target, test, status, diagnostic=()):
    return {"target": target, "test": test, "status": status, "diagnostic": list(diagnostic)}


def fixture(test, counts, outcome="produced Psi", target="tests\\loans.rs", files="main.omg"):
    return {"target": target, "test": test, "files": files, "obligations": counts, "outcome": outcome}


SPEC_TARGET, SPEC_TEST = evidence.SPEC_AGGREGATE
SPEC_OK = result(SPEC_TARGET.replace("/", "\\"), SPEC_TEST, "ok")


def verdicts(**labels):
    """Validated spec receipts as `spec_receipts` returns them; only the verdict decides standing."""
    return {label: {"label": label, "verdict": verdict} for label, verdict in labels.items()}
LOWERED_OK, LOWERED_PARTIAL, REFUSED, UNATTRIBUTED, UNOBSERVED = (evidence.OBLIGATION_STATES[i] for i in (1, 2, 4, 3, 5))


class ObligationTests(unittest.TestCase):
    LOCK = [{"line": 3, "kinds": ["Move", "Borrow", "Recast", "ExactCast"], "links": []}]

    def classify(self, observed, states=None, receipts=None):
        errors = observed["measurement_errors"]
        inputs = evidence.obligation_inputs(observed, evidence.test_results(observed), errors, receipts)
        return inputs, evidence.classify_obligations(observed, inputs, states or {}, self.LOCK, errors)

    def observed(self, obligations=None, tests=(), **fields):
        kinds = {"Move": {}, "Borrow": {}, "Recast": {}, "ExactCast": {}}
        kinds.update(obligations or {})
        return report(obligations=kinds, suites={"L5": {"tests": list(tests)}}, **fields)

    def test_the_spec_case_outcome_decides_whether_a_counted_example_lowered(self):
        observed = self.observed(
            {"Recast": {"a.md:1": 1, "b.md:1": 2}, "Move": {"c.md:1": 1}, "ExactCast": {"d.md:1": 1, "e.md:1": 3}},
            [SPEC_OK], cases={"spec/a.md:1": {"stage": "L5", "status": "open gap"}, "spec/b.md:1": {"stage": "L5", "status": "missing context"},
                              "spec/c.md:1": {"stage": "unattributed", "status": "inspection failed"},
                              "spec/d.md:1": {"stage": "L5", "status": "executable product inspected"},
                              "spec/e.md:1": {"stage": "L5", "status": "declaration or empty product"}})
        _, kinds = self.classify(observed, receipts=verdicts(**{"d.md:1": "passed", "e.md:1": "passed", "a.md:1": "known gap"}))
        self.assertEqual({kind: row["state"] for kind, row in kinds.items()},
                         {"Recast": REFUSED, "Move": UNATTRIBUTED, "ExactCast": LOWERED_OK, "Borrow": UNOBSERVED})
        self.assertEqual((kinds["ExactCast"]["inputs"], kinds["ExactCast"]["occurrences"]), ({"spec": 2, "fixture": 0}, 4))
        self.assertEqual(kinds["Recast"]["refused"], ["spec/a.md:1", "spec/b.md:1"])
        self.assertEqual(observed["measurement_errors"], [])

    def test_a_failed_spec_aggregate_credits_each_example_only_by_its_own_receipt_without_reading_failure_text(self):
        failed = result(SPEC_TARGET, SPEC_TEST, "FAILED", ["spec examples with open or unexpected outcomes:", "a.md:10: expected L5 lowered f"])
        cases = {"spec/a.md:1": {"stage": "L5", "status": "executable product inspected"},
                 "spec/a.md:10": {"stage": "L5", "status": "executable product inspected"},
                 "spec/a.md:20": {"stage": "L5", "status": "executable product inspected"}}
        observed = self.observed({"Move": {"a.md:1": 1}, "ExactCast": {"a.md:10": 1}, "Borrow": {"a.md:20": 1}}, [failed], cases=cases)
        inputs, kinds = self.classify(observed, receipts=verdicts(**{"a.md:1": "passed", "a.md:10": "mismatched expectation"}))
        self.assertEqual({row["site"]: row["standing"] for row in inputs},
                         {"spec/a.md:1": "confirmed", "spec/a.md:10": "partial", "spec/a.md:20": "unreported"})
        self.assertEqual({row["site"]: row["receipt"] for row in inputs},
                         {"spec/a.md:1": "passed", "spec/a.md:10": "mismatched expectation", "spec/a.md:20": "absent"})
        self.assertEqual({kind: kinds[kind]["state"] for kind in ("Move", "ExactCast", "Borrow")},
                         {"Move": LOWERED_OK, "ExactCast": LOWERED_PARTIAL, "Borrow": LOWERED_PARTIAL})
        self.assertEqual(kinds["ExactCast"]["lowered"], {"confirmed": [], "partial": ["spec/a.md:10"], "unreported": []})
        data = {"obligations": kinds, "obligation_inputs": evidence.input_table(inputs), "spec_aggregate": "FAILED", "assertions": {},
                "spec_receipts": {"cases": 3, "accepted": 2, "verdicts": {"passed": 1, "mismatched expectation": 1}}}
        text = "\n".join(evidence.obligation_summary(data))
        self.assertIn("2 valid receipts for 3 spec cases (1 mismatched expectation, 1 passed)", text)
        self.assertIn("| Spec examples | example receipt passed | 1 (1) | 1 | 0 | 0 |", text)
        self.assertIn("| Spec examples | example receipt reports a gap or failure | 1 (1) | 1 | 0 | 0 |", text)
        self.assertIn("| Spec examples | no valid example receipt | 1 (1) | 1 | 0 | 0 |", text)
        # Without a reported aggregate result even a passed receipt confirms nothing.
        observed = self.observed({"Move": {"a.md:1": 1}}, cases=cases)
        inputs, _ = self.classify(observed, receipts=verdicts(**{"a.md:1": "passed"}))
        self.assertEqual(inputs[0]["standing"], "unreported")

    def test_repeated_helper_calls_are_separate_inputs_and_occurrences_are_counted_apart(self):
        observed = self.observed(tests=[result("tests\\loans.rs", "pair", "ok")], package_fixtures=[
            fixture("pair", {"Move": 1}), fixture("pair", {"Move": 1}), fixture("pair", {"Borrow": 2}, "refused: second at b.omg", files="b.omg")])
        inputs, kinds = self.classify(observed)
        self.assertEqual((kinds["Move"]["inputs"]["fixture"], kinds["Move"]["distinct_inputs"], kinds["Move"]["occurrences"]), (2, 1, 2))
        self.assertEqual(kinds["Move"]["lowered"]["confirmed"], ["tests/loans.rs::pair (main.omg)"] * 2)
        self.assertEqual((kinds["Borrow"]["state"], kinds["Borrow"]["occurrences"]), (REFUSED, 2))
        self.assertEqual(evidence.input_table(inputs),
                         {"fixture/confirmed": {"inputs": 3, "distinct": 2, "lowered": 2, "refused": 1, "unattributed": 0}})
        self.assertEqual(observed["measurement_errors"], [])

    def test_a_refused_fixture_in_a_failed_test_does_not_credit_its_kind(self):
        observed = self.observed(tests=[result("tests/recasts.rs", "recast_refused", "FAILED")], package_fixtures=[
            fixture("recast_refused", {"Recast": 1}, "refused: unsupported recast at main.omg", target="tests/recasts.rs")])
        _, kinds = self.classify(observed)
        self.assertEqual(kinds["Recast"]["state"], REFUSED)
        self.assertEqual(kinds["Recast"]["lowered"], {"confirmed": [], "partial": [], "unreported": []})
        self.assertEqual(observed["measurement_errors"], [])

    def test_windows_targets_and_should_panic_names_match_results(self):
        observed = self.observed(tests=[result("tests\\recasts.rs", "refuses - should panic", "ok")],
                                 package_fixtures=[fixture("refuses", {"Recast": 1}, target="tests\\recasts.rs")])
        inputs, _ = self.classify(observed)
        self.assertEqual((inputs[0]["site"], inputs[0]["standing"]), ("tests/recasts.rs::refuses (main.omg)", "confirmed"))
        self.assertEqual(observed["measurement_errors"], [])

    def test_unattributable_inputs_are_not_credited(self):
        observed = self.observed({"Move": {"early.md:1": 1}, "Borrow": {"nocase.md:1": 1}},
                                 cases={"spec/early.md:1": {"stage": "L3", "status": "recorded refusal"}},
                                 package_fixtures=[fixture("lost", {"Recast": 1}, target="tests/recasts.rs"),
                                                   fixture("odd", {"ExactCast": 1}, "lowered fine", target="tests/recasts.rs")],
                                 tests=[result("tests/recasts.rs", "odd", "ok")])
        _, kinds = self.classify(observed)
        # The runner reports a missing or pre-L5 spec case and a malformed outcome. This module only declines credit.
        self.assertEqual(observed["measurement_errors"], [
            f"spec obligation counts have no result for {SPEC_TARGET}::{SPEC_TEST}",
            "package fixture tests/recasts.rs::lost has no test result in this run"])
        self.assertEqual({kind: row["state"] for kind, row in kinds.items()},
                         {"Move": UNATTRIBUTED, "Borrow": UNATTRIBUTED, "Recast": LOWERED_PARTIAL, "ExactCast": UNATTRIBUTED})

    def test_a_psi_property_is_credited_only_through_a_registered_assertion_on_its_own_input(self):
        def check(test, claim, obligations):
            return evidence.Assertion("entry", "producer", "tests/loans.rs", test, claim, obligations)
        registered = {"x/move": check("moves", "the move names field b", ("Move",)),
                      "x/borrow": check("borrows", "the loan names field b", ("Borrow",)),
                      "x/recast": check("recasts", "the recast keeps its loan", ("Recast",)),
                      "x/zero": check("moves", "a zero field", ("Zero",)),
                      "x/named": check("casts", "one ExactCast per cast", ())}
        observed = self.observed(
            tests=[result("tests/loans.rs", name, "FAILED" if name == "recasts" else "ok") for name in ("moves", "borrows", "recasts", "casts", "other")],
            package_fixtures=[fixture("moves", {"Move": 1}), fixture("borrows", {"Move": 1}), fixture("other", {"Borrow": 1}),
                              fixture("borrows", {"Borrow": 1}, "refused: unsupported borrow"),
                              fixture("recasts", {"Recast": 1}), fixture("casts", {"ExactCast": 1})])
        states = {"x/move": "credited", "x/borrow": "credited", "x/recast": "failed", "x/zero": "credited", "x/named": "credited"}
        with mock.patch.dict(evidence.ASSERTIONS, registered, clear=True):
            inputs, kinds = self.classify(observed, states)
        self.assertEqual({kind: (row["state"], row["property_checks"], row["failed_property_checks"]) for kind, row in kinds.items()}, {
            "Move": (evidence.OBLIGATION_STATES[0], ["x/move"], []), "Borrow": (LOWERED_OK, [], []),
            "Recast": (LOWERED_PARTIAL, [], ["x/recast"]), "ExactCast": (LOWERED_OK, [], [])})
        self.assertEqual(observed["measurement_errors"], [
            "L5 assertion x/borrow registers Borrow, but tests/loans.rs::borrows lowered no input carrying it",
            "L5 assertion x/zero registers Zero, which the prepared obligation inventory lacks"])
        data = {"obligations": kinds, "obligation_inputs": evidence.input_table(inputs), "spec_aggregate": "absent",
                "assertions": {label: {**row._asdict(), "state": states[label]} for label, row in registered.items()}}
        text = "\n".join(evidence.obligation_summary(data))
        self.assertIn("credited for an obligation kind: 1, covering 1 of 4 kinds", text)
        self.assertIn("- Move: the move names field b (x/move)", text)
        self.assertIn("- Recast: failed check x/recast", text)
        self.assertNotIn("meaning asserted", text)

    def test_lock_rows_reconcile_with_the_prepared_inventory(self):
        observed = self.observed()
        lock = [{"line": 3, "kinds": ["Move", "Borrow", "Recast"], "links": []}, {"line": 4, "kinds": ["Move", "Gone"], "links": []}]
        kinds = evidence.classify_obligations(observed, [], {}, lock, observed["measurement_errors"])
        self.assertEqual((kinds["Move"]["lock_row"], kinds["ExactCast"]["lock_row"]), (3, None))
        self.assertEqual(observed["measurement_errors"], [
            "L5 LOCK_EVIDENCE.md line 4: Move is already listed on line 3",
            "typed obligation ExactCast has no reconciliation row in L5 LOCK_EVIDENCE.md",
            "L5 LOCK_EVIDENCE.md lists Gone, which the prepared obligation inventory lacks"])


# Records of the spec aggregate as captured in a real L5 run (target/layer-readiness/20261120T124709Z-834sfk0g/L5.log),
# reduced to six examples. A held-sum witness panic stands in for a broken semantic check. The final receipts follow the
# summary line, where the aggregate prints them once every frontend, snapshot and semantic check ran.
RECEIPT = evidence.SPEC_RECEIPT
GAP, PANIC, EXECUTABLE = "language/machine_values.md:44", "language/patterns.md:50", "language/machines.md:33"
WITNESSED = "frontend=matched,lowering=matched,semantic=witness"
SPEC_CAPTURE = [
    "OMEGA_INVENTORY\tBorrow,CallPrecondition,EstablishDomain,OperationContract,VerifyLibraryImplementation,ZeroValue",
    "OMEGA_EXPECTED\tspec\t6",
    "OMEGA_CASE\tspec\tbuild/assumed_binding_contracts.md:27\tL2\trecorded refusal\texample.omg:1:1: unresolved name Build::assume_binding_contract (bytes 0..30)",
    "OMEGA_OBLIGATION\tlanguage/domains.md:572\tEstablishDomain\t1",
    "OMEGA_OBLIGATION\tlanguage/domains.md:572\tOperationContract\t1",
    "OMEGA_OBLIGATION\tlanguage/domains.md:572\tVerifyLibraryImplementation\t1",
    "OMEGA_CASE\tspec\tlanguage/domains.md:572\tL5\texecutable product inspected\tL5 lowered previous | domains i32::Km, i32::Positive",
    f"OMEGA_OBLIGATION\t{GAP}\tVerifyLibraryImplementation\t1",
    f"OMEGA_SPEC_LOCATION\t{GAP}\tfragment\t8\t5\t191\t197",
    f"OMEGA_CASE\tspec\t{GAP}\tL5\topen gap\tL5 open gap: no Psi lowering for carrier runtime callable machine(u32) -> u8 | reader",
    f"OMEGA_OBLIGATION\t{EXECUTABLE}\tBorrow\t1",
    f"OMEGA_OBLIGATION\t{EXECUTABLE}\tVerifyLibraryImplementation\t1",
    f"OMEGA_OBLIGATION\t{EXECUTABLE}\tZeroValue\t1",
    f"OMEGA_CASE\tspec\t{EXECUTABLE}\tL5\texecutable product inspected\tL5 lowered clear, example | schemas Counter",
    f"OMEGA_OBLIGATION\t{PANIC}\tBorrow\t1",
    f"OMEGA_OBLIGATION\t{PANIC}\tVerifyLibraryImplementation\t1",
    f"OMEGA_CASE\tspec\t{PANIC}\tunattributed\tinspection failed\tinspect must return the callee's verdict",
    "OMEGA_OBLIGATION\tproofs/contracts.md:826\tBorrow\t1",
    "OMEGA_OBLIGATION\tproofs/contracts.md:826\tCallPrecondition\t1",
    "OMEGA_OBLIGATION\tproofs/contracts.md:826\tOperationContract\t1",
    "OMEGA_OBLIGATION\tproofs/contracts.md:826\tVerifyLibraryImplementation\t1",
    "OMEGA_CASE\tspec\tproofs/contracts.md:826\tL5\texecutable product inspected\tL5 lowered area, get",
    "6 fragments. 1 front-end refusals, 0 package-context refusals with inspected contextual products, 0 with unverified context, "
    "1 open lowering gaps, 0 empty products, 0 alias-only products, 0 declaration interfaces, 0 domain-only products, "
    "0 schema-only products, 3 inspected executable products and 0 uninspected executable products.",
    f"{RECEIPT}\tbuild/assumed_binding_contracts.md:27\trecorded refusal\trecorded refusal\tfrontend=matched,lowering=none,semantic=none",
    f"{RECEIPT}\tlanguage/domains.md:572\texecutable product inspected\tpassed\t{WITNESSED}",
    f"{RECEIPT}\t{GAP}\topen gap\tknown gap\tfrontend=matched,lowering=matched,semantic=none",
    f"{RECEIPT}\t{EXECUTABLE}\texecutable product inspected\tpassed\t{WITNESSED}",
    f"{RECEIPT}\t{PANIC}\tinspection failed\tpanicked\tfrontend=unchecked,lowering=extra,semantic=panicked",
    f"{RECEIPT}\tproofs/contracts.md:826\texecutable product inspected\tpassed\t{WITNESSED}",
]
LOCK = [{"line": 3, "kinds": ["Borrow", "CallPrecondition", "EstablishDomain", "OperationContract", "VerifyLibraryImplementation", "ZeroValue"],
         "links": []}]


def receipt_line(label):
    return next(line for line in SPEC_CAPTURE if line.startswith(f"{RECEIPT}\t{label}\t"))


def green_capture():
    """The same run without its open gap and panic, so the aggregate legitimately passes."""
    lines = [line for line in SPEC_CAPTURE if f"\t{GAP}\t" not in line and f"\t{PANIC}\t" not in line]
    return [line.replace("OMEGA_EXPECTED\tspec\t6", "OMEGA_EXPECTED\tspec\t4") for line in lines]


class SpecReceiptTests(unittest.TestCase):
    """Per-example receipts decide which spec inputs a failed or passing aggregate confirms."""

    @unittest.skipUnless(os.environ.get("OMEGA_FAULT_EVIDENCE"), "set OMEGA_FAULT_EVIDENCE to replay a full readiness run")
    def test_removing_one_real_example_result_preserves_every_other_verdict(self):
        log = (Path(os.environ["OMEGA_FAULT_EVIDENCE"]) / "L5.log").read_text(encoding="utf-8")
        observed = report(cohort_sources={})
        tests, _ = collect(log, "L5", observed)
        observed["suites"]["L5"]["tests"] = tests
        outcomes = evidence.test_results(observed)
        errors = []
        baseline = evidence.spec_receipts(log, observed, outcomes, errors)
        self.assertEqual(errors, [])
        victim = next(label for label, row in baseline.items() if row["verdict"] == "passed")
        marker = f"{evidence.SPEC_RECEIPT}\t{victim}\t"
        changed = "\n".join(line for line in log.splitlines() if not line.startswith(marker))
        remaining = evidence.spec_receipts(changed, observed, outcomes, errors)
        self.assertEqual(remaining, {label: row for label, row in baseline.items() if label != victim})
        self.assertEqual(len(errors), 1, errors)
        self.assertIn(f"spec/{victim}: no final example receipt", errors[0])

    def run_capture(self, lines=SPEC_CAPTURE, status="FAILED", replace=None, extra=""):
        lines = [replace.get(line, line) if replace else line for line in lines]
        lines = [line for line in lines if line is not None]
        observed = report()
        log = libtest("tests\\spec_corpus.rs", (SPEC_TEST, status, lines)) + extra
        tests, _ = collect(log, "L5", observed)
        self.assertEqual(observed["measurement_errors"], [], "the runner accepts every record of the capture")
        observed["suites"]["L5"] = {"tests": tests}
        errors = observed["measurement_errors"]
        results = evidence.test_results(observed)
        _, probe = evidence.observe(log, errors)
        self.assertEqual(probe["examples"], {})
        examples = evidence.spec_receipts(log, observed, results, errors)
        inputs = evidence.obligation_inputs(observed, results, errors, examples)
        kinds = evidence.classify_obligations(observed, inputs, {}, LOCK, errors)
        return examples, {row["site"].removeprefix("spec/"): row["standing"] for row in inputs}, kinds, errors

    def test_a_failed_aggregate_confirms_only_the_examples_whose_own_checks_passed(self):
        examples, standing, kinds, errors = self.run_capture()
        self.assertEqual(errors, [])
        self.assertEqual({label: row["verdict"] for label, row in examples.items()}, {
            "build/assumed_binding_contracts.md:27": "recorded refusal", "language/domains.md:572": "passed", GAP: "known gap",
            EXECUTABLE: "passed", PANIC: "panicked", "proofs/contracts.md:826": "passed"})
        self.assertEqual(standing, {"language/domains.md:572": "confirmed", GAP: "partial", EXECUTABLE: "confirmed",
                                    PANIC: "partial", "proofs/contracts.md:826": "confirmed"})
        # The open gap and the panic are not passes; the kinds only they carry stay unconfirmed.
        self.assertEqual(kinds["ZeroValue"]["lowered"], {"confirmed": [f"spec/{EXECUTABLE}"], "partial": [], "unreported": []})
        self.assertEqual(kinds["ZeroValue"]["state"], LOWERED_OK)
        self.assertEqual(kinds["Borrow"]["unattributed"], [f"spec/{PANIC}"])
        self.assertEqual(kinds["VerifyLibraryImplementation"]["refused"], [f"spec/{GAP}"])

    def test_a_dropped_receipt_in_an_otherwise_green_file_confirms_nothing_for_that_example(self):
        examples, standing, kinds, errors = self.run_capture(green_capture(), status="ok", replace={receipt_line(EXECUTABLE): None})
        self.assertEqual(errors, [f"spec/{EXECUTABLE}: no final example receipt from {SPEC_TARGET}::{SPEC_TEST}"])
        self.assertNotIn(EXECUTABLE, examples)
        self.assertEqual(standing[EXECUTABLE], "unreported")
        self.assertEqual(standing["proofs/contracts.md:826"], "confirmed")
        self.assertEqual(kinds["ZeroValue"]["state"], LOWERED_PARTIAL)
        # The same file with every receipt dropped reports the missing evidence once.
        _, standing, _, errors = self.run_capture(
            [line for line in green_capture() if not line.startswith(RECEIPT)], status="ok")
        self.assertEqual(errors, [f"{SPEC_TARGET}::{SPEC_TEST} printed no example receipts; 4 spec examples unreported"])
        self.assertEqual(set(standing.values()), {"unreported"})

    def test_unexpected_and_repeated_receipts_are_rejected(self):
        unexpected = f"{RECEIPT}\tlanguage/patterns.md:9\texecutable product inspected\tpassed\t{WITNESSED}"
        repeated = receipt_line("proofs/contracts.md:826")
        examples, standing, _, errors = self.run_capture(SPEC_CAPTURE + [unexpected, repeated])
        self.assertEqual(errors, [f"repeated spec example receipt for proofs/contracts.md:826: {repeated}",
                                  "inconsistent spec example receipt for language/patterns.md:9: no spec case result"])
        self.assertNotIn("language/patterns.md:9", examples)
        # Neither copy of a repeated receipt is credited.
        self.assertNotIn("proofs/contracts.md:826", examples)
        self.assertEqual(standing["proofs/contracts.md:826"], "unreported")
        self.assertEqual(standing[EXECUTABLE], "confirmed")

    def test_a_receipt_forged_by_another_test_is_not_credited(self):
        forged = libtest("tests\\contracts.rs", ("contract_ordering_keeps_same_carrier_subjects_and_strictness", "ok",
                                                 [f"{RECEIPT}\t{GAP}\topen gap\tpassed\tfrontend=matched,lowering=matched,semantic=none",
                                                  receipt_line(EXECUTABLE)]))
        examples, standing, _, errors = self.run_capture(replace={receipt_line(EXECUTABLE): None, receipt_line(GAP): None}, extra=forged)
        self.assertEqual(len(errors), 4, errors)
        self.assertTrue(all("outside tests/spec_corpus.rs::" in error and "tests/contracts.rs::" in error for error in errors[:2]), errors)
        self.assertEqual(errors[2:], [f"spec/{GAP}: no final example receipt from {SPEC_TARGET}::{SPEC_TEST}",
                                      f"spec/{EXECUTABLE}: no final example receipt from {SPEC_TARGET}::{SPEC_TEST}"])
        self.assertFalse(examples.keys() & {GAP, EXECUTABLE})
        self.assertEqual((standing[GAP], standing[EXECUTABLE]), ("unreported", "unreported"))

    def test_receipts_inconsistent_with_their_case_or_their_own_checks_are_rejected(self):
        executable = receipt_line(EXECUTABLE)
        for line, why in [
            (f"{RECEIPT}\t{GAP}\topen gap\tpassed\tfrontend=matched,lowering=matched,semantic=none", "its checks imply known gap"),
            (f"{RECEIPT}\t{GAP}\texecutable product inspected\tpassed\t{WITNESSED}", "case status open gap"),
            (executable.replace("lowering=matched", "lowering=mismatched"), "its checks imply mismatched expectation"),
            (executable.replace("semantic=witness", "semantic=declarations"), "semantic=declarations cannot describe"),
            (executable.replace("frontend=matched", "frontend=unchecked"), "frontend=unchecked cannot describe"),
            (receipt_line(PANIC).replace("\tpanicked\t", "\tpassed\t"), "its checks imply panicked"),
            (f"{RECEIPT}\tbuild/assumed_binding_contracts.md:27\trecorded refusal\trecorded refusal\tfrontend=matched,lowering=matched,semantic=none",
             "lowering=matched cannot describe"),
        ]:
            with self.subTest(line=line):
                label = line.split("\t")[1]
                original = receipt_line(label)
                examples, standing, _, errors = self.run_capture(replace={original: line})
                self.assertEqual(len(errors), 1, errors)
                self.assertTrue(errors[0].startswith(f"inconsistent spec example receipt for {label}: "), errors)
                self.assertIn(why, errors[0])
                self.assertNotIn(label, examples)
                if label in standing:
                    self.assertEqual(standing[label], "unreported")

    def test_malformed_receipts_are_rejected(self):
        executable = receipt_line(EXECUTABLE)
        for line in [executable.replace("\tpassed\t", "\tpass\t"), executable.replace(",semantic=witness", ""),
                     executable + ",semantic=witness", executable.replace("lowering=matched", "lowering=skipped"), executable + "\textra"]:
            with self.subTest(line=line):
                examples, standing, _, errors = self.run_capture(replace={executable: line})
                self.assertEqual(errors, [f"malformed spec example receipt: {line}",
                                          f"spec/{EXECUTABLE}: no final example receipt from {SPEC_TARGET}::{SPEC_TEST}"])
                self.assertEqual(standing[EXECUTABLE], "unreported")

    def test_a_passing_aggregate_cannot_carry_a_failing_receipt(self):
        gap = receipt_line(GAP)
        lines = [line for line in SPEC_CAPTURE if f"\t{PANIC}\t" not in line]
        lines = [line.replace("OMEGA_EXPECTED\tspec\t6", "OMEGA_EXPECTED\tspec\t5") for line in lines]
        examples, standing, _, errors = self.run_capture(lines, status="ok")
        self.assertEqual(errors, [f"inconsistent spec example receipt for {GAP}: verdict known gap, but the aggregate passed"])
        self.assertNotIn(GAP, examples)
        self.assertEqual(standing[EXECUTABLE], "confirmed")
        self.assertIn(gap, lines)


class FullRunTests(unittest.TestCase):
    def test_a_full_run_without_receipts_reports_unmeasured_evidence(self):
        observed = report()
        Path("target").mkdir(exist_ok=True)
        with TemporaryDirectory(dir="target") as temp:
            (Path(temp) / "L5.log").write_text("test result: ok. 0 passed; 0 failed; 0 ignored\n", encoding="utf-8")
            evidence.finish(observed, Path(temp))
            details = (Path(temp) / "lowering.md").read_text(encoding="utf-8")
        data = observed["lowering_evidence"]
        self.assertTrue(data["contract"])
        self.assertTrue(all(row["status"] == "unmeasured" for row in data["contract"]))
        self.assertFalse(data["outputs"]["measured"])
        errors = observed["measurement_errors"]
        self.assertTrue(any("missing L5 assertion receipt" in error for error in errors))
        self.assertTrue(any("Psi output forms unmeasured" in error for error in errors))
        text = "\n".join(evidence.summary(observed))
        self.assertIn("## L5 contract outputs", text)
        self.assertIn(f"0 of {len(evidence.ASSERTIONS)} registered checks were credited", text)
        self.assertIn("Psi output forms are unmeasured", text)
        self.assertIn("## Psi output forms", details)


if __name__ == "__main__":
    unittest.main()
