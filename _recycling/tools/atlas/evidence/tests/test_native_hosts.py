"""A missing or mismatched process cannot establish a four-host execution result."""

import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from check_native_hosts import HOSTS, LANES, RECORD, collect_runs


class NativeHostEvidence(unittest.TestCase):
    def setUp(self):
        targets = json.loads(RECORD.read_text())["targets"]
        self.manifest = {"expected_exit": 0, "images": {
            target: {lane: row["sha256"] for lane, row in job["images"].items()}
            for target, job in targets.items()
        }}
        self.rows = []
        for target, job in targets.items():
            for lane, image in job["images"].items():
                self.rows.append({
                    "target": target, "host": job["host"], "format": job["format"],
                    "lane": lane, "sha256": image["sha256"], "size": image["bytes"],
                    "exit": image["exit"],
                })

    def collect(self, rows):
        log = "\n".join(row["target"] + "\trun\ttime " + json.dumps(row, sort_keys=True)
                        for row in rows)
        return collect_runs(log, self.manifest)

    def test_complete_matching_matrix(self):
        self.assertEqual(set(self.collect(self.rows)), {(t, lane) for t in HOSTS for lane in LANES})

    def test_a_successful_job_requires_two_separate_processes(self):
        with self.assertRaisesRegex(ValueError, "incomplete"):
            self.collect(self.rows[1:])
        with self.assertRaisesRegex(ValueError, "duplicate"):
            self.collect(self.rows + self.rows[:1])

    def test_an_exit_requires_the_matching_os_cpu_binary_and_status(self):
        for key, value in (("host", ["Linux", "x86", "X86"]), ("sha256", "0" * 64), ("exit", 1)):
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.collect([{**self.rows[0], key: value}, *self.rows[1:]])
