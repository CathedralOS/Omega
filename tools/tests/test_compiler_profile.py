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
        root = Path(self.temporary.name)
        self.binary = root / "runner/out/corpus_runner-0123456789abcdef"
        library = root / "library/out/libcompiler-0123456789abcdef.rlib"
        for artifact, name in [(self.binary, "corpus_runner"), (library, "compiler")]:
            artifact.parent.mkdir(parents=True)
            artifact.write_bytes(b"historical compiler artifact")
            fingerprint = artifact.parent.parent / "fingerprint"
            fingerprint.mkdir()
            (fingerprint / ("lib-" + name + ".json")).write_text(json.dumps(dict(rustflags=[])))
        self.binary.write_bytes(b"historical runner")
        def message(name, level, executable=None):
            return json.dumps(dict(reason="compiler-artifact", target=dict(name=name),
                                   profile=dict(opt_level=level), fresh=True, executable=executable,
                                   filenames=[str(library if name == "compiler" else self.binary)]))
        records = "\n".join([message("compiler", "1"), message("corpus_runner", "1", str(self.binary))])
        profile.record_cargo_build(self.binary, records)
        receipt_path = Path(str(self.binary) + ".compiler-profile.json")
        original_receipt = receipt_path.read_bytes()
        damaged = json.loads(original_receipt)
        damaged["effective_codegen"]["compiler"]["fingerprint"]["contents"] = '{"rustflags": ["-Copt-level=0"]}'
        receipt_path.write_text(json.dumps(damaged))
        with self.assertRaisesRegex(profile.CompilerProfileError, "fingerprint evidence changed"):
            profile.require_optimized(self.binary)
        receipt_path.write_bytes(original_receipt)
        with patch.object(profile.subprocess, "run") as query:
            self.assertEqual(profile.require_optimized(self.binary)["profile"]["opt_level"], "1")
            query.assert_not_called()
            self.binary.write_bytes(b"replacement compiler")
            with self.assertRaisesRegex(profile.CompilerProfileError, "different binary digest"):
                profile.require_optimized(self.binary)
        for invalid in [records.replace('"1"', '"0"'), message("corpus_runner", "1", str(self.binary))]:
            with self.assertRaises(profile.CompilerProfileError):
                profile.record_cargo_build(self.binary, invalid)
        with self.assertRaisesRegex(profile.CompilerProfileError, "fingerprint"):
            profile.artifact_codegen(dict(target=dict(name="compiler"), profile=dict(opt_level="1"), filenames=[]))
        receipt = json.loads(Path(str(self.binary) + ".compiler-profile.json").read_text())
        receipt.pop("effective_codegen")
        receipt["binary_sha256"] = profile.hashlib.sha256(self.binary.read_bytes()).hexdigest()
        Path(str(self.binary) + ".compiler-profile.json").write_text(json.dumps(receipt))
        with self.assertRaisesRegex(profile.CompilerProfileError, "effective Cargo"):
            profile.require_optimized(self.binary)


class TopLevelExecutableProfileTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name) / "debug"
        self.directory.mkdir()

    def artifact(self, layout, *, suffix="", unit="0123456789abcdef", flags=None):
        binary = self.directory / ("tool-name" + suffix)
        binary.write_bytes(b"exact executable")
        if layout == "build":
            output = self.directory / "build" / "package" / unit / "out" / binary.name
            fingerprint = output.parent.parent / "fingerprint" / "bin-tool_name.json"
        else:
            output = self.directory / "deps" / ("tool_name-" + unit + suffix)
            fingerprint = self.directory / ".fingerprint" / ("package-" + unit) / "bin-tool_name.json"
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(binary.read_bytes())
        fingerprint.parent.mkdir(parents=True, exist_ok=True)
        fingerprint.write_text(json.dumps(dict(rustflags=[] if flags is None else flags)))
        item = dict(target=dict(name="tool-name", kind=["bin"]), profile=dict(opt_level="1"),
                    executable=str(binary), filenames=[str(binary)])
        return item, output, fingerprint

    def test_top_level_binary_binds_exact_output_in_both_cargo_layouts(self):
        for layout, suffix in [("build", ""), ("deps", ".exe")]:
            with self.subTest(layout=layout, suffix=suffix):
                item, output, fingerprint = self.artifact(layout, suffix=suffix)
                result = profile.artifact_codegen(item)
                self.assertEqual(result["opt_level"], "1")
                self.assertEqual(result["fingerprint"]["path"], str(fingerprint))
                output.unlink()

    def test_same_name_and_profile_do_not_admit_different_output_bytes(self):
        item, output, _ = self.artifact("build")
        output.write_bytes(b"different executable")
        with self.assertRaisesRegex(profile.CompilerProfileError, "exact Cargo compiler fingerprint"):
            profile.artifact_codegen(item)

    def test_identical_stale_outputs_are_ambiguous_even_with_matching_flags(self):
        item, _, _ = self.artifact("build")
        self.artifact("build", unit="fedcba9876543210")
        with self.assertRaisesRegex(profile.CompilerProfileError, "exact Cargo compiler fingerprint"):
            profile.artifact_codegen(item)

    def test_missing_or_library_fingerprint_cannot_supply_bin_profile(self):
        item, _, fingerprint = self.artifact("build")
        fingerprint.rename(fingerprint.with_name("lib-tool_name.json"))
        with self.assertRaisesRegex(profile.CompilerProfileError, "exact Cargo compiler fingerprint"):
            profile.artifact_codegen(item)

    def test_matching_output_with_missing_fingerprint_still_makes_lookup_ambiguous(self):
        item, _, fingerprint = self.artifact("deps")
        fingerprint.unlink()
        self.artifact("deps", unit="fedcba9876543210")
        with self.assertRaisesRegex(profile.CompilerProfileError, "exact Cargo compiler fingerprint"):
            profile.artifact_codegen(item)

    def test_different_target_name_cannot_supply_executable_fingerprint(self):
        item, _, _ = self.artifact("build")
        item["target"]["name"] = "different-target"
        with self.assertRaisesRegex(profile.CompilerProfileError, "exact Cargo compiler fingerprint"):
            profile.artifact_codegen(item)

    def test_executable_target_kind_is_required(self):
        item, _, _ = self.artifact("build")
        item["target"]["kind"] = ["lib"]
        with self.assertRaisesRegex(profile.CompilerProfileError, "exact Cargo compiler fingerprint"):
            profile.artifact_codegen(item)

    def test_top_level_bin_uses_effective_flags_instead_of_profile_label(self):
        item, _, _ = self.artifact("build", flags=["-C", "opt-level=0"])
        with self.assertRaisesRegex(profile.CompilerProfileError, "effective opt-level"):
            profile.artifact_codegen(item)


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
    def test_actual_cargo_fingerprints_override_the_misleading_profile_label(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "src").mkdir()
            (root / "tests").mkdir()
            (root / "Cargo.toml").write_text(
                '[package]\nname="compiler"\nversion="0.0.0"\nedition="2024"\n'
                '[[test]]\nname="corpus_runner"\nharness=false\n'
                '[profile.test]\nopt-level=1\n')
            (root / "src/lib.rs").write_text('pub fn fixture() {}\n')
            (root / "tests/corpus_runner.rs").write_text('fn main() { compiler::fixture(); }\n')
            for level in ["0", "2"]:
                environment = dict(os.environ, CARGO_TARGET_DIR=str(root / "target"),
                                   CARGO_PROFILE_TEST_OPT_LEVEL="1", RUSTFLAGS="-C opt-level=" + level)
                environment.pop("CARGO_ENCODED_RUSTFLAGS", None)
                run = subprocess.run(["cargo", "test", "--no-run", "--test", "corpus_runner",
                                      "--offline", "--message-format=json"], cwd=root,
                                     env=environment, capture_output=True, text=True, timeout=90)
                self.assertEqual(run.returncode, 0, run.stderr)
                messages = [json.loads(line) for line in run.stdout.splitlines() if line.startswith("{")]
                item = next(item for item in messages if item.get("executable"))
                self.assertEqual(item["profile"]["opt_level"], "1")
                binary = Path(item["executable"])
                if level == "0":
                    with self.assertRaisesRegex(profile.CompilerProfileError, "effective opt-level"):
                        profile.record_cargo_build(binary, run.stdout)
                    self.assertFalse(Path(str(binary) + ".compiler-profile.json").exists())
                else:
                    profile.record_cargo_build(binary, run.stdout)
                    self.assertEqual(profile.require_optimized(binary)["profile"]["opt_level"], "2")
            (root / ".cargo").mkdir()
            (root / ".cargo/config.toml").write_text('[build]\nrustflags=["--codegen=opt-level=0"]\n')
            environment.pop("RUSTFLAGS")
            run = subprocess.run(["cargo", "test", "--no-run", "--test", "corpus_runner",
                                  "--offline", "--message-format=json"], cwd=root,
                                 env=environment, capture_output=True, text=True, timeout=90)
            self.assertEqual(run.returncode, 0, run.stderr)
            messages = [json.loads(line) for line in run.stdout.splitlines() if line.startswith("{")]
            item = next(item for item in messages if item.get("executable"))
            self.assertEqual(item["profile"]["opt_level"], "1")
            with self.assertRaisesRegex(profile.CompilerProfileError, "effective opt-level"):
                profile.record_cargo_build(Path(item["executable"]), run.stdout)
            with self.assertRaisesRegex(profile.CompilerProfileError, "response file"):
                profile.effective_optimization("1", ["@uninspected-flags"])
            with self.assertRaisesRegex(profile.CompilerProfileError, "response file"):
                profile.effective_optimization("1", ["-C", "@uninspected-flags"])

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
            response = root / "flags.rsp"
            response.write_text("opt-level=0\n")
            refused = run("build", CARGO_ENCODED_RUSTFLAGS="-C\x1f@" + str(response))
            self.assertNotEqual(refused.returncode, 0)
            self.assertIn("response file", refused.stderr)


if __name__ == "__main__":
    unittest.main()
