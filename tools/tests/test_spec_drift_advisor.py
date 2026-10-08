"""Tests for the spec-drift advisor's Jev verdict plumbing (post mocked)."""

import contextlib
import io
import os
from pathlib import Path
import sys
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import spec_drift_advisor
import typesafe_key


class JevVerdictTests(unittest.TestCase):
    def call_verdict(self):
        return spec_drift_advisor.jev_verdict(
            "doc.md", "the claim", "the evidence", "test-key")

    def test_returns_the_choice_from_the_response(self):
        with mock.patch.object(spec_drift_advisor.typesafe_key, "post",
                               return_value={"answers": {"verdict": {
                                   "choice": "drifted"}}}) as post:
            self.assertEqual(self.call_verdict(), "drifted")
        post.assert_called_once()
        arguments, keywords = post.call_args
        self.assertEqual(arguments[:2], (typesafe_key.ENDPOINT, "test-key"))
        self.assertEqual(keywords.get("timeout"), 20)

    def test_none_response_yields_unverifiable(self):
        with mock.patch.object(spec_drift_advisor.typesafe_key, "post",
                               return_value=None) as post:
            self.assertEqual(self.call_verdict(), "unverifiable")
        self.assertEqual(post.call_args.kwargs.get("timeout"), 20)

    def test_offline_suppresses_the_semantic_pass(self):
        # Exercise main() directly: offline must return before the key lookup.
        with mock.patch.dict(os.environ, {"OMEGA_JEV_OFFLINE": "1"}), \
                mock.patch.object(spec_drift_advisor.typesafe_key,
                                  "post") as post, \
                mock.patch.object(spec_drift_advisor.typesafe_key,
                                  "find_key",
                                  side_effect=AssertionError("key lookup")), \
                mock.patch.object(spec_drift_advisor, "path_findings",
                                  return_value=([], [])), \
                mock.patch.object(spec_drift_advisor, "repository_root",
                                  return_value=Path(".")), \
                mock.patch.object(sys, "argv",
                                  ["spec_drift_advisor", str(Path(__file__))]), \
                contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(spec_drift_advisor.main(), 0)
            post.assert_not_called()


if __name__ == "__main__":
    unittest.main()
