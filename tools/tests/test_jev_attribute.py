"""jev-attribute exit contract: silent without a key, ranked output with one."""

import contextlib
import importlib.util
import io
from pathlib import Path
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


if __name__ == "__main__":
    unittest.main()
