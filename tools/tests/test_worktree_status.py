#!/usr/bin/env python3
"""Worktree status tests; stdlib only, no network (--offline).

A listed-but-deleted worktree directory (git marks it "prunable") must not
crash the report: present worktrees still report live git state, the missing
entry is visibly marked, and --markdown still renders.
"""

import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import shutil
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


class WorktreeStatusTests(unittest.TestCase):
    def setUp(self):
        self.module = load_module()
        self.temporary = tempfile.TemporaryDirectory(prefix="omega ws test ")
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
        self.keep = self.root / "wt-keep"
        self.gone = self.root / "wt-gone"
        self.git(self.repository, "worktree", "add",
                 str(self.keep), "-b", "keep")
        self.git(self.repository, "worktree", "add",
                 str(self.gone), "-b", "gone")
        # A dirty present worktree so the live-state fields are exercised.
        (self.keep / "file.txt").write_text("modified\n", encoding="utf-8")
        (self.keep / "new.txt").write_text("new\n", encoding="utf-8")
        # Delete the worktree directory but leave the stale registration.
        shutil.rmtree(self.gone)
        listed = self.git(self.repository, "worktree", "list", "--porcelain")
        self.assertIn("prunable", listed)

    def git(self, directory, *arguments):
        result = subprocess.run(["git", "-C", str(directory), *arguments],
                                capture_output=True, text=True, encoding="utf-8",
                                env=self.environment, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout.strip()

    def run_main(self, *arguments):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            code = self.module.main([
                "--repository", str(self.repository),
                "--base", "main", "--offline", *arguments])
        return code, output.getvalue()

    def record(self):
        code, output = self.run_main()
        self.assertEqual(code, 0, "main() must not fail on a prunable worktree")
        return json.loads(output)

    def test_missing_worktree_does_not_fail_the_report(self):
        code, _ = self.run_main()
        self.assertEqual(code, 0)

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
        code, output = self.run_main("--markdown")
        self.assertEqual(code, 0)
        self.assertIn("wt-gone", output)
        self.assertRegex(
            output, re.compile(r"missing|prunable|absent|gone|deleted",
                               re.IGNORECASE),
            "markdown must visibly mark the missing worktree")


if __name__ == "__main__":
    unittest.main()
