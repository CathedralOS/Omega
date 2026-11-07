"""Explicit migration debt never blesses a wrong result or checker refusal."""
from pathlib import Path
import contextlib
import copy
import io
import json
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import corpus_records
import corpus_gate
import corpus_scoreboard


class MigrationEvidence(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.corpus = self.root / "tests" / "omega"
        self.fixture = "pending/arithmetic/control"
        self.directory = self.corpus / self.fixture
        self.directory.mkdir(parents=True)
        (self.directory / "main.omg").write_text("authored canary")
        (self.directory / "expected_native_exit.txt").write_text("70")
        (self.directory / "expected_stdout.txt").write_text("authored output")
        self.entry = {"fixture": self.fixture, "capability": "scalar-operation",
                      "task": "TERMINAL-CONST-SCALARS", "route": "compile"}
        self.path = self.root / "roster.json"
        self.roster = self.load([self.entry])

    def load(self, entries):
        self.path.write_text(json.dumps(entries))
        return corpus_records.migration_roster(self.path, self.corpus)

    def marker(self, route="compile", task="TERMINAL-CONST-SCALARS"):
        return (f"unimplemented: capability=scalar-operation task={task} "
                f"route={route}: exact operation is not represented")

    def record(self, status="rejected", facts=None, diagnostics=None):
        return {"fixture": self.fixture, "tier": "pending", "status": status,
                "millis": 1, "expected_satisfied": None, "facts": facts or {},
                "diagnostics": diagnostics if diagnostics is not None else [self.marker()],
                "regressed_from": None, "recorded_at": None}

    def classify(self, record, roster=None):
        return corpus_records.classify_migration([record], roster or self.roster)[0]

    def test_exact_refusal_reports_task_without_changing_oracles(self):
        record = self.classify(self.record())
        self.assertEqual(record["status"], "unimplemented")
        self.assertEqual(record["facts"]["migration-task"], "TERMINAL-CONST-SCALARS")
        self.assertEqual(corpus_gate.native_exit_failures([record], self.corpus, "windows_x86_64"), [])
        self.assertEqual(corpus_gate.interpreter_exit_failures([record], self.corpus, "windows_x86_64"), [])
        self.assertEqual(corpus_gate.execution_output_failures([record], self.corpus, "native"), [])
        self.assertEqual((self.directory / "expected_native_exit.txt").read_text(), "70")
        self.assertEqual((self.directory / "expected_stdout.txt").read_text(), "authored output")
        self.assertEqual(corpus_records.parse(corpus_records.render([record])), [record])
        board = corpus_scoreboard.native_scoreboard([record], lambda _: "70")
        self.assertEqual((board["unimplemented"], board["authored"], board["rejected"]), (1, 0, 0))

    def test_roster_does_not_skip_working_or_wrong_results(self):
        for value, fails in (("70", False), ("7", True)):
            record = self.classify(self.record("built", {"exit": value, "stdout": "match",
                                                        "interpreter-exit": value,
                                                        "interpreter-stdout": "match"}, []))
            self.assertIsNone(corpus_records.migration_gap_route(record))
            self.assertEqual(bool(corpus_gate.native_exit_failures([record], self.corpus, "windows_x86_64")), fails)
            self.assertEqual(bool(corpus_gate.interpreter_exit_failures([record], self.corpus, "windows_x86_64")), fails)

    def test_mixed_checker_error_timeout_crash_and_partial_execution_are_not_gaps(self):
        for record in (self.record(diagnostics=[self.marker(), "cannot prove divisor != 0"]),
                       self.record("timeout"), self.record("crashed"),
                       self.record(facts={"exit": "70"}),
                       self.record(facts={"interpreter-exit": "70"}),
                       self.record(diagnostics=["unsupported operation"]),
                       self.record(diagnostics=[self.marker(task="OTHER-TASK")])):
            with self.subTest(record=record):
                self.classify(record)
                self.assertIsNone(corpus_records.migration_gap_route(record))
                self.assertTrue(corpus_gate.native_exit_failures([record], self.corpus, "windows_x86_64"))

    def test_interpreter_gap_does_not_exempt_native_exit_or_stdout(self):
        entry = dict(self.entry, route="interpreter")
        roster = self.load([entry])
        record = self.classify(self.record("built", {"exit": "7", "stdout": "differs"},
                                          [self.marker("interpreter")]), roster)
        self.assertEqual(corpus_records.migration_gap_route(record), "interpreter")
        self.assertTrue(corpus_gate.native_exit_failures([record], self.corpus, "windows_x86_64"))
        self.assertTrue(corpus_gate.execution_output_failures([record], self.corpus, "native"))
        self.assertEqual(corpus_gate.interpreter_exit_failures([record], self.corpus, "windows_x86_64"), [])
        self.assertEqual(corpus_gate.execution_output_failures([record], self.corpus, "interpreter"), [])

    def test_fail_fixture_cannot_be_designated_or_classified(self):
        fail = self.record()
        fail.update(fixture="fail/arithmetic/control", tier="fail")
        self.classify(fail)
        self.assertEqual(fail["status"], "rejected")
        with self.assertRaises(ValueError):
            self.load([dict(self.entry, fixture="fail/arithmetic/control")])

    def test_spoofed_gap_facts_fail_closed(self):
        for record in (self.record("unimplemented", diagnostics=[]),
                       self.record("checked", {"interpreter": "unimplemented"}, [])):
            with self.assertRaises(ValueError):
                self.classify(record)

    def test_roster_requires_existing_canary_oracle_and_unique_route(self):
        for entries in ([self.entry, self.entry], [dict(self.entry, fixture="../outside")],
                        [dict(self.entry, capability="made up")],
                        [dict(self.entry, fixture="pending/missing")]):
            with self.assertRaises(ValueError):
                self.load(entries)
        (self.directory / "expected_native_exit.txt").unlink()
        (self.directory / "expected_stdout.txt").unlink()
        with self.assertRaises(ValueError):
            self.load([self.entry])

    def test_roster_path_identity_is_validated_before_filesystem_access(self):
        for fixture in ("pending/group/name\npending/other/canary",
                        "pending/group/ name", " pending/group/name",
                        "pending/group/name\t", "pending/group/name\r",
                        "pending/group/name\x00", "pending/group/name\x80",
                        "pending/group/name\u00a0", "pending/group/name\u2003",
                        "pending/group:name", "pending/group\\name",
                        "pending/group", "pending//name", "pending/./name",
                        "pending/group/../name", "pending/group/name/"):
            with self.subTest(fixture=fixture), \
                 patch.object(Path, "is_file", side_effect=AssertionError("path reached filesystem")):
                with self.assertRaisesRegex(ValueError, "invalid migration fixture path"):
                    self.load([dict(self.entry, fixture=fixture)])

    def test_existing_route_failure_cannot_be_relabelled(self):
        roster = self.load([dict(self.entry, route="interpreter")])
        for outcome in ("declined", "unchecked", "disagrees", "trapped"):
            record = self.record("built", {"interpreter": outcome}, [self.marker("interpreter")])
            self.classify(record, roster)
            self.assertIsNone(corpus_records.migration_gap_route(record))
        record = self.record("built", {"interpreter-signal": "11"}, [self.marker("interpreter")])
        self.classify(record, roster)
        self.assertIsNone(corpus_records.migration_gap_route(record))

    def test_native_gap_preserves_interpreter_oracle(self):
        roster = self.load([dict(self.entry, route="native")])
        for value, fails in (("70", False), ("7", True)):
            record = self.classify(self.record("rejected", {"interpreter-exit": value,
                "interpreter-stdout": "match"}, [self.marker("native")]), roster)
            self.assertEqual(corpus_records.migration_gap_route(record), "native")
            self.assertEqual(bool(corpus_gate.interpreter_exit_failures(
                [record], self.corpus, "windows_x86_64")), fails)
            self.assertEqual(corpus_gate.execution_output_failures(
                [record], self.corpus, "interpreter"), [])

    def test_missing_pending_canary_requires_runner_hook(self):
        self.assertEqual(corpus_records.missing_migration_canaries([], self.roster), [self.fixture])
        self.assertEqual(corpus_records.missing_migration_canaries([], self.roster, "run/other"), [])
        self.assertEqual(corpus_records.missing_migration_canaries([], self.roster, "control"), [self.fixture])
        shards = [corpus_records.missing_migration_canaries([], self.roster, "", f"{index}/4")
                  for index in range(4)]
        self.assertEqual(sum(bool(rows) for rows in shards), 1)
        for shard in ("0/18446744073709551616", "0/1_000", "0/\u0664", "-1/4", "4/4", "bad"):
            self.assertEqual(corpus_records.missing_migration_canaries(
                [], self.roster, "", shard), [self.fixture])

    def test_absent_roster_record_fails_gate_without_blessing_golden(self):
        record = self.record("built", {"exit": "70"}, [])
        record.update(fixture="run/unrelated", tier="run")
        self.assertEqual(self.run_gate(record, [])[0], 4)
        self.assertEqual(self.run_gate(record, ["--filter", "run/unrelated"])[0], 0)

    def test_malformed_gap_cannot_be_counted_as_authored(self):
        record = self.classify(self.record())
        record["facts"]["exit"] = "70"
        board = corpus_scoreboard.native_scoreboard([record], lambda _: "70")
        self.assertEqual((board["authored"], board["executed"]), (0, 0))
        self.assertEqual(board["malformed_migration"], [self.fixture])

    def test_interpreter_trap_requires_typed_observation(self):
        (self.directory / "expected_native_exit.txt").write_text("trap")
        for target in ("windows_x86_64", "linux_x86_64", "macos_arm64"):
            self.assertEqual(corpus_gate.interpreter_exit_failures(
                [self.record("checked", {"interpreter": "trapped"}, [])], self.corpus, target), [])
        for facts in ({"interpreter": "declined"}, {"interpreter-exit": "-1073741795"},
                      {"interpreter": "trapped", "interpreter-exit": "70"},
                      {"interpreter": "trapped", "interpreter-signal": "11"},
                      {"interpreter": "exhausted"}, {"interpreter": "error"},
                      {"interpreter": "aborted"}):
            self.assertTrue(corpus_gate.interpreter_exit_failures(
                [self.record("checked", facts, [])], self.corpus, "windows_x86_64"))

    def test_subset_record_does_not_call_persisted_debt_complete(self):
        gap = self.classify(self.record())
        green = self.record("built", {"exit": "70"}, [])
        green.update(fixture="run/other", tier="run")
        golden = self.root / "golden.txt"
        golden.write_text(corpus_records.render([gap, green]))
        self.load([self.entry])
        with patch.object(corpus_gate, "ROOT", self.root), \
             patch.object(corpus_gate, "run_runner", return_value=[green]), \
             patch.object(corpus_gate, "host_target", return_value="windows_x86_64"), \
             patch.dict(corpus_gate.os.environ, {}, clear=True), \
             patch.object(sys, "argv", ["corpus_gate", "--runner", "unused", "--record",
                                       "--filter", "run/other", "--golden", str(golden),
                                       "--migration-roster", str(self.path)]), \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(corpus_gate.main(), 1)
        self.assertEqual(corpus_records.read(golden)[0]["status"], "unimplemented")

    def test_interpreter_host_specific_and_malformed_exit_oracles(self):
        record = self.record("checked", {"interpreter-exit": "70"}, [])
        (self.directory / "expected_native_exit.windows_x86_64.txt").write_text("7")
        self.assertTrue(corpus_gate.interpreter_exit_failures([record], self.corpus, "windows_x86_64"))
        self.assertEqual(corpus_gate.interpreter_exit_failures([record], self.corpus, "linux_x86_64"), [])
        for value in ("70\n7", "2147483648", "", "made up"):
            (self.directory / "expected_native_exit.txt").write_text(value)
            self.assertTrue(corpus_gate.interpreter_exit_failures([record], self.corpus, "linux_x86_64"))

    def test_reviewed_pending_paths_reach_runner_once(self):
        self.load([self.entry, dict(self.entry, route="interpreter")])
        record = self.record("built", {"exit": "70", "stdout": "match"}, [])
        golden = self.root / "golden.txt"
        golden.write_text(corpus_records.render([record]))

        def run_selected(_binary, *, freshly_built):
            self.assertFalse(freshly_built)
            self.assertEqual(corpus_gate.os.environ["OMEGA_CORPUS_MIGRATION_FIXTURES"], self.fixture)
            return [record]

        with patch.object(corpus_gate, "ROOT", self.root), \
             patch.object(corpus_gate, "run_runner", side_effect=run_selected), \
             patch.object(corpus_gate, "host_target", return_value="windows_x86_64"), \
             patch.dict(corpus_gate.os.environ, {}, clear=True), \
             patch.object(sys, "argv", ["corpus_gate", "--runner", "unused", "--native",
                                       "--golden", str(golden), "--migration-roster", str(self.path)]), \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(corpus_gate.main(), 0)

    def test_invalid_roster_fails_before_runner_or_build(self):
        for contents in ("not JSON", "{}", json.dumps([dict(self.entry, fixture="pending/missing/canary")])):
            self.path.write_text(contents)
            with patch.object(corpus_gate, "ROOT", self.root), \
                 patch.object(corpus_gate, "run_runner") as runner, \
                 patch.object(corpus_gate, "build_runner") as build, \
                 patch.dict(corpus_gate.os.environ, {}, clear=True), \
                 patch.object(sys, "argv", ["corpus_gate", "--migration-roster", str(self.path)]), \
                 contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(corpus_gate.main(), 4)
                runner.assert_not_called()
                build.assert_not_called()

    def test_unreviewed_pending_environment_is_cleared(self):
        record = self.record("built", {"exit": "70"}, [])
        golden = self.root / "golden.txt"
        golden.write_text(corpus_records.render([record]))

        def run_selected(_binary, *, freshly_built):
            self.assertFalse(freshly_built)
            self.assertEqual(corpus_gate.os.environ["OMEGA_CORPUS_MIGRATION_FIXTURES"], "")
            return [record]

        with patch.object(corpus_gate, "ROOT", self.root), \
             patch.object(corpus_gate, "run_runner", side_effect=run_selected), \
             patch.object(corpus_gate, "host_target", return_value="windows_x86_64"), \
             patch.dict(corpus_gate.os.environ, {"OMEGA_CORPUS_MIGRATION_FIXTURES": "pending/unreviewed"}, clear=True), \
             patch.object(sys, "argv", ["corpus_gate", "--runner", "unused", "--native", "--golden", str(golden)]), \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(corpus_gate.main(), 3)  # Authored stdout remains mandatory.

    def run_gate(self, record, extra):
        golden = self.root / "golden.txt"
        golden.write_text(corpus_records.render([record]))
        self.load([self.entry])
        with patch.object(corpus_gate, "ROOT", self.root), \
             patch.object(corpus_gate, "run_runner", return_value=[copy.deepcopy(record)]), \
             patch.object(corpus_gate, "host_target", return_value="windows_x86_64"), \
             patch.dict(corpus_gate.os.environ, {}, clear=True), \
             patch.object(sys, "argv", ["corpus_gate", "--runner", "unused", "--native",
                                       "--golden", str(golden), "--migration-roster", str(self.path)] + extra), \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            code = corpus_gate.main()
        return code, golden

    def test_record_can_store_debt_but_returns_incomplete(self):
        code, golden = self.run_gate(self.record(), ["--record"])
        self.assertEqual(code, 1)
        self.assertEqual(corpus_records.read(golden)[0]["status"], "unimplemented")

    def test_matching_debt_golden_is_not_clean_coverage(self):
        self.assertEqual(self.run_gate(self.classify(self.record()), [])[0], 1)

    def test_record_still_refuses_wrong_exit(self):
        code, golden = self.run_gate(self.record("built", {"exit": "7", "stdout": "match"}, []), ["--record"])
        self.assertEqual(code, 3)
        self.assertNotIn("unimplemented", golden.read_text())

    def test_interpreter_exit_uses_authored_host_presentation(self):
        (self.directory / "expected_native_exit.txt").write_text("212")
        record = self.record("checked", {"interpreter-exit": "-44"}, [])
        self.assertEqual(corpus_gate.interpreter_exit_failures([record], self.corpus, "linux_x86_64"), [])
        self.assertTrue(corpus_gate.interpreter_exit_failures([record], self.corpus, "windows_x86_64"))


if __name__ == "__main__":
    unittest.main()
