#!/usr/bin/env python3
"""The corpus record text format round-trips every field, including
diagnostics that carry backslashes, tabs and line breaks.

    python3 tools/tests/test_corpus_records.py -v
"""

from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import corpus_records  # noqa: E402


class RoundTrip(unittest.TestCase):
    def test_every_field_survives_render_and_parse(self):
        records = [
            {"fixture": "fail/core/escapes", "tier": "fail", "status": "rejected",
             "millis": 12, "expected_satisfied": True, "facts": {},
             "diagnostics": ["path C:\\x\\y", "two\nlines\tand tab", "trailing \\"],
             "recorded_at": None},
            {"fixture": "fail/core/wrong", "tier": "fail", "status": "rejected",
             "millis": 0, "expected_satisfied": False, "facts": {},
             "diagnostics": ["other"], "recorded_at": "3db04008aa12"},
            {"fixture": "pass/core/ok", "tier": "pass", "status": "checked",
             "millis": 3400, "expected_satisfied": None, "facts": {},
             "diagnostics": [], "recorded_at": None},
            {"fixture": "run/loop", "tier": "run", "status": "built",
             "millis": 9000, "expected_satisfied": None,
             "facts": {"exit": "0", "stdout": "match"}, "diagnostics": [],
             "recorded_at": "3db04008aa12+wt"},
        ]
        text = corpus_records.render(records)
        self.assertEqual(corpus_records.parse(text), records)
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
                     "pass/core/ok built 3ms @\n"):
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
        self.assertIn("40 commits behind HEAD (@bbbbbbbbbbbb)", line)

    def test_context_line_round_trips_and_stays_out_of_records(self):
        records = [{"fixture": "pass/core/ok", "tier": "pass",
                    "status": "checked", "millis": 3,
                    "expected_satisfied": None, "facts": {},
                    "diagnostics": [], "recorded_at": "aaaaaaaaaaaa"}]
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
                    "diagnostics": [], "recorded_at": None}]
        self.assertIsNone(
            corpus_records.context_from(corpus_records.render(records)))
        self.assertIsNone(corpus_records.context_from("# context: oops\n"))
        self.assertIsNone(corpus_records.context_from(
            "# context: [1,2]\npass/a/x checked 1ms\n"))
        # A context line after the first record is ignored: context belongs
        # to the header, before any fixture row.
        self.assertIsNone(corpus_records.context_from(
            "pass/a/x checked 1ms\n# context: {}\n"))


if __name__ == "__main__":
    unittest.main()
