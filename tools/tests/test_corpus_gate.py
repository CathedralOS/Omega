"""Native fixture oracles cannot be repinned into a passing golden, and the
gate's exit code is a machine-readable verdict."""

from pathlib import Path
import contextlib
import io
import json
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import corpus_gate
import corpus_records


def record(fixture, status="checked", millis=1, expected=None, facts=None,
           diagnostics=None, recorded_at=None):
    return {"fixture": fixture, "tier": fixture.split("/", 1)[0],
            "status": status, "millis": millis,
            "expected_satisfied": expected, "facts": facts or {},
            "diagnostics": diagnostics or [], "recorded_at": recorded_at}


class NativeExitExpectations(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.fixture = self.root / "pass" / "arithmetic" / "conversion"
        self.fixture.mkdir(parents=True)
        self.record = {"fixture": "pass/arithmetic/conversion", "tier": "pass",
                       "status": "built", "facts": {"exit": "70"}}

    def failures(self, expectation, facts, target="macos_arm64", status="built"):
        (self.fixture / "expected_native_exit.txt").write_text(expectation)
        self.record.update(facts=facts, status=status)
        return corpus_gate.native_exit_failures([self.record], self.root, target)

    def test_explicit_exit_is_checked_without_a_golden(self):
        self.assertEqual(self.failures("70\n", {"exit": "70"}), [])
        self.assertTrue(self.failures("70", {"exit": "7"}))

    def test_trap_requires_the_target_specific_termination(self):
        for target, facts in [
            ("macos_arm64", {"exit": "signal", "signal": "5"}),
            ("linux_arm64", {"exit": "signal", "signal": "5"}),
            ("linux_x86_64", {"exit": "signal", "signal": "4"}),
            ("windows_x86_64", {"exit": str(-1073741795)}),
        ]:
            with self.subTest(target=target):
                self.assertEqual(self.failures("trap", facts, target), [])
                for wrong in ({"exit": "70"}, {"exit": "signal", "signal": "11"},
                              {"exit": str(-1073741819)},  # Windows access violation.
                              {"exit": "timeout"}, {"exit": "unrunnable"}, {}):
                    self.assertTrue(self.failures("trap", wrong, target))

    def test_missing_execution_or_invalid_expectation_fails(self):
        self.assertTrue(self.failures("70", {}, status="checked"))
        self.assertTrue(self.failures("trap", {}, status="rejected"))
        self.assertTrue(self.failures("trap", {"exit": "signal"}))
        for expectation in ("", "signal", "70\n7", "2147483648"):
            self.assertTrue(self.failures(expectation, {"exit": "70"}))

    def test_target_specific_oracle_overrides_the_portable_one(self):
        (self.fixture / "expected_native_exit.txt").write_text("70\n")
        (self.fixture / "expected_native_exit.linux_x86_64.txt").write_text("212\n")
        self.record.update(facts={"exit": "212"})
        self.assertEqual(corpus_gate.native_exit_failures(
            [self.record], self.root, "linux_x86_64"), [])
        self.assertTrue(corpus_gate.native_exit_failures(
            [self.record], self.root, "macos_arm64"))

    def test_target_specific_oracle_alone_promises_only_its_target(self):
        (self.fixture / "expected_native_exit.macos_arm64.txt").write_text("212\n")
        self.record.update(facts={"exit": "-44"})
        self.assertTrue(corpus_gate.native_exit_failures(
            [self.record], self.root, "macos_arm64"))
        self.assertEqual(corpus_gate.native_exit_failures(
            [self.record], self.root, "windows_x86_64"), [])

    def test_unannotated_fixture_preserves_observation_only_behavior(self):
        self.assertEqual(corpus_gate.native_exit_failures(
            [self.record], self.root, "macos_arm64"), [])

    def test_record_refuses_a_failed_oracle_without_overwriting_golden(self):
        fixture = self.root / "tests" / "omega" / "pass" / "arithmetic" / "conversion"
        fixture.mkdir(parents=True)
        (fixture / "expected_native_exit.txt").write_text("trap\n")
        golden = self.root / "golden.txt"
        golden.write_text("retained golden\n")
        self.record.update(millis=1, expected_satisfied=None, diagnostics=[])
        with patch.object(corpus_gate, "ROOT", self.root), \
             patch.object(corpus_gate, "host_target", return_value="macos_arm64"), \
             patch.object(corpus_gate, "run_runner", return_value=[self.record]), \
             patch.dict(corpus_gate.os.environ, {}, clear=True), \
             patch.object(sys, "argv", ["corpus_gate", "--native", "--record",
                                       "--runner", "unused", "--golden", str(golden)]):
            self.assertEqual(corpus_gate.main(), 3)
        self.assertEqual(golden.read_text(), "retained golden\n")


class Classify(unittest.TestCase):
    def verdict(self, before, after):
        return corpus_gate.classify(before, after)

    def test_new_and_removed(self):
        after = record("pass/a/x")
        self.assertEqual(self.verdict(None, after), ("new", ["fixture added"]))
        self.assertEqual(self.verdict(after, None),
                         ("removed", ["fixture removed"]))

    def test_pass_tier_status_rank(self):
        built = record("pass/a/x", status="built")
        checked = record("pass/a/x", status="checked")
        rejected = record("pass/a/x", status="rejected")
        verdict, reasons = self.verdict(built, rejected)
        self.assertEqual(verdict, "regressed")
        self.assertEqual(reasons, ["status built -> rejected"])
        self.assertEqual(self.verdict(checked, built)[0], "improved")
        self.assertEqual(self.verdict(built, checked)[0], "regressed")
        self.assertEqual(self.verdict(checked, checked)[0], "clean")

    def test_fail_tier_stopping_to_reject_is_a_regression(self):
        rejected = record("fail/a/x", status="rejected")
        checked = record("fail/a/x", status="checked")
        verdict, reasons = self.verdict(rejected, checked)
        self.assertEqual(verdict, "regressed")
        self.assertEqual(reasons, ["fail fixture stopped rejecting"])
        self.assertEqual(self.verdict(checked, rejected)[0], "improved")

    def test_expected_fragment_flip(self):
        satisfied = record("fail/a/x", status="rejected", expected=True)
        wrong = record("fail/a/x", status="rejected", expected=False)
        self.assertEqual(self.verdict(satisfied, wrong)[0], "regressed")
        self.assertEqual(self.verdict(wrong, satisfied)[0], "improved")

    def test_facts_and_exit_moves(self):
        match = record("run/a/x", status="built",
                       facts={"exit": "0", "stdout": "match"})
        differs = record("run/a/x", status="built",
                         facts={"exit": "0", "stdout": "differs"})
        absent = record("run/a/x", status="built", facts={"exit": "0"})
        self.assertEqual(self.verdict(match, differs)[0], "regressed")
        self.assertEqual(self.verdict(match, absent)[0], "regressed")
        self.assertEqual(self.verdict(differs, match)[0], "improved")
        other = record("run/a/x", status="built",
                       facts={"exit": "3", "stdout": "match"})
        verdict, reasons = self.verdict(match, other)
        self.assertEqual(verdict, "moved")
        self.assertEqual(reasons, ["exit 0 -> 3"])

    def test_diagnostics_compare_normalized(self):
        before = record("pass/a/x", diagnostics=[
            "native-artifact Terminal production failed: no root"])
        raw_change = record("pass/a/x", diagnostics=[
            "terminal-artifact production failed: no root"])
        real_change = record("pass/a/x", diagnostics=[
            "terminal-artifact production failed: different cause"])
        self.assertEqual(self.verdict(before, raw_change)[0], "clean")
        self.assertEqual(self.verdict(before, real_change)[0], "moved")

    def test_regressed_dominates_improved_and_moved(self):
        before = record("pass/a/x", status="checked",
                        facts={"exit": "0"}, diagnostics=["d1"])
        after = record("pass/a/x", status="rejected",
                       facts={"exit": "2"}, diagnostics=["d2"])
        verdict, reasons = self.verdict(before, after)
        self.assertEqual(verdict, "regressed")
        self.assertIn("status checked -> rejected", reasons)
        self.assertIn("exit 0 -> 2", reasons)
        self.assertIn("diagnostics moved", reasons)

    def test_perf_only_when_otherwise_clean(self):
        before = record("pass/a/x", millis=3000)
        slower = record("pass/a/x", millis=9000)
        self.assertEqual(self.verdict(before, slower)[0], "perf_slower")
        fast_before = record("pass/a/x", millis=9000)
        faster = record("pass/a/x", millis=4000)
        self.assertEqual(self.verdict(fast_before, faster)[0], "perf_faster")
        moved_slower = record("pass/a/x", millis=9000, diagnostics=["d"])
        self.assertEqual(self.verdict(before, moved_slower)[0], "moved")
        small = record("pass/a/x", millis=100)
        self.assertEqual(self.verdict(small, record("pass/a/x", millis=500))[0],
                         "clean")


class PerfContext(unittest.TestCase):
    """Perf moves compare millis across two recording conditions; the
    warnings name when those conditions themselves explain the delta."""

    def test_contended_judges_only_the_pre_run_load(self):
        # The gate's own workers saturate the end sample; a quiet start with
        # a pegged end is a normal full run, not contention.
        self.assertFalse(corpus_gate.contended(
            {"cpus": 12, "load1_start": 1.0, "load1_end": 14.0}))
        self.assertFalse(corpus_gate.contended(
            {"cpus": 12, "load1_start": 5.19, "load1_end": 8.43}))
        self.assertTrue(corpus_gate.contended(
            {"cpus": 12, "load1_start": 6.0, "load1_end": 1.0}))
        self.assertTrue(corpus_gate.contended(
            {"cpus": 8, "load1_start": 12.0, "load1_end": 12.0}))
        self.assertFalse(corpus_gate.contended(None))
        # Hosts without getloadavg (Windows) record no load samples and so
        # never claim contention.
        self.assertFalse(corpus_gate.contended(
            {"cpus": 8, "load1_start": None, "load1_end": None}))

    def test_warnings_name_the_direction_of_noise(self):
        quiet = {"jobs": None, "cpus": 8, "load1_start": 1.0,
                 "load1_end": 2.0}
        busy = {"jobs": None, "cpus": 8, "load1_start": 9.0,
                "load1_end": 10.0}
        golden_busy = corpus_gate.perf_context_warnings(busy, quiet)
        self.assertEqual(len(golden_busy), 1)
        self.assertIn("golden", golden_busy[0])
        current_busy = corpus_gate.perf_context_warnings(quiet, busy)
        self.assertIn("this run", current_busy[0])
        self.assertEqual(corpus_gate.perf_context_warnings(quiet, quiet), [])

    def test_worker_count_mismatch_and_missing_context_warn(self):
        quiet = {"jobs": "4", "cpus": 8, "load1_start": 1.0,
                 "load1_end": 2.0}
        warnings = corpus_gate.perf_context_warnings(
            quiet, dict(quiet, jobs="8"))
        self.assertTrue(any("worker counts differ" in w for w in warnings))
        # Unstamped golden: no basis for trusting perf moves at all.
        self.assertTrue(
            corpus_gate.perf_context_warnings(None, quiet))
        # One side with a default worker count cannot be compared.
        self.assertEqual(corpus_gate.perf_context_warnings(
            quiet, dict(quiet, jobs=None)), [])


class MainVerdicts(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.golden = self.root / "golden.txt"
        self.json_path = self.root / "verdict.json"

    def write_golden(self, records, context=None):
        self.golden.write_text(corpus_records.render(records, context),
                               encoding="utf-8")

    def run_gate(self, records, *extra, golden_records=None,
                 golden_context=None):
        if golden_records is None:
            golden_records = records
        self.write_golden(golden_records, context=golden_context)
        argv = ["corpus_gate", "--runner", "unused",
                "--golden", str(self.golden), *extra]
        patches = [patch.object(corpus_gate, "ROOT", self.root),
                   patch.object(corpus_gate, "run_runner",
                                return_value=records),
                   patch.object(corpus_gate, "measurement_stamp",
                                return_value="aaaaaaaaaaaa"),
                   patch.dict(corpus_gate.os.environ, {}, clear=True),
                   patch.object(sys, "argv", argv)]
        with contextlib.ExitStack() as stack:
            for patcher in patches:
                stack.enter_context(patcher)
            return corpus_gate.main()

    def test_clean_moves_and_regressed_exit_codes(self):
        rows = [record("pass/a/x")]
        self.assertEqual(self.run_gate(rows), 0)
        moved = [record("pass/a/x", diagnostics=["new text"])]
        self.assertEqual(self.run_gate(moved, golden_records=rows), 1)
        regressed = [record("pass/a/x", status="rejected")]
        self.assertEqual(self.run_gate(regressed, golden_records=
                                       [record("pass/a/x", status="built")]), 3)

    def test_infrastructure_failure_is_4_and_125_under_bisect(self):
        self.write_golden([record("pass/a/x")])
        argv = ["corpus_gate", "--runner", "unused", "--golden",
                str(self.golden), "--json", str(self.json_path)]
        with patch.object(corpus_gate, "ROOT", self.root), \
             patch.object(corpus_gate, "run_runner",
                          side_effect=corpus_gate.InfrastructureFailure(
                              "corpus_gate: runner exited 2")), \
             patch.dict(corpus_gate.os.environ, {}, clear=True), \
             patch.object(sys, "argv", argv):
            self.assertEqual(corpus_gate.main(), 4)
        document = json.loads(self.json_path.read_text())
        self.assertEqual(document["verdict"], "infrastructure_failed")
        self.assertEqual(document["exit_code"], 4)
        self.assertEqual(document["infrastructure_error"],
                         "corpus_gate: runner exited 2")
        argv.append("--bisect")
        with patch.object(corpus_gate, "ROOT", self.root), \
             patch.object(corpus_gate, "run_runner",
                          side_effect=corpus_gate.InfrastructureFailure("x")), \
             patch.dict(corpus_gate.os.environ, {}, clear=True), \
             patch.object(sys, "argv", argv):
            self.assertEqual(corpus_gate.main(), 125)

    def test_bisect_maps_regressed_to_bad_and_moves_to_good(self):
        golden = [record("pass/a/x", status="built")]
        regressed = [record("pass/a/x", status="rejected")]
        moved = [record("pass/a/x", status="built", diagnostics=["moved"])]
        self.assertEqual(self.run_gate(regressed, "--bisect",
                                       golden_records=golden), 1)
        self.assertEqual(self.run_gate(moved, "--bisect",
                                       golden_records=golden), 0)
        self.assertEqual(self.run_gate(regressed, golden_records=golden), 3)

    def test_json_document_records_verdict_and_fixtures(self):
        golden = [record("pass/a/x", status="built"),
                  record("pass/a/y", status="checked")]
        after = [record("pass/a/x", status="rejected"),
                 record("pass/a/y", status="checked", diagnostics=["d"])]
        self.assertEqual(self.run_gate(after, "--json", str(self.json_path),
                                       golden_records=golden), 3)
        document = json.loads(self.json_path.read_text())
        self.assertEqual(document["schema"], "omega-corpus-verdict-v1")
        self.assertEqual(document["verdict"], "regressed")
        self.assertEqual(document["exit_code"], 3)
        self.assertEqual(document["leg"], "checked")
        fixtures = {entry["fixture"]: entry for entry in document["fixtures"]}
        self.assertEqual(fixtures["pass/a/x"]["verdict"], "regressed")
        self.assertEqual(fixtures["pass/a/y"]["verdict"], "moved")
        self.assertIn("golden_provenance", document)

    def test_record_stamps_rows_and_subset_preserves_prior_stamps(self):
        prior = [record("pass/a/x", recorded_at="bbbbbbbbbbbb"),
                 record("pass/a/y")]
        code = self.run_gate([record("pass/a/x")], "--record",
                             "--filter", "a/x", golden_records=prior)
        self.assertEqual(code, 0)
        merged = {row["fixture"]: row
                  for row in corpus_records.read(self.golden)}
        self.assertEqual(merged["pass/a/x"]["recorded_at"], "aaaaaaaaaaaa")
        self.assertIsNone(merged["pass/a/y"]["recorded_at"])

    def test_authored_stdout_refuses_bad_evidence_and_preserves_golden(self):
        fixture = self.root / "tests" / "omega" / "run" / "a" / "x"
        fixture.mkdir(parents=True)
        (fixture / "expected_stdout.txt").write_text("authored output\n")
        exit_oracle = fixture / "expected_native_exit.txt"
        for explicit_exit in (False, True):
            if explicit_exit:
                exit_oracle.write_text("70\n")
            for facts in ({"exit": "70", "stdout": "differs"},
                          {"exit": "70"}):
                rows = [record("run/a/x", status="built", facts=facts)]
                for leg in ("--native", "--both"):
                    for flags in ((), ("--record",), ("--record", "--bisect")):
                        with self.subTest(explicit_exit=explicit_exit,
                                          facts=facts, leg=leg, flags=flags), \
                             patch.object(corpus_gate, "host_target",
                                          return_value="linux_x86_64"):
                            expected_code = 1 if "--bisect" in flags else 3
                            self.assertEqual(self.run_gate(
                                rows, leg, *flags, "--json", str(self.json_path)),
                                expected_code)
                            self.assertEqual(self.golden.read_text(),
                                             corpus_records.render(rows))
                            document = json.loads(self.json_path.read_text())
                            self.assertEqual(document["verdict"], "regressed")
                            self.assertEqual(document["exit_code"], expected_code)
                            self.assertIn("stdout", document["oracle_failures"][0])

    def test_authored_stdout_requires_completed_native_execution(self):
        fixture = self.root / "tests" / "omega" / "run" / "a" / "x"
        fixture.mkdir(parents=True)
        (fixture / "expected_stdout.txt").write_text("")
        # A foreign-target build or declaration-only fixture has no runtime
        # facts. Like an exit oracle, authored stdout remains unproved there.
        cases = [("built", {}), ("checked", {}), ("rejected", {}),
                 ("checked", {"exit": "0", "stdout": "match"}),
                 ("built", {"stdout": "match"}),
                 ("built", {"exit": "timeout", "stdout": "match"}),
                 ("built", {"exit": "unrunnable", "stdout": "match"})]
        for status, facts in cases:
            with self.subTest(status=status, facts=facts), \
                 patch.object(corpus_gate, "host_target",
                              return_value="linux_x86_64"):
                rows = [record("run/a/x", status=status, facts=facts)]
                self.assertEqual(self.run_gate(rows, "--native", "--record"), 3)
                self.assertEqual(self.golden.read_text(), corpus_records.render(rows))

    def test_authored_stdout_match_records_without_inventing_an_exit_oracle(self):
        fixture = self.root / "tests" / "omega" / "run" / "a" / "x"
        fixture.mkdir(parents=True)
        (fixture / "expected_stdout.txt").write_text("")
        for exit_code in ("0", "70"):
            with self.subTest(exit_code=exit_code), \
                 patch.object(corpus_gate, "host_target",
                              return_value="linux_x86_64"):
                rows = [record("run/a/x", status="built",
                               facts={"exit": exit_code, "stdout": "match"})]
                self.assertEqual(self.run_gate(rows, "--native", "--record"), 0)
                self.assertEqual(corpus_records.read(self.golden)[0]["facts"],
                                 rows[0]["facts"])
        # Checked-only runs do not make a native execution claim.
        self.assertEqual(self.run_gate([record("run/a/x")], "--record"), 0)

    def test_unannotated_stdout_keeps_observation_only_recording(self):
        rows = [record("run/a/x", status="built",
                       facts={"exit": "70", "stdout": "differs"})]
        with patch.object(corpus_gate, "host_target", return_value="linux_x86_64"):
            self.assertEqual(self.run_gate(rows, "--native", "--record"), 0)

    def test_authored_interpreter_output_cannot_be_hidden_by_matching_exit_codes(self):
        fixture = self.root / "tests" / "omega" / "run" / "a" / "x"
        fixture.mkdir(parents=True)
        (fixture / "expected_stdout.txt").write_text("BA\n")
        for observed in ({"interpreter-exit": "0", "interpreter-stdout": "differs"},
                         {"interpreter-exit": "0"},
                         {"interpreter": "declined"},
                         {"interpreter-stdout": "match"}):
            for leg in ("--interpret", "--both"):
                with self.subTest(observed=observed, leg=leg), \
                     patch.object(corpus_gate, "host_target", return_value="linux_x86_64"):
                    facts = {"exit": "0", "stdout": "match", **observed}
                    rows = [record("run/a/x", status="built", facts=facts)]
                    self.assertEqual(self.run_gate(rows, leg, "--record",
                                                   "--json", str(self.json_path)), 3)
                    self.assertEqual(self.golden.read_text(), corpus_records.render(rows))
                    document = json.loads(self.json_path.read_text())
                    self.assertIn("interpreter stdout", document["oracle_failures"][0])

    def test_authored_output_records_when_every_requested_engine_matches(self):
        fixture = self.root / "tests" / "omega" / "run" / "a" / "x"
        fixture.mkdir(parents=True)
        (fixture / "expected_stdout.txt").write_text("BA\n")
        for leg, status in (("--interpret", "checked"), ("--both", "built")):
            with self.subTest(leg=leg), \
                 patch.object(corpus_gate, "host_target", return_value="linux_x86_64"):
                facts = {"interpreter-exit": "0", "interpreter-stdout": "match"}
                if leg == "--both":
                    facts.update({"exit": "0", "stdout": "match"})
                rows = [record("run/a/x", status=status, facts=facts)]
                self.assertEqual(self.run_gate(rows, leg, "--record"), 0)
                self.assertEqual(corpus_records.read(self.golden)[0]["facts"], facts)

    def test_duplicate_runner_facts_refuse_recording_as_infrastructure_failure(self):
        rows = [record("run/a/x", status="built", facts={"exit": "70"})]
        self.write_golden(rows)
        original = self.golden.read_text()
        for flags, expected_code in (((), 4), (("--bisect",), 125)):
            process = SimpleNamespace(
                stdout=io.StringIO("run/a/x built 1ms exit:7 exit:70\n"),
                stderr=io.StringIO(""), returncode=0, wait=lambda: 0)
            argv = ["corpus_gate", "--runner", "unused", "--record",
                    "--golden", str(self.golden), "--json", str(self.json_path),
                    *flags]
            with self.subTest(flags=flags), \
                 patch.object(corpus_gate, "ROOT", self.root), \
                 patch.object(corpus_gate, "measurement_stamp", return_value="aaaaaaaaaaaa"), \
                 patch.object(corpus_gate, "git_out", return_value=None), \
                 patch.object(corpus_gate.subprocess, "Popen", return_value=process), \
                 patch.dict(corpus_gate.os.environ, {}, clear=True), \
                 patch.object(sys, "argv", argv):
                self.assertEqual(corpus_gate.main(), expected_code)
                self.assertEqual(self.golden.read_text(), original)
                document = json.loads(self.json_path.read_text())
                self.assertEqual(document["verdict"], "infrastructure_failed")
                self.assertIn("duplicate fact", document["infrastructure_error"])

    def test_record_writes_context_and_diff_carries_perf_warnings(self):
        rows = [record("pass/a/x", millis=3000)]
        self.assertEqual(self.run_gate(rows, "--record"), 0)
        context = corpus_records.read_context(self.golden)
        self.assertEqual(context["schema"], "omega-corpus-context-v1")
        self.assertIn("cpus", context)

        # A golden recorded under contention warns on perf moves; patch the
        # golden's context and a faster current row to prove it surfaces.
        busy = {"schema": "omega-corpus-context-v1", "jobs": None,
                "cpus": 8, "platform": "darwin", "machine": "arm64",
                "load1_start": 9.0, "load1_end": 10.0}
        slower = [record("pass/a/x", millis=3000)]
        code = self.run_gate(slower, "--json", str(self.json_path),
                             golden_records=[record("pass/a/x",
                                                    millis=9000)],
                             golden_context=busy)
        self.assertEqual(code, 0)  # perf_faster is a note, not a move
        document = json.loads(self.json_path.read_text())
        self.assertTrue(document["perf_context"]["warnings"])
        self.assertEqual(document["perf_context"]["golden"]["load1_end"],
                         10.0)


if __name__ == "__main__":
    unittest.main()
