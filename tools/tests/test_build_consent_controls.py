#!/usr/bin/env python3
"""Refuse missing SDK observations and empty or incomplete filesystem runs."""

import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from tools.ci import build_consent_controls as controls


def unit_events(target):
    names = [controls.TEST_ROOT + name for name in controls.COMMON_TESTS]
    if not target.startswith("windows_"):
        names.append(controls.TEST_ROOT + controls.LINK_TEST)
    return [dict(type="test", event="ok", name=name) for name in names] + [
        dict(type="suite", event="ok", failed=0),
    ]


def encode(rows):
    return "\n".join(json.dumps(row) for row in rows)


class BuildConsentEvidence(unittest.TestCase):
    def test_windows_sdk_linker_gets_the_existing_cli_stack_budget(self):
        arguments = ["build", "--locked", "-p", "compiler", "--example", "build_charge"]
        command = controls.build_command("cargo", "build_charge", arguments, windows=True)
        self.assertEqual(command, ["cargo", "rustc", *arguments[1:], "--message-format=json",
                                   "--", "-C", "link-arg=/STACK:268435456"])
        self.assertEqual(controls.build_command("cargo", "build_charge", arguments, windows=False),
                         ["cargo", *arguments, "--message-format=json"])
        unit = ["test", "--locked", "-p", "package-compilation", "--lib", "--no-run"]
        self.assertEqual(controls.build_command("cargo", "package_compilation", unit, windows=True),
                         ["cargo", *unit, "--message-format=json"])

    def test_both_source_produced_sdk_observations_are_required(self):
        controls.validate_sdk_output("\n".join(controls.MARKERS))
        for output in ("", controls.MARKERS[0], controls.MARKERS[1],
                       "\n".join((*controls.MARKERS, controls.MARKERS[0]))):
            with self.subTest(output=output), self.assertRaises(ValueError):
                controls.validate_sdk_output(output)

    def test_complete_host_rosters_pass(self):
        for target in ("linux_x86_64", "linux_arm64", "macos_arm64", "windows_x86_64"):
            with self.subTest(target=target):
                observed = controls.validate_test_events(encode(unit_events(target)), target)
                self.assertEqual(len(observed), 4 if target.startswith("windows_") else 5)

    def test_empty_incomplete_failed_and_duplicate_runs_refuse(self):
        rows = unit_events("linux_x86_64")
        failed = [dict(row) for row in rows]
        failed[0]["event"] = "failed"
        for observed in ([], rows[-1:], rows[1:], rows[:-1], rows + rows[:1], failed):
            with self.subTest(observed=observed), self.assertRaises(ValueError):
                controls.validate_test_events(encode(observed), "linux_x86_64")

    def test_unix_link_control_cannot_be_omitted(self):
        with self.assertRaises(ValueError):
            controls.validate_test_events(encode(unit_events("windows_x86_64")), "macos_arm64")


if __name__ == "__main__":
    unittest.main()
