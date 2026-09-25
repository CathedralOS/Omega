#!/usr/bin/env python3
"""The corpus record text format round-trips every field, including
diagnostics that carry backslashes, tabs and line breaks.

    python3 tools/tests/test_corpus_records.py -v
"""

from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import corpus_records  # noqa: E402


class RoundTrip(unittest.TestCase):
    def test_every_field_survives_render_and_parse(self):
        records = [
            {"fixture": "fail/core/escapes", "tier": "fail", "status": "rejected",
             "millis": 12, "expected_satisfied": True, "facts": {},
             "diagnostics": ["path C:\\x\\y", "two\nlines\tand tab", "trailing \\"]},
            {"fixture": "fail/core/wrong", "tier": "fail", "status": "rejected",
             "millis": 0, "expected_satisfied": False, "facts": {},
             "diagnostics": ["other"]},
            {"fixture": "pass/core/ok", "tier": "pass", "status": "checked",
             "millis": 3400, "expected_satisfied": None, "facts": {}, "diagnostics": []},
            {"fixture": "run/loop", "tier": "run", "status": "built",
             "millis": 9000, "expected_satisfied": None,
             "facts": {"exit": "0", "stdout": "match"}, "diagnostics": []},
        ]
        text = corpus_records.render(records)
        self.assertEqual(corpus_records.parse(text), records)
        self.assertTrue(all(line.startswith(("#", "\t", "fail/", "pass/", "run/"))
                            for line in text.splitlines()))

    def test_malformed_lines_are_refused(self):
        for text in ("\tdiagnostic first\n", "pass/core/ok checked\n",
                     "pass/core/ok checked 3ms maybe\n",
                     "pass/core/ok built 3ms exit:0 expected\n"):
            with self.assertRaises(ValueError):
                corpus_records.parse(text)


if __name__ == "__main__":
    unittest.main()
