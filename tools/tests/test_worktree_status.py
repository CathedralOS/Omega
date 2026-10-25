#!/usr/bin/env python3
"""Worktree status tests; stdlib only, no network (--offline).

A listed-but-deleted worktree directory (git marks it "prunable") must not
crash the report: present worktrees still report live git state, the missing
entry is visibly marked, and --markdown still renders.
"""

import json
from pathlib import Path
import re
import shutil
import unittest
from unittest import mock

import worktree_testkit as kit


UNDER_TEST = Path(__file__).resolve().parents[1] / "swarm" / "worktree_status.py"




class WorktreeStatusTests(unittest.TestCase):
    def setUp(self):
        self.module = kit.load_status_module()
        self.root, self.repository = kit.seed_repository(self, 'omega ws test ')
        self.keep = self.root / "wt-keep"
        self.gone = self.root / "wt-gone"
        kit.git(self, self.repository, "worktree", "add",
                 str(self.keep), "-b", "keep")
        kit.git(self, self.repository, "worktree", "add",
                 str(self.gone), "-b", "gone")
        # A dirty present worktree so the live-state fields are exercised.
        (self.keep / "file.txt").write_text("modified\n", encoding="utf-8")
        (self.keep / "new.txt").write_text("new\n", encoding="utf-8")
        # Delete the worktree directory but leave the stale registration.
        shutil.rmtree(self.gone)
        listed = kit.git(self, self.repository, "worktree", "list", "--porcelain")
        self.assertIn("prunable", listed)


    def record(self):
        code, output = kit.run_main(self.module, self.repository)
        self.assertEqual(code, 0, "main() must not fail on a prunable worktree")
        return json.loads(output)

    def test_missing_worktree_does_not_fail_the_report(self):
        code, _ = kit.run_main(self.module, self.repository)
        self.assertEqual(code, 0)

    def test_native_queue_state_and_absent_configuration_are_distinct(self):
        queue = dict(entries=dict(pageInfo=dict(hasNextPage=True), nodes=[
            dict(pullRequest=dict(url="https://github.com/example/repo/pull/1"))]))
        with mock.patch.object(self.module.landing, "queue_status", return_value=queue):
            status = self.module.landing_state(self.repository, "origin")
        self.assertEqual(status["state"], "github-merge-queue")
        self.assertEqual(status["queue_depth"], 1)
        self.assertTrue(status["has_more"])
        self.assertIsNone(status["head_expires_utc"])
        with mock.patch.object(self.module.landing, "queue_status", return_value=None):
            self.assertEqual(self.module.landing_state(self.repository, "origin")["state"], "not-configured")

    def test_missing_worktree_is_marked(self):
        rows = {row["name"]: row for row in self.record()["worktrees"]}
        self.assertIn("wt-gone", rows,
                      "the missing worktree must not be dropped from the report")
        self.assertTrue(rows["wt-gone"].get("missing"),
                        "the missing worktree must be visibly marked")
        for field in ("dirty", "untracked", "ahead", "behind", "landed",
                      "branch", "head_subject"):
            self.assertIsNone(rows["wt-gone"].get(field),
                              f"missing worktree must not report live {field}")
        self.assertFalse(rows["wt-keep"].get("missing"),
                         "present worktrees must not be marked missing")

    def test_present_worktrees_still_report_git_state(self):
        rows = {row["name"]: row for row in self.record()["worktrees"]}
        self.assertIn("wt-keep", rows)
        keep = rows["wt-keep"]
        self.assertEqual(keep["branch"], "keep")
        self.assertEqual(keep["dirty"], 1)
        self.assertEqual(keep["untracked"], 1)
        self.assertIsInstance(keep["ahead"], int)
        self.assertIsInstance(keep["behind"], int)
        self.assertIsInstance(keep["landed"], bool)
        self.assertIsInstance(keep["head_subject"], str)

    def test_markdown_renders_and_marks_the_missing_row(self):
        code, output = kit.run_main(self.module, self.repository, "--markdown")
        self.assertEqual(code, 0)
        self.assertIn("wt-gone", output)
        self.assertRegex(
            output, re.compile(r"missing|prunable|absent|gone|deleted",
                               re.IGNORECASE),
            "markdown must visibly mark the missing worktree")


if __name__ == "__main__":
    unittest.main()
