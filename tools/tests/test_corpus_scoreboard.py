#!/usr/bin/env python3
"""Native scoreboard counts and diffs over in-memory golden records; the
authored oracle is injected as a dict so no working tree or git revision is
needed.

    python3 tools/tests/test_corpus_scoreboard.py -v
"""

from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import corpus_records  # noqa: E402
import corpus_scoreboard  # noqa: E402


def parse(lines):
    return corpus_records.parse("\n".join(lines) + "\n")


class Scoreboard(unittest.TestCase):
    def test_counts_status_execution_and_oracles(self):
        records = parse([
            "pass/core/ok checked 10ms",
            "pass/core/built built 20ms",
            "fail/core/bad rejected 5ms",
            "run/weird ??? 7ms",
            "run/loop built 100ms exit:0 stdout:match",
            "run/exit built 100ms exit:70 stdout:differs",
            "run/trapped built 100ms exit:signal signal:5",
            "run/mismatched built 100ms exit:3",
            "run/nooracle built 100ms exit:0",
        ])
        oracles = {"run/loop": "0", "run/exit": "70", "run/trapped": "trap",
                   "run/mismatched": "42"}
        board = corpus_scoreboard.native_scoreboard(records, oracles.get)
        self.assertEqual(board["fixtures"], 9)
        self.assertEqual(board["built"], 6)
        self.assertEqual(board["checked"], 1)
        self.assertEqual(board["rejected"], 1)
        self.assertEqual(board["other"], 1)
        self.assertEqual(board["executed"], 5)
        self.assertEqual(board["authored"], 3)
        self.assertEqual(board["unverified"], 1)
        self.assertEqual(board["oracle_mismatch"], ["run/mismatched"])
        self.assertEqual(board["stdout_match"], 1)
        self.assertEqual(board["stdout_differs"], 1)

    def test_trap_oracle_matches_signal_only(self):
        signal = corpus_records.parse(
            "run/trap built 1ms exit:signal signal:5\n")[0]
        clean = corpus_records.parse("run/ok built 1ms exit:0\n")[0]
        self.assertTrue(
            corpus_scoreboard.oracle_matches("trap", signal["facts"]))
        self.assertFalse(
            corpus_scoreboard.oracle_matches("trap", clean["facts"]))
        self.assertTrue(corpus_scoreboard.oracle_matches("0", clean["facts"]))
        self.assertFalse(
            corpus_scoreboard.oracle_matches("0", signal["facts"]))

    def test_diff_reports_authored_and_executed_moves(self):
        base = parse([
            "run/kept built 1ms exit:0",
            "run/regressed built 1ms exit:0",
            "run/gone checked 1ms",
            "run/changed built 1ms exit:0",
            "run/onlyexec checked 1ms",
            "pass/still checked 1ms",
        ])
        head = parse([
            "run/kept built 1ms exit:0",
            "run/regressed checked 1ms",
            "run/gone checked 1ms",
            "run/changed built 1ms exit:42",
            "run/onlyexec built 1ms exit:3",
            "run/new built 1ms exit:0",
            "pass/still checked 1ms",
        ])
        oracles = {"run/kept": "0", "run/regressed": "0", "run/gone": "0",
                   "run/changed": "42", "run/new": "0"}
        diff = corpus_scoreboard.diff_scoreboards(base, head, oracles.get)
        self.assertEqual(diff["base"]["authored"], 2)
        self.assertEqual(diff["head"]["authored"], 3)
        self.assertEqual(diff["authored_delta"], 1)
        self.assertEqual(diff["executed_delta"], 1)
        self.assertEqual(diff["gained"], ["run/changed", "run/new"])
        self.assertEqual(diff["lost"], ["run/regressed"])
        self.assertEqual(diff["executed_gained"],
                         ["run/new", "run/onlyexec"])
        self.assertEqual(diff["executed_lost"], ["run/regressed"])
        self.assertEqual(diff["exit_changed"], ["run/changed"])

    def test_summary_line(self):
        board = {"fixtures": 746, "built": 221, "checked": 288,
                 "rejected": 237, "other": 0, "executed": 172,
                 "authored": 64, "unverified": 108, "oracle_mismatch": [],
                 "stdout_match": 2, "stdout_differs": 0}
        self.assertEqual(
            corpus_scoreboard.summary_line(board, "macos_arm64"),
            "macos_arm64 native: 64 authored / 172 executed / 746 fixtures, "
            "237 rejected (108 executed without an oracle)")


class CollectBaseGolden(unittest.TestCase):
    """--base-golden diffs the working golden against any recorded file —
    the scratch-recording workflow that used to need an ad-hoc script."""

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.corpus = self.root / "tests" / "omega"
        for fixture in ("run/loop", "run/new", "run/lost"):
            directory = self.corpus / fixture
            directory.mkdir(parents=True)
            (directory / "expected_native_exit.txt").write_text(
                "0\n", encoding="utf-8")
        self.golden = self.corpus / "corpus_native_macos_arm64.txt"
        self.scratch = self.root / "scratch_golden.txt"

    def write(self, path, lines):
        path.write_text("\n".join(lines) + "\n", encoding="utf-8")

    def test_collect_diffs_working_golden_against_scratch_file(self):
        self.write(self.scratch, ["run/loop built 1ms exit:0",
                                  "run/lost built 1ms exit:0"])
        self.write(self.golden, ["run/loop built 1ms exit:0",
                                 "run/new built 1ms exit:0"])
        board = corpus_scoreboard.collect(self.root, golden=self.golden,
                                          base_golden=self.scratch)
        self.assertEqual(board["base_label"], str(self.scratch))
        self.assertIsNone(board["base_rev"])
        self.assertEqual(board["gained"], ["run/new"])
        self.assertEqual(board["lost"], ["run/lost"])
        self.assertEqual(board["executed_lost"], ["run/lost"])

    def test_base_and_base_golden_are_mutually_exclusive(self):
        self.write(self.golden, ["run/loop built 1ms exit:0"])
        with self.assertRaises(corpus_scoreboard.ScoreboardError):
            corpus_scoreboard.collect(self.root, golden=self.golden,
                                      base_rev="abc123",
                                      base_golden=self.scratch)

    def test_missing_base_golden_is_an_error(self):
        self.write(self.golden, ["run/loop built 1ms exit:0"])
        with self.assertRaises(corpus_scoreboard.ScoreboardError):
            corpus_scoreboard.collect(self.root, golden=self.golden,
                                      base_golden=self.root / "absent.txt")


class CrossGolden(unittest.TestCase):
    """The two-file-read detector for a native verdict outliving a check-leg
    regression — the one false green no diagnostic detector can find."""

    def test_a_native_success_the_check_golden_refuses_is_listed(self):
        native = parse(["pass/a/stale built 5ms exit:70",
                        "pass/a/also_stale checked 5ms"])
        checked = parse(["pass/a/stale rejected 5ms", "\tdivisor nonzero",
                         "pass/a/also_stale incomplete 5ms"])
        found = corpus_scoreboard.cross_golden_disagreements(native, checked)
        self.assertEqual([entry["fixture"] for entry in found],
                         ["pass/a/stale", "pass/a/also_stale"])
        self.assertEqual(found[0]["native_facts"], {"exit": "70"})
        self.assertEqual(found[0]["check"], "rejected")
        self.assertEqual(found[0]["check_diagnostic"], "divisor nonzero")
        # `incomplete` is not a success either: one of the eight rows this was
        # written for was recorded that way, and a detector keyed to
        # `rejected` alone would have missed it.
        self.assertEqual(found[1]["check"], "incomplete")

    def test_agreement_absence_and_a_recorded_loss_are_not_listed(self):
        native = parse([
            "pass/a/fine built 5ms exit:0",
            "pass/a/agrees rejected 5ms",
            "pass/a/only_native built 5ms",
            "pass/a/recorded rejected 5ms regressed-from:built+exit:70"])
        checked = parse(["pass/a/fine checked 5ms",
                         "pass/a/agrees rejected 5ms",
                         "pass/a/recorded rejected 5ms"])
        self.assertEqual(
            corpus_scoreboard.cross_golden_disagreements(native, checked), [])

    def test_the_scoreboard_counts_and_names_a_recorded_loss(self):
        records = parse([
            "pass/a/lost rejected 5ms regressed-from:built+exit:70",
            "pass/a/never rejected 5ms",
            "pass/a/fine built 5ms exit:70"])
        board = corpus_scoreboard.native_scoreboard(records, lambda _: "70")
        self.assertEqual(board["regressed"], 1)
        self.assertEqual(board["regressed_rows"], [
            {"fixture": "pass/a/lost", "status": "rejected",
             "lost": {"status": "built", "facts": {"exit": "70"}}}])
        # A row recording a loss is not an authored run and not executed.
        self.assertEqual((board["authored"], board["executed"],
                          board["rejected"]), (1, 1, 2))
        self.assertIn("1 regressed",
                      corpus_scoreboard.summary_line(board, "macos_arm64"))


if __name__ == "__main__":
    unittest.main()
