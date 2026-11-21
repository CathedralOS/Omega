"""Regression checks: an L5 refusal pinned by a green test cannot pass unaccounted."""
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import handoff_closures
import refusal_evidence
from refusal_evidence import Refusal
from check_layer_readiness import case_blocks_readiness, readiness_exit

LIMIT = Refusal("tests/contracts.rs", "unsupported_predicate_call_is_refused_at_its_occurrence", "contracts.omg",
                "unsupported typed contract predicate record", "1", "limitation", "test:call in a contract predicate",
                "LOCK_EVIDENCE.md", "predicate calls remain open")
INVALID = Refusal("tests/owned_arrays.rs", "an_ordinal_outside_the_declared_extent_rejects", "arrays.omg",
                  "owned element transfer selects outside its fixed array", "1", "invalid", "-", "CONTRACT.md", "the selected element does not exist")
UNRESOLVED = Refusal("tests/ranking.rs", "unsupported_or_incomplete_ranked_components_refuse", "ranking.omg",
                     "ranking subject must name a machine formal", "1", "unresolved", "registry:ranking subject outside the formals",
                     "src/declarations/ranking.rs", "owner undecided")


def report(fixtures, tests, cases=None):
    return {"last_layer": "L5", "measurement_errors": [], "cases": cases or {},
            "suites": {"L5": {"exit": 0, "tests": [{"target": t, "test": n, "status": s} for t, n, s in tests]}},
            "package_fixtures": [{"target": t, "test": n, "files": f, "obligations": {}, "outcome": o} for t, n, f, o in fixtures]}


def refused(row, count=1, test=None, target=None):
    return [(target or row.target.replace("/", "\\"), test or row.test, row.file, f"refused: {row.message} at {row.file}")] * count


def gap(row, label):
    return {f"handoff/{label}": {"cohort": "handoff", "stage": "L5", "status": "open gap",
                                 "detail": f"needs it. Observed {row.message} at {row.file}: predicate()"}}


class RefusalAccountingTests(unittest.TestCase):
    def test_a_named_stop_must_match_the_location_returned_by_lowering(self):
        label = "call in a contract predicate"
        case = gap(LIMIT, label)
        actual = {"package": "app", "file": LIMIT.file, "line": 2, "byte_column": 4,
                  "byte_start": 18, "byte_end": 29, "occurrence": "predicate()"}
        case["handoff/" + label]["fixture_locations"] = [{**actual, "byte_start": 17}]
        observed = report(refused(LIMIT), [(LIMIT.target, LIMIT.test, "ok")], case)
        observed["package_fixtures"][0]["location"] = actual
        refusal_evidence.apply(observed, [LIMIT])
        self.assertRegex(observed["measurement_errors"][0], "labelled stop does not match")

    def test_the_registry_loads_and_never_credits_wording_that_admits_a_missing_lowering(self):
        rows = refusal_evidence.registrations()
        self.assertTrue(any(row.cls == "invalid" for row in rows))
        for message in ("unsupported typed contract predicate record", "domain alias requires indexed qualification lowering",
                        "no Psi lowering for carrier @mutable [u32]"):
            with self.assertRaises(ValueError):
                refusal_evidence.validate([INVALID._replace(message=message)])
        with self.assertRaises(ValueError):
            refusal_evidence.validate([INVALID._replace(reference="src/machines/casts.rs")])
        with self.assertRaises(ValueError):
            refusal_evidence.validate([LIMIT._replace(gap="-")])

    def test_an_unregistered_refusal_in_a_green_test_blocks(self):
        r = report(refused(INVALID), [(INVALID.target, INVALID.test, "ok")])
        refusal_evidence.apply(r, [])
        self.assertTrue(any(e.startswith("unclassified L5 refusal") for e in r["measurement_errors"]))
        self.assertEqual(readiness_exit(r), 1)

    def test_the_same_message_in_another_test_is_not_covered_by_a_registration(self):
        other = "unsupported_data_requirement_refuses_the_package_at_the_predicate_call"
        r = report(refused(LIMIT) + refused(LIMIT, test=other, target="tests\\declarations.rs"),
                   [(LIMIT.target, LIMIT.test, "ok"), ("tests\\declarations.rs", other, "ok")], gap(LIMIT, "call in a contract predicate"))
        refusal_evidence.apply(r, [LIMIT])
        self.assertEqual(len([e for e in r["measurement_errors"] if other in e and "unclassified" in e]), 1)
        self.assertEqual(len(r["measurement_errors"]), 1, r["measurement_errors"])

    def test_removing_a_gap_report_cannot_turn_the_verdict_green(self):
        with_report = report(refused(LIMIT), [(LIMIT.target, LIMIT.test, "ok")], gap(LIMIT, "call in a contract predicate"))
        refusal_evidence.apply(with_report, [LIMIT])
        self.assertEqual(with_report["measurement_errors"], [])
        self.assertEqual(readiness_exit(with_report), 1)
        without = report(refused(LIMIT), [(LIMIT.target, LIMIT.test, "ok")])
        refusal_evidence.apply(without, [LIMIT])
        self.assertIn("missing gap report", without["measurement_errors"][0])
        self.assertEqual(readiness_exit(without), 1)

    def test_a_gap_report_must_record_the_registered_refusal(self):
        cases = gap(LIMIT, "call in a contract predicate")
        cases["handoff/call in a contract predicate"]["detail"] = "Observed something else at contracts.omg: x"
        r = report(refused(LIMIT), [(LIMIT.target, LIMIT.test, "ok")], cases)
        refusal_evidence.apply(r, [LIMIT])
        self.assertIn("does not record the registered refusal", r["measurement_errors"][0])

    def test_registry_limitations_and_unresolved_refusals_become_blocking_gaps(self):
        r = report(refused(UNRESOLVED), [(UNRESOLVED.target, UNRESOLVED.test, "ok")])
        refusal_evidence.apply(r, [UNRESOLVED])
        self.assertEqual(r["measurement_errors"], [])
        case = r["cases"]["handoff/ranking subject outside the formals"]
        self.assertEqual((case["stage"], case["status"]), ("L5", "unresolved refusal"))
        self.assertTrue(case["detail"].startswith("Unresolved refusal"))
        self.assertEqual(readiness_exit(r), 1)

    def test_an_intended_rejection_is_credited_only_from_its_passing_test(self):
        checker = INVALID._replace(cls="checker", gap="registry:checker proof")
        early = report(refused(checker), [(checker.target, checker.test, "ok")])
        refusal_evidence.apply(early, [checker])
        self.assertEqual(early["cases"]["handoff/checker proof"]["status"], "checker input blocked")
        self.assertEqual(early["refusal_accounting"]["credited"], [])
        self.assertEqual(readiness_exit(early), 1)
        r = report(refused(INVALID), [(INVALID.target, INVALID.test, "ok")])
        refusal_evidence.apply(r, [INVALID])
        self.assertEqual((r["measurement_errors"], r["cases"]), ([], {}))
        self.assertEqual(len(r["refusal_accounting"]["credited"]), 1)
        self.assertEqual(readiness_exit(r), 0)
        failed = report(refused(INVALID), [(INVALID.target, INVALID.test, "FAILED")])
        refusal_evidence.apply(failed, [INVALID])
        self.assertEqual(failed["refusal_accounting"]["credited"], [])
        missing = report([], [(INVALID.target, INVALID.test, "ok")])
        refusal_evidence.apply(missing, [INVALID])
        self.assertIn("not observed in a passing test", missing["measurement_errors"][0])

    def test_a_reclassified_case_keeps_its_name_only_with_the_exact_refusal(self):
        row = INVALID._replace(gap="registry:owned element move")
        key = "handoff/owned element move"
        tests = [(row.target, row.test, "ok")]
        matched = report(refused(row), tests)
        refusal_evidence.apply(matched, [row])
        self.assertEqual(matched["measurement_errors"], [])
        self.assertEqual(matched["cases"][key]["status"], "intended rejection verified")
        missing = report([], tests)
        refusal_evidence.apply(missing, [row])
        self.assertNotIn(key, missing["cases"])
        self.assertTrue(missing["measurement_errors"])
        with self.assertRaisesRegex(ValueError, "duplicate"):
            refusal_evidence.validate([row, UNRESOLVED._replace(gap=row.gap)])

    def test_an_extra_occurrence_of_a_registered_message_is_not_absorbed(self):
        r = report(refused(INVALID, 2), [(INVALID.target, INVALID.test, "ok")])
        refusal_evidence.apply(r, [INVALID])
        self.assertIn("occurrence count changed", r["measurement_errors"][0])
        self.assertEqual(r["refusal_accounting"]["credited"], [])

    def test_an_unregistered_gap_report_is_noticed(self):
        r = report([], [], gap(LIMIT, "new gap"))
        refusal_evidence.apply(r, [])
        self.assertTrue(any(e.startswith("unregistered L5 gap report: handoff/new gap;") for e in r["measurement_errors"]))

    def test_a_run_ending_at_l4_is_not_accounted(self):
        r = report(refused(INVALID), [])
        r["last_layer"] = "L4"
        refusal_evidence.apply(r, [])
        self.assertEqual(r["measurement_errors"], [])

    def test_a_duplicate_gap_label_blocks_without_crediting_either_row(self):
        label = "owned element move"
        first, second = INVALID._replace(gap="registry:" + label), UNRESOLVED._replace(gap="registry:" + label)
        rows = [first, second, LIMIT]
        r = report(refused(first) + refused(second) + refused(LIMIT),
                   [(row.target, row.test, "ok") for row in rows], gap(LIMIT, "call in a contract predicate"))
        refusal_evidence.apply(r, rows)
        self.assertEqual(len(r["measurement_errors"]), 1, r["measurement_errors"])
        error = r["measurement_errors"][0]
        self.assertIn("duplicate gap label: " + label, error)
        self.assertIn(f"{first.target}::{first.test}", error)
        self.assertIn(f"{second.target}::{second.test}", error)
        accounting = r["refusal_accounting"]
        self.assertEqual((accounting["registered"], accounting["credited"], accounting["unclassified"]), (3, [], []))
        self.assertEqual(len(accounting["unaccounted"]), 2)
        # The unrelated limitation is still accounted exactly as with a valid registry.
        self.assertEqual(len(accounting["open"]), 1)
        self.assertEqual(r["cases"]["handoff/call in a contract predicate"]["refusal_reason"], LIMIT.message)
        case = r["cases"]["handoff/" + label]
        self.assertEqual(case["status"], "inspection failed")
        self.assertTrue(case_blocks_readiness(case))
        self.assertIn(first.message, case["detail"])
        self.assertIn(second.message, case["detail"])
        self.assertEqual(readiness_exit(r), 1)
        self.assertTrue(any("registry error" in line for line in refusal_evidence.summary(r)))

    def test_a_damaged_registry_file_is_reported_instead_of_raised(self):
        header = "\t".join([*Refusal._fields[:5], "class", *Refusal._fields[6:]]) + "\n"

        def line(row):
            return "\t".join(row) + "\n"
        duplicate = UNRESOLVED._replace(target=LIMIT.target, test=LIMIT.test, file=LIMIT.file, message="other refusal",
                                        gap="registry:call in a contract predicate")
        bodies = {
            "duplicate": (header + line(LIMIT._replace(gap="registry:call in a contract predicate")) + line(duplicate),
                          "duplicate gap label", True),
            "malformed": (header + line(LIMIT) + "tests/x.rs\tonly\n", "malformed L5 refusal registration at line 3", False),
            "header": ("target\ttest\n" + line(LIMIT), "invalid L5 refusal registry header", False),
            "empty": (header, "empty L5 refusal registry", False),
        }
        for name, (body, message, trusted) in bodies.items():
            with self.subTest(name), tempfile.TemporaryDirectory() as directory:
                path = Path(directory, "l5_refusals.tsv")
                path.write_text(body, encoding="utf-8", newline="")
                registry = refusal_evidence.read(path)
                self.assertEqual(registry.trusted, trusted)
                r = report(refused(LIMIT), [(LIMIT.target, LIMIT.test, "ok")], gap(LIMIT, "call in a contract predicate"))
                with patch.object(refusal_evidence, "load", return_value=registry):
                    refusal_evidence.apply(r)
                self.assertTrue(any(e.startswith("l5_refusals.tsv: " + message) for e in r["measurement_errors"]),
                                r["measurement_errors"])
                self.assertEqual(r["refusal_accounting"]["credited"], [])
                self.assertEqual(r["refusal_accounting"]["open"], [])
                self.assertEqual(len(r["refusal_accounting"]["unaccounted"]), 1)
                self.assertEqual(readiness_exit(r), 1)

    def test_the_driver_hooks_read_a_damaged_registry_once_and_credit_no_closure(self):
        rows = refusal_evidence.registrations()
        self.assertIs(refusal_evidence.load(), refusal_evidence.load())
        self.assertEqual(rows, refusal_evidence.load().rows)
        labelled = next(row for row in rows if row.gap_labels)
        copy = labelled._replace(test="a_second_registration_of_the_same_label")
        header = "\t".join([*Refusal._fields[:5], "class", *Refusal._fields[6:]])
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory, "l5_refusals.tsv")
            path.write_text("\n".join([header, *("\t".join(row) for row in (*rows, copy))]) + "\n",
                            encoding="utf-8", newline="")
            refusal_evidence.load.cache_clear()
            refusal_evidence.registrations.cache_clear()
            self.addCleanup(refusal_evidence.registrations.cache_clear)
            self.addCleanup(refusal_evidence.load.cache_clear)
            r = report([], [])
            r["lowering_evidence"] = {"assertions": {}}
            with patch.object(refusal_evidence, "REGISTRY", path), \
                    patch.object(refusal_evidence, "read", wraps=refusal_evidence.read) as read:
                refusal_evidence.apply(r)
                handoff_closures.apply(r)
                self.assertEqual(read.call_count, 1)
        self.assertTrue(any(e.startswith("l5_refusals.tsv: duplicate gap label: " + labelled.gap_labels[0])
                            for e in r["measurement_errors"]), r["measurement_errors"])
        self.assertTrue(any("closures not credited because l5_refusals.tsv is invalid" in e for e in r["measurement_errors"]))
        closures = [case for case in r["cases"].values() if "required_output_checks" in case]
        self.assertTrue(closures)
        self.assertTrue(all(case["status"] == "inspection failed" for case in closures))
        self.assertEqual(readiness_exit(r), 1)

if __name__ == "__main__":
    unittest.main()
