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
        dates = patch.object(dashboard, "execution_revision_dates", return_value={})
        dates.start()
        self.addCleanup(dates.stop)

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
        with patch.object(dashboard, "git", return_value="2026-10-29T00:00:00Z"):
            host = dashboard.collect_native(self.root, warnings)[3]
        self.assertFalse(host["available"])
        self.assertIn("error", host)
        self.assertEqual(sum("Cannot read native golden linux_arm64" in warning for warning in warnings), 1)

    def test_recording_hosts_match_the_corpus_gate(self):
        import corpus_gate
        self.assertEqual(dashboard.RECORDING_HOSTS, corpus_gate.HOST_TARGETS)

    def test_qemu_recorded_linux_arm64_rows_stay_and_are_marked_emulated(self):
        corpus = self.root / "tests/omega"
        corpus.mkdir(parents=True)
        (corpus / "corpus_both_linux_arm64.txt").write_text(
            '# context: {"schema": "omega-corpus-context-v1", "platform": "linux", "machine": "x86_64"}\n'
            "pass/g/a built 1ms exit:0\n")
        warnings = []
        with patch.object(dashboard, "git", return_value="2026-10-30T00:00:00Z"):
            hosts = dashboard.collect_native(self.root, warnings)
        self.assertEqual([host["target"] for host in hosts], list(dashboard.TARGETS))
        arm = hosts[3]
        self.assertEqual(arm["target"], "linux_arm64")
        self.assertTrue(arm["available"])
        self.assertTrue(arm["emulated"])
        self.assertEqual(arm["golden_paths"], ["tests/omega/corpus_both_linux_arm64.txt"])
        self.assertEqual(arm["counts"]["executed"], 1)
        self.assertFalse(any("Skipped" in warning for warning in warnings))
        self.assertFalse(hosts[1]["available"])
        self.assertFalse(hosts[1]["emulated"])
        (corpus / "corpus_native_linux_arm64.txt").write_text(
            '# context: {"schema": "omega-corpus-context-v1", "platform": "linux", "machine": "aarch64"}\n'
            "pass/g/native built 1ms exit:0\n")
        warnings.clear()
        with patch.object(dashboard, "git", return_value="2026-10-30T00:00:00Z"):
            arm = dashboard.collect_native(self.root, warnings)[3]
        self.assertTrue(arm["available"])
        self.assertFalse(arm["emulated"])
        self.assertEqual(arm["golden_paths"], ["tests/omega/corpus_native_linux_arm64.txt"])
        self.assertEqual(arm["counts"]["executed"], 1)
        self.assertEqual(arm["source_counts"], {"tests/omega/corpus_native_linux_arm64.txt": 1})
        self.assertFalse(any("Skipped" in warning for warning in warnings))

    def test_matching_aarch64_context_remains_linux_arm64_host_evidence(self):
        corpus = self.root / "tests/omega"
        corpus.mkdir(parents=True)
        (corpus / "corpus_both_linux_arm64.txt").write_text(
            '# context: {"schema": "omega-corpus-context-v1", "platform": "linux", "machine": "aarch64"}\n'
            "pass/g/a built 1ms exit:0\n")
        with patch.object(dashboard, "git", return_value="2026-10-30T00:00:00Z"):
            host = dashboard.collect_native(self.root, [])[3]
        self.assertTrue(host["available"])
        self.assertFalse(host["emulated"])
        self.assertEqual(host["counts"]["executed"], 1)
        self.assertEqual(host["golden_paths"], ["tests/omega/corpus_both_linux_arm64.txt"])

    def test_emulated_history_starts_when_the_recorder_matches_the_target(self):
        path = "tests/omega/corpus_both_linux_arm64.txt"
        qemu = ('# context: {"platform": "linux", "machine": "x86_64"}\n'
                "pass/g/a built 1ms exit:0\n")
        native = ('# context: {"platform": "linux", "machine": "aarch64"}\n'
                  "pass/g/a built 1ms exit:0\npass/g/b built 1ms exit:0\n")
        responses = ["2026-10-30T12:00:00Z",
                     "native 2026-10-30T12:00:00Z\nqemu 2026-10-29T12:00:00Z",
                     qemu, native]
        with patch.object(dashboard, "git", side_effect=responses):
            history = dashboard.native_history(self.root, path)
        self.assertEqual([(point["revision"], point["executed"], point["fixtures"], point["emulated"])
                          for point in history],
                         [("qemu", 1, 1, True), ("native", 2, 2, False)])

    def test_execution_goldens_reconcile_per_fixture_and_keep_unmeasured_native_rows(self):
        corpus = self.root / "tests/omega"
        corpus.mkdir(parents=True)
        native = corpus / "corpus_native_macos_arm64.txt"
        both = corpus / "corpus_both_macos_arm64.txt"
        native.write_text("pass/g/a built 1ms exit:0\npass/g/b built 1ms exit:0\n")
        both.write_text("pass/g/a rejected 1ms\npass/g/c built 1ms exit:0\n")
        with patch.object(dashboard, "git", return_value="2026-10-30T00:00:00Z"), patch.object(dashboard, "collect_cohort", return_value={"available": True}):
            host = dashboard.collect_native(self.root, [])[2]
        self.assertEqual((host["counts"]["executed"], host["counts"]["fixtures"]), (2, 3))
        self.assertEqual(host["source_counts"], {
            "tests/omega/corpus_native_macos_arm64.txt": 1,
            "tests/omega/corpus_both_macos_arm64.txt": 2})
        both.write_text("invalid golden")
        with patch.object(dashboard, "git", return_value="2026-10-30T00:00:00Z"):
            host = dashboard.collect_native(self.root, [])[2]
        self.assertFalse(host["available"])

    def test_row_revision_beats_file_update_and_never_selects_the_highest_success(self):
        import corpus_records
        native = corpus_records.parse("pass/g/a rejected 1ms @aaaaaaa\npass/g/b built 1ms exit:0 @ccccccc\n")
        both = corpus_records.parse("pass/g/a built 1ms exit:0 @bbbbbbb\npass/g/b rejected 1ms @ccccccc\npass/g/c built 1ms exit:0\n")
        dates = {"a" * 40: dashboard.instant("2026-10-30T00:00:00Z"),
                 "b" * 40: dashboard.instant("2026-10-29T00:00:00Z"),
                 "c" * 40: dashboard.instant("2026-10-28T00:00:00Z")}
        rows = dashboard.merge_execution_records(self.root, [
            ("corpus_native_host.txt", native), ("corpus_both_host.txt", both)], dates)
        self.assertEqual({r["fixture"]:r["status"] for r in rows},
                         {"pass/g/a":"rejected","pass/g/b":"rejected","pass/g/c":"built"})
        self.assertEqual(rows[0]["golden_path"], "corpus_native_host.txt")
        self.assertEqual(rows[1]["golden_path"], "corpus_both_host.txt")
        self.assertIsNone(rows[2]["recorded_at"])

    def test_history_reconciles_files_at_each_historical_revision(self):
        native, both = "corpus_native_host.txt", "corpus_both_host.txt"
        responses = ["2026-10-30T12:00:00Z", "new 2026-10-30T12:00:00Z\nold 2026-10-29T12:00:00Z",
                     "pass/g/a built 1ms exit:0\npass/g/b built 1ms exit:0\n",
                     ValueError("path does not exist in old"),
                     "pass/g/a built 1ms exit:0\npass/g/b built 1ms exit:0\n",
                     "pass/g/a rejected 1ms\npass/g/c built 1ms exit:0\n"]
        with patch.object(dashboard, "git", side_effect=responses):
            history = dashboard.native_history(self.root, [native, both])
        self.assertEqual([(p["revision"], p["executed"], p["fixtures"]) for p in history],
                         [("old", 2, 2), ("new", 2, 3)])
        self.assertEqual(history[0]["paths"], [native])
        self.assertEqual(history[1]["paths"], [native,both])

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

    def test_fixed_cohort_does_not_count_new_runs_as_recovered_programs(self):
        import corpus_records
        old = corpus_records.parse("pass/g/still built 1ms exit:0\npass/g/fixed checked 1ms\npass/g/lost built 1ms exit:0\n")
        now = corpus_records.parse("pass/g/still built 1ms exit:0\npass/g/fixed built 1ms exit:0\npass/g/lost checked 1ms\npass/g/new built 1ms exit:0\n")
        result = dashboard.cohort_comparison(old, now)
        self.assertEqual((result["baseline_total"], result["baseline_runs"], result["same_now_runs"]), (3, 2, 2))
        self.assertEqual((result["recovered"], result["lost"], result["new_running"]), (1, 1, 1))
        self.assertEqual((result["current_total"], result["current_runs"]), (4, 3))

    def test_missing_fixture_stays_in_fixed_denominator_but_is_not_a_measured_regression(self):
        import corpus_records
        old = corpus_records.parse("pass/g/removed built 1ms exit:0\npass/g/still checked 1ms\n")
        now = corpus_records.parse("pass/g/new built 1ms exit:0\n")
        result = dashboard.cohort_comparison(old, now)
        self.assertEqual((result["baseline_total"], result["same_now_runs"]), (2, 0))
        self.assertEqual((result["missing"], result["missing_previously_running"], result["lost"]), (2, 1, 0))

    def test_cohort_rejects_empty_or_duplicate_baselines_and_ignores_fail_tier(self):
        import corpus_records
        with self.assertRaises(ValueError):
            dashboard.cohort_comparison([], [])
        rows = corpus_records.parse("pass/g/a checked 1ms\npass/g/a built 1ms exit:0\n")
        with self.assertRaises(ValueError):
            dashboard.cohort_comparison(rows, [])
        old = corpus_records.parse("pass/g/a checked 1ms\nfail/g/b rejected 1ms expected\n")
        now = corpus_records.parse("pass/g/a unimplemented 1ms exit:0\n")
        self.assertEqual(dashboard.cohort_comparison(old, now)["same_now_runs"], 0)

    def test_unavailable_configured_cohort_is_explained_without_hiding_native_counts(self):
        corpus=self.root / 'tests/omega'
        corpus.mkdir(parents=True)
        (corpus / 'corpus_native_windows_x86_64.txt').write_text('pass/g/a built 1ms exit:0\n')
        warnings=[]
        with patch.object(dashboard, 'git', return_value='2026-10-29T00:00:00Z'), patch.object(dashboard, 'collect_cohort', return_value={'available':False,'error':'baseline object missing'}):
            hosts=dashboard.collect_native(self.root,warnings)
        self.assertTrue(hosts[0]['available'])
        self.assertEqual(hosts[0]['counts']['executed'],1)
        self.assertTrue(any('windows_x86_64' in warning and 'baseline object missing' in warning for warning in warnings))


if __name__ == "__main__":
    unittest.main()
