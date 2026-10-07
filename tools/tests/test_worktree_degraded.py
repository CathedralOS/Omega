#!/usr/bin/env python3
"""Degraded-worktree tolerance tests for worktree_status; stdlib, no network.

A worktree whose per-worktree git probes fail — deleted directory, or an
unborn branch (orphan checkout: rev-list/log fail with 128) — must degrade to
a marked row, not abort the report.
"""

import json
from pathlib import Path
import re
import shutil
import unittest

import worktree_testkit as kit


UNDER_TEST = Path(__file__).resolve().parents[1] / "swarm" / "worktree_status.py"




class DegradedWorktreeTests(unittest.TestCase):
    def setUp(self):
        self.module = kit.load_status_module()
        self.root, self.repository = kit.seed_repository(self, 'omega ws degraded ')
        self.keep = self.root / "wt-keep"
        self.gone = self.root / "wt-gone"
        self.orphan = self.root / "wt-orphan"
        kit.git(self, self.repository, "worktree", "add",
                 str(self.keep), "-b", "keep")
        kit.git(self, self.repository, "worktree", "add",
                 str(self.gone), "-b", "gone")
        kit.git(self, self.repository, "worktree", "add",
                 str(self.orphan), "-b", "orphan-base")
        # Orphan checkout: HEAD is born no longer; rev-list/log fail.
        kit.git(self, self.orphan, "checkout", "--orphan", "unborn")
        (self.keep / "file.txt").write_text("modified\n", encoding="utf-8")
        shutil.rmtree(self.gone)


    def record(self):
        code, output = kit.run_main(self.module, self.repository)
        self.assertEqual(code, 0, "main() must tolerate degraded worktrees")
        return json.loads(output)

    def rows(self):
        return {row["name"]: row for row in self.record()["worktrees"]}

    def test_unborn_worktree_does_not_crash(self):
        rows = self.rows()
        self.assertIn("wt-orphan", rows)
        orphan = rows["wt-orphan"]
        self.assertTrue(orphan.get("missing") or orphan.get("degraded"),
                        "unborn worktree must be visibly marked")
        self.assertEqual(orphan["branch"], "unborn")
        for field in ("behind", "ahead", "head_subject"):
            self.assertIsNone(orphan.get(field),
                              f"unborn worktree cannot report {field}")
        self.assertFalse(orphan.get("landed"),
                         "unborn worktree must not claim landed")

    def test_deleted_worktree_still_tolerated(self):
        rows = self.rows()
        self.assertIn("wt-gone", rows)
        self.assertTrue(rows["wt-gone"].get("missing")
                        or rows["wt-gone"].get("degraded"))

    def test_present_worktree_keeps_live_state(self):
        rows = self.rows()
        self.assertIn("wt-keep", rows)
        keep = rows["wt-keep"]
        self.assertFalse(keep.get("missing") or keep.get("degraded"))
        self.assertEqual(keep["branch"], "keep")
        self.assertIsInstance(keep["dirty"], int)
        self.assertIsInstance(keep["landed"], bool)

    def test_markdown_renders_all_rows(self):
        code, output = kit.run_main(self.module, self.repository, "--markdown")
        self.assertEqual(code, 0)
        text = output
        for name in ("wt-keep", "wt-gone", "wt-orphan"):
            self.assertIn(name, text)
        self.assertRegex(text, re.compile(
            r"missing|prunable|absent|gone|deleted|degraded|unborn|no commits",
            re.IGNORECASE))


if __name__ == "__main__":
    unittest.main()
