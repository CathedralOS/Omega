"""Optimization is established before execution, independent of runtime env."""

import json
import contextlib
import io
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import compiler_profile as profile
import corpus_gate
import sample_gate
import sample_native_gate


class CompilerProfileTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.binary = Path(self.temporary.name) / "compiler"
        self.binary.write_bytes(profile.SCHEMA.encode())

    def response(self, level="1"):
        return subprocess.CompletedProcess([], 0, json.dumps(dict(schema=profile.SCHEMA, opt_level=level)), "")

    def test_runtime_profile_override_cannot_bless_an_unoptimized_binary(self):
        with patch.dict(os.environ, CARGO_PROFILE_DEV_OPT_LEVEL="1"), \
                patch.object(profile.subprocess, "run", return_value=self.response("0")):
            with self.assertRaisesRegex(profile.CompilerProfileError, "unoptimized"):
                profile.require_optimized(self.binary)

    def test_actual_optimized_profile_is_bound_to_binary_bytes(self):
        with patch.dict(os.environ, CARGO_PROFILE_DEV_OPT_LEVEL="0"), \
                patch.object(profile.subprocess, "run", return_value=self.response()) as query:
            result = profile.require_optimized(self.binary)
        self.assertEqual(result["profile"]["opt_level"], "1")
        self.assertEqual(result["sha256"], profile.hashlib.sha256(self.binary.read_bytes()).hexdigest())
        self.assertEqual(query.call_args.args[0], [str(self.binary), "--build-profile"])
        self.assertEqual(query.call_args.kwargs["stdin"], subprocess.DEVNULL)

    def test_unknown_historical_runner_is_never_launched_to_probe_a_flag(self):
        self.binary.write_bytes(b"old corpus runner that ignores unknown arguments")
        with patch.object(profile.subprocess, "run") as query:
            with self.assertRaises(profile.CompilerProfileError):
                profile.require_optimized(self.binary)
        query.assert_not_called()

    def test_malformed_failed_or_slow_profile_query_refuses(self):
        for result in [subprocess.CompletedProcess([], 64, "", "unknown flag"),
                       subprocess.CompletedProcess([], 0, "[]", ""),
                       subprocess.CompletedProcess([], 0, "not JSON", ""),
                       self.response("unknown"), self.response([1])]:
            with self.subTest(result=result), patch.object(profile.subprocess, "run", return_value=result):
                with self.assertRaises(profile.CompilerProfileError):
                    profile.require_optimized(self.binary)
        with patch.object(profile.subprocess, "run", side_effect=subprocess.TimeoutExpired("compiler", 10)):
            with self.assertRaises(profile.CompilerProfileError):
                profile.require_optimized(self.binary)

    def test_binary_replacement_during_probe_refuses(self):
        def replace(*args, **kwargs):
            self.binary.write_bytes(b"another compiler")
            return self.response()
        with patch.object(profile.subprocess, "run", side_effect=replace):
            with self.assertRaisesRegex(profile.CompilerProfileError, "changed during"):
                profile.require_optimized(self.binary)

    def test_fresh_cargo_receipt_supports_historical_comparison_and_checks_digest(self):
        self.binary.write_bytes(b"historical runner")
        def message(name, level, executable=None):
            return json.dumps(dict(reason="compiler-artifact", target=dict(name=name),
                                   profile=dict(opt_level=level), fresh=True, executable=executable))
        records = "\n".join([message("compiler", "1"), message("corpus_runner", "1", str(self.binary))])
        profile.record_cargo_build(self.binary, records)
        with patch.object(profile.subprocess, "run") as query:
            self.assertEqual(profile.require_optimized(self.binary)["profile"]["opt_level"], "1")
            query.assert_not_called()
            self.binary.write_bytes(b"replacement compiler")
            with self.assertRaisesRegex(profile.CompilerProfileError, "different binary digest"):
                profile.require_optimized(self.binary)
        for invalid in [records.replace('"1"', '"0"'), message("corpus_runner", "1", str(self.binary))]:
            with self.assertRaises(profile.CompilerProfileError):
                profile.record_cargo_build(self.binary, invalid)


class GateCompilerAdmissionTests(unittest.TestCase):
    def test_unknown_supplied_cli_never_starts_sample_work(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "old-omega"
            binary.write_bytes(b"unknown compiler without optimization metadata")
            with patch.object(sys, "argv", ["sample_gate.py", "--cli", str(binary)]), \
                    patch.object(sample_gate, "sample_roots", return_value=[Path(directory)]), \
                    patch.object(sample_gate, "check_one") as check, contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(sample_gate.main(), 2)
                check.assert_not_called()
            with patch.object(sys, "argv", ["sample_native_gate.py", "--cli", str(binary)]), \
                    patch.object(sample_native_gate, "shared_runs_directory", return_value=directory), \
                    patch.object(sample_native_gate, "discover") as discover, \
                    patch.object(sample_native_gate, "timed_measure") as measure, \
                    contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(sample_native_gate.main(), 2)
                discover.assert_not_called()
                measure.assert_not_called()

    def test_unknown_supplied_runner_never_starts_the_corpus(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "old-runner"
            binary.write_bytes(b"old runner ignores unknown flags")
            with patch.object(corpus_gate.subprocess, "Popen") as launch:
                with self.assertRaises(corpus_gate.InfrastructureFailure):
                    corpus_gate.run_runner(binary)
                launch.assert_not_called()


@unittest.skipUnless(shutil.which("cargo"), "Cargo is needed to exercise the actual build guard")
class CargoOptimizationGuardTests(unittest.TestCase):
    def test_cargo_profiles_and_rustflags_cannot_produce_an_unoptimized_compiler(self):
        guard = Path(__file__).resolve().parents[2] / "omega-rust/omega/compilation/driver/build_profile.rs"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "src").mkdir()
            (root / "Cargo.toml").write_text(
                '[package]\nname="compiler-profile-guard-test"\nversion="0.0.0"\nedition="2024"\n'
                '[profile.dev]\nopt-level=1\n[profile.test]\nopt-level=1\n')
            (root / "build.rs").write_text(
                '#[path = r#"' + str(guard) + '"#] mod build_profile;\n'
                'fn main() -> std::io::Result<()> {\n'
                'let level = build_profile::optimization_level()?;\n'
                'println!("cargo:rustc-env=MEASURED_OPT_LEVEL={level}"); Ok(()) }\n')
            (root / "src/main.rs").write_text('fn main() { println!("{}", env!("MEASURED_OPT_LEVEL")); }\n')
            environment = dict(os.environ, CARGO_TARGET_DIR=str(root / "target"))
            for name in list(environment):
                if name.startswith(("CARGO_PROFILE_", "CARGO_ENCODED_RUSTFLAGS", "RUSTFLAGS")):
                    environment.pop(name)
            def run(*arguments, **overrides):
                return subprocess.run(["cargo", *arguments, "--offline", "--quiet"], cwd=root,
                                      env=dict(environment, **overrides), capture_output=True, text=True, timeout=90)
            optimized = run("run")
            self.assertEqual(optimized.returncode, 0, optimized.stderr)
            self.assertEqual(optimized.stdout.strip(), "1")
            for overrides in [dict(CARGO_PROFILE_DEV_OPT_LEVEL="0"),
                              dict(RUSTFLAGS="-C opt-level=0"),
                              dict(CARGO_ENCODED_RUSTFLAGS="--codegen=opt-level=0")]:
                with self.subTest(overrides=overrides):
                    refused = run("build", **overrides)
                    self.assertNotEqual(refused.returncode, 0)
                    self.assertIn("requires an optimized compiler", refused.stderr)
            refused = run("test", "--no-run", CARGO_PROFILE_TEST_OPT_LEVEL="0")
            self.assertNotEqual(refused.returncode, 0)
            self.assertIn("requires an optimized compiler", refused.stderr)


if __name__ == "__main__":
    unittest.main()
