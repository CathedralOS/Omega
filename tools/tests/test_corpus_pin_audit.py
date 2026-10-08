"""Tests for corpus_pin_audit: weak pins and cross-rule diagnostic hits."""

from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import corpus_pin_audit


GOLDEN = """\
# a comment line must be ignored
fail/group/a rejected expected @abc
	cannot prove index `x` is within length 4
fail/group/b rejected expected @abc
	cannot prove index `y` is within length 2
	value `z` was transferred or consumed at statement 1: it holds no value
fail/group/c rejected expected @abc
	value `w` was transferred or consumed at statement 2: it holds no value
pass/group/ok built expected @abc
"""

FRAGMENTS = {
    "fail/group/a": ["cannot prove"],
    "fail/group/b": ["is within length"],
    "fail/group/c": ["was transferred or consumed at statement"],
}


class GoldenRowsTests(unittest.TestCase):
    def test_parses_rows_and_attaches_tabbed_diagnostics(self):
        rows = corpus_pin_audit.golden_rows(GOLDEN)
        self.assertEqual(set(rows), {"fail/group/a", "fail/group/b",
                                     "fail/group/c", "pass/group/ok"})
        status, diagnostics = rows["fail/group/b"]
        self.assertEqual(status, "rejected")
        self.assertEqual(diagnostics[1],
                         "value `z` was transferred or consumed at "
                         "statement 1: it holds no value")
        self.assertEqual(rows["pass/group/ok"], ("built", []))


class WeakPinsTests(unittest.TestCase):
    def setUp(self):
        self.rows = corpus_pin_audit.golden_rows(GOLDEN)

    def test_threshold_one_reports_all_shared_fragments(self):
        findings = corpus_pin_audit.weak_pins(self.rows, FRAGMENTS, 1)
        by_name = {(name, fragment): count
                   for count, name, fragment in findings}
        self.assertEqual(by_name, {
            ("fail/group/a", "cannot prove"): 1,
            ("fail/group/b", "is within length"): 1,
            ("fail/group/c", "was transferred or consumed at statement"): 1})

    def test_threshold_two_reports_nothing(self):
        self.assertEqual(
            corpus_pin_audit.weak_pins(self.rows, FRAGMENTS, 2), [])


class CrossRuleHitsTests(unittest.TestCase):
    def test_reports_only_the_diagnostic_tripping_a_siblings_rule(self):
        rows = corpus_pin_audit.golden_rows(GOLDEN)
        hits = corpus_pin_audit.cross_rule_hits(rows, FRAGMENTS)
        self.assertEqual(hits, [(
            "fail/group/b", "fail/group/c",
            "value `z` was transferred or consumed at statement 1: "
            "it holds no value")])


class ExpectedFragmentsTests(unittest.TestCase):
    def test_reads_non_empty_lines_and_tolerates_absence(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fixture = root / "tests" / "omega" / "fail" / "group" / "a"
            fixture.mkdir(parents=True)
            (fixture / "expected.txt").write_text(
                "cannot prove\n\nsecond fragment\n", encoding="utf-8")
            self.assertEqual(
                corpus_pin_audit.expected_fragments(root, "fail/group/a"),
                ["cannot prove", "second fragment"])
            self.assertEqual(
                corpus_pin_audit.expected_fragments(root, "fail/group/missing"),
                [])


if __name__ == "__main__":
    unittest.main()
