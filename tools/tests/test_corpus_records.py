#!/usr/bin/env python3
"""The corpus record text format round-trips every field, including
diagnostics that carry backslashes, tabs and line breaks, and the lost-success
token a row carries when it no longer reaches a success it once recorded.

    python3 tools/tests/test_corpus_records.py -v
"""

from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import corpus_records  # noqa: E402


class CarriedRegression(unittest.TestCase):
    """What `--record` derives when it replaces a row."""

    def row(self, status, facts=None, regressed_from=None):
        return {"fixture": "pass/a/x", "tier": "pass", "status": status,
                "millis": 1, "expected_satisfied": None, "facts": facts or {},
                "diagnostics": [], "regressed_from": regressed_from,
                "recorded_at": None}

    def test_a_success_becoming_a_refusal_names_the_lost_outcome(self):
        self.assertEqual(
            corpus_records.carried_regression(
                self.row("built", {"exit": "70"}), self.row("rejected")),
            {"status": "built", "facts": {"exit": "70"}})

    def test_no_recorded_success_leaves_nothing_to_lose(self):
        for prior in (None, self.row("rejected"), self.row("incomplete")):
            self.assertIsNone(corpus_records.carried_regression(
                prior, self.row("rejected")))

    def test_a_weaker_success_is_told_by_the_status_column(self):
        self.assertIsNone(corpus_records.carried_regression(
            self.row("built", {"exit": "0"}), self.row("checked")))

    def test_a_continuing_failure_keeps_the_original_success(self):
        lost = {"status": "built", "facts": {"exit": "70"}}
        self.assertEqual(corpus_records.carried_regression(
            self.row("rejected", regressed_from=lost), self.row("timeout")),
            lost)

    def test_a_repair_drops_the_token(self):
        lost = {"status": "built", "facts": {"exit": "70"}}
        self.assertIsNone(corpus_records.carried_regression(
            self.row("rejected", regressed_from=lost),
            self.row("built", {"exit": "70"})))

    def test_lost_successes_selects_only_rows_carrying_a_token(self):
        lost = {"status": "built", "facts": {}}
        rows = [self.row("built"), self.row("rejected"),
                self.row("rejected", regressed_from=lost)]
        self.assertEqual(corpus_records.lost_successes(rows), [rows[2]])

    def test_a_refusal_cannot_be_formatted_as_a_lost_success(self):
        with self.assertRaises(ValueError):
            corpus_records.format_regression({"status": "rejected", "facts": {}})
        with self.assertRaises(ValueError):
            corpus_records.format_regression(
                {"status": "built", "facts": {"exit": "7 0"}})


class Splice(unittest.TestCase):
    """A subset record changes only the rows it measured."""

    def row(self, fixture, status="checked", diagnostics=()):
        return {"fixture": fixture, "tier": fixture.split("/")[0],
                "status": status, "millis": 5, "expected_satisfied": None,
                "facts": {}, "diagnostics": list(diagnostics),
                "regressed_from": None, "recorded_at": "aaaaaaaaaaaa"}

    def test_rows_replace_in_place_and_insert_sorted(self):
        text = ("# header comment\n"
                "# context: {\"jobs\": \"3\"}\n"
                "pass/a/b rejected 1ms\n"
                "\told diagnostic\n"
                "# an annotation another lane wrote\n"
                "pass/a/d checked 2ms\n"
                "pass/a/c checked 3ms @bbbbbbbbbbbb\n")
        spliced = corpus_records.splice(text, [
            self.row("pass/a/b", diagnostics=["fresh"]),
            self.row("pass/a/a"),
            self.row("pass/a/z"),
        ])
        self.assertEqual(spliced, (
            "# header comment\n"
            "# context: {\"jobs\": \"3\"}\n"
            "pass/a/a checked 5ms @aaaaaaaaaaaa\n"
            "pass/a/b checked 5ms @aaaaaaaaaaaa\n"
            "\tfresh\n"
            "# an annotation another lane wrote\n"
            "pass/a/d checked 2ms\n"
            "pass/a/c checked 3ms @bbbbbbbbbbbb\n"
            "pass/a/z checked 5ms @aaaaaaaaaaaa\n"))

    def test_an_unchanged_row_round_trips_byte_for_byte(self):
        record = self.row("pass/a/b", diagnostics=["two\nlines"])
        text = corpus_records.render([record])
        self.assertEqual(corpus_records.splice(text, [record]), text)


class RoundTrip(unittest.TestCase):
    def test_every_field_survives_render_and_parse(self):
        records = [
            {"fixture": "fail/core/escapes", "tier": "fail", "status": "rejected",
             "millis": 12, "expected_satisfied": True, "facts": {},
             "diagnostics": ["path C:\\x\\y", "two\nlines\tand tab", "trailing \\"],
             "regressed_from": None, "recorded_at": None},
            {"fixture": "fail/core/wrong", "tier": "fail", "status": "rejected",
             "millis": 0, "expected_satisfied": False, "facts": {},
             "diagnostics": ["other"], "regressed_from": None,
             "recorded_at": "3db04008aa12"},
            {"fixture": "pass/core/ok", "tier": "pass", "status": "checked",
             "millis": 3400, "expected_satisfied": None, "facts": {},
             "diagnostics": [], "regressed_from": None, "recorded_at": None},
            {"fixture": "run/loop", "tier": "run", "status": "built",
             "millis": 9000, "expected_satisfied": None,
             "facts": {"exit": "0", "stdout": "match"}, "diagnostics": [],
             "regressed_from": None, "recorded_at": "3db04008aa12+wt"},
            {"fixture": "run/lost", "tier": "run", "status": "rejected",
             "millis": 40, "expected_satisfied": None, "facts": {},
             "diagnostics": ["a later refusal"],
             "regressed_from": {"status": "built",
                                "facts": {"exit": "70", "stdout": "match"}},
             "recorded_at": "3db04008aa12"},
        ]
        text = corpus_records.render(records)
        self.assertEqual(corpus_records.parse(text), records)
        lost = [line for line in text.splitlines()
                if line.startswith("run/lost")][0]
        self.assertIn(" regressed-from:built+exit:70+stdout:match @", lost)
        self.assertTrue(all(line.startswith(("#", "\t", "fail/", "pass/", "run/"))
                            for line in text.splitlines()))
        stamped = [line for line in text.splitlines()
                   if line.startswith("fail/core/wrong")][0]
        self.assertTrue(stamped.endswith(" @3db04008aa12"))

    def test_malformed_lines_are_refused(self):
        for text in ("\tdiagnostic first\n", "pass/core/ok checked\n",
                     "pass/core/ok checked 3ms maybe\n",
                     "pass/core/ok built 3ms exit:0 expected\n",
                     "pass/core/ok built 3ms @rev exit:0\n",
                     "pass/core/ok built 3ms @\n",
                     # Only a success can be lost, and only once per row.
                     "pass/core/ok rejected 3ms regressed-from:rejected\n",
                     "pass/core/ok rejected 3ms regressed-from:built+exit\n",
                     "pass/core/ok rejected 3ms regressed-from:built+exit:0+exit:1\n",
                     "pass/core/ok rejected 3ms regressed-from:built"
                     " regressed-from:checked\n",
                     "pass/core/ok rejected 3ms regressed-from:built expected\n"):
            with self.assertRaises(ValueError):
                corpus_records.parse(text)

    def test_legacy_rows_have_no_stamp(self):
        record = corpus_records.parse("pass/core/ok checked 3ms\n")[0]
        self.assertIsNone(record["recorded_at"])

    def test_duplicate_facts_cannot_overwrite_runtime_evidence(self):
        for facts in ("exit:7 exit:70", "stdout:differs stdout:match",
                      "signal:11 signal:4", "exit:70 exit:70",
                      "interpreter-stdout:differs interpreter-stdout:match"):
            with self.subTest(facts=facts), self.assertRaisesRegex(
                    ValueError, "line 1: duplicate fact"):
                corpus_records.parse(f"run/a/x built 1ms {facts}\n")

    def test_diagnostic_alias_normalizes_only_the_retired_prefix(self):
        old = "native-artifact Terminal production failed: lowering x"
        new = "terminal-artifact production failed: lowering x"
        self.assertEqual(corpus_records.normalized_diagnostic(old), new)
        self.assertEqual(corpus_records.normalized_diagnostics([old, "keep"]),
                         [new, "keep"])
        self.assertEqual(corpus_records.normalized_diagnostic(new), new)
        self.assertEqual(
            corpus_records.normalized_diagnostic("other prefix: x"),
            "other prefix: x")

    def test_provenance_counts_stamps_and_reports_oldest(self):
        records = corpus_records.parse(
            "a/x checked 1ms @aaaaaaaaaaaa\n"
            "a/y checked 1ms @bbbbbbbbbbbb+wt\n"
            "a/z checked 1ms\n"
            "a/w checked 1ms @cccccccccccc\n")
        commits = {"aaaaaaaaaaaa": 3, "bbbbbbbbbbbb": 40}
        report = corpus_records.provenance(
            records, lambda revision: commits.get(revision))
        self.assertEqual(report["stamped"], 3)
        self.assertEqual(report["legacy"], 1)
        self.assertEqual(report["unknown_revisions"], 1)
        self.assertEqual(report["oldest"],
                         {"revision": "bbbbbbbbbbbb", "commits_behind": 40})
        line = corpus_records.provenance_line(report)
        self.assertIn("3 rows stamped", line)
        self.assertIn("1 legacy", line)
        self.assertIn("1 revision distance(s) unavailable", line)
        self.assertIn("40 commits behind HEAD (@bbbbbbbbbbbb)", line)

    def test_context_line_round_trips_and_stays_out_of_records(self):
        records = [{"fixture": "pass/core/ok", "tier": "pass",
                    "status": "checked", "millis": 3,
                    "expected_satisfied": None, "facts": {},
                    "diagnostics": [], "regressed_from": None,
                    "recorded_at": "aaaaaaaaaaaa"}]
        context = {"schema": "omega-corpus-context-v1", "jobs": "8",
                   "cpus": 10, "platform": "darwin", "machine": "arm64",
                   "load1_start": 1.5, "load1_end": 9.25}
        text = corpus_records.render(records, context)
        self.assertIn("# context: ", text)
        self.assertEqual(corpus_records.context_from(text), context)
        self.assertEqual(corpus_records.parse(text), records)

    def test_context_absent_or_malformed_reads_none(self):
        records = [{"fixture": "pass/core/ok", "tier": "pass",
                    "status": "checked", "millis": 3,
                    "expected_satisfied": None, "facts": {},
                    "diagnostics": [], "regressed_from": None,
                    "recorded_at": None}]
        self.assertIsNone(
            corpus_records.context_from(corpus_records.render(records)))
        self.assertIsNone(corpus_records.context_from("# context: oops\n"))
        self.assertIsNone(corpus_records.context_from(
            "# context: [1,2]\npass/a/x checked 1ms\n"))
        # A context line after the first record is ignored: context belongs
        # to the header, before any fixture row.
        self.assertIsNone(corpus_records.context_from(
            "pass/a/x checked 1ms\n# context: {}\n"))


class PinnedOffTarget(unittest.TestCase):
    def row(self, fixture, status, regressed_from=None):
        return {"fixture": fixture, "tier": fixture.split("/", 1)[0],
                "status": status, "millis": 1, "expected_satisfied": None,
                "facts": {}, "diagnostics": [],
                "regressed_from": regressed_from, "recorded_at": None}

    def test_selects_rows_pinned_against_their_tier(self):
        rows = [self.row("pass/a/rejected", "rejected"),
                self.row("pass/a/checked", "checked"),
                self.row("fail/a/checked", "checked"),
                self.row("fail/a/rejected", "rejected"),
                self.row("run/a/incomplete", "incomplete")]
        self.assertEqual(
            [record["fixture"]
             for record in corpus_records.pinned_off_target(rows)],
            ["pass/a/rejected", "fail/a/checked", "run/a/incomplete"])


class CompareRecords(unittest.TestCase):
    """`--compare` diffs two fresh runs of one selection."""

    def row(self, fixture, status, expected=None, facts=None):
        return {"fixture": fixture, "tier": fixture.split("/", 1)[0],
                "status": status, "millis": 1, "expected_satisfied": expected,
                "facts": facts or {}, "diagnostics": [],
                "regressed_from": None, "recorded_at": None}

    def test_identical_runs_have_no_moves(self):
        rows = [self.row("pass/a/x", "built", facts={"exit": "70"}),
                self.row("fail/a/y", "rejected", expected=True)]
        self.assertEqual(corpus_records.compare_records(rows, rows),
                         {"losses": [], "gains": [], "fixtures": 2})

    def test_weaker_outcomes_unmet_fragments_and_changed_facts_are_losses(self):
        base = [self.row("pass/a/built", "built"),
                self.row("pass/a/exit", "built", facts={"exit": "70"}),
                self.row("fail/a/reason", "rejected", expected=True),
                self.row("fail/a/accepts", "rejected"),
                self.row("pass/a/gone", "checked")]
        head = [self.row("pass/a/built", "checked"),
                self.row("pass/a/exit", "built", facts={"exit": "72"}),
                self.row("fail/a/reason", "rejected", expected=False),
                self.row("fail/a/accepts", "checked")]
        self.assertEqual(corpus_records.compare_records(base, head)["losses"], [
            "fail/a/accepts: rejected -> checked",
            "fail/a/reason: expected fragments no longer met",
            "pass/a/built: built -> checked",
            "pass/a/exit: exit 70 -> 72",
            "pass/a/gone: absent from head"])

    def test_repairs_and_new_fixtures_are_gains(self):
        base = [self.row("pass/a/x", "rejected")]
        head = [self.row("pass/a/x", "built", facts={"exit": "70"}),
                self.row("pass/a/new", "checked")]
        result = corpus_records.compare_records(base, head)
        self.assertEqual(result["losses"], [])
        self.assertEqual(result["gains"], [
            "pass/a/new: new checked",
            "pass/a/x: rejected -> built",
            "pass/a/x: exit - -> 70"])


    def test_completed_engine_agreement_is_a_gain_after_no_agreement(self):
        for previous in ("unchecked", "declined", "exhausted", "disagrees"):
            with self.subTest(previous=previous):
                base = [self.row("pass/a/x", "built", facts={"interpreter": previous})]
                head = [self.row("pass/a/x", "built", facts={"interpreter": "agrees"})]
                result = corpus_records.compare_records(base, head)
                self.assertEqual(result["losses"], [])
                self.assertEqual(result["gains"], [f"pass/a/x: interpreter {previous} -> agrees"])
                self.assertEqual(corpus_records.compare_records(head, base)["losses"],
                                 [f"pass/a/x: interpreter agrees -> {previous}"])

    def test_new_agreement_does_not_hide_a_changed_exit(self):
        base = [self.row("pass/a/x", "built", facts={"exit": "70", "interpreter": "declined"})]
        head = [self.row("pass/a/x", "built", facts={"exit": "71", "interpreter": "agrees"})]
        result = corpus_records.compare_records(base, head)
        self.assertEqual(result["losses"], ["pass/a/x: exit 70 -> 71"])
        self.assertEqual(result["gains"], ["pass/a/x: interpreter declined -> agrees"])


if __name__ == "__main__":
    unittest.main()
