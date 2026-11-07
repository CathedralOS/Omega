"""A completed hold leaves one history line naming who held the slot and for how long."""

from pathlib import Path
import json
import os
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import build_slot


class HoldHistory(unittest.TestCase):
    def test_completed_hold_appends_one_history_line(self):
        with tempfile.TemporaryDirectory() as directory, \
             patch.dict(os.environ, {}, clear=False):
            os.environ.pop("OMEGA_BUILD_SLOT_HELD", None)
            code = build_slot.main(["--repository", directory, "--",
                                    sys.executable, "-c", "raise SystemExit(3)"])
            self.assertEqual(code, 3)
            history = build_slot.slot_directory(directory) / "history.jsonl"
            entries = [json.loads(line) for line in history.read_text().splitlines()]
        self.assertEqual(len(entries), 1)
        self.assertEqual(entries[0]["exit"], 3)
        self.assertIn("-c", entries[0]["command"])
        self.assertGreaterEqual(entries[0]["held_seconds"], 0)

    def test_nested_hold_records_nothing(self):
        with tempfile.TemporaryDirectory() as directory, \
             patch.dict(os.environ, {"OMEGA_BUILD_SLOT_HELD": "1"}):
            build_slot.main(["--repository", directory, "--", sys.executable, "-c", "pass"])
            self.assertFalse((build_slot.slot_directory(directory) / "history.jsonl").exists())


if __name__ == "__main__":
    unittest.main()
