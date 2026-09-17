#!/usr/bin/env python3
"""Degraded-worktree tolerance tests for worktree_status; stdlib, no network.

A worktree whose per-worktree git probes fail — deleted directory, or an
unborn branch (orphan checkout: rev-list/log fail with 128) — must degrade to
a marked row, not abort the report.
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


class DegradedWorktreeTests(unittest.TestCase):
    def setUp(self):
        self.module = load_module()
        self.temporary = tempfile.TemporaryDirectory(prefix="omega ws degraded ")
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
        self.orphan = self.root / "wt-orphan"
        self.git(self.repository, "worktree", "add",
                 str(self.keep), "-b", "keep")
        self.git(self.repository, "worktree", "add",
                 str(self.gone), "-b", "gone")
        self.git(self.repository, "worktree", "add",
                 str(self.orphan), "-b", "orphan-base")
        # Orphan checkout: HEAD is born no longer; rev-list/log fail.
        self.git(self.orphan, "checkout", "--orphan", "unborn")
        (self.keep / "file.txt").write_text("modified\n", encoding="utf-8")
        shutil.rmtree(self.gone)

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
        self.assertEqual(code, 0, "main() must tolerate degraded worktrees")
        return json.loads(output.getvalue())

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
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            code = self.module.main([
                "--repository", str(self.repository),
                "--base", "main", "--offline", "--markdown"])
        self.assertEqual(code, 0)
        text = output.getvalue()
        for name in ("wt-keep", "wt-gone", "wt-orphan"):
            self.assertIn(name, text)
        self.assertRegex(text, re.compile(
            r"missing|prunable|absent|gone|deleted|degraded|unborn|no commits",
            re.IGNORECASE))


if __name__ == "__main__":
    unittest.main()
