#!/usr/bin/env python3
"""Guard the release-record substrate: the gate manifest must stay
verbatim-equal to the completion contract's command blocks, the validator
must reject records that read closed without every required row, a
timed-out, unavailable or limited command must leave its gate open, the
four-lane join must reject mixed or misattributed records and close only a
complete same-commit set, and committed records must re-validate.

Runs locally only; no GitHub access or non-standard Python packages:

    python3 tools/tests/test_release_record.py -v
"""

import copy
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
TOOL = ROOT / "tools" / "release" / "release_record.py"
CONTRACT = ROOT / "wiki" / "drafts" / "reference" / "rust_compiler_completion.md"
RECORDS = ROOT / "tools" / "release" / "records"


def load_tool():
    specification = importlib.util.spec_from_file_location(
        "release_record_under_test", TOOL)
    module = importlib.util.module_from_spec(specification)
    previous = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        specification.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


release_record = load_tool()


def passing_output(command):
    """The summary each kind of gate command prints after a complete run."""
    if "nextest" in command:
        return "     Summary [   1.000s] 5 tests run: 5 passed, 0 skipped\n"
    if "--doc" in command:
        return ("test result: ok. 2 passed; 0 failed; 0 ignored; "
                "0 measured; 0 filtered out; finished in 0.10s\n")
    if "corpus_gate.py" in command:
        return "corpus_gate: clean — 10 fixtures ({'checked': 10})\n"
    if "refresh-samples" in command:
        return "3 of 3 samples built across 4 threads\n"
    return ""


def minimal_command_result(command, exit_code=0):
    return {
        "command": command,
        "argv": release_record.shlex.split(command),
        "exit": exit_code,
        "elapsed_ms": 1.0,
        "skipped_tests": 0,
        "skipped_names": [],
        "output_tail": passing_output(command),
    }


def minimal_observation(exit_code=0):
    """A passing direct-execution observation on a lane's own row."""
    return {
        "command": "probe --emit-and-run",
        "argv": ["probe", "--emit-and-run"],
        "exit": exit_code,
        "elapsed_ms": 1.0,
        "skipped_tests": 0,
        "skipped_names": [],
        "output_tail": "",
    }


def minimal_record():
    gates = [{
        "name": name,
        "capability": gate["capability"],
        "status": "pass",
        "commands": [minimal_command_result(command)
                     for command in gate["commands"]],
    } for name, gate in release_record.GATES.items()]
    return {
        "schema": release_record.SCHEMA,
        "recorded_utc": "2026-09-20T00:00:00Z",
        "commit": "0" * 40,
        "toolchain": {
            "channel": "nightly-2026-09-01",
            "rustc": "rustc 1.100.0-nightly",
            "runner": "mbx",
        },
        "host": {"os": "linux", "machine": "x86_64", "python": "3.10.0"},
        "target": "linux_x86_64",
        "emulator": None,
        "worktree_clean": True,
        "environment_filters": {},
        "gates": gates,
        "platform_runs": [{
            "runner": row["runner"],
            "target": row["target"],
            "status": ("recorded" if row["target"] == "linux_x86_64"
                       else "open"),
            "emulator": None,
            "observation": (minimal_observation()
                            if row["target"] == "linux_x86_64" else None),
        } for row in release_record.PLATFORM_RUNNERS],
        "expected_skips": [],
        "unlisted_skips": [],
        "closure": {
            "status": "open",
            "open_rows": ["linux_arm64", "macos_arm64", "windows_x86_64"],
        },
    }


class ManifestPinsContract(unittest.TestCase):
    def test_every_gate_command_appears_in_the_contract(self):
        contract = CONTRACT.read_text(encoding="utf-8")
        normalized = " ".join(contract.split())
        for name, gate in release_record.GATES.items():
            for command in gate["commands"]:
                if command.startswith("mbx test --doc"):
                    # The contract composes this leg: "also run the same
                    # package selection with `mbx test --doc`."
                    self.assertIn("mbx test --doc", normalized)
                    nextest = gate["commands"][0]
                    def packages(text):
                        argv = release_record.shlex.split(text)
                        return [argv[i + 1] for i, token in enumerate(argv)
                                if token == "-p"]
                    self.assertEqual(packages(nextest), packages(command),
                                     "{} doc-test leg must keep the same "
                                     "package selection".format(name))
                    continue
                self.assertIn(
                    " ".join(command.split()), normalized,
                    "{} command drifted from the contract".format(name))

    def test_every_contract_command_appears_in_a_gate(self):
        """The reverse of the inclusion check above.

        Checking only recorder-to-contract cannot catch a command the
        contract REQUIRES and the recorder never runs: a gate that silently
        drops one still reports a pass. Walk the contract's own command
        sources -- the fenced gate blocks and the backticked commands in the
        gate table -- and require each to appear in some recorded command.
        """
        contract = CONTRACT.read_text(encoding="utf-8")
        required = set()
        fenced = False
        for line in contract.splitlines():
            stripped = line.strip()
            if stripped.startswith("```"):
                fenced = not fenced
                continue
            if fenced:
                if stripped.startswith(("mbx ", "python tools/")):
                    required.add(" ".join(stripped.split()))
                continue
            for span in re.findall(r"`([^`]+)`", line):
                if span.startswith(("mbx ", "python tools/")):
                    required.add(" ".join(span.split()))
        self.assertTrue(required, "the contract names no commands")

        recorded = [
            " ".join(command.split())
            for gate in release_record.GATES.values()
            for command in gate["commands"]
        ]
        for command in sorted(required):
            self.assertTrue(
                any(command in candidate for candidate in recorded),
                "the contract requires `{}` but no gate records it".format(
                    command),
            )

    def test_six_named_gates(self):
        self.assertEqual(
            ["RC-REPOSITORY", "RC-SOURCE-SEMANTICS", "RC-BUILD-AND-PACKAGES",
             "RC-NATIVE-MATRIX", "RC-DIAGNOSTICS",
             "RC-REPRESENTATIVE-PROGRAMS"],
            list(release_record.GATES))

    def test_four_required_platform_runs(self):
        self.assertEqual(
            ["linux_x86_64", "linux_arm64", "macos_arm64", "windows_x86_64"],
            [row["target"] for row in release_record.PLATFORM_RUNNERS])


class Validator(unittest.TestCase):
    def assert_invalid(self, record, fragment):
        with self.assertRaises(ValueError) as caught:
            release_record.validate_record(record, "fixture.json")
        self.assertIn(fragment, str(caught.exception))

    def test_minimal_open_record_is_valid(self):
        release_record.validate_record(minimal_record(), "fixture.json")

    def test_closed_record_is_valid(self):
        # A single lane's record can only evidence its own row, so a closed
        # record fixture marks the other lanes recorded as if their own
        # records produced them — each still needs a passing observation.
        record = minimal_record()
        for row in record["platform_runs"]:
            row["status"] = "recorded"
            row["observation"] = minimal_observation()
        record["closure"] = {"status": "closed", "open_rows": []}
        self.assert_invalid(record, "another lane")

    def test_rejects_recorded_row_without_observation(self):
        record = minimal_record()
        record["platform_runs"][0]["observation"] = None
        self.assert_invalid(record, "observation")

    def test_rejects_recorded_row_with_failed_observation(self):
        record = minimal_record()
        record["platform_runs"][0]["observation"] = minimal_observation(
            exit_code=65)
        self.assert_invalid(record, "observation")

    def test_rejects_recorded_row_on_unmatched_host(self):
        # darwin/arm64 cannot record the linux_x86_64 lane; the recorded
        # host must be able to execute the lane's emitted programs.
        record = minimal_record()
        record["host"] = {"os": "darwin", "machine": "arm64",
                          "python": "3.10.0"}
        self.assert_invalid(record, "cannot execute lane")

    def test_linux_arm64_emulator_lane_is_valid(self):
        # The contract's named-emulator lane: a non-aarch64 host records
        # linux_arm64 only when the record names the emulator and carries
        # the passed observation.
        record = minimal_record()
        record["target"] = "linux_arm64"
        record["emulator"] = "qemu-aarch64 9.0.0"
        row = record["platform_runs"][1]
        row["status"] = "recorded"
        row["emulator"] = "qemu-aarch64 9.0.0"
        row["observation"] = minimal_observation()
        record["platform_runs"][0]["status"] = "open"
        record["platform_runs"][0]["observation"] = None
        record["closure"] = {"status": "open",
                             "open_rows": ["linux_x86_64", "macos_arm64",
                                           "windows_x86_64"]}
        release_record.validate_record(record, "fixture.json")

    def test_linux_arm64_without_emulator_needs_matching_host(self):
        record = minimal_record()
        record["target"] = "linux_arm64"
        row = record["platform_runs"][1]
        row["status"] = "recorded"
        row["observation"] = minimal_observation()
        record["platform_runs"][0]["status"] = "open"
        record["platform_runs"][0]["observation"] = None
        record["closure"] = {"status": "open",
                             "open_rows": ["linux_x86_64", "macos_arm64",
                                           "windows_x86_64"]}
        # linux/x86_64 host has no emulator named: not the lane's host.
        self.assert_invalid(record, "cannot execute lane")

    def test_rejects_schema_drift(self):
        record = minimal_record()
        record["schema"] = "omega-release-record/1"
        self.assert_invalid(record, "schema")

    def test_rejects_non_contract_command(self):
        record = minimal_record()
        record["gates"][0]["commands"][0]["command"] = "cargo fmt --all"
        self.assert_invalid(record, "drifted")

    def test_rejects_stale_expected_skip(self):
        record = minimal_record()
        record["gates"][0]["commands"][0]["skipped_tests"] = 1
        record["gates"][0]["commands"][0]["skipped_names"] = ["pkg::other"]
        record["expected_skips"] = [{
            "gate": "RC-REPOSITORY", "test": "pkg::other",
            "reason": "irrelevant here"}]
        # declared and observed agree
        release_record.validate_record(record, "fixture.json")
        stale = copy.deepcopy(record)
        stale["expected_skips"][0]["test"] = "pkg::not_skipped"
        self.assert_invalid(stale, "not observed")

    def test_rejects_unlisted_skips(self):
        record = minimal_record()
        record["unlisted_skips"] = ["pkg::skipped_test"]
        self.assert_invalid(record, "expected_skips")

    def test_rejects_emulator_off_linux_arm64(self):
        record = minimal_record()
        record["emulator"] = "qemu 9.0"
        record["platform_runs"][0]["emulator"] = "qemu 9.0"
        self.assert_invalid(record, "emulator")

    def test_rejects_closure_overclaim(self):
        record = minimal_record()
        record["closure"] = {"status": "closed", "open_rows": []}
        self.assert_invalid(record, "closure")

    def test_rejects_failed_gate_reading_closed(self):
        record = minimal_record()
        record["gates"][2]["status"] = "fail"
        record["closure"] = {"status": "closed", "open_rows": []}
        self.assert_invalid(record, "closure")

    def test_rejects_pass_over_nonzero_exit(self):
        record = minimal_record()
        record["gates"][0]["commands"][1]["exit"] = 101
        self.assert_invalid(record, "RC-REPOSITORY status pass contradicts")

    def test_failed_gate_is_valid_when_listed_open(self):
        record = minimal_record()
        record["gates"][0]["commands"][1]["exit"] = 101
        record["gates"][0]["status"] = "fail"
        record["closure"]["open_rows"].insert(0, "RC-REPOSITORY")
        release_record.validate_record(record, "fixture.json")

    def test_rejects_empty_open_rows_beside_open_lanes(self):
        record = minimal_record()
        record["closure"]["open_rows"] = []
        self.assert_invalid(record, "open_rows")

    def test_rejects_unnamed_skip_count(self):
        record = minimal_record()
        record["gates"][0]["commands"][3]["skipped_tests"] = 3
        self.assert_invalid(record, "RC-REPOSITORY status pass contradicts")
        record["gates"][0]["status"] = "open"
        record["closure"]["open_rows"].insert(0, "RC-REPOSITORY")
        release_record.validate_record(record, "fixture.json")

    def test_rejects_empty_test_run(self):
        record = minimal_record()
        record["gates"][2]["commands"][0]["output_tail"] = (
            "     Summary [   0.001s] 0 tests run: 0 passed, 0 skipped\n")
        self.assert_invalid(
            record, "RC-BUILD-AND-PACKAGES status pass contradicts")

    def test_rejects_interrupted_run_without_summary(self):
        record = minimal_record()
        record["gates"][1]["commands"][0]["output_tail"] = (
            "corpus_runner: 400/1147 done, 11 compile(s) in flight\n")
        self.assert_invalid(
            record, "RC-SOURCE-SEMANTICS status pass contradicts")

    def test_rejects_filtered_doc_tests(self):
        record = minimal_record()
        record["gates"][2]["commands"][1]["output_tail"] = (
            "test result: ok. 2 passed; 0 failed; 0 ignored; "
            "0 measured; 4 filtered out; finished in 0.10s\n")
        self.assert_invalid(
            record, "RC-BUILD-AND-PACKAGES status pass contradicts")

    def test_rejects_pass_under_filtering_environment(self):
        record = minimal_record()
        record["environment_filters"] = {
            "OMEGA_CORPUS_FIXTURE_FILTER": "fail/"}
        self.assert_invalid(record, "status pass contradicts")
        for gate in record["gates"]:
            gate["status"] = "open"
        record["closure"]["open_rows"] = (
            list(release_record.GATES) + record["closure"]["open_rows"])
        release_record.validate_record(record, "fixture.json")

    def test_rejects_pass_from_a_dirty_checkout(self):
        record = minimal_record()
        record["worktree_clean"] = False
        self.assert_invalid(record, "status pass contradicts")

    def test_rejects_record_without_run_evidence(self):
        record = minimal_record()
        del record["environment_filters"]
        self.assert_invalid(record, "environment_filters")

    def test_rejects_lane_recorded_from_empty_observation(self):
        record = minimal_record()
        observation = record["platform_runs"][0]["observation"]
        observation["command"] = "python3 tools/corpus_gate.py --native"
        observation["argv"] = ["python3", "tools/corpus_gate.py", "--native"]
        self.assert_invalid(record, "complete direct-execution observation")
        observation["output_tail"] = passing_output(observation["command"])
        release_record.validate_record(record, "fixture.json")


class SkipAccounting(unittest.TestCase):
    def test_parse_expected_skip(self):
        entry = release_record.parse_expected_skip(
            "RC-NATIVE-MATRIX|native::uefi|no UEFI host")
        self.assertEqual(
            {"gate": "RC-NATIVE-MATRIX", "test": "native::uefi",
             "reason": "no UEFI host"}, entry)

    def test_parse_expected_skip_rejects_malformed(self):
        with self.assertRaises(ValueError):
            release_record.parse_expected_skip("no-separators")

    def test_parse_expected_skip_rejects_unknown_gate(self):
        with self.assertRaises(ValueError):
            release_record.parse_expected_skip(
                "RC-UNKNOWN|test::x|reason")

    def test_run_command_counts_nextest_skips(self):
        # The exact line shapes nextest 0.9.140 prints at `skip` level, with
        # and without the progress counter column.
        result = release_record.run_command(
            "python3 -c 'print(\"        SKIP [         ] (\\u2500\\u2500\\u2500) "
            "pkg::suite a::one\"); print(\"        SKIP [         ] "
            "pkg::suite a::two\"); print(\"     Summary [   1.000s] 1 test "
            "run: 1 passed, 2 skipped\")'",
            sys.executable)
        self.assertEqual(0, result["exit"])
        self.assertEqual(2, result["skipped_tests"])
        self.assertEqual(["pkg::suite a::one", "pkg::suite a::two"],
                         result["skipped_names"])
        self.assertTrue(release_record.command_evidence_complete(result))

    def test_run_command_keeps_an_unnamed_skip_count(self):
        # A summary count without per-test lines stays a count: the record
        # then cannot pass the gate rather than inventing a name.
        result = release_record.run_command(
            "python3 -c 'print(\"Summary 4 tests run: 4 passed, "
            "2 skipped\")'", sys.executable)
        self.assertEqual(2, result["skipped_tests"])
        self.assertEqual([], result["skipped_names"])
        self.assertFalse(release_record.command_evidence_complete(result))

    def test_run_command_names_ignored_doc_tests(self):
        result = release_record.run_command(
            "python3 -c 'print(\"test src/lib.rs - f (line 3) ... "
            "ignored\"); print(\"test result: ok. 1 passed; 0 failed; "
            "1 ignored; 0 measured; 0 filtered out\")'", sys.executable)
        self.assertEqual(1, result["skipped_tests"])
        self.assertEqual(["src/lib.rs - f (line 3)"], result["skipped_names"])

    def test_run_command_asks_nextest_to_name_skips(self):
        result = release_record.run_command(
            "python3 -c 'import os; "
            "print(os.environ[\"NEXTEST_STATUS_LEVEL\"])'", sys.executable)
        self.assertEqual("skip", result["output_tail"].strip())

    def test_environment_filters_name_only_narrowing_variables(self):
        self.assertEqual(
            {"OMEGA_CORPUS_SHARD": "1/4"},
            release_record.environment_filters({
                "OMEGA_CORPUS_SHARD": "1/4",
                "OMEGA_CORPUS_FIXTURE_FILTER": "  ",
                "OMEGA_CORPUS_JOBS": "8",
            }))


class ExecutedUnits(unittest.TestCase):
    def units(self, command, output):
        return release_record.executed_units(
            release_record.shlex.split(command), output)

    def test_compile_only_commands_execute_nothing(self):
        self.assertIsNone(self.units("mbx check --workspace", "Finished"))

    def test_each_gate_command_kind_reports_its_units(self):
        for name, gate in release_record.GATES.items():
            for command in gate["commands"]:
                expected = self.units(command, passing_output(command))
                if passing_output(command):
                    self.assertGreater(expected, 0, command)
                else:
                    self.assertIsNone(expected, command)

    def test_missing_summary_reads_as_zero(self):
        self.assertEqual(0, self.units("python tools/corpus_gate.py", ""))
        self.assertEqual(0, self.units("mbx nextest run -p x", "error"))


class CommandResolution(unittest.TestCase):
    def test_mbx_commands_resolve_to_the_selected_runner(self):
        self.assertEqual(
            ["cargo", "nextest", "run"],
            release_record.resolve_command("mbx nextest run", "cargo"))

    def test_the_formatting_leg_is_never_rewritten_to_the_runner(self):
        # The contract's formatting leg is `python tools/fmt.py --check`, the
        # portable route, not a Cargo subcommand: an installed `mbx` must not
        # capture it.
        self.assertEqual(
            [sys.executable, "tools/fmt.py", "--check"],
            release_record.resolve_command(
                "python tools/fmt.py --check", "mbx"))

    def test_the_formatting_leg_is_available_without_any_runner(self):
        self.assertIsNone(release_record.command_unavailable(
            "python tools/fmt.py --check", None))

    def test_mbx_command_is_unavailable_without_a_runner(self):
        self.assertEqual(
            "mbx and Cargo unavailable",
            release_record.command_unavailable("mbx check --workspace", None))

    def test_nextest_command_is_unavailable_without_cargo_nextest(self):
        with mock.patch.object(release_record, "nextest_available",
                               return_value=False):
            self.assertEqual(
                "cargo-nextest unavailable",
                release_record.command_unavailable(
                    "mbx nextest run --workspace --lib", "cargo"))
            self.assertIsNone(release_record.command_unavailable(
                "mbx check --workspace --all-targets", "cargo"))

    def test_every_contract_command_resolves_to_an_argument_vector(self):
        for name, gate in release_record.GATES.items():
            for command in gate["commands"]:
                argv = release_record.resolve_command(command, "cargo")
                self.assertTrue(argv, "{} empty command".format(name))
                self.assertTrue(all(isinstance(part, str) for part in argv))
                self.assertNotIn(argv[0], ("mbx", "python"), command)


class HostLanes(unittest.TestCase):
    def test_each_required_host_maps_to_its_lane(self):
        cases = [("Linux", "x86_64", "linux_x86_64"),
                 ("Linux", "aarch64", "linux_arm64"),
                 ("Darwin", "arm64", "macos_arm64"),
                 ("Windows", "AMD64", "windows_x86_64")]
        for os_name, machine, lane in cases:
            with self.subTest(host=(os_name, machine)):
                self.assertEqual(
                    [lane], release_record.host_lanes(os_name, machine))

    def test_unlisted_host_records_no_lane(self):
        self.assertEqual([], release_record.host_lanes("FreeBSD", "amd64"))
        self.assertFalse(release_record.lane_host_match(
            "linux_x86_64", os_name="FreeBSD", machine="amd64"))

    def test_only_linux_arm64_accepts_a_named_emulator(self):
        self.assertTrue(release_record.lane_host_match(
            "linux_arm64", emulator="qemu-aarch64 9.0.0",
            os_name="linux", machine="x86_64"))
        self.assertFalse(release_record.lane_host_match(
            "windows_x86_64", emulator="wine 9.0",
            os_name="linux", machine="x86_64"))


class GateExecution(unittest.TestCase):
    """`run_gate_commands` against real processes on this interpreter."""

    def run_gate(self, commands, **options):
        with tempfile.TemporaryDirectory() as scratch, \
                mock.patch("builtins.print"):
            rows = release_record.run_gate_commands(
                "RC-TEST", commands, "cargo", log_dir=Path(scratch),
                **options)
            logs = sorted(path.name for path in Path(scratch).iterdir())
        return rows, logs

    def status(self, rows):
        return release_record.gate_status(rows, run_complete=True)

    def test_all_zero_exits_pass_and_keep_full_logs(self):
        rows, logs = self.run_gate([
            "python -c pass",
            "python -c \"print('evidence')\"",
        ])
        self.assertEqual([0, 0], [row["exit"] for row in rows])
        self.assertEqual([None, None], [row["incomplete"] for row in rows])
        self.assertEqual(["RC-TEST-0.log", "RC-TEST-1.log"], logs)
        self.assertEqual("evidence", rows[1]["output_tail"].strip())
        self.assertEqual("pass", self.status(rows))

    def test_nonzero_exit_fails_the_gate(self):
        rows, _ = self.run_gate(["python -c \"import sys; sys.exit(3)\""])
        self.assertEqual(3, rows[0]["exit"])
        self.assertEqual("fail", self.status(rows))

    def test_timeout_leaves_the_gate_open(self):
        rows, _ = self.run_gate(
            ["python -c \"import time; time.sleep(30)\""], timeout=0.5)
        self.assertIsNone(rows[0]["exit"])
        self.assertTrue(rows[0]["incomplete"].startswith("timeout"))
        self.assertEqual("open", self.status(rows))

    def test_unavailable_command_is_recorded_not_run(self):
        with mock.patch.object(release_record, "nextest_available",
                               return_value=False):
            rows, logs = self.run_gate(["mbx nextest run -p x"])
        self.assertIsNone(rows[0]["exit"])
        self.assertEqual("cargo-nextest unavailable", rows[0]["incomplete"])
        self.assertEqual([], logs)
        self.assertEqual("open", self.status(rows))

    def test_command_that_cannot_start_leaves_the_gate_open(self):
        rows, _ = self.run_gate(["definitely-not-a-real-release-tool"])
        self.assertIsNone(rows[0]["exit"])
        self.assertTrue(rows[0]["incomplete"].startswith("could not start"))
        self.assertEqual("open", self.status(rows))

    def test_limit_records_the_remaining_commands_not_run(self):
        rows, logs = self.run_gate(
            ["python -c pass", "python -c pass"], limit=1)
        self.assertEqual(2, len(rows), "the contract's command count stays")
        self.assertEqual(0, rows[0]["exit"])
        self.assertEqual("beyond --limit", rows[1]["incomplete"])
        self.assertEqual(["RC-TEST-0.log"], logs)
        self.assertEqual("open", self.status(rows))

    def test_nonzero_exit_outranks_an_incomplete_command(self):
        rows, _ = self.run_gate(
            ["python -c \"import sys; sys.exit(1)\"", "python -c pass"],
            limit=1)
        self.assertEqual("fail", self.status(rows))


class IncompleteCommandRecords(unittest.TestCase):
    def test_timed_out_command_validates_as_an_open_gate(self):
        record = minimal_record()
        row = record["gates"][1]["commands"][0]
        row.update({"exit": None, "incomplete": "timeout after 7200 s",
                    "output_tail": ""})
        with self.assertRaises(ValueError) as caught:
            release_record.validate_record(record, "fixture.json")
        self.assertIn("RC-SOURCE-SEMANTICS status pass contradicts",
                      str(caught.exception))
        record["gates"][1]["status"] = "open"
        record["closure"]["open_rows"].insert(0, "RC-SOURCE-SEMANTICS")
        release_record.validate_record(record, "fixture.json")

    def test_rejects_missing_exit_without_a_reason(self):
        record = minimal_record()
        record["gates"][1]["commands"][0]["exit"] = None
        record["gates"][1]["status"] = "open"
        record["closure"]["open_rows"].insert(0, "RC-SOURCE-SEMANTICS")
        with self.assertRaises(ValueError) as caught:
            release_record.validate_record(record, "fixture.json")
        self.assertIn("exit/elapsed evidence", str(caught.exception))

    def test_rejects_incomplete_reason_beside_an_exit(self):
        record = minimal_record()
        record["gates"][1]["commands"][0]["incomplete"] = "timeout"
        with self.assertRaises(ValueError) as caught:
            release_record.validate_record(record, "fixture.json")
        self.assertIn("exit/elapsed evidence", str(caught.exception))


class ExpectedSkipReconciliation(unittest.TestCase):
    def skipping_record(self):
        record = minimal_record()
        row = record["gates"][2]["commands"][0]
        row["skipped_tests"] = 1
        row["skipped_names"] = ["package-source host::only"]
        return record

    def test_undeclared_skip_keeps_the_gate_open(self):
        record = self.skipping_record()
        record["unlisted_skips"] = ["package-source host::only"]
        with self.assertRaises(ValueError) as caught:
            release_record.validate_record(record, "fixture.json")
        self.assertIn("RC-BUILD-AND-PACKAGES status pass contradicts",
                      str(caught.exception))
        record["gates"][2]["status"] = "open"
        record["closure"]["open_rows"].insert(0, "RC-BUILD-AND-PACKAGES")
        release_record.validate_record(record, "fixture.json")

    def test_skip_declared_for_the_same_gate_passes(self):
        record = self.skipping_record()
        record["expected_skips"] = [{
            "gate": "RC-BUILD-AND-PACKAGES", "test": "package-source host::only",
            "reason": "irrelevant to this runner"}]
        release_record.validate_record(record, "fixture.json")

    def test_skip_declared_for_another_gate_is_unlisted(self):
        record = self.skipping_record()
        record["expected_skips"] = [{
            "gate": "RC-REPOSITORY", "test": "package-source host::only",
            "reason": "irrelevant to this runner"}]
        record["unlisted_skips"] = ["package-source host::only"]
        record["gates"][2]["status"] = "open"
        record["closure"]["open_rows"].insert(0, "RC-BUILD-AND-PACKAGES")
        with self.assertRaises(ValueError) as caught:
            release_record.validate_record(record, "fixture.json")
        # RC-REPOSITORY observed no skip, so its declaration is stale.
        self.assertIn("not observed in RC-REPOSITORY", str(caught.exception))

    def test_rejects_declared_skip_when_the_gate_skipped_nothing(self):
        record = minimal_record()
        record["expected_skips"] = [{
            "gate": "RC-DIAGNOSTICS", "test": "pkg::never_skipped",
            "reason": "irrelevant to this runner"}]
        with self.assertRaises(ValueError) as caught:
            release_record.validate_record(record, "fixture.json")
        self.assertIn("not observed", str(caught.exception))

    def test_rejects_declared_skip_for_a_gate_not_run(self):
        record = minimal_record()
        record["gates"] = [gate for gate in record["gates"]
                           if gate["name"] != "RC-DIAGNOSTICS"]
        record["closure"]["open_rows"].insert(0, "RC-DIAGNOSTICS")
        release_record.validate_record(record, "fixture.json")
        record["expected_skips"] = [{
            "gate": "RC-DIAGNOSTICS", "test": "pkg::x", "reason": "because"}]
        with self.assertRaises(ValueError) as caught:
            release_record.validate_record(record, "fixture.json")
        self.assertIn("did not run", str(caught.exception))

    def test_doc_test_skips_are_named_by_crate_harness(self):
        output = ("   Doc-tests terminal_codec\n\nrunning 2 tests\n"
                  "test src/lib.rs - f (line 5) ... ignored\n"
                  "test src/lib.rs - f (line 2) ... ok\n\n"
                  "test result: ok. 1 passed; 0 failed; 1 ignored; "
                  "0 measured; 0 filtered out; finished in 0.99s\n"
                  "   Doc-tests terminal_verifier\n\nrunning 1 test\n"
                  "test src/lib.rs - f (line 5) ... ignored\n\n"
                  "test result: ok. 0 passed; 0 failed; 1 ignored; "
                  "0 measured; 0 filtered out; finished in 0.50s\n")
        self.assertEqual(
            ["terminal_codec src/lib.rs - f (line 5)",
             "terminal_verifier src/lib.rs - f (line 5)"],
            release_record.skipped_test_names(output))
        self.assertEqual(2, release_record.skipped_test_count(output))


LANE_HOSTS = {
    "linux_x86_64": ("linux", "x86_64"),
    "linux_arm64": ("linux", "aarch64"),
    "macos_arm64": ("darwin", "arm64"),
    "windows_x86_64": ("windows", "amd64"),
}
RELEASE_COMMIT = "a" * 40
PER_HOST_GATES = ["RC-NATIVE-MATRIX", "RC-REPRESENTATIVE-PROGRAMS"]


def lane_record(target, gate_names=None, commit=RELEASE_COMMIT):
    """A valid record of one lane made on that lane's own host.

    Every listed gate passes and the lane's own observation passes; the
    record's own closure lists the gates it did not run and the other three
    lanes, as a single lane's record always must.
    """
    names = list(release_record.GATES) if gate_names is None else gate_names
    record = minimal_record()
    record["gates"] = [gate for gate in record["gates"]
                       if gate["name"] in names]
    record["commit"] = commit
    record["target"] = target
    os_name, machine = LANE_HOSTS[target]
    record["host"] = {"os": os_name, "machine": machine, "python": "3.10.0"}
    for row in record["platform_runs"]:
        own = row["target"] == target
        row["status"] = "recorded" if own else "open"
        row["observation"] = minimal_observation() if own else None
    record["closure"] = {
        "status": "open",
        "open_rows": ([name for name in release_record.GATES
                       if name not in names]
                      + [lane for lane in LANE_HOSTS if lane != target]),
    }
    return record


def fail_gate(record, gate_name):
    gate = next(gate for gate in record["gates"] if gate["name"] == gate_name)
    gate["commands"][0]["exit"] = 101
    gate["status"] = "fail"
    rows = [name for name in release_record.GATES
            if name == gate_name or name in record["closure"]["open_rows"]]
    lanes = [lane for lane in record["closure"]["open_rows"]
             if lane in LANE_HOSTS]
    record["closure"]["open_rows"] = rows + lanes


def entries(records):
    return [("{}.json".format(record["target"]), record, "0" * 64)
            for record in records]


def four_lanes(**overrides):
    return [overrides.get(lane) or lane_record(lane) for lane in LANE_HOSTS]


class Closure(unittest.TestCase):
    def close(self, records, commit=None):
        return release_record.close_records(entries(records), commit=commit)

    def assert_rejected(self, records, fragment, commit=None):
        with self.assertRaises(ValueError) as caught:
            self.close(records, commit=commit)
        self.assertIn(fragment, str(caught.exception))

    def test_four_complete_same_commit_records_close(self):
        closure = self.close(four_lanes())
        self.assertEqual(release_record.CLOSURE_SCHEMA, closure["schema"])
        self.assertEqual("closed", closure["status"])
        self.assertEqual(RELEASE_COMMIT, closure["commit"])
        self.assertEqual({lane: [] for lane in LANE_HOSTS},
                         closure["open_rows"])
        self.assertEqual([], closure["unrun_gates"])
        self.assertEqual(list(LANE_HOSTS),
                         [lane["target"] for lane in closure["lanes"]])
        for gate in closure["gates"]:
            self.assertEqual({lane: "pass" for lane in LANE_HOSTS},
                             gate["hosts"], gate["name"])

    def test_shared_gates_on_one_lane_and_per_host_gates_everywhere_close(self):
        records = four_lanes(**{
            lane: lane_record(lane, PER_HOST_GATES)
            for lane in ("linux_arm64", "macos_arm64", "windows_x86_64")})
        self.assertEqual("closed", self.close(records)["status"])

    def test_per_host_gate_missing_on_a_lane_keeps_closure_open(self):
        records = four_lanes(
            macos_arm64=lane_record("macos_arm64", ["RC-NATIVE-MATRIX"]))
        closure = self.close(records)
        self.assertEqual("open", closure["status"])
        self.assertEqual(["RC-REPRESENTATIVE-PROGRAMS"],
                         closure["open_rows"]["macos_arm64"])
        self.assertEqual([], closure["open_rows"]["linux_x86_64"])

    def test_gate_no_lane_ran_keeps_closure_open(self):
        names = [name for name in release_record.GATES
                 if name != "RC-DIAGNOSTICS"]
        closure = self.close([lane_record(lane, names)
                              for lane in LANE_HOSTS])
        self.assertEqual("open", closure["status"])
        self.assertEqual(["RC-DIAGNOSTICS"], closure["unrun_gates"])
        self.assertEqual({lane: [] for lane in LANE_HOSTS},
                         closure["open_rows"])

    def test_open_gate_keeps_closure_open(self):
        windows = lane_record("windows_x86_64")
        fail_gate(windows, "RC-NATIVE-MATRIX")
        closure = self.close(four_lanes(windows_x86_64=windows))
        self.assertEqual("open", closure["status"])
        self.assertEqual(["RC-NATIVE-MATRIX"],
                         closure["open_rows"]["windows_x86_64"])
        self.assertEqual([], closure["open_rows"]["linux_x86_64"])

    def test_shared_gate_failing_on_any_lane_that_ran_it_stays_open(self):
        windows = lane_record("windows_x86_64")
        fail_gate(windows, "RC-REPOSITORY")
        closure = self.close(four_lanes(windows_x86_64=windows))
        self.assertEqual("open", closure["status"])
        self.assertEqual(["RC-REPOSITORY"],
                         closure["open_rows"]["windows_x86_64"])

    def test_lane_without_a_passing_observation_stays_open(self):
        macos = lane_record("macos_arm64")
        own = next(row for row in macos["platform_runs"]
                   if row["target"] == "macos_arm64")
        own["observation"] = minimal_observation(exit_code=1)
        own["status"] = "open"
        macos["closure"]["open_rows"] = [
            "linux_x86_64", "linux_arm64", "macos_arm64", "windows_x86_64"]
        closure = self.close(four_lanes(macos_arm64=macos))
        self.assertEqual("open", closure["status"])
        self.assertEqual(["macos_arm64"], closure["open_rows"]["macos_arm64"])

    def test_undeclared_skip_keeps_its_lane_open_until_declared(self):
        arm = lane_record("linux_arm64")
        native = next(gate for gate in arm["gates"]
                      if gate["name"] == "RC-NATIVE-MATRIX")
        native["commands"][0]["skipped_tests"] = 1
        native["commands"][0]["skipped_names"] = ["native x86::only"]
        native["status"] = "open"
        arm["unlisted_skips"] = ["native x86::only"]
        arm["closure"]["open_rows"].insert(0, "RC-NATIVE-MATRIX")
        closure = self.close(four_lanes(linux_arm64=arm))
        self.assertEqual(["RC-NATIVE-MATRIX"],
                         closure["open_rows"]["linux_arm64"])

        declared = copy.deepcopy(arm)
        declared["expected_skips"] = [{
            "gate": "RC-NATIVE-MATRIX", "test": "native x86::only",
            "reason": "x86-only encoding probe"}]
        declared["unlisted_skips"] = []
        native = next(gate for gate in declared["gates"]
                      if gate["name"] == "RC-NATIVE-MATRIX")
        native["status"] = "pass"
        declared["closure"]["open_rows"].remove("RC-NATIVE-MATRIX")
        closure = self.close(four_lanes(linux_arm64=declared))
        self.assertEqual("closed", closure["status"])
        lane = next(lane for lane in closure["lanes"]
                    if lane["target"] == "linux_arm64")
        self.assertEqual(declared["expected_skips"], lane["expected_skips"])

    def test_linux_arm64_under_a_named_emulator_joins(self):
        arm = lane_record("linux_arm64")
        arm["host"] = {"os": "linux", "machine": "x86_64", "python": "3.10.0"}
        arm["emulator"] = "qemu-aarch64 9.0.0"
        next(row for row in arm["platform_runs"]
             if row["target"] == "linux_arm64")["emulator"] = arm["emulator"]
        closure = self.close(four_lanes(linux_arm64=arm))
        self.assertEqual("closed", closure["status"])
        lane = next(lane for lane in closure["lanes"]
                    if lane["target"] == "linux_arm64")
        self.assertEqual("qemu-aarch64 9.0.0", lane["emulator"])

    def test_mixed_commits_reject(self):
        records = four_lanes(
            macos_arm64=lane_record("macos_arm64", commit="b" * 40))
        self.assert_rejected(records, "mix commits")

    def test_stale_records_reject_against_the_release_commit(self):
        self.assert_rejected(four_lanes(), "stale", commit="c" * 40)
        self.assertEqual("closed",
                         self.close(four_lanes(),
                                    commit=RELEASE_COMMIT)["status"])

    def test_mixed_toolchains_reject(self):
        windows = lane_record("windows_x86_64")
        windows["toolchain"]["rustc"] = "rustc 1.101.0-nightly"
        self.assert_rejected(four_lanes(windows_x86_64=windows),
                             "mix toolchains")

    def test_missing_lane_rejects(self):
        records = [record for record in four_lanes()
                   if record["target"] != "linux_arm64"]
        self.assert_rejected(records, "lane linux_arm64 has no record")

    def test_duplicate_lane_rejects(self):
        records = four_lanes() + [lane_record("macos_arm64")]
        self.assert_rejected(records, "lane macos_arm64 has 2 records")

    def test_lane_recorded_by_another_lanes_record_rejects(self):
        linux = lane_record("linux_x86_64")
        borrowed = next(row for row in linux["platform_runs"]
                        if row["target"] == "macos_arm64")
        borrowed["status"] = "recorded"
        borrowed["observation"] = minimal_observation()
        linux["closure"]["open_rows"].remove("macos_arm64")
        self.assert_rejected(four_lanes(linux_x86_64=linux), "another lane")

    def test_record_made_on_another_lanes_host_rejects(self):
        # The macos_arm64 record claims its lane from a Linux host. With its
        # own row left open the record is internally valid, but it cannot
        # stand for the macOS lane in the join.
        macos = lane_record("macos_arm64")
        macos["host"] = {"os": "linux", "machine": "x86_64",
                         "python": "3.10.0"}
        own = next(row for row in macos["platform_runs"]
                   if row["target"] == "macos_arm64")
        own["status"] = "open"
        own["observation"] = None
        macos["closure"]["open_rows"] = list(LANE_HOSTS)
        release_record.validate_record(macos, "macos.json")
        self.assert_rejected(four_lanes(macos_arm64=macos),
                             "cannot record lane macos_arm64")

    def test_invalid_record_rejects_the_join(self):
        linux = lane_record("linux_x86_64")
        linux["closure"] = {"status": "closed", "open_rows": []}
        self.assert_rejected(four_lanes(linux_x86_64=linux), "closure")


class CommandLine(unittest.TestCase):
    def invoke(self, *arguments):
        return subprocess.run(
            [sys.executable, str(TOOL), *arguments],
            cwd=ROOT, check=False, capture_output=True, text=True)

    def write_records(self, directory, records):
        paths = []
        for record in records:
            path = Path(directory) / "{}.json".format(record["target"])
            path.write_text(json.dumps(record), encoding="utf-8")
            paths.append(str(path))
        return paths

    def test_plan_lists_every_gate_this_host_and_the_lanes(self):
        result = self.invoke("plan")
        self.assertEqual(0, result.returncode, result.stderr)
        for name in release_record.GATES:
            self.assertIn(name, result.stdout)
        for lane in LANE_HOSTS:
            self.assertIn(lane, result.stdout)
        self.assertIn("this host:", result.stdout)

    def test_unknown_gate_refuses_before_any_work(self):
        result = self.invoke("plan", "--gate", "RC-NOT-A-GATE")
        self.assertEqual(2, result.returncode)

    def test_close_exits_zero_only_for_a_closed_release(self):
        with tempfile.TemporaryDirectory() as scratch:
            paths = self.write_records(scratch, four_lanes())
            output = Path(scratch) / "closure.json"
            result = self.invoke("close", *paths, "--commit", RELEASE_COMMIT,
                                 "--output", str(output))
            self.assertEqual(0, result.returncode, result.stderr)
            closure = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual("closed", closure["status"])
            self.assertTrue(all(len(lane["sha256"]) == 64
                                for lane in closure["lanes"]))

    def test_close_writes_an_open_closure_and_exits_nonzero(self):
        windows = lane_record("windows_x86_64")
        fail_gate(windows, "RC-NATIVE-MATRIX")
        with tempfile.TemporaryDirectory() as scratch:
            paths = self.write_records(
                scratch, four_lanes(windows_x86_64=windows))
            result = self.invoke("close", *paths)
            self.assertEqual(1, result.returncode, result.stderr)
            closure = json.loads(result.stdout)
            self.assertEqual("open", closure["status"])
            self.assertEqual(["RC-NATIVE-MATRIX"],
                             closure["open_rows"]["windows_x86_64"])

    def test_close_rejects_a_missing_lane_without_a_document(self):
        records = [record for record in four_lanes()
                   if record["target"] != "windows_x86_64"]
        with tempfile.TemporaryDirectory() as scratch:
            paths = self.write_records(scratch, records)
            output = Path(scratch) / "closure.json"
            result = self.invoke("close", *paths, "--output", str(output))
            self.assertEqual(1, result.returncode)
            self.assertIn("lane windows_x86_64 has no record", result.stderr)
            self.assertFalse(output.exists())


class CommittedRecords(unittest.TestCase):
    def test_stored_records_revalidate(self):
        if not RECORDS.is_dir():
            self.skipTest("no records committed yet")
        records = sorted(RECORDS.glob("*.json"))
        for path in records:
            release_record.validate_record(
                json.loads(path.read_text(encoding="utf-8")), path)
        self.assertTrue(records, "records/ holds no JSON records")


if __name__ == "__main__":
    unittest.main()
