#!/usr/bin/env python3
"""Native scoreboard counts and diffs over in-memory golden records; the
authored oracle is injected as a dict so no working tree or git revision is
needed.

    python3 tools/tests/test_corpus_scoreboard.py -v
"""

from pathlib import Path
import sys
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


if __name__ == "__main__":
    unittest.main()
