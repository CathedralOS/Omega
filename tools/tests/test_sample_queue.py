"""The sample queue lists failing sample programs, smallest first."""

from contextlib import redirect_stdout, redirect_stderr
from datetime import datetime, timedelta, timezone
import io
import json
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    "sample_queue", Path(__file__).resolve().parents[1] / "sample_queue.py")
queue_module = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(queue_module)


class SampleQueueTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        patcher = patch.object(queue_module, "ROOT", self.root)
        patcher.start()
        self.addCleanup(patcher.stop)

    def sample(self, path, lines):
        directory = self.root / path
        directory.mkdir(parents=True)
        (directory / "build.omg").write_text("machine build() {}\n" * 50)
        (directory / "main.omg").write_text("// Expected exit: 0.\n" + "// line\n" * (lines - 1))
        return path

    def record(self, *rows):
        return {"results": [{"sample": sample, "state": state, "detail": detail}
                            for sample, state, detail in rows]}

    def test_failing_programs_come_smallest_first(self):
        big = self.sample("samples/cli/big", 300)
        small = self.sample("samples/cli/small", 20)
        passing = self.sample("samples/cli/passing", 5)
        rows = queue_module.queue(self.record(
            (big, "no-native-build", "big refusal"),
            (small, "oracle-failed", "small refusal"),
            (passing, "pass", "")))
        self.assertEqual([row["sample"] for row in rows], [small, big])
        self.assertEqual(rows[0]["lines"], 20)

    def claim(self, sample, expired=False, paths=None):
        expiry = datetime.now(timezone.utc) + timedelta(hours=-1 if expired else 1)
        return {"item": sample, "owner": "peer session", "ticket": "a" * 32,
                "note": "repairing the generic field operation", "paths": paths or [],
                "expires_utc": expiry.isoformat()}

    def test_unclaimed_programs_precede_smaller_live_assignments(self):
        occupied = self.sample("samples/cli/occupied", 5)
        available = self.sample("samples/cli/available", 20)
        claim = self.claim(occupied + "/main.omg")
        rows = queue_module.queue(self.record((occupied, "no-native-build", ""),
                                             (available, "no-native-build", "")),
                                  live_claims=[claim])
        self.assertEqual([row["sample"] for row in rows], [available, occupied])
        self.assertEqual(rows[1]["claims"], [claim])
        self.assertTrue(all(row["claims_known"] for row in rows))

    def test_expired_claims_and_shared_paths_do_not_occupy_an_outcome(self):
        small = self.sample("samples/cli/small", 5)
        big = self.sample("samples/cli/big", 20)
        rows = queue_module.queue(self.record((small, "no-native-build", ""),
                                             (big, "no-native-build", "")),
                                  live_claims=[self.claim(small, expired=True),
                                               self.claim("another outcome", paths=["samples/cli"])])
        self.assertEqual([row["sample"] for row in rows], [small, big])
        self.assertTrue(all(row["claims"] == [] for row in rows))

    def test_cli_keeps_claim_owners_and_notes_visible(self):
        sample = self.sample("samples/cli/work", 5)
        record = self.root / "gate.json"
        record.write_text(json.dumps(self.record((sample, "no-native-build", "refusal"))))
        output = io.StringIO()
        with patch.object(queue_module.sys, "argv", ["sample_queue.py", "--record", str(record)]), \
             patch.object(queue_module, "live_ownership", return_value=[self.claim(sample)]), \
             redirect_stdout(output):
            self.assertEqual(queue_module.main(), 0)
        self.assertIn("active: peer session", output.getvalue())
        self.assertIn("repairing the generic field operation", output.getvalue())

    def test_json_limit_selects_available_outcome_and_preserves_unknown_state(self):
        occupied = self.sample("samples/cli/occupied", 5)
        available = self.sample("samples/cli/available", 20)
        record = self.root / "gate.json"
        record.write_text(json.dumps(self.record((occupied, "no-native-build", ""),
                                                (available, "no-native-build", ""))))
        for unavailable in (False, True):
            with self.subTest(unavailable=unavailable):
                output, errors = io.StringIO(), io.StringIO()
                snapshot = patch.object(queue_module, "live_ownership", side_effect=queue_module.outcome_claims.ClaimsError("read timed out")) if unavailable else patch.object(queue_module, "live_ownership", return_value=[self.claim(occupied)])
                with patch.object(queue_module.sys, "argv", ["sample_queue.py", "--record", str(record), "--json", "--limit", "1"]), snapshot, redirect_stdout(output), redirect_stderr(errors):
                    self.assertEqual(queue_module.main(), 0)
                rows = json.loads(output.getvalue())
                self.assertEqual(rows[0]["sample"], occupied if unavailable else available)
                self.assertEqual(rows[0]["claims_known"], not unavailable)
                if unavailable:
                    self.assertIn("ownership unknown", errors.getvalue())

    def test_nested_projects_and_build_files_do_not_count(self):
        parent = self.sample("samples/cli/lab", 10)
        self.sample("samples/cli/lab/controls/invalid", 400)
        rows = queue_module.queue(self.record((parent, "no-native-build", "")))
        self.assertEqual(rows[0]["lines"], 10)

    def test_probes_and_controls_only_with_all(self):
        probe = self.sample("samples/cli/probes/gap", 5)
        control = self.sample("samples/cli/lab/controls/invalid_case", 5)
        program = self.sample("samples/cli/app", 50)
        record = self.record((probe, "check-fail", ""),
                             (control, "rejection-failed", ""),
                             (program, "no-native-build", ""))
        self.assertEqual([row["sample"] for row in queue_module.queue(record)], [program])
        everything = queue_module.queue(record, include_all=True)
        self.assertEqual([row["kind"] for row in everything], ["program", "probe", "control"])

    def test_owner_gated_and_oracle_less_programs_follow_the_work(self):
        gated = self.sample("samples/cli/gated", 10)
        (self.root / gated / "README.md").write_text(
            "# gated\n\nWaiting on [OWNER_QUESTIONS.md Q1](x).\n")
        oracle_less = self.sample("samples/gui/app", 10)
        (self.root / oracle_less / "main.omg").write_text("// a window\n" * 10)
        work = self.sample("samples/cli/work", 90)
        rows = queue_module.queue(self.record(
            (gated, "no-native-build", "refusal"),
            (oracle_less, "no-native-build", "refusal"),
            (work, "no-native-build", "refusal")))
        self.assertEqual([(row["sample"], row["kind"]) for row in rows],
                         [(work, "program"), (gated, "waiting"),
                          (oracle_less, "no oracle")])
        self.assertIn("OWNER_QUESTIONS.md Q1", rows[1]["refusal"])

    def test_latest_record_skips_a_newer_filtered_run(self):
        runs = self.root / "runs"
        runs.mkdir()
        (runs / "20261001T000000Z-macos_arm64.json").write_text('{"filter": null}')
        (runs / "20261002T000000Z-macos_arm64.json").write_text('{"filter": "cli/"}')
        with patch.object(queue_module, "RUNS", runs):
            self.assertEqual(Path(queue_module.latest_record()).name,
                             "20261001T000000Z-macos_arm64.json")

    def test_application_record_does_not_claim_probe_coverage(self):
        runs = self.root / "runs"
        runs.mkdir()
        (runs / "20261001T000000Z-macos_arm64.json").write_text('{"filter": null}')
        (runs / "20261002T000000Z-macos_arm64.json").write_text(
            '{"filter": "", "scope": "programs"}')
        (runs / "20261003T000000Z-macos_arm64.json").write_text(
            '{"filter": "cli/app", "scope": "programs"}')
        (runs / "20261004T000000Z-macos_arm64.json").write_text(
            '{"filter": "", "scope": "unknown"}')
        with patch.object(queue_module, "RUNS", runs):
            self.assertEqual(Path(queue_module.latest_record()).name,
                             "20261002T000000Z-macos_arm64.json")
            self.assertEqual(Path(queue_module.latest_record(include_all=True)).name,
                             "20261001T000000Z-macos_arm64.json")

    def test_explicit_application_record_cannot_satisfy_all(self):
        record = self.root / "programs.json"
        record.write_text('{"filter": "", "scope": "programs", "results": []}')
        with patch.object(queue_module.sys, "argv", ["sample_queue.py", "--all", "--record", str(record)]):
            self.assertEqual(queue_module.main(), 2)



if __name__ == "__main__":
    unittest.main()
