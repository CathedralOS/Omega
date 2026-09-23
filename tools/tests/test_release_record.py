#!/usr/bin/env python3
"""Guard the release-record substrate: the gate manifest must stay
verbatim-equal to the completion contract's command blocks, the validator
must reject records that read closed without every required row, and
committed records must re-validate.

Runs locally only; no GitHub access or non-standard Python packages:

    python3 tools/tests/test_release_record.py -v
"""

import copy
import importlib.util
import json
from pathlib import Path
import sys
import re
import unittest


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


def minimal_command_result(command, exit_code=0):
    return {
        "command": command,
        "argv": release_record.shlex.split(command),
        "exit": exit_code,
        "elapsed_ms": 1.0,
        "skipped_tests": 0,
        "skipped_names": [],
        "output_tail": "",
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
                if span.startswith("mbx "):
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

    def test_eight_named_gates(self):
        self.assertEqual(
            ["RC-REPOSITORY", "RC-SOURCE-SEMANTICS", "RC-PCC-REPLAY",
             "RC-PORTABLE-PSI", "RC-BUILD-AND-PACKAGES", "RC-NATIVE-MATRIX",
             "RC-DIAGNOSTICS", "RC-REPRESENTATIVE-PROGRAMS"],
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
        record["schema"] = "omega-release-record/2"
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
        result = release_record.run_command(
            "python3 -c 'import sys; print(\"    SKIP [0.001s] pkg::a\"); "
            "print(\"Summary 1 tests run: 0 passed, 1 skipped\")'",
            sys.executable)
        self.assertEqual(0, result["exit"])
        self.assertEqual(1, result["skipped_tests"])
        self.assertEqual(["pkg::a"], result["skipped_names"])


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
