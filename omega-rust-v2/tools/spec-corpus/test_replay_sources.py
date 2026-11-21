import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from replay_sources import retain_replay_source


class ReplaySourcesTest(unittest.TestCase):
    def test_single_case_replays_keep_original_bytes_and_paths(self):
        with TemporaryDirectory() as tmp:
            worker = Path(tmp)
            first = retain_replay_source(worker, "changed", b"first\r\n")
            second = retain_replay_source(worker, "changed", b"second\n")
            self.assertNotEqual(first, second)
            self.assertEqual(first.read_bytes(), b"first\r\n")
            self.assertEqual(second.read_bytes(), b"second\n")
            self.assertEqual(retain_replay_source(worker, "changed", first.read_bytes()), first)
            control = retain_replay_source(worker, "control", first.read_bytes())
            self.assertNotEqual(control, first)
            self.assertEqual(control.read_bytes(), first.read_bytes())
