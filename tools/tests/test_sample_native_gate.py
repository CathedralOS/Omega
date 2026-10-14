"""The sample gate edits decisions, never the package review's byte framing."""

import contextlib
import importlib.util
import io
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    "sample_native_gate", Path(__file__).resolve().parents[1] / "sample_native_gate.py"
)
gate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gate)

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import sample_projects


class WorkDiscoveryTests(unittest.TestCase):
    def test_stops_before_scheduling_unrelated_tail_and_remeasures_lead(self):
        import sample_queue
        from types import SimpleNamespace
        arguments = SimpleNamespace(cli="omega", target="linux_x86_64", timeout=300,
                                    build_unscored=False)
        failure = {"sample": "samples/first", "state": "no-native-build", "detail": "real refusal"}
        with patch.object(sample_queue, "live_ownership", return_value=[]), \
             patch.object(sample_queue, "latest_record", return_value=None), \
             patch.object(sample_queue, "discovery_candidates", return_value=["samples/first", "samples/tail"]), \
             patch.object(sample_queue, "queue", return_value=[failure]), \
             patch.object(gate, "timed_measure", return_value=failure) as measure:
            results = gate.find_work(arguments, ["samples/first", "samples/tail"])
        self.assertEqual(results, [failure])
        measure.assert_called_once_with("omega", "samples/first", "linux_x86_64", 300, False)

    def test_passing_old_failure_does_not_stop_search(self):
        import sample_queue
        from types import SimpleNamespace
        arguments = SimpleNamespace(cli="omega", target="linux_x86_64", timeout=300,
                                    build_unscored=False)
        passing = {"sample": "samples/repaired", "state": "pass"}
        failure = {"sample": "samples/next", "state": "oracle-failed"}
        with patch.object(sample_queue, "live_ownership", return_value=[]), \
             patch.object(sample_queue, "latest_record", return_value=None), \
             patch.object(sample_queue, "discovery_candidates", return_value=["samples/repaired", "samples/next"]), \
             patch.object(sample_queue, "queue", side_effect=[[], [failure]]), \
             patch.object(gate, "timed_measure", side_effect=[passing, failure]) as measure:
            self.assertEqual(gate.find_work(arguments, []), [passing, failure])
        self.assertEqual(measure.call_count, 2)


class ReviewAcceptanceTests(unittest.TestCase):
    def test_only_pending_decisions_change(self):
        with tempfile.TemporaryDirectory() as directory:
            review = Path(directory) / "build/package-manager/review-windows_x86_64.txt"
            review.parent.mkdir(parents=True)
            original = (
                'omega-package-review 1\npackage "mathematics ∀"\n'
                'finding pending\ndecision row ab12 pending\n'
                'decision row cd34 reject\nend-review\n'
            ).encode("utf-8")
            review.write_bytes(original)
            success = subprocess.CompletedProcess([], 0, "", "")
            with patch.object(gate, "run", return_value=success) as runner:
                reason, _ = gate.accept_review("omega", directory, "windows_x86_64", 10)
            self.assertIsNone(reason)
            self.assertEqual(
                review.read_bytes(),
                original.replace(b"decision row ab12 pending", b"decision row ab12 accept"),
            )
            self.assertEqual(runner.call_count, 2)
            self.assertEqual(runner.call_args.args[0][-1], "--resume")

    def test_unsupported_lock_is_removed_and_reviewed_afresh(self):
        with tempfile.TemporaryDirectory() as directory:
            lock = Path(directory) / "omega.lock"
            lock.write_bytes(b"omega-lock 0\n")
            review = Path(directory) / "build/package-manager/review-macos_arm64.txt"
            review.parent.mkdir(parents=True)
            review.write_bytes(b"omega-package-review 1\nend-review\n")
            refusal = subprocess.CompletedProcess(
                [], 0, "", "cannot read accepted omega.lock: unsupported package lock "
                "version; restore a compatible lock or explicitly remove it")
            success = subprocess.CompletedProcess([], 0, "Published build.omg and omega.lock", "")
            with patch.object(gate, "run", side_effect=[refusal, success]) as runner:
                reason, _ = gate.accept_review("omega", directory, "macos_arm64", 10)
            self.assertIsNone(reason)
            self.assertFalse(lock.exists())
            self.assertEqual(runner.call_count, 2)
            self.assertEqual(runner.call_args.args[0][1], "update")

    def test_resume_failure_is_not_successful_acceptance(self):
        with tempfile.TemporaryDirectory() as directory:
            review = Path(directory) / "build/package-manager/review-windows_x86_64.txt"
            review.parent.mkdir(parents=True)
            review.write_bytes(b"decision row ab12 pending\n")
            results = [
                subprocess.CompletedProcess([], 0, "review ready", ""),
                subprocess.CompletedProcess([], 1, "", "review findings changed"),
            ]
            with patch.object(gate, "run", side_effect=results):
                reason, output = gate.accept_review("omega", directory, "windows_x86_64", 10)
            self.assertEqual(reason, "resume-failed")
            self.assertEqual(output, "review findings changed")


class FixtureOracleTests(unittest.TestCase):
    """`input.txt` beside the entry is the run's stdin; `expected_*` files
    score their channels exactly instead of the header oracle."""

    def project(self, directory, header="// Expected exit: 0.\n"):
        root = Path(directory)
        (root / "main.omg").write_text(header + "module probe;\n", encoding="utf-8")
        (root / "build.omg").write_text(
            'machine build(builder: &mut Build) {\n'
            '    Build::application(builder, "probe");\n'
            '    Build::bind(builder, macos_arm64::ProgramEntry, probe::Main::main);\n'
            '}\n',
            encoding="utf-8",
        )
        return root

    def measure(self, directory, stdout, stderr, exit_code=0):
        """Run measure() with `run` faked: reviews succeed, `omega run`
        returns the supplied program result and records its stdin."""
        calls = []

        def fake_run(command, timeout, stdin=subprocess.DEVNULL, cwd=None):
            calls.append((command, stdin, cwd))
            if "run" in command:
                return subprocess.CompletedProcess(command, exit_code, stdout, stderr)
            return subprocess.CompletedProcess(command, 0, "", "")

        root = Path(directory)
        project = sample_projects.Project(
            directory=root, entry=root / "main.omg", role="program")
        with patch.object(gate, "run", side_effect=fake_run), \
                patch.object(gate, "project_at", return_value=project):
            result = gate.measure("omega", directory, "macos_arm64", 10)
        runs = [stdin for command, stdin, cwd in calls if "run" in command]
        self.run_directories = [cwd for command, stdin, cwd in calls if "run" in command]
        return result, runs

    def test_program_without_an_oracle_is_still_built(self):
        with tempfile.TemporaryDirectory() as directory:
            Path(directory, "main.omg").write_text("machine main() {}\n")
            calls = []

            def fake_run(command, timeout, stdin=subprocess.DEVNULL, cwd=None):
                calls.append(command)
                return subprocess.CompletedProcess(command, 0, "", "")

            root = Path(directory)
            project = sample_projects.Project(
                directory=root, entry=root / "main.omg", role="program")
            with patch.object(gate, "run", side_effect=fake_run), \
                    patch.object(gate, "project_at", return_value=project):
                result = gate.measure("omega", directory, "macos_arm64", 10)
            self.assertEqual(result["state"], "no-oracle")
            self.assertTrue(calls)

    def test_probe_without_an_oracle_is_not_compiled(self):
        with tempfile.TemporaryDirectory() as parent:
            directory = str(Path(parent, "probes", "gap"))
            Path(directory).mkdir(parents=True)
            Path(directory, "main.omg").write_text("machine main() {}\n")
            calls = []

            def fake_run(command, timeout, stdin=subprocess.DEVNULL, cwd=None):
                calls.append(command)
                return subprocess.CompletedProcess(command, 0, "", "")

            root = Path(directory)
            project = sample_projects.Project(
                directory=root, entry=root / "main.omg", role="program")
            with patch.object(gate, "run", side_effect=fake_run), \
                    patch.object(gate, "project_at", return_value=project):
                result = gate.measure("omega", directory, "macos_arm64", 10)
                self.assertEqual(result["state"], "no-oracle")
                self.assertEqual(calls, [])
                built = gate.measure("omega", directory, "macos_arm64", 10,
                                     build_unscored=True)
            self.assertEqual(built["state"], "no-oracle")
            self.assertTrue(calls)

    def test_input_fixture_feeds_input_and_scores_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = self.project(directory)
            (root / "input.txt").write_bytes(b"A")
            (root / "expected_native_exit.txt").write_text("0\n", encoding="utf-8")
            (root / "expected_stdout.txt").write_text(
                "float equality: equal and unequal\n", encoding="utf-8")
            (root / "expected_stderr.txt").write_text("", encoding="utf-8")
            result, runs = self.measure(
                directory,
                "float equality: equal and unequal\n",
                "native exit 0\n",
            )
            self.assertEqual(result["state"], "pass")
            self.assertEqual(len(runs), 1)
            self.assertIsNot(runs[0], subprocess.DEVNULL)
            self.assertEqual(runs[0].mode, "rb")

    def test_fixture_stdout_mismatch_is_oracle_failed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = self.project(directory)
            (root / "input.txt").write_bytes(b"A")
            (root / "expected_stdout.txt").write_text("wanted\n", encoding="utf-8")
            result, _ = self.measure(directory, "other\n", "native exit 0\n")
            self.assertEqual(result["state"], "oracle-failed")
            self.assertIn("stdout differs", result["detail"])

    def test_program_stderr_is_compared_without_wrapper_lines(self):
        with tempfile.TemporaryDirectory() as directory:
            root = self.project(directory)
            (root / "input.txt").write_bytes(b"A")
            (root / "expected_stderr.txt").write_text("", encoding="utf-8")
            result, _ = self.measure(directory, "", "warning\nnative exit 0\n")
            self.assertEqual(result["state"], "oracle-failed")
            self.assertIn("stderr differs", result["detail"])

    def test_programs_run_from_a_scratch_directory_in_build(self):
        with tempfile.TemporaryDirectory() as directory:
            self.project(directory)
            result, _ = self.measure(directory, "", "native exit 0\n")
            self.assertEqual(result["state"], "pass")
            self.assertEqual(self.run_directories, [str(Path(directory, "build", "gate-run"))])
            self.assertTrue(Path(directory, "build", "gate-run").is_dir())

    def test_header_oracle_keeps_empty_stdin(self):
        with tempfile.TemporaryDirectory() as directory:
            self.project(
                directory,
                header="// Expected exit: 2.\n// Expected output contains: INVALID\n")
            result, runs = self.measure(directory, "INVALID\n", "native exit 2\n", exit_code=2)
            self.assertEqual(result["state"], "pass")
            self.assertEqual(runs, [subprocess.DEVNULL])


class HostScopeAndDiagnosticTests(unittest.TestCase):
    """Samples are scored on the targets they author, and failures keep the
    whole refusal, including the review's own error."""

    def project(self, directory, binds=("macos_arm64",), role="program"):
        root = Path(directory)
        (root / "main.omg").write_text("// Expected exit: 0.\nmodule probe;\n", encoding="utf-8")
        lines = "".join(
            f"    Build::bind(builder, {target}::ProgramEntry, probe::Main::main);\n"
            for target in binds)
        (root / "build.omg").write_text(
            'machine build(builder: &mut Build) {\n'
            '    Build::application(builder, "probe");\n'
            '    // Build::bind(builder, windows_x86_64::ProgramEntry, probe::Main::main);\n'
            + lines + '}\n',
            encoding="utf-8",
        )
        return sample_projects.Project(directory=root, entry=root / "main.omg", role=role)

    def measure(self, directory, project, fake_run):
        with patch.object(gate, "run", side_effect=fake_run), \
                patch.object(gate, "project_at", return_value=project):
            return gate.measure("omega", directory, "macos_arm64", 10)

    def test_sample_without_this_target_is_not_host_runnable(self):
        with tempfile.TemporaryDirectory() as directory:
            project = self.project(directory, binds=("linux_x86_64",))
            calls = []
            result = self.measure(directory, project, lambda *a, **k: calls.append(a))
            self.assertEqual(result["state"], "not-host-runnable")
            self.assertIn("linux_x86_64", result["detail"])
            self.assertEqual(calls, [], "an unbound host must not build or run the sample")

    def test_commented_binding_does_not_count(self):
        with tempfile.TemporaryDirectory() as directory:
            self.project(directory, binds=("linux_x86_64",))
            self.assertEqual(gate.bound_targets(directory), {"linux_x86_64"})

    def test_failed_review_reports_its_own_error(self):
        with tempfile.TemporaryDirectory() as directory:
            project = self.project(directory)
            refusal = ("package candidate checking failed: checked compilation failed\n"
                       "  error: selected ProgramEntry Binding field `Main::fs` requires a "
                       "selected Fused provider for boundary `FilesystemHost`\n")

            def fake_run(command, timeout, stdin=subprocess.DEVNULL):
                if "update" in command:
                    return subprocess.CompletedProcess(command, 1, "", refusal)
                self.fail(f"nothing runs after a failed review: {command}")

            result = self.measure(directory, project, fake_run)
            self.assertEqual(result["state"], "no-native-build")
            self.assertIn("FilesystemHost", result["diagnostic"])
            self.assertTrue(result["detail"].startswith("package review: "))

    def test_failed_review_of_a_probe_stays_a_check_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            project = self.project(directory, role="check-only")
            refusal = "  error: cannot resolve `Missing`\n"
            result = self.measure(
                directory, project,
                lambda command, timeout, stdin=None: subprocess.CompletedProcess(command, 1, "", refusal))
            self.assertEqual(result["state"], "check-fail")

    def test_pending_decisions_are_not_a_review_failure(self):
        # `omega update` exits nonzero while decisions are pending; that is
        # the normal path, not a refusal.
        with tempfile.TemporaryDirectory() as directory:
            pending = subprocess.CompletedProcess([], 3, "review: pending decisions", "")
            with patch.object(gate, "run", return_value=pending):
                reason, _ = gate.accept_review("omega", directory, "macos_arm64", 10)
            self.assertIsNone(reason)

    def test_pending_review_mentioning_an_error_is_still_accepted(self):
        # A finding may quote `error:` text; pending decisions still win.
        with tempfile.TemporaryDirectory() as directory:
            review = Path(directory) / "build/package-manager/review-macos_arm64.txt"
            review.parent.mkdir(parents=True)
            review.write_bytes(b"finding: error: quoted\ndecision row ab12 pending\n")
            results = [
                subprocess.CompletedProcess([], 3, "finding: error: quoted", ""),
                subprocess.CompletedProcess([], 0, "", ""),
            ]
            with patch.object(gate, "run", side_effect=results):
                reason, _ = gate.accept_review("omega", directory, "macos_arm64", 10)
            self.assertIsNone(reason)
            self.assertIn(b"decision row ab12 accept", review.read_bytes())

    def test_diagnostic_keeps_the_construct_name(self):
        construct = "state graph: terminator: guarded transition into a borrowed window"
        message = ("error: `Main::main` has no executable body: this compiler does not "
                   f"lower a state of it yet ({construct})")
        self.assertNotIn(construct, gate.classify(message))
        self.assertIn(construct, gate.classify(message, limit=None))


class TimingTests(unittest.TestCase):
    """Every compiler call's time lands on the sample and phase that made it."""

    def test_phases_are_attributed_by_command(self):
        def slow_process(command, **_):
            gate.time.sleep(0.05)
            return subprocess.CompletedProcess(command, 0, "", "")

        def fake_measure(cli, sample, target, timeout, build_unscored=False):
            gate.run([cli, "update", "--project", sample], timeout)
            gate.run([cli, "--check", "main.omg"], timeout)
            gate.run([cli, "run", "main.omg"], timeout)
            gate.run([cli, "run", "main.omg"], timeout)
            return {"sample": sample, "state": "pass", "detail": ""}

        with patch.object(gate.subprocess, "run", side_effect=slow_process), \
                patch.object(gate, "measure", side_effect=fake_measure):
            result = gate.timed_measure("omega", "samples/a", "macos_arm64", 10)
        self.assertEqual(set(result["phases"]), {"review", "check", "run"})
        self.assertGreaterEqual(result["phases"]["run"], 0.09)
        self.assertGreaterEqual(result["seconds"], sum(result["phases"].values()) - 0.01)

    def test_calls_outside_a_measurement_are_not_attributed(self):
        with patch.object(gate.subprocess, "run",
                          return_value=subprocess.CompletedProcess([], 0, "", "")):
            gate.run(["omega", "update"], 10)
        self.assertIsNone(getattr(gate._phases, "times", None))

    def test_summary_orders_slowest_first_and_totals_phases(self):
        results = [
            {"sample": "a", "state": "pass", "seconds": 2.0, "phases": {"review": 1.5, "run": 0.5}},
            {"sample": "b", "state": "no-native-build", "seconds": 9.0, "phases": {"review": 8.0}},
        ]
        summary = gate.timing_summary(results, 9.5)
        self.assertEqual([row["sample"] for row in summary["slowest"]], ["b", "a"])
        self.assertEqual(summary["phase_seconds"], {"review": 9.5, "run": 0.5})
        self.assertEqual(summary["sample_seconds"], 11.0)


class NestedGroupTests(unittest.TestCase):
    """A package and the projects nested in its source tree never run
    concurrently; unrelated samples still form separate groups."""

    def test_nested_projects_share_their_outermost_ancestors_group(self):
        samples = [
            "samples/cli/lab/controls/invalid",
            "samples/cli/lab",
            "samples/cli/lab/evidence/deep/repro",
            "samples/cli/lab-variant",
            "samples/cli/other",
        ]
        self.assertEqual(gate.nested_groups(samples), [
            ["samples/cli/lab", "samples/cli/lab/controls/invalid",
             "samples/cli/lab/evidence/deep/repro"],
            ["samples/cli/lab-variant"],
            ["samples/cli/other"],
        ])

    def test_unrelated_siblings_stay_parallel(self):
        self.assertEqual(len(gate.nested_groups(["samples/a", "samples/b", "samples/c"])), 3)


class StalePackageStateTests(unittest.TestCase):
    """Only failures about this checkout's package state get one clean retry."""

    def test_stale_state_is_cleared_and_measured_again(self):
        with tempfile.TemporaryDirectory() as directory:
            Path(directory, "build", "package-manager").mkdir(parents=True)
            Path(directory, "omega.lock").write_text("stale")
            Path(directory, "main.omg").write_text("machine main() {}\n")
            stale = {"sample": directory, "state": "no-native-build",
                     "detail": "target root slot ... needs an accepted program-entry binding"}
            passing = {"sample": directory, "state": "pass", "detail": ""}
            with patch.object(gate, "measure", side_effect=[stale, passing]) as measure:
                result = gate.timed_measure("omega", directory, "macos_arm64", 10)
            self.assertEqual(measure.call_count, 2)
            self.assertEqual(result["state"], "pass")
            self.assertIn("program-entry binding", result["retried"])
            self.assertFalse(Path(directory, "build").exists())
            self.assertFalse(Path(directory, "omega.lock").exists())
            self.assertTrue(Path(directory, "main.omg").exists(), "sources are never removed")

    def test_a_compiler_refusal_is_not_retried(self):
        refusal = {"sample": "s", "state": "no-native-build",
                   "detail": "`Main::main` has no executable body"}
        with patch.object(gate, "measure", return_value=refusal) as measure:
            result = gate.timed_measure("omega", "s", "macos_arm64", 10)
        self.assertEqual(measure.call_count, 1)
        self.assertNotIn("retried", result)


class GateCliTests(unittest.TestCase):
    """By default the gate measures a fresh opt-level 1 build; an explicit
    CLI older than the checkout's compiler is flagged."""

    def test_default_build_is_optimized_and_isolated(self):
        built = subprocess.CompletedProcess([], 0, "", "")
        with patch.object(gate.shutil, "which", return_value=None), \
                patch.object(gate.subprocess, "run", return_value=built) as runner, \
                contextlib.redirect_stderr(io.StringIO()):
            cli = gate.build_gate_cli()
        command = runner.call_args.args[0]
        environment = runner.call_args.kwargs["env"]
        self.assertEqual(command[:2], ["cargo", "build"])
        self.assertEqual(environment["CARGO_PROFILE_DEV_OPT_LEVEL"], "1")
        self.assertEqual(environment["CARGO_TARGET_DIR"], str(gate.GATE_CLI_DIR))
        self.assertTrue(cli.startswith(str(gate.GATE_CLI_DIR)))

    def test_failed_build_returns_nothing(self):
        failed = subprocess.CompletedProcess([], 101, "", "error[E0425]")
        with patch.object(gate.subprocess, "run", return_value=failed), \
                contextlib.redirect_stderr(io.StringIO()):
            self.assertIsNone(gate.build_gate_cli())

    def test_cli_older_than_last_compiler_commit_is_stale(self):
        with tempfile.NamedTemporaryFile() as cli:
            built = Path(cli.name).stat().st_mtime

            def git(command, **_):
                if "log" in command:
                    return subprocess.CompletedProcess(command, 0, f"abc1234 {int(built) + 60}\n", "")
                return subprocess.CompletedProcess(command, 0, "", "")

            with patch.object(gate.subprocess, "run", side_effect=git):
                self.assertIn("abc1234", gate.cli_staleness(cli.name))

    def test_cli_newer_than_compiler_is_current(self):
        with tempfile.NamedTemporaryFile() as cli:
            built = Path(cli.name).stat().st_mtime

            def git(command, **_):
                if "log" in command:
                    return subprocess.CompletedProcess(command, 0, f"abc1234 {int(built) - 60}\n", "")
                return subprocess.CompletedProcess(command, 0, "", "")

            with patch.object(gate.subprocess, "run", side_effect=git):
                self.assertEqual(gate.cli_staleness(cli.name), "")


class RunHistoryTests(unittest.TestCase):
    def test_program_discovery_is_explicit_and_full_gate_still_includes_controls(self):
        samples = ["samples/cli/app", "samples/cli/probes/gap",
                   "samples/cli/app/controls/invalid_case"]
        with tempfile.TemporaryDirectory() as directory:
            for flags, expected, scope in (([], samples, "all"),
                                            (["--programs-only"], samples[:1], "programs")):
                with self.subTest(scope=scope):
                    arguments = ["sample_native_gate.py", "--cli", "omega", "--jobs", "1",
                                 "--history-dir", directory, *flags]
                    def result(_cli, sample, _target, _timeout, _build_unscored=False):
                        return {"sample": sample, "state": "pass", "detail": "", "seconds": 0}
                    with patch.object(sys, "argv", arguments), \
                            patch.object(gate, "discover", return_value=samples), \
                            patch.object(gate, "cli_staleness", return_value=""), \
                            patch.object(gate, "timed_measure", side_effect=result) as measure, \
                            contextlib.redirect_stdout(io.StringIO()):
                        self.assertEqual(gate.main(), 0)
                    self.assertEqual([call.args[1] for call in measure.call_args_list], sorted(expected))
                    records = [gate.json.loads(path.read_text()) for path in Path(directory).glob("*.json")]
                    self.assertTrue(any(record["scope"] == scope and
                                        [row["sample"] for row in record["results"]] == sorted(expected)
                                        for record in records))

    def test_failure_is_flushed_before_the_aggregate_run_finishes(self):
        failure = {"sample": "samples/cli/app", "state": "no-native-build",
                   "detail": "short", "diagnostic": "complete refusal"}
        with patch.object(gate, "measure", return_value=failure), \
                patch("builtins.print") as report:
            gate.timed_measure("omega", failure["sample"], "macos_arm64", 10)
        report.assert_called_once_with(
            "sample_native_gate: no-native-build: samples/cli/app: complete refusal",
            file=sys.stderr, flush=True)

    def test_main_creates_result_parents_and_preserves_the_gate_verdict(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "reports/nested/results.json"
            history = Path(directory) / "history"
            sample = "samples/cli/collections/example"
            for state, expected_exit in (("pass", 0), ("oracle-failed", 1)):
                with self.subTest(state=state):
                    result = {"sample": sample, "state": state,
                              "detail": "", "seconds": 0.0}
                    arguments = ["sample_native_gate.py", "--cli", "omega",
                                 "--target", "macos_arm64", "--jobs", "1",
                                 "--write-results", str(output),
                                 "--history-dir", str(history)]
                    with patch.object(sys, "argv", arguments), \
                            patch.object(gate, "discover", return_value=[sample]), \
                            patch.object(gate, "cli_staleness", return_value=""), \
                            patch.object(gate, "timed_measure", return_value=result), \
                            contextlib.redirect_stdout(io.StringIO()):
                        self.assertEqual(gate.main(), expected_exit)
                    self.assertEqual(gate.json.loads(output.read_text()), [result])
                    records = [gate.json.loads(path.read_text())
                               for path in history.glob("*.json")]
                    self.assertTrue(any(record["results"] == [result] for record in records))

    def test_each_run_writes_a_dated_record(self):
        with tempfile.TemporaryDirectory() as directory:
            arguments = gate.argparse.Namespace(
                history_dir=directory, target="macos_arm64", cli="omega", filter="games")
            started = gate.datetime.datetime(2026, 10, 12, 22, 0, tzinfo=gate.datetime.timezone.utc)
            results = [{"sample": "samples/cli/games/a", "state": "pass", "detail": ""}]
            gate.record_run(arguments, started, results, {"pass": results}, 1, 1)
            written = Path(directory) / "20261012T220000Z-macos_arm64.json"
            record = gate.json.loads(written.read_text())
            self.assertEqual(record["summary"],
                             {"passed": 1, "scored": 1, "states": {"pass": 1}, "retried": 0})
            self.assertEqual(record["filter"], "games")
            self.assertEqual(record["results"], results)


if __name__ == "__main__":
    unittest.main()
