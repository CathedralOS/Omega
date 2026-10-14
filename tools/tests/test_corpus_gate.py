"""Native fixture oracles cannot be repinned into a passing golden, a lost
success is recorded as a loss rather than silently dropped, and the gate's
exit code is a machine-readable verdict."""

from pathlib import Path
import contextlib
import io
import json
import os
import subprocess
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



def fake_runner(records: str, stdout: str = ""):
    """A Popen stand-in that writes `records` where the gate asks for them."""
    def popen(arguments, **options):
        Path(options["env"]["OMEGA_CORPUS_RECORDS"]).write_text(records)
        return SimpleNamespace(stdout=io.StringIO(stdout), stderr=io.StringIO(""),
                               returncode=0, wait=lambda: 0)
    return popen

class OptionalHistoryProvenance(unittest.TestCase):
    def test_slow_history_lookup_terminates_a_real_subprocess(self):
        execute = subprocess.run
        def slow_history(*arguments, **options):
            return execute([sys.executable, "-c", "import time; time.sleep(30)"],
                           capture_output=True, text=True, timeout=0.05)
        warning = io.StringIO()
        with patch.object(corpus_gate.subprocess, "run", side_effect=slow_history), \
             contextlib.redirect_stderr(warning):
            self.assertIsNone(corpus_gate.commits_between("historical-revision"))
        self.assertIn("revision distance unavailable", warning.getvalue())

    def test_available_history_keeps_its_actual_distance(self):
        with patch.object(corpus_gate.subprocess, "run", return_value=
                          subprocess.CompletedProcess([], 0, "481\n", "")) as run:
            self.assertEqual(corpus_gate.commits_between("historical-revision"), 481)
        self.assertEqual(run.call_args.kwargs["timeout"], 2)

    def test_failed_history_does_not_invent_a_distance(self):
        with patch.object(corpus_gate.subprocess, "run", return_value=
                          subprocess.CompletedProcess([], 128, "481\n", "missing revision")):
            self.assertIsNone(corpus_gate.commits_between("historical-revision"))


class RunnerLibrarySelection(unittest.TestCase):
    def runner_environment(self, environment):
        rows = corpus_records.render([record("pass/library/selection")])
        with patch.dict(os.environ, environment, clear=True), \
             patch.object(corpus_gate.subprocess, "Popen",
                          side_effect=fake_runner(rows)) as invocation:
            corpus_gate.run_runner(Path("corpus_runner"))
        return invocation.call_args.kwargs["env"]

    def test_default_library_belongs_to_the_measured_checkout(self):
        environment = self.runner_environment({})
        self.assertEqual(environment["OMEGA_LIBRARY_ROOT"],
                         str(corpus_gate.ROOT / "source/library"))

    def test_explicit_library_selection_is_preserved(self):
        explicit = str(Path(tempfile.gettempdir()) / "selected-omega-library")
        environment = self.runner_environment({"OMEGA_LIBRARY_ROOT": explicit})
        self.assertEqual(environment["OMEGA_LIBRARY_ROOT"], explicit)


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

    def test_streams_require_independent_completed_observations(self):
        for stream in ("stdout", "stderr"):
            (self.fixture / f"expected_{stream}.txt").write_text("")
        for engine, prefix in (("native", ""), ("interpreter", "interpreter-")):
            good = {prefix + "exit": "0", prefix + "stdout": "match",
                    prefix + "stderr": "match"}
            self.record.update(facts=good)
            self.assertEqual(corpus_gate.execution_output_failures(
                [self.record], self.root, engine), [])
            for stream in ("stdout", "stderr"):
                for observed in (None, "differs", "unavailable"):
                    facts = dict(good)
                    if observed is None:
                        del facts[prefix + stream]
                    else:
                        facts[prefix + stream] = observed
                    self.record.update(facts=facts)
                    failures = corpus_gate.execution_output_failures(
                        [self.record], self.root, engine)
                    self.assertEqual(len(failures), 1)
                    self.assertIn(stream, str(failures[0]))
            self.record.update(facts=dict(good, **{prefix + "exit": "timeout"}))
            self.assertEqual(len(corpus_gate.execution_output_failures(
                [self.record], self.root, engine)), 2)

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

    def test_fail_tier_incomplete_is_a_refusal(self):
        rejected = record("fail/a/x", status="rejected")
        incomplete = record("fail/a/x", status="incomplete")
        checked = record("fail/a/x", status="checked")
        self.assertEqual(self.verdict(rejected, incomplete),
                         ("moved", ["status rejected -> incomplete"]))
        self.assertEqual(self.verdict(incomplete, checked),
                         ("regressed", ["fail fixture stopped rejecting"]))

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
                 golden_context=None, keep_golden=False):
        if golden_records is None:
            golden_records = records
        if not keep_golden:
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

    def test_clean_path_reports_rows_pinned_against_their_tier(self):
        rows = [record("pass/a/x", status="rejected"),
                record("fail/a/y", status="rejected")]
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            code = self.run_gate(rows)
        self.assertEqual(code, 0)
        text = output.getvalue()
        self.assertIn("corpus_gate: 1 pass/run row(s) pinned without a "
                      "success and 0 fail row(s) pinned as a success "
                      "(0 carry regressed-from)", text)
        self.assertIn("--list-pinned-regressions", text)
        self.assertNotIn("corpus_gate:   pass/a/x", text)

        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            code = self.run_gate(rows, "--list-pinned-regressions")
        self.assertEqual(code, 0)
        text = output.getvalue()
        self.assertIn("corpus_gate:   pass/a/x rejected", text)
        self.assertNotIn("list them with", text)

    def test_pinned_regression_lines_names_lost_successes(self):
        rows = corpus_records.parse(
            "pass/a/x rejected 1ms\n"
            "run/a/y timeout 1ms regressed-from:built+exit:70\n")
        lines = corpus_gate.pinned_regression_lines(rows, list_rows=True)
        self.assertEqual(lines[0],
                         "corpus_gate: 2 pass/run row(s) pinned without a "
                         "success and 0 fail row(s) pinned as a success "
                         "(1 carry regressed-from)")
        self.assertEqual(lines[1], "corpus_gate:   pass/a/x rejected")
        self.assertEqual(lines[2],
                         "corpus_gate:   run/a/y timeout (lost built+exit:70)")
        self.assertEqual(corpus_gate.pinned_regression_lines([]), [])
        only_flag = corpus_gate.pinned_regression_lines(rows)
        self.assertEqual(len(only_flag), 2)
        self.assertIn("--list-pinned-regressions", only_flag[1])

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

    def test_runner_launch_errors_skip_bisect_and_leave_no_records_file(self):
        self.write_golden([record("pass/a/x")])
        original = self.golden.read_text()
        create_records_file = tempfile.NamedTemporaryFile
        records_paths = []

        def create_local_records_file(**options):
            records_file = create_records_file(dir=self.root, **options)
            records_paths.append(Path(records_file.name))
            return records_file

        for error in (FileNotFoundError("runner missing"),
                      PermissionError("runner not executable"),
                      OSError("runner format unsupported")):
            for flags, expected_code in (((), 4), (("--bisect",), 125)):
                argv = ["corpus_gate", "--runner", "unlaunchable", "--record",
                        "--golden", str(self.golden), "--json",
                        str(self.json_path), *flags]
                with self.subTest(error=type(error).__name__, flags=flags), \
                     patch.object(corpus_gate, "ROOT", self.root), \
                     patch.object(corpus_gate, "host_target", return_value="macos_arm64"), \
                     patch.object(corpus_gate.tempfile, "NamedTemporaryFile",
                                  side_effect=create_local_records_file), \
                     patch.object(corpus_gate.subprocess, "Popen", side_effect=error), \
                     patch.dict(corpus_gate.os.environ, {}, clear=True), \
                     patch.object(sys, "argv", argv):
                    self.assertEqual(corpus_gate.main(), expected_code)
                    self.assertEqual(self.golden.read_text(), original)
                    document = json.loads(self.json_path.read_text())
                    self.assertEqual(document["verdict"], "infrastructure_failed")
                    self.assertEqual(document["exit_code"], expected_code)
                    self.assertIn("unlaunchable", document["infrastructure_error"])
                    self.assertIn(str(error), document["infrastructure_error"])
                    self.assertFalse(records_paths[-1].exists())

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

    def test_json_oracle_failure_retains_observed_diagnostics(self):
        fixture = self.root / "tests" / "omega" / "pass" / "host" / "buffer"
        fixture.mkdir(parents=True)
        (fixture / "expected_native_exit.txt").write_text("0\n")
        built = record("pass/host/buffer", status="built", facts={"exit": "0"})
        refused = record("pass/host/buffer", status="rejected",
                         diagnostics=["borrowed source custody mismatch"])
        with patch.object(corpus_gate, "host_target", return_value="macos_arm64"):
            self.assertEqual(self.run_gate(
                [refused], "--native", "--json", str(self.json_path),
                golden_records=[built]), 3)
        document = json.loads(self.json_path.read_text())
        self.assertEqual(document["observations"], [refused])
        self.assertEqual(document["fixtures"], [])
        self.assertEqual(document["counts"]["fixtures"], 1)
        self.assertEqual(self.golden.read_text(), corpus_records.render([built]))

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

    def test_subset_record_leaves_unmeasured_text_untouched(self):
        prior = [record("pass/a/y", recorded_at="bbbbbbbbbbbb"),
                 record("pass/a/x")]
        self.write_golden(prior)
        annotated = (self.golden.read_text()
                     + "# an annotation another lane wrote\n")
        self.golden.write_text(annotated)
        untouched = [line for line in annotated.splitlines()
                     if not line.startswith("pass/a/x")]
        code = self.run_gate([record("pass/a/x")], "--record", "--filter", "a/x",
                             golden_records=None, keep_golden=True)
        self.assertEqual(code, 0)
        written = self.golden.read_text().splitlines()
        self.assertEqual([line for line in written
                          if not line.startswith("pass/a/x")], untouched)
        self.assertTrue(any(line.startswith("pass/a/x") and "@aaaaaaaaaaaa" in line
                            for line in written))

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

    def test_authored_stderr_cannot_be_repinned_as_success(self):
        fixture = self.root / "tests" / "omega" / "run" / "a" / "x"
        fixture.mkdir(parents=True)
        (fixture / "expected_stderr.txt").write_text("authored error stream\n")
        rows = [record("run/a/x", status="built", facts={
            "exit": "0", "stdout": "match", "stderr": "differs",
            "interpreter-exit": "0", "interpreter-stderr": "match",
            "interpreter": "agrees"})]
        with patch.object(corpus_gate, "host_target", return_value="macos_arm64"):
            self.assertEqual(self.run_gate(rows, "--both", "--record",
                                          "--json", str(self.json_path)), 3)
        document = json.loads(self.json_path.read_text())
        self.assertIn("stderr", str(document["oracle_failures"]))
        self.assertEqual(self.golden.read_text(), corpus_records.render(rows))

    def test_authored_stdout_requires_completed_native_execution(self):
        fixture = self.root / "tests" / "omega" / "run" / "a" / "x"
        fixture.mkdir(parents=True)
        (fixture / "expected_stdout.txt").write_text("")
        # A foreign-target build or declaration-only fixture has no runtime
        # facts. Like an exit oracle, authored stdout remains unproved there.
        # A success-shaped row contradicts the promise and cannot be written;
        # a refusal produced nothing to contradict it and is written as the
        # observation it is, with the verdict still `regressed`.
        cases = [("built", {}), ("checked", {}),
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
        with patch.object(corpus_gate, "host_target", return_value="linux_x86_64"):
            rows = [record("run/a/x", status="rejected")]
            self.assertEqual(self.run_gate(
                rows, "--native", "--record",
                golden_records=[record("run/a/x", status="built",
                                       facts={"exit": "0", "stdout": "match"})]), 3)
            written = corpus_records.read(self.golden)[0]
            self.assertEqual(written["status"], "rejected")
            self.assertEqual(written["regressed_from"],
                             {"status": "built",
                              "facts": {"exit": "0", "stdout": "match"}})

    def test_lost_success_records_the_loss_and_still_exits_regressed(self):
        fixture = self.root / "tests" / "omega" / "pass" / "text" / "decimal"
        fixture.mkdir(parents=True)
        (fixture / "expected_native_exit.txt").write_text("70\n")
        built = record("pass/text/decimal", status="built", facts={"exit": "70"})
        refused = record("pass/text/decimal", status="rejected",
                         diagnostics=["the divisor must be proven nonzero"])
        with patch.object(corpus_gate, "host_target", return_value="macos_arm64"):
            self.assertEqual(self.run_gate(
                [refused], "--native", "--record", "--json", str(self.json_path),
                golden_records=[built]), 3)
        written = corpus_records.read(self.golden)[0]
        self.assertEqual(written["status"], "rejected")
        self.assertEqual(written["facts"], {})
        self.assertEqual(written["regressed_from"],
                         {"status": "built", "facts": {"exit": "70"}})
        document = json.loads(self.json_path.read_text())
        self.assertEqual(document["verdict"], "recorded_regression")
        self.assertEqual(document["exit_code"], 3)
        self.assertEqual(document["counts"]["lost_successes"], 1)
        self.assertEqual(document["counts"]["lost_success_fixtures"],
                         ["pass/text/decimal"])
        self.assertIn("expected native 70", document["oracle_failures"][0])
        # Recording the loss does not make the next run clean: the row now
        # matches the golden structurally and the unmet promise still fails.
        with patch.object(corpus_gate, "host_target", return_value="macos_arm64"):
            self.assertEqual(self.run_gate(
                [refused], "--native", golden_records=[written]), 3)
        # A row that keeps failing carries the ORIGINAL success forward
        # rather than re-pointing at yesterday's refusal.
        later = record("pass/text/decimal", status="incomplete")
        with patch.object(corpus_gate, "host_target", return_value="macos_arm64"):
            self.assertEqual(self.run_gate(
                [later], "--native", "--record", golden_records=[written]), 3)
        self.assertEqual(corpus_records.read(self.golden)[0]["regressed_from"],
                         {"status": "built", "facts": {"exit": "70"}})
        # The repair drops the token, so a later fix is a visible move.
        with patch.object(corpus_gate, "host_target", return_value="macos_arm64"):
            self.assertEqual(self.run_gate(
                [built], "--native", "--record",
                golden_records=corpus_records.read(self.golden)), 0)
        self.assertIsNone(corpus_records.read(self.golden)[0]["regressed_from"])

    def test_a_fixture_that_never_built_here_records_no_lost_success(self):
        fixture = self.root / "tests" / "omega" / "pass" / "text" / "fresh"
        fixture.mkdir(parents=True)
        (fixture / "expected_native_exit.txt").write_text("70\n")
        refused = record("pass/text/fresh", status="rejected")
        for prior in ([refused], [record("pass/text/fresh", status="incomplete")]):
            with self.subTest(prior=prior[0]["status"]), \
                 patch.object(corpus_gate, "host_target", return_value="macos_arm64"):
                self.assertEqual(self.run_gate(
                    [refused], "--native", "--record", golden_records=prior), 3)
                self.assertIsNone(
                    corpus_records.read(self.golden)[0]["regressed_from"])
        # A fixture absent from the golden has no success to lose either.
        with patch.object(corpus_gate, "host_target", return_value="macos_arm64"):
            self.assertEqual(self.run_gate(
                [refused], "--native", "--record",
                golden_records=[record("pass/text/other", status="built")]), 3)
        rows = {row["fixture"]: row for row in corpus_records.read(self.golden)}
        self.assertIsNone(rows["pass/text/fresh"]["regressed_from"])

    def test_record_refuses_a_contradicted_oracle_and_names_the_fixture(self):
        fixture = self.root / "tests" / "omega" / "pass" / "text" / "wrong"
        fixture.mkdir(parents=True)
        (fixture / "expected_native_exit.txt").write_text("70\n")
        golden = [record("pass/text/wrong", status="built", facts={"exit": "70"})]
        ran_differently = [record("pass/text/wrong", status="built",
                                  facts={"exit": "71"})]
        stream = io.StringIO()
        with patch.object(corpus_gate, "host_target", return_value="macos_arm64"), \
             contextlib.redirect_stderr(stream):
            self.assertEqual(self.run_gate(
                ran_differently, "--native", "--record",
                "--json", str(self.json_path), golden_records=golden), 3)
        self.assertEqual(self.golden.read_text(), corpus_records.render(golden))
        self.assertIn("pass/text/wrong", stream.getvalue())
        self.assertIn("--record refused", stream.getvalue())
        self.assertEqual(json.loads(self.json_path.read_text())["verdict"],
                         "regressed")

    def test_unreadable_golden_is_a_usage_error_but_record_replaces_it(self):
        self.golden.write_text("pass/a/x built nonsense\n", encoding="utf-8")
        rows = [record("pass/a/x", status="built")]
        argv_base = ["corpus_gate", "--runner", "unused", "--golden", str(self.golden)]

        def invoke(*extra):
            with patch.object(corpus_gate, "ROOT", self.root), \
                 patch.object(corpus_gate, "run_runner", return_value=rows), \
                 patch.object(corpus_gate, "measurement_stamp",
                              return_value="aaaaaaaaaaaa"), \
                 patch.dict(corpus_gate.os.environ, {}, clear=True), \
                 patch.object(sys, "argv", argv_base + list(extra)), \
                 contextlib.redirect_stderr(io.StringIO()):
                return corpus_gate.main()

        self.assertEqual(invoke(), 2)
        self.assertEqual(invoke("--record"), 0)
        self.assertEqual(corpus_records.read(self.golden)[0]["status"], "built")

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
            popen = fake_runner("run/a/x built 1ms exit:7 exit:70\n")
            argv = ["corpus_gate", "--runner", "unused", "--record",
                    "--golden", str(self.golden), "--json", str(self.json_path),
                    *flags]
            with self.subTest(flags=flags), \
                 patch.object(corpus_gate, "ROOT", self.root), \
                 patch.object(corpus_gate, "measurement_stamp", return_value="aaaaaaaaaaaa"), \
                 patch.object(corpus_gate, "git_out", return_value=None), \
                 patch.object(corpus_gate.subprocess, "Popen", side_effect=popen), \
                 patch.dict(corpus_gate.os.environ, {}, clear=True), \
                 patch.object(sys, "argv", argv):
                self.assertEqual(corpus_gate.main(), expected_code)
                self.assertEqual(self.golden.read_text(), original)
                document = json.loads(self.json_path.read_text())
                self.assertEqual(document["verdict"], "infrastructure_failed")
                self.assertIn("duplicate fact", document["infrastructure_error"])

    def test_build_log_on_runner_stdout_is_not_parsed_as_a_record(self):
        rows = [record("pass/build/log", status="checked")]
        self.write_golden(rows)
        popen = fake_runner(
            corpus_records.render(rows), stdout="omega build log witness\n")
        argv = ["corpus_gate", "--runner", "unused", "--golden", str(self.golden),
                "--json", str(self.json_path)]
        with patch.object(corpus_gate, "ROOT", self.root), \
             patch.object(corpus_gate, "measurement_stamp", return_value="aaaaaaaaaaaa"), \
             patch.object(corpus_gate, "git_out", return_value=None), \
             patch.object(corpus_gate.subprocess, "Popen", side_effect=popen), \
             patch.dict(corpus_gate.os.environ, {}, clear=True), \
             patch.object(sys, "argv", argv):
            self.assertEqual(corpus_gate.main(), 0)
        document = json.loads(self.json_path.read_text())
        self.assertNotEqual(document["verdict"], "infrastructure_failed")

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
