"""Evidence distinctions the dashboard must not flatten into a green count."""
from datetime import datetime, timezone
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import compiler_dashboard as dashboard


class DashboardTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.addCleanup(self.temporary.cleanup)

    def record(self, filename, *, target="windows_x86_64", started="2026-10-29T10:00:00Z", results=None):
        record = {"started": started, "target": target, "revision": "abc",
                  "dirty": False, "compiler_profile": {"profile": {"opt_level": "1"}},
                  "results": results or [{"sample": "samples/cli/facts/length/local", "state": "pass"}]}
        (self.root / filename).write_text(json.dumps(record), encoding="utf-8")

    def test_latest_is_per_place_target_and_control_and_uses_time_not_filename(self):
        self.record("z-old.json")
        self.record("a-new.json", started="2026-10-29T12:00:00Z", results=[
            {"sample": "samples/cli/facts/length/local", "state": "oracle-failed"},
            {"sample": "samples/cli/facts/length/local/invalid", "state": "rejected"}])
        self.record("linux.json", target="linux_x86_64")
        warnings = []
        rows = dashboard.fact_records(self.root, warnings)
        self.assertEqual(rows[("length", "local", "windows_x86_64", "")]["state"], "oracle-failed")
        self.assertEqual(rows[("length", "local", "windows_x86_64", "invalid")]["state"], "rejected")
        self.assertEqual(rows[("length", "local", "linux_x86_64", "")]["state"], "pass")
        self.assertFalse(warnings)

    def test_bad_record_is_visible_and_does_not_invent_measurements(self):
        (self.root / "bad.json").write_text('{"started":"nope"}', encoding="utf-8")
        warnings = []
        self.assertEqual(dashboard.fact_records(self.root, warnings), {})
        self.assertEqual(len(warnings), 1)

    def test_missing_profile_and_dirty_default_remain_qualified(self):
        self.record("record.json")
        path = self.root / "record.json"
        record = json.loads(path.read_text())
        del record["dirty"], record["compiler_profile"]
        path.write_text(json.dumps(record))
        row = next(iter(dashboard.fact_records(self.root, []).values()))
        self.assertTrue(row["dirty"])
        self.assertFalse(row["profile_verified"])

    def test_source_presence_is_not_a_run_and_control_requires_main(self):
        place = self.root / "samples/cli/facts/length/local"
        place.mkdir(parents=True)
        (place / "main.omg").write_text("machine Main {}")
        (place / "controls/empty").mkdir(parents=True)
        facts = dashboard.collect_facts(self.root, {})
        self.assertEqual(len(facts[0]["places"]), 9)
        local = facts[0]["places"][0]
        self.assertTrue(local["program"])
        self.assertEqual(local["twins"], [])
        self.assertEqual(local["observations"], {})
        self.assertFalse(facts[0]["places"][1]["program"])

    def test_native_counts_reuse_exit_oracles_and_exclude_invalid_tier(self):
        corpus = self.root / "tests/omega"
        corpus.mkdir(parents=True)
        golden = corpus / "corpus_native_windows_x86_64.txt"
        golden.write_text("pass/group/ok built 1ms exit:7 @abc\n"
                          "pass/group/wrong built 1ms exit:0 @old+wt\n"
                          "pass/group/compile checked 1ms @abc\n"
                          "fail/group/invalid rejected 1ms expected @abc\n")
        for name in ("ok", "wrong"):
            fixture = corpus / "pass/group" / name
            fixture.mkdir(parents=True)
            (fixture / "expected_native_exit.txt").write_text("7")
        with patch.object(dashboard, "git", return_value="2026-10-29T00:00:00Z"):
            hosts = dashboard.collect_native(self.root, [])
        self.assertEqual(hosts[0]["counts"]["fixtures"], 3)
        self.assertEqual(hosts[0]["counts"]["executed"], 2)
        self.assertEqual(hosts[0]["counts"]["authored"], 1)
        self.assertEqual(hosts[0]["stamps"]["old+wt"], 1)
        self.assertFalse(hosts[1]["available"])

    def test_malformed_native_is_unavailable_not_zero_success(self):
        corpus = self.root / "tests/omega"
        corpus.mkdir(parents=True)
        (corpus / "corpus_native_linux_arm64.txt").write_text("invalid golden")
        warnings = []
        host = dashboard.collect_native(self.root, warnings)[3]
        self.assertFalse(host["available"])
        self.assertIn("error", host)
        self.assertEqual(len(warnings), 1)

    def test_registry_error_is_unknown_not_an_empty_active_list(self):
        with patch.object(dashboard.subprocess, "run", side_effect=OSError("offline")):
            claims = dashboard.collect_claims(self.root)
        self.assertFalse(claims["available"])
        self.assertNotIn("active", claims)

    def test_export_cannot_break_out_of_script_and_has_no_live_endpoint(self):
        html = dashboard.render({"note": "</script><img src=x onerror=alert(1)>"})
        self.assertNotIn('</script><img', html)
        self.assertIn('\\u003c/script>', html)
        self.assertIn('const live = false;', html)
        self.assertIn('const live = true;', dashboard.render({}, live=True))

    def test_timestamps_require_an_explicit_timezone(self):
        with self.assertRaises(ValueError):
            dashboard.instant("2026-10-29T10:00:00")
        self.assertEqual(dashboard.instant("2026-10-29T10:00:00Z").tzinfo, timezone.utc)

    def test_worktrees_default_to_the_shared_sample_gate_record_directory(self):
        with patch.object(dashboard, "git", return_value=str(self.root / ".git")):
            self.assertEqual(dashboard.shared_evidence_root(self.root / "worktree"), self.root)

    def test_history_uses_real_commit_dates_and_executed_valid_rows(self):
        responses = ["2026-10-29T10:00:00Z", "new 2026-10-29T10:00:00Z\nold 2026-10-21T09:00:00Z",
                     "pass/g/a built 1ms exit:0 @old\nfail/g/b rejected 1ms expected @old\n",
                     "pass/g/a built 1ms exit:0 @new\nrun/g/c checked 1ms @new\n"]
        with patch.object(dashboard, "git", side_effect=responses):
            history = dashboard.native_history(self.root, "tests/omega/golden.txt")
        self.assertEqual([point["revision"] for point in history], ["old", "new"])
        self.assertEqual(history[0]["date"], "2026-10-21T09:00:00Z")
        self.assertEqual([point["executed"] for point in history], [1, 1])
        self.assertEqual([point["fixtures"] for point in history], [1, 2])

    def test_history_keeps_last_golden_each_utc_day_without_inventing_gap_days(self):
        responses = ["2026-10-29T18:00:00Z", "new 2026-10-29T18:00:00Z\nearly 2026-10-29T10:00:00Z\nold 2026-10-21T09:00:00Z",
                     "pass/g/a built 1ms exit:0 @old\n", "pass/g/a built 1ms exit:0 @new\n"]
        with patch.object(dashboard, "git", side_effect=responses):
            history = dashboard.native_history(self.root, "golden.txt")
        self.assertEqual([point["revision"] for point in history], ["old", "new"])

    def test_unreadable_historical_golden_does_not_hide_later_real_observations(self):
        responses = ["2026-10-29T18:00:00Z", "new 2026-10-29T18:00:00Z\nold 2026-10-21T09:00:00Z",
                     "\torphan diagnostic\n", "pass/g/a built 1ms exit:0 @new\n"]
        warnings = []
        with patch.object(dashboard, "git", side_effect=responses):
            history = dashboard.native_history(self.root, "golden.txt", warnings)
        self.assertEqual([point["revision"] for point in history], ["new"])
        self.assertIn("old", warnings[0])


if __name__ == "__main__":
    unittest.main()
