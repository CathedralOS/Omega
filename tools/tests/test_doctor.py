#!/usr/bin/env python3
"""Doctor's checks are exercised against a fake command table — the point is
that each check maps a subprocess outcome to the right severity, not that
this host's toolchain answers a particular way.

    python3 tools/tests/test_doctor.py -v
"""

import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest

import worktree_testkit as kit
from unittest.mock import patch


DOCTOR = Path(__file__).resolve().parents[1] / "doctor.py"


def load_doctor():
    return kit.load_module(DOCTOR, 'doctor_under_test')


def fake_run(table):
    """Dispatch on argument prefixes; longer prefixes must be listed first.
    A missing entry means the command failed to run (returncode-style None).
    """
    def run(arguments, timeout=45, cwd=None):
        for prefix, result in table:
            if tuple(arguments[:len(prefix)]) == tuple(prefix):
                return result
        return None
    return run


class CheckTests(unittest.TestCase):
    def setUp(self):
        self.module = load_doctor()

    def test_python_reports_running_interpreter(self):
        entry = self.module.check_python()
        self.assertEqual(entry["name"], "python")
        self.assertEqual(entry["status"], "ok"
                         if sys.version_info >= (3, 9) else "fail")

    def test_rust_toolchain_reports_resolved_and_missing(self):
        module = self.module
        with patch.object(module, "run", fake_run([
                (("rustc", "--version"),
                 (0, "rustc 1.100.0-nightly (a69a63265 2026-09-03)", ""))])):
            entry = module.check_rust_toolchain()
        self.assertEqual(entry["status"], "ok")
        self.assertIn("nightly", entry["detail"])

        with patch.object(module, "run", fake_run([
                (("rustc", "--version"),
                 (1, "", "error: toolchain 'nightly-2026-09-04' is not "
                         "installed"))])):
            entry = module.check_rust_toolchain()
        self.assertEqual(entry["status"], "fail")
        self.assertIn("not installed", entry["detail"])
        self.assertIn("fix", entry)

    def test_stable_toolchain_under_nightly_pin_warns(self):
        module = self.module
        with patch.object(module, "run", fake_run([
                (("rustc", "--version"),
                 (0, "rustc 1.99.0 (hash 2026-08-01)", ""))])):
            entry = module.check_rust_toolchain()
        if module.toolchain_channel().startswith("nightly"):
            self.assertEqual(entry["status"], "warn")

    def test_nextest_version_floor(self):
        module = self.module
        current = fake_run([
            (("cargo", "nextest", "--version"),
             (0, "cargo-nextest 0.9.143", ""))])
        with patch.object(module, "run", current):
            self.assertEqual(module.check_nextest()["status"], "ok")
        ancient = fake_run([
            (("cargo", "nextest", "--version"),
             (0, "cargo-nextest 0.9.100", ""))])
        with patch.object(module, "run", ancient):
            entry = module.check_nextest()
            self.assertEqual(entry["status"], "warn")
            self.assertIn("0.9.140", entry["detail"])
        with patch.object(module, "run", fake_run([])):
            self.assertEqual(module.check_nextest()["status"], "warn")

    def test_link_probe_fails_on_linker_error_and_succeeds_on_run(self):
        module = self.module
        broken = fake_run([
            (("rustc",), (101, "",
                          "ld: tapi error: malformed file\n"
                          "error: unknown architecture"))])
        with patch.object(module, "run", broken), \
                patch.dict(os.environ, {"SDKROOT": ""}):
            entry = module.check_rust_link_probe()
        self.assertEqual(entry["status"], "fail")
        self.assertIn("probe", entry["detail"])

        def healthy(arguments, timeout=45, cwd=None):
            if arguments[0] == "rustc" and len(arguments) > 1:
                return (0, "", "")
            if arguments[0].endswith(("probe", "probe.exe")):
                return (7, "", "")
            return None
        with patch.object(module, "run", healthy), \
                patch.dict(os.environ, {"SDKROOT": ""}):
            entry = module.check_rust_link_probe()
        self.assertEqual(entry["status"], "ok")

    def healthy_probe(self, calls):
        def run(arguments, timeout=45, cwd=None):
            calls.append((arguments[0], cwd))
            if arguments[0] == "rustc" and len(arguments) > 1:
                return (0, "", "")
            if arguments[0].endswith(("probe", "probe.exe")):
                return (7, "", "")
            return None
        return run

    def test_probe_compiles_from_the_repository_so_the_pin_applies(self):
        module = self.module
        calls = []
        with patch.object(module, "run", self.healthy_probe(calls)), \
                patch.dict(os.environ, {"SDKROOT": ""}):
            entry = module.check_rust_link_probe()
        self.assertEqual(entry["status"], "ok")
        self.assertIn("pinned toolchain", entry["detail"])
        self.assertEqual([cwd for name, cwd in calls if name == "rustc"],
                         [module.ROOT])

    def test_unversioned_sdkroot_is_fine_and_versioned_warns(self):
        module = self.module
        self.assertFalse(module.versioned_sdk(
            "/Applications/Xcode.app/Contents/Developer/Platforms/"
            "MacOSX.platform/Developer/SDKs/MacOSX.sdk"))
        self.assertTrue(module.versioned_sdk("/x/SDKs/MacOSX26.2.sdk"))
        self.assertTrue(module.versioned_sdk("/x/SDKs/MacOSX27.sdk"))
        with tempfile.TemporaryDirectory() as scratch:
            unversioned = Path(scratch) / "MacOSX.sdk"
            versioned = Path(scratch) / "MacOSX26.2.sdk"
            unversioned.mkdir()
            versioned.mkdir()
            for sdk, expected in ((unversioned, "ok"), (versioned, "warn")):
                with patch.object(module, "run", self.healthy_probe([])), \
                        patch.dict(os.environ, {"SDKROOT": str(sdk)}):
                    self.assertEqual(
                        module.check_rust_link_probe()["status"], expected,
                        sdk.name)

    def test_sdkroot_naming_a_missing_sdk_fails_before_probing(self):
        module = self.module
        with patch.dict(os.environ,
                        {"SDKROOT": "/nonexistent/MacOSX99.sdk"}):
            entry = module.check_rust_link_probe()
        self.assertEqual(entry["status"], "fail")
        self.assertIn("MacOSX99.sdk", entry["detail"])

    def test_claims_remote_unreachable_warns(self):
        module = self.module
        table = fake_run([
            (("git", "remote", "get-url"), (0, "git@example.invalid:r", "")),
            (("git", "ls-remote"), None),
        ])
        with patch.object(module, "run", table):
            entry = module.check_claims_remote()
        self.assertEqual(entry["status"], "warn")
        self.assertIn("unreachable", entry["detail"])

    def test_main_exit_code_follows_failures(self):
        module = self.module
        clean = [module.check("a", "ok", "fine"),
                 module.check("b", "warn", "degraded")]
        with patch.object(module, "collect_checks", return_value=clean):
            self.assertEqual(module.main([]), 0)
        broken = clean + [module.check("c", "fail", "no", fix="do x")]
        with tempfile.TemporaryDirectory() as scratch:
            report = Path(scratch) / "report.json"
            with patch.object(module, "collect_checks",
                              return_value=broken):
                self.assertEqual(module.main(["--json", str(report)]), 1)
            document = json.loads(report.read_text())
            self.assertEqual(document["schema"], "omega-doctor-v1")
            self.assertEqual(document["failed"], 1)


if __name__ == "__main__":
    unittest.main()
