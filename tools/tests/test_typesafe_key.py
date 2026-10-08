"""Contract tests for the shared TypeSafe key/request plumbing."""

import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
import urllib.error
from unittest.mock import MagicMock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import typesafe_key


class FindKeyTests(unittest.TestCase):
    def test_environment_variable_wins(self):
        with patch.dict(os.environ, {"TYPESAFE_API_KEY": " env-key "}):
            self.assertEqual(typesafe_key.find_key(None), "env-key")

    def test_quoted_assignment_in_explicit_file(self):
        with tempfile.TemporaryDirectory() as directory:
            key_file = Path(directory) / "typesafe.env.txt"
            key_file.write_text('TYPESAFE_API_KEY="abc"\n', encoding="utf-8")
            with patch.dict(os.environ, {"TYPESAFE_API_KEY": ""}):
                self.assertEqual(typesafe_key.find_key(key_file), "abc")

    def test_ambiguous_file_yields_no_key(self):
        with tempfile.TemporaryDirectory() as directory:
            cwd = os.getcwd()
            os.chdir(directory)
            try:
                key_file = Path(directory) / "typesafe.env.txt"
                key_file.write_text(
                    "TYPESAFE_API_KEY=one\nTYPESAFE_API_KEY=two\n",
                    encoding="utf-8")
                with patch.dict(os.environ, {"TYPESAFE_API_KEY": ""}), \
                        patch.object(typesafe_key, "main_checkout_root",
                                     return_value=None), \
                        patch.object(Path, "home",
                                     return_value=Path(directory)):
                    self.assertEqual(typesafe_key.find_key(key_file), "")
            finally:
                os.chdir(cwd)


class PostTests(unittest.TestCase):
    def test_transport_failure_returns_none(self):
        with patch("urllib.request.build_opener",
                   side_effect=urllib.error.URLError("down")):
            self.assertIsNone(
                typesafe_key.post(typesafe_key.ENDPOINT, "key", {}))

    def test_response_echoing_key_is_refused(self):
        opener = MagicMock()
        opener.open.return_value.__enter__.return_value = io.BytesIO(
            json.dumps({"echo": "sk-live"}).encode())
        with patch("urllib.request.build_opener", return_value=opener):
            with self.assertRaises(RuntimeError):
                typesafe_key.post(typesafe_key.ENDPOINT, "sk-live", {})


if __name__ == "__main__":
    unittest.main()
