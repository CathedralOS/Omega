"""Check the readiness comparison against case records copied from real reports.

Each record below is verbatim from target/layer-readiness/<run>/evidence.json for
the named run. Set OMEGA_FAULT_EVIDENCE to a full run directory to also replay its
L5 log through the report's own collector and compare the result with the recorded
evidence.
"""
from contextlib import redirect_stdout
import copy
import csv
from io import StringIO
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from diagnostic_fields import unescape

import check_layer_readiness
import compare_readiness as compare

SAMPLE_TEST = {"target": "tests\\sample_baseline.rs", "test": "fixed_sample_configurations_through_l5"}
# 20261121T094024Z-j7ql3pxg and 20261121T095021Z-c5gk01_c.
IMPORT_BEFORE = {"cohort": "sample", "stage": "setup", "status": "missing context",
                 "detail": "requires Build dependency alias kit -> dep (mod_kernel), selecting other.omg", **SAMPLE_TEST}
IMPORT_REFUSAL = {"file": "main.omg", "byte_start": 140, "byte_end": 149, "line": 3, "byte_column": 5, "stage": "L2",
                  "after_static_evaluation": False, "later_layers_not_reached": ["L3", "L4", "L5"], "package": "root",
                  "occurrence_form": "text", "occurrence": "kit\\other", "reason": "unavailable import kit\\other"}
IMPORT_AFTER = {"cohort": "sample", "stage": "L2", "status": "compatibility blocked", "detail": "main.omg:3:5 unavailable import kit\\other",
                **SAMPLE_TEST, "after_static_evaluation": False, "later_layers_not_reached": ["L3", "L4", "L5"],
                "compiler_refusal": IMPORT_REFUSAL}
ZERO_FIELD = {"package": "app", "occurrence": "Options{count:1,..}", "file": "zero_field.omg", "byte_start": 63, "byte_end": 82, "line": 1, "byte_column": 64}
OMITTED_BLOCKED = {"cohort": "handoff", "stage": "L5", "status": "checker input blocked",
                   "detail": "L5 stopped before the checker could judge the obligation. The omission owes ZeroField for a reference, but supplies no backing place. L5 stops rather than fabricate a descriptor or prove zero validity. Observed cannot lower implicit zero of type `@u8` for omitted field `Options::handle` at zero_field.omg in tests/formation_handoff.rs::zero_field_cannot_supply_a_reference_or_discharge_an_independent_predicate.",
                   "registered_refusal": "checker", "fixture_locations": [ZERO_FIELD]}
# 20261121T095845Z-qo1b81g_: the same case returns as an intended rejection under a renamed test.
OMITTED_INVALID = {"cohort": "handoff", "stage": "L5", "status": "intended rejection verified",
                   "detail": "Invalid input. A `@u8` value borrows one referent, and no zero of that type borrows anything, so the omission can never be supplied. Changing only the field type to `u8` lowers the same literal with a zero in the handle slot. Observed omitted field `Options::handle` of type `@u8` needs a borrowed reference, which `..` never supplies at zero_field.omg in tests/formation_handoff.rs::zero_field_cannot_supply_a_reference.",
                   "registered_refusal": "invalid", "fixture_locations": [ZERO_FIELD]}
# 20261121T082025Z-26b5c5zp and 20261121T095845Z-qo1b81g_: only the justification before `Observed` changed.
ACCESS_SITE = {"package": "app", "occurrence": "access Rule", "file": "access_plan.omg", "byte_start": 163, "byte_end": 174, "line": 4, "byte_column": 38}
ACCESS_TEST = {"target": "tests\\formation_handoff.rs", "test": "an_access_plan_stops_at_its_own_policy_clause",
               "after_static_evaluation": False, "later_layers_not_reached": []}
ACCESS_BEFORE = {"cohort": "handoff", "stage": "L5", "status": "open gap",
                 "detail": "The domain needs its policy, arguments and plan machine for evaluation. Observed unsupported domain member: Access at access_plan.omg: access Rule",
                 **ACCESS_TEST, "fixture_locations": [ACCESS_SITE]}
ACCESS_AFTER = {**ACCESS_BEFORE, "detail": "Typing selects Rule::plan, but preparation has not supplied its evaluated result on the domain schema. Observed unsupported domain member: Access at access_plan.omg: access Rule"}
# 20261121T030129Z-rrhbzou5 and 20261121T031036Z-1wol3rnq: same site, different reason and status.
RANKED_SITE = {"package": "app", "occurrence": "{ let Lease {mark as _}=x; again(3); }", "file": "ranking.omg", "byte_start": 147, "byte_end": 185, "line": 3, "byte_column": 33}
RANKED_BEFORE = {"cohort": "handoff", "stage": "L5", "status": "unresolved refusal",
                 "detail": "Unresolved refusal (validity or owner undecided). The rule is stated only by the L5 diagnostic. The contract assigns termination to L6 and does not say an unwitnessed member is invalid. Observed every member of a ranked recursive component requires its own `terminates by` witness at ranking.omg in tests/ranking.rs::selected_cleanup_participates_in_the_recursive_component.",
                 "registered_refusal": "unresolved", "fixture_locations": [RANKED_SITE]}
RANKED_AFTER = {"cohort": "handoff", "stage": "L5", "status": "checker input blocked",
                "detail": "L5 stopped before the checker could judge the obligation. The cleanup calls again(3) and closes a cycle that can repeat the same rank. The unranked control exposes the cleanup edge and actual argument in Psi. Checking strict descent needs those edges. Observed lowering has no entry-rank mapping for this member of the recursive component at ranking.omg in tests/ranking.rs::selected_cleanup_participates_in_the_recursive_component.",
                "registered_refusal": "checker", "fixture_locations": [RANKED_SITE]}
# 20261121T035647Z-2f23afal and 20261121T040859Z-eek8vinp.
CONFORMANCE_PASSED = {"cohort": "context", "stage": "front end", "status": "contextual witnesses verified",
                      "detail": "PASS encode and decode retain their selected requirements / PASS reject lib_b\\decode in the lib_a\\encode slot — L3 codec.omg:3:13: machine has no matching requirement signature (bytes 90..137) / PASS reject lib_a\\encode in the lib_b\\decode slot — L3 codec.omg:4:13: machine has no matching requirement signature (bytes 151..198)"}
CONFORMANCE_FAILED = {"cohort": "context", "stage": "front end", "status": "inspection failed", "detail": "see context.log"}
# 20261121T095845Z-qo1b81g_.
SPEC_REFUSAL = {"cohort": "spec", "stage": "L2", "status": "recorded refusal",
                "detail": "example.omg:1:1: unresolved name Build::assume_binding_contract (bytes 0..30)",
                "target": "tests\\spec_corpus.rs", "test": "spec_fragments_have_reviewed_lowering_outcomes",
                "after_static_evaluation": False, "later_layers_not_reached": ["L3", "L4", "L5"],
                "expectation_check": {"verdict": "recorded refusal", "frontend": "matched", "lowering": "none", "semantic": "none"}}

IMPORT = "sample/import-from-dependency"
OMITTED = "handoff/omitted reference field without a backing value"
ACCESS = "handoff/access plan evaluation at its domain"
RANKED = "handoff/cleanup in a ranked component"
CONFORMANCE = "context/language/conformances.md:204"
SPEC = "spec/build/assumed_binding_contracts.md:27"


def report(cases, **fields):
    return {"revision": "e1c9fb0255e4f5d1c53af74b37773aca6b09f5a9", "fingerprint": "f9dacd6ac942", "last_layer": "L5",
            "measurement_errors": [], "suites": {"L5": {"exit": 0}}, "expected": {}, "cases": copy.deepcopy(cases), **fields}


def grouped(before, after):
    groups, unchanged = compare.compare(before, after)
    placed = {entry["case"]: group for group, entries in groups.items() for entry in entries}
    return groups, placed, unchanged


class ReadinessComparisonTests(unittest.TestCase):
    def test_a_disappeared_blocker_is_removed_and_never_unblocked(self):
        # 094024Z -> 095021Z: absent from the later report.
        groups, placed, _ = grouped(report({OMITTED: OMITTED_BLOCKED}), report({}))
        self.assertEqual(placed, {OMITTED: "removed-blocker"})
        self.assertEqual(groups["unblocked"], [])
        text = compare.markdown(report({OMITTED: OMITTED_BLOCKED}), report({}), groups, [])
        self.assertIn("No longer blocking while still measured: 0. Blockers no longer measured: 1.", text)
        self.assertIn("not a repair", text)
        self.assertIn("app/zero_field.omg:1:64 bytes 63..82", text)
        # The returning measurement is the only case the comparison credits as no longer blocking.
        _, placed, _ = grouped(report({OMITTED: OMITTED_BLOCKED}), report({OMITTED: OMITTED_INVALID}))
        self.assertEqual(placed, {OMITTED: "unblocked"})

    def test_new_blockers_and_regressions_are_kept_apart(self):
        before = report({CONFORMANCE: CONFORMANCE_PASSED})
        after = report({CONFORMANCE: CONFORMANCE_FAILED, IMPORT: IMPORT_AFTER, SPEC: SPEC_REFUSAL})
        _, placed, _ = grouped(before, after)
        self.assertEqual(placed, {CONFORMANCE: "regression", IMPORT: "new-blocker", SPEC: "new"})
        for case in (CONFORMANCE_FAILED, IMPORT_AFTER):
            self.assertTrue(check_layer_readiness.case_blocks_readiness(case))
        self.assertFalse(check_layer_readiness.case_blocks_readiness(SPEC_REFUSAL))

    def test_a_changed_stop_reports_each_structured_field(self):
        groups, placed, _ = grouped(report({IMPORT: IMPORT_BEFORE}), report({IMPORT: IMPORT_AFTER}))
        self.assertEqual(placed, {IMPORT: "stop-changed"})
        changes = {field: (old, new) for field, old, new in groups["stop-changed"][0]["changes"]}
        self.assertEqual(set(changes), {"status", "stage", "reason", "locations"})
        self.assertEqual(changes["stage"], ("setup", "L2"))
        self.assertEqual(changes["reason"][1], "unavailable import kit\\other")
        self.assertEqual(compare.value_text("locations", changes["locations"][1]), 'root/main.omg:3:5 bytes 140..149 "kit\\\\other"')

    def test_the_same_site_with_a_new_reason_does_not_report_a_location_change(self):
        groups, placed, _ = grouped(report({RANKED: RANKED_BEFORE}), report({RANKED: RANKED_AFTER}))
        self.assertEqual(placed, {RANKED: "stop-changed"})
        changes = {field: (old, new) for field, old, new in groups["stop-changed"][0]["changes"]}
        self.assertEqual(set(changes), {"status", "reason"})
        self.assertEqual(changes["reason"], ("every member of a ranked recursive component requires its own `terminates by` witness",
                                             "lowering has no entry-rank mapping for this member of the recursive component"))

    def test_only_explanatory_text_is_not_a_changed_stop(self):
        _, placed, _ = grouped(report({ACCESS: ACCESS_BEFORE}), report({ACCESS: ACCESS_AFTER}))
        self.assertEqual(placed, {ACCESS: "text-only"})
        # Control: moving the same refusal by one byte is a changed stop.
        moved = copy.deepcopy(ACCESS_AFTER)
        moved["fixture_locations"][0]["byte_start"] = 164
        groups, placed, _ = grouped(report({ACCESS: ACCESS_BEFORE}), report({ACCESS: moved}))
        self.assertEqual(placed, {ACCESS: "stop-changed"})
        self.assertEqual([field for field, _, _ in groups["stop-changed"][0]["changes"]], ["locations"])
        # Control: the same text and site under a renamed emitting test.
        renamed = {**ACCESS_AFTER, "test": "an_access_plan_stops_elsewhere"}
        groups, placed, _ = grouped(report({ACCESS: ACCESS_BEFORE}), report({ACCESS: renamed}))
        self.assertEqual(placed[ACCESS], "witness-changed")
        self.assertEqual([field for field, _, _ in groups["witness-changed"][0]["changes"]], ["test"])
        # A duplicated identical site is still one stop.
        doubled = {**ACCESS_BEFORE, "fixture_locations": [ACCESS_SITE, ACCESS_SITE]}
        _, placed, unchanged = grouped(report({ACCESS: ACCESS_BEFORE}), report({ACCESS: doubled}))
        self.assertEqual((placed, unchanged), ({}, [ACCESS]))

    def test_added_inputs_preserve_each_original_source_and_stop(self):
        other_site = {**ACCESS_SITE, "byte_start": 164, "byte_end": 175}
        reason = "unsupported domain member: Access"
        def fixture(source, location):
            return {**ACCESS_TEST, "location": location,
                    "replay": {"message": reason, "entry": "access_plan.omg", "files": [["access_plan.omg", source]]}}
        def measured(locations, fixtures):
            case = {**ACCESS_BEFORE, "refusal_reason": reason, "fixture_locations": locations}
            return report({ACCESS: case}, package_fixtures=fixtures)
        original = fixture("original source", ACCESS_SITE)
        before = measured([ACCESS_SITE], [original])
        def group(fixtures):
            after = measured([ACCESS_SITE, other_site], fixtures)
            return grouped(before, after)[1][ACCESS]
        self.assertEqual(group([original, fixture("extra source", other_site)]), "fixtures-added")
        self.assertEqual(group([original, fixture("original source", other_site)]), "stop-changed")
        self.assertEqual(group([fixture("replacement source", ACCESS_SITE), fixture("extra source", other_site)]), "stop-changed")
        self.assertEqual(group([original]), "stop-changed")

    def test_stops_read_the_producers_structured_fields(self):
        stop = compare.first_stop(SPEC, SPEC_REFUSAL, report({}))
        self.assertEqual((stop["stage"], stop["reason"], stop["verdict"]), ("L2", "unresolved name Build::assume_binding_contract", "recorded refusal"))
        self.assertEqual(stop["locations"], (("example", "example.omg", 0, 30, 1, 1, ""),))
        stop = compare.first_stop(OMITTED, OMITTED_INVALID, report({}))
        self.assertEqual(stop["reason"], "omitted field `Options::handle` of type `@u8` needs a borrowed reference, which `..` never supplies")
        self.assertEqual(stop["test"], "tests/formation_handoff.rs::zero_field_cannot_supply_a_reference")
        # An L5 spec fragment carries its lowering location in the report root (20261121T095845Z-qo1b81g_).
        label = "language/data_and_literals.md:90"
        located = report({}, lowering_locations={label: {"fragment": {"file": "example.omg", "byte_start": 101, "byte_end": 175, "line": 8, "byte_column": 1, "spec_line": 98}}})
        case = {"cohort": "spec", "stage": "L5", "status": "contextual product inspected", "detail": "L5 fragment only: unsupported Psi declaration: Let | let body: Envelope<u32>::Body = Envelope::<u32>::Body::Value { value: 7 };"}
        self.assertEqual(compare.first_stop("spec/" + label, case, located)["locations"], (("spec fragment", "example.omg", 101, 175, 8, 1, ""),))
        # Without a delimited reason the whole detail stands in, so a change still shows.
        self.assertEqual(compare.first_stop(CONFORMANCE, CONFORMANCE_FAILED, report({}))["reason"], "see context.log")

    def test_measurement_differences_are_stated(self):
        before = report({OMITTED: OMITTED_BLOCKED}, expected={"sample": 10, "spec": 134})
        after = report({}, last_layer="L4", measurement_errors=["L1 roster: list command exited 101"], suites={"L1": {"exit": 101}})
        notes = compare.warnings(before, after)
        self.assertTrue(any("different layer ranges (through L5 and L4)" in note for note in notes))
        self.assertTrue(any("later report has 1 incomplete measurements" in note for note in notes))
        self.assertTrue(any("nonzero exit: L1" in note for note in notes))
        self.assertTrue(any("expected sample count changed from 10 to none" in note for note in notes))
        self.assertEqual(compare.warnings(before, copy.deepcopy(before)), [])

    def test_tab_separated_rows_round_trip_and_omit_an_empty_final_column(self):
        before = report({OMITTED: OMITTED_BLOCKED, IMPORT: IMPORT_BEFORE, ACCESS: ACCESS_BEFORE})
        after = report({IMPORT: IMPORT_AFTER, ACCESS: {**ACCESS_AFTER, "detail": ACCESS_AFTER["detail"] + "\tand\nmore 100%"}})
        groups, _ = compare.compare(before, after)
        text = compare.tsv(before, after, groups)
        lines = text.split("\n")
        self.assertEqual(lines[0], "group\tcase\tfield\tbefore\tafter")
        self.assertIn(f"removed-blocker\t{OMITTED}\tstatus\tchecker input blocked", lines)
        self.assertIn(f"stop-changed\t{IMPORT}\tlocations\t\troot/main.omg:3:5 bytes 140..149 \"kit\\\\other\"", lines)
        rows = [{field: unescape(value or "").decode() for field, value in row.items()}
                for row in csv.DictReader(StringIO(text), delimiter="\t", quoting=csv.QUOTE_NONE)]
        self.assertEqual([row for row in rows if row["group"] == "removed-blocker"],
                         [{"group": "removed-blocker", "case": OMITTED, "field": "status", "before": "checker input blocked", "after": ""}])
        self.assertTrue(any(row["group"] == "text-only" and row["after"].endswith("\tand\nmore 100%") for row in rows))
        self.assertEqual(sum(row["group"] == "report" for row in rows), 6)

    def test_the_command_reads_directories_and_refuses_unreadable_input(self):
        with tempfile.TemporaryDirectory() as directory:
            first, second = Path(directory, "20261121T094024Z-j7ql3pxg"), Path(directory, "20261121T095845Z-qo1b81g_")
            for path, cases in [(first, {OMITTED: OMITTED_BLOCKED}), (second, {OMITTED: OMITTED_INVALID})]:
                path.mkdir()
                (path / "evidence.json").write_text(json.dumps(report(cases)), encoding="utf-8")
            output = StringIO()
            with redirect_stdout(output):
                self.assertEqual(compare.main([str(first), str(second / "evidence.json")]), 0)
            self.assertIn("| 20261121T095845Z-qo1b81g_ |", output.getvalue())
            self.assertIn(f"- {OMITTED} · was L5 · checker input blocked", output.getvalue())
            (second / "evidence.json").write_text("{}", encoding="utf-8")
            with redirect_stdout(StringIO()), patch("sys.stderr", StringIO()) as errors:
                self.assertEqual(compare.main([str(first), str(second)]), 2)
            self.assertIn("has no case results", errors.getvalue())

    @unittest.skipUnless(os.environ.get("OMEGA_FAULT_EVIDENCE"), "set OMEGA_FAULT_EVIDENCE to replay a recorded readiness run")
    def test_replaying_the_recorded_l5_log_changes_no_case_it_produces(self):
        directory = Path(os.environ["OMEGA_FAULT_EVIDENCE"])
        recorded = compare.load(directory)
        replay = {"cases": {}, "expected": {}, "obligations": {}, "package_fixtures": [], "measurement_errors": [], "suites": {}, "last_layer": "L5"}
        check_layer_readiness.collect((directory / "L5.log").read_text(encoding="utf-8", errors="replace"), "L5", replay)
        self.assertEqual(replay["measurement_errors"], [])
        self.assertTrue(replay["cases"])
        groups, placed, _ = grouped(recorded, replay)
        # Cases written after collection (registries, closures, contextual probes) are absent from the replay.
        self.assertEqual({group for group in placed.values()} - {"removed", "removed-blocker"}, set())
        self.assertEqual(set(placed), recorded["cases"].keys() - replay["cases"].keys())


if __name__ == "__main__":
    unittest.main()
