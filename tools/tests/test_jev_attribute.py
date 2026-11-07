"""jev-attribute exit contract: silent without a key, ranked output with one."""

import contextlib
import importlib.util
import io
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    "jev_attribute", Path(__file__).resolve().parents[1] / "jev_attribute.py")
attribute = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(attribute)


class MainTests(unittest.TestCase):
    def run_main(self, *argv):
        arguments = ["jev_attribute.py", *argv]
        stdout, stderr = io.StringIO(), io.StringIO()
        with patch.object(sys, "argv", arguments), \
                contextlib.redirect_stdout(stdout), \
                contextlib.redirect_stderr(stderr):
            code = attribute.main()
        return code, stdout.getvalue(), stderr.getvalue()

    def test_no_key_exits_two(self):
        with patch.object(attribute.typesafe_key, "find_key",
                          return_value=""):
            code, out, err = self.run_main(
                "--signature", "x", "--window", "HEAD~1..HEAD")
        self.assertEqual(code, 2)
        self.assertEqual(out, "")
        self.assertIn("TYPESAFE_API_KEY", err)

    def test_ranking_prints_window_summary(self):
        response = {"answers": {"evidence_0": {"choice": "absent"},
                                "no_suspect": {"noul": 0.9}}}
        with patch.object(attribute.typesafe_key, "find_key",
                          return_value="k"), \
                patch.object(attribute.typesafe_key, "post",
                             return_value=response):
            code, out, _ = self.run_main(
                "--signature", "x", "--window", "HEAD~1..HEAD")
        self.assertEqual(code, 0)
        self.assertIn("window: 1 candidates", out)

    def test_window_batches_under_the_payload_budget(self):
        responses = [{"answers": {"evidence_0": {"choice": "absent"},
                                  "no_suspect": {"noul": value}}}
                     for value in (0.9, 0.3, 0.8)]
        with patch.object(attribute, "BATCH_BUDGET_BYTES", 1), \
                patch.object(attribute.typesafe_key, "find_key",
                             return_value="k"), \
                patch.object(attribute.typesafe_key, "post",
                             side_effect=responses) as post:
            code, out, _ = self.run_main(
                "--signature", "x", "--window", "HEAD~3..HEAD")
        self.assertEqual(code, 0)
        self.assertIn("window: 3 candidates in 3 batch(es) | "
                      "cause-outside-window 0.30", out)
        self.assertEqual(post.call_count, 3)

    def test_paths_restricts_the_diff_excerpt(self):
        response = {"answers": {"evidence_0": {"choice": "absent"},
                                "no_suspect": {"noul": 0.9}}}
        with patch.object(attribute.typesafe_key, "find_key",
                          return_value="k"), \
                patch.object(attribute.typesafe_key, "post",
                             return_value=response) as post:
            code, _, _ = self.run_main(
                "--signature", "x", "--window", "HEAD~1..HEAD",
                "--with-diff", "5", "--paths", "tools/nonexistent-path-zzz")
            self.assertEqual(code, 0)
            request = post.call_args[0][2]
            self.assertEqual(
                request["state"]["candidate_commits"][0]["diff_added_lines"],
                "(no changes in the excerpt scope)")

        touched = subprocess.run(
            ["git", "diff", "--name-only", "HEAD~1", "HEAD"],
            capture_output=True, text=True, check=True).stdout.split()
        touches_tools = any(path.startswith("tools/") for path in touched)
        with patch.object(attribute.typesafe_key, "find_key",
                          return_value="k"), \
                patch.object(attribute.typesafe_key, "post",
                             return_value=response) as post:
            code, _, _ = self.run_main(
                "--signature", "x", "--window", "HEAD~1..HEAD",
                "--with-diff", "5", "--paths", "tools")
            self.assertEqual(code, 0)
            request = post.call_args[0][2]
            excerpt = (request["state"]["candidate_commits"][0]
                       ["diff_added_lines"])
            if touches_tools:
                self.assertTrue(excerpt)
            else:
                self.assertEqual(excerpt,
                                 "(no changes in the excerpt scope)")

    def test_empty_scope_skips_the_solo_reask(self):
        response = {"answers": {"evidence_0": {"choice": "contains"},
                                "no_suspect": {"noul": 0.9}}}
        with patch.object(attribute.typesafe_key, "find_key",
                          return_value="k"), \
                patch.object(attribute.typesafe_key, "post",
                             return_value=response) as post:
            code, _, _ = self.run_main(
                "--signature", "x", "--window", "HEAD~1..HEAD",
                "--with-diff", "5", "--paths", "tools/nonexistent-path-zzz")
            self.assertEqual(code, 0)
            request = post.call_args[0][2]
            candidate = request["state"]["candidate_commits"][0]
            self.assertFalse(candidate["touches_excerpt_scope"])
            self.assertEqual(candidate["diff_added_lines"],
                             "(no changes in the excerpt scope)")
            self.assertEqual(post.call_count, 1)


if __name__ == "__main__":
    unittest.main()
