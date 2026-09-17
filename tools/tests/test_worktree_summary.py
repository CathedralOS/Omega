#!/usr/bin/env python3
"""Markdown summary-line feature tests for worktree_status; stdlib, no network.

--markdown output must carry a summary line stating the total worktree count
and how many are missing/degraded, so a wave coordinator sees the shape
without scanning rows.
"""

import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import re
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


class MarkdownSummaryTests(unittest.TestCase):
    def setUp(self):
        self.module = load_module()
        self.temporary = tempfile.TemporaryDirectory(prefix="omega ws summary ")
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
        for name, branch in (("wt-a", "a"), ("wt-b", "b")):
            self.git(self.repository, "worktree", "add",
                     str(self.root / name), "-b", branch)
        (self.root / "wt-b" / "file.txt").write_text("modified\n",
                                                    encoding="utf-8")

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

    def test_markdown_has_count_summary(self):
        code, output = self.run_main("--markdown")
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
        code, output = self.run_main()
        self.assertEqual(code, 0)
        record = json.loads(output)
        self.assertEqual(len(record["worktrees"]), 3)
        self.assertIn("generated_utc", record)


if __name__ == "__main__":
    unittest.main()
