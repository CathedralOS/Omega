#!/usr/bin/env python3
"""dirty/untracked separation tests for worktree_status; stdlib, no network.

`dirty` must count only tracked changes (modified/staged porcelain entries);
`??` untracked lines belong exclusively to `untracked`. Currently dirty is
len(status_lines), which double-counts untracked files.
"""

import json
from pathlib import Path
import unittest

import worktree_testkit as kit


UNDER_TEST = Path(__file__).resolve().parents[1] / "swarm" / "worktree_status.py"




class DirtySplitTests(unittest.TestCase):
    def setUp(self):
        self.module = kit.load_status_module()
        self.root, self.repository = kit.seed_repository(self, 'omega ws dirty ')
        self.work = self.root / "wt-work"
        kit.git(self, self.repository, "worktree", "add",
                 str(self.work), "-b", "work")
        # one tracked modification + one staged + two untracked
        (self.work / "file.txt").write_text("modified\n", encoding="utf-8")
        (self.work / "staged.txt").write_text("staged\n", encoding="utf-8")
        kit.git(self, self.work, "add", "staged.txt")
        (self.work / "new-a.txt").write_text("a\n", encoding="utf-8")
        (self.work / "new-b.txt").write_text("b\n", encoding="utf-8")


    def record(self):
        code, output = kit.run_main(self.module, self.repository)
        self.assertEqual(code, 0)
        return json.loads(output)

    def test_dirty_excludes_untracked(self):
        rows = {row["name"]: row for row in self.record()["worktrees"]}
        work = rows["wt-work"]
        self.assertEqual(work["untracked"], 2)
        self.assertEqual(work["dirty"], 2,
                         "dirty must count tracked entries only: one modified "
                         "file + one staged file, not the two ?? lines")

    def test_clean_worktree_reports_zero(self):
        rows = {row["name"]: row for row in self.record()["worktrees"]}
        self.assertEqual(rows["repo"]["dirty"], 0)
        self.assertEqual(rows["repo"]["untracked"], 0)


if __name__ == "__main__":
    unittest.main()
