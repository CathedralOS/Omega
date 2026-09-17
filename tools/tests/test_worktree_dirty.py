#!/usr/bin/env python3
"""dirty/untracked separation tests for worktree_status; stdlib, no network.

`dirty` must count only tracked changes (modified/staged porcelain entries);
`??` untracked lines belong exclusively to `untracked`. Currently dirty is
len(status_lines), which double-counts untracked files.
"""

import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


UNDER_TEST = Path(__file__).resolve().parents[1] / "swarm" / "worktree_status.py"


def load_module():
    specification = importlib.util.spec_from_file_location(
        "worktree_status_under_test", UNDER_TEST)
    module = importlib.util.module_from_spec(specification)
    previous = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        specification.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


class DirtySplitTests(unittest.TestCase):
    def setUp(self):
        self.module = load_module()
        self.temporary = tempfile.TemporaryDirectory(prefix="omega ws dirty ")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.repository = self.root / "repo"
        self.repository.mkdir()
        self.environment = {key: value for key, value in os.environ.items()
                            if not key.startswith("GIT_")}
        self.environment.update(GIT_CONFIG_NOSYSTEM="1",
                                GIT_CONFIG_GLOBAL=os.devnull,
                                GIT_TERMINAL_PROMPT="0")
        self.git(self.repository, "init", "-b", "main")
        for name, value in (("user.name", "Worktree Test"),
                            ("user.email", "test@example.invalid"),
                            ("commit.gpgSign", "false")):
            self.git(self.repository, "config", name, value)
        (self.repository / "file.txt").write_text("initial\n", encoding="utf-8")
        self.git(self.repository, "add", "-A")
        self.git(self.repository, "commit", "-m", "initial")
        self.work = self.root / "wt-work"
        self.git(self.repository, "worktree", "add",
                 str(self.work), "-b", "work")
        # one tracked modification + one staged + two untracked
        (self.work / "file.txt").write_text("modified\n", encoding="utf-8")
        (self.work / "staged.txt").write_text("staged\n", encoding="utf-8")
        self.git(self.work, "add", "staged.txt")
        (self.work / "new-a.txt").write_text("a\n", encoding="utf-8")
        (self.work / "new-b.txt").write_text("b\n", encoding="utf-8")

    def git(self, directory, *arguments):
        result = subprocess.run(["git", "-C", str(directory), *arguments],
                                capture_output=True, text=True, encoding="utf-8",
                                env=self.environment, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout.strip()

    def record(self):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            code = self.module.main([
                "--repository", str(self.repository),
                "--base", "main", "--offline"])
        self.assertEqual(code, 0)
        return json.loads(output.getvalue())

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
