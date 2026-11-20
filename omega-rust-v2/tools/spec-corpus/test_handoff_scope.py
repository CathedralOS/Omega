"""A missing consumer keeps its reason without becoming consumer evidence."""
from pathlib import Path
import unittest

import handoff_evidence
from check_layer_readiness import readiness_exit

ROOT = Path(__file__).absolute().parent.parent.parent
L4_CONTRACT = ROOT / "layers" / handoff_evidence.DOCUMENTS["L4"][0]
L5 = ROOT / "layers" / "05_prepared-trees-to-terminal-psi"
FIELD, HELPER, PROOF, RECEIPT = handoff_evidence.PREPARATION_CONSUMERS


def row(prefix, line, consumer=(), outside=()):
    return {"rule": prefix + " promise", "line": line, "direct": ["exact product"], "consumer": list(consumer),
            "refusal_only_rule": False, **({"outside_run": list(outside)} if outside else {})}


def outlines(later_l3=True):
    l3 = row("Domains", 38, outside=["consumer: L5 tests/domains.rs::checks_the_carrier"] if later_l3 else ())
    l4 = [row(FIELD, 9), row(HELPER, 21), row(PROOF, 15), row(RECEIPT, 23),
          row("Named reads connect", 29, outside=["consumer: L5 tests/target.rs::reads_the_input"])]
    return {"L3": [l3], "L4": l4}


def stop(label, test, status="open gap", target="tests\\preparation_handoff_gaps.rs"):
    return {f"handoff/{label}": {"cohort": "handoff", "stage": "L5", "status": status, "detail": f"stop for {label}",
                                 "target": target, "test": test}}


def report(last_layer, cases=None, gap_status="ok"):
    results = {}
    for _, _, pinned in handoff_evidence.PREPARATION_CONSUMERS.values():
        if pinned:
            results[("L5", handoff_evidence.GAP_TESTS, pinned[0])] = gap_status
    return {"last_layer": last_layer, "measurement_errors": [], "cases": cases or {}}, results


def all_stops():
    cases = {}
    for _, _, pinned in handoff_evidence.PREPARATION_CONSUMERS.values():
        if pinned:
            for label in pinned[1]:
                cases.update(stop(label, pinned[0]))
    return cases


def scopes(rows):
    return {(stage, r["line"]): r.get("consumer_scope") for stage, entries in rows.items() for r in entries}


class ConsumerScopeTests(unittest.TestCase):
    def test_full_run_separates_l5_refusals_from_unmeasured_and_non_pipeline_consumers(self):
        rows = outlines(later_l3=False)
        rows["L4"].pop()  # A full run has no suite outside its scope.
        data, results = report("L5", all_stops())
        handoff_evidence.classify_consumers(data, rows, results)
        self.assertEqual(scopes(rows), {("L3", 38): "unmeasured", ("L4", 9): "l5 refusal", ("L4", 21): "l5 refusal",
                                        ("L4", 15): "unmeasured", ("L4", 23): "outside pipeline"})
        self.assertEqual(len(next(r for r in rows["L4"] if r["line"] == 21)["observed_stops"]), 2)
        gaps = handoff_evidence.unverified_contracts(rows, "L5")
        # Every reason still leaves the consumer unverified and blocks the full run.
        self.assertEqual([(g["line"], g["missing"], g["consumer_scope"]) for g in gaps],
                         [(38, ["consumer"], "unmeasured"), (9, ["consumer"], "l5 refusal"), (21, ["consumer"], "l5 refusal"),
                          (15, ["consumer"], "unmeasured"), (23, ["consumer"], "outside pipeline")])
        self.assertTrue(all(not r["consumer"] for entries in rows.values() for r in entries))
        self.assertEqual(readiness_exit({"cases": {}, "measurement_errors": [], "suites": {}, "unverified_contracts": gaps}), 1)

    def test_a_stop_counts_only_from_its_passing_registered_test(self):
        label = handoff_evidence.PREPARATION_CONSUMERS[FIELD][2][1][0]
        test = handoff_evidence.PREPARATION_CONSUMERS[FIELD][2][0]
        for cases, status in [(stop(label, "another_test"), "ok"),
                              (stop(label, test, target="tests\\other.rs"), "ok"),
                              (stop(label, test, status="recorded refusal"), "ok"),
                              (stop(label, test), "FAILED"),
                              ({}, "ok")]:
            with self.subTest(cases=cases, status=status):
                rows = outlines(later_l3=False)
                data, results = report("L5", cases, status)
                handoff_evidence.classify_consumers(data, rows, results)
                field = next(r for r in rows["L4"] if r["line"] == 9)
                self.assertEqual((field["consumer_scope"], field["observed_stops"]), ("unmeasured", []))
                self.assertIn("was not observed", field["consumer_context"])

    def test_every_pinned_label_must_be_observed(self):
        pinned = handoff_evidence.PREPARATION_CONSUMERS[HELPER][2]
        rows = outlines(later_l3=False)
        data, results = report("L5", stop(pinned[1][0], pinned[0]))
        handoff_evidence.classify_consumers(data, rows, results)
        self.assertEqual(next(r for r in rows["L4"] if r["line"] == 21)["consumer_scope"], "unmeasured")

    def test_before_l5_separates_deferred_comparisons_from_open_l4_consumers(self):
        rows = outlines()
        data, results = report("L4")
        handoff_evidence.classify_consumers(data, rows, results)
        self.assertEqual(scopes(rows), {("L3", 38): "later suite", ("L4", 9): "l5 refusal outside run", ("L4", 21): "l5 refusal outside run",
                                        ("L4", 15): "unmeasured", ("L4", 23): "outside pipeline", ("L4", 29): "later suite"})
        self.assertNotIn("observed_stops", rows["L4"][0])
        gaps = handoff_evidence.unverified_contracts(rows, "L4")
        self.assertEqual([(g["stage"], g["consumer_scope"], g["later_checks"]) for g in gaps],
                         [("L3", "later suite", ["consumer: L5 tests/domains.rs::checks_the_carrier"])])
        data["handoff_rules"] = rows
        text = "\n".join(handoff_evidence.consumer_scope_summary(data))
        self.assertIn("| comparison registered in a suite outside this run | 2 | L3 line 38, L4 line 29 |", text)
        self.assertIn("| L5 refusal pinned by a test outside this run | 2 | L4 line 9, L4 line 21 |", text)
        self.assertIn("| no L5 contract paragraph or lowering code reads the product | 1 | L4 line 23 |", text)
        self.assertIn("| no consumer comparison or pinned L5 refusal | 1 | L4 line 15 |", text)
        self.assertIn("stay open in the full run", text)

    def test_observed_consumer_has_no_reason_and_a_stale_reason_is_reported(self):
        rows = outlines()
        rows["L4"][0]["consumer"].append("L5 tests/fields.rs::reads_extents · assert_eq!(a, b)")
        rows["L4"][0]["consumer_scope"] = "l5 refusal"
        rows["L4"].pop(1)
        data, results = report("L5", all_stops())
        handoff_evidence.classify_consumers(data, rows, results)
        self.assertNotIn("consumer_scope", rows["L4"][0])
        self.assertEqual(data["measurement_errors"], [f"L4: expected one contract entry for consumer reason {HELPER!r}"])

    def test_each_reason_names_one_current_contract_entry(self):
        rules = handoff_evidence.document_rules("L4", L4_CONTRACT.read_text(encoding="utf-8"))
        for prefix in handoff_evidence.PREPARATION_CONSUMERS:
            self.assertEqual(sum(r["rule"].startswith(prefix) for r in rules), 1, prefix)

    def test_pinned_l5_refusals_exist_in_the_gap_tests(self):
        source = (L5 / handoff_evidence.GAP_TESTS).read_text(encoding="utf-8")
        for kind, _, pinned in handoff_evidence.PREPARATION_CONSUMERS.values():
            self.assertEqual(pinned is not None, kind == "l5 refusal")
            if pinned:
                self.assertIn(f"fn {pinned[0]}()", source)

    def test_outside_pipeline_reason_holds_only_while_lowering_ignores_the_receipt(self):
        readers = [path for path in (L5 / "src").rglob("*.rs") if "resource_receipt" in path.read_text(encoding="utf-8")]
        self.assertEqual(readers, [], "lowering reads the resource receipt; register its consumer comparison instead")
        self.assertNotIn("resource_receipt", (L5 / "CONTRACT.md").read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
