#!/usr/bin/env python3
"""Markdown summary-line feature tests for worktree_status; stdlib, no network.

--markdown output must carry a summary line stating the total worktree count
and how many are missing/degraded, so a wave coordinator sees the shape
without scanning rows.
"""

import json
from pathlib import Path
import re
import unittest

import worktree_testkit as kit


UNDER_TEST = Path(__file__).resolve().parents[1] / "swarm" / "worktree_status.py"




class MarkdownSummaryTests(unittest.TestCase):
    def setUp(self):
        self.module = kit.load_status_module()
        self.root, self.repository = kit.seed_repository(self, 'omega ws summary ')
        for name, branch in (("wt-a", "a"), ("wt-b", "b")):
            kit.git(self, self.repository, "worktree", "add",
                     str(self.root / name), "-b", branch)
        (self.root / "wt-b" / "file.txt").write_text("modified\n",
                                                    encoding="utf-8")



    def test_markdown_has_count_summary(self):
        code, output = kit.run_main(self.module, self.repository, "--markdown")
        self.assertEqual(code, 0)
        self.assertRegex(output, re.compile(r"3 worktrees?"),
                         "summary must state the total worktree count")
        self.assertRegex(output, re.compile(r"0 missing|0 degraded|no missing|none missing",
                                            re.IGNORECASE),
                         "summary must state the missing/degraded count")
        self.assertRegex(output, re.compile(r"1 (dirty|with changes|uncommitted)",
                                            re.IGNORECASE),
                         "summary must state the dirty-worktree count")

    def test_json_record_unchanged_shape(self):
        code, output = kit.run_main(self.module, self.repository)
        self.assertEqual(code, 0)
        record = json.loads(output)
        self.assertEqual(len(record["worktrees"]), 3)
        self.assertIn("generated_utc", record)


if __name__ == "__main__":
    unittest.main()
