#!/usr/bin/env python3
"""Exercise unlocked native compilation through the shipped CLI.

Run with `python3 tools/tests/test_development_run.py --omega <binary>`.
Uses the existing recursive-call corpus program without changing its source.
Project mutations stay in temporary copies; no repository lock is accepted.
"""

import argparse
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tempfile
import unittest


REPOSITORY = Path(__file__).resolve().parents[2]
FIXTURE = REPOSITORY / "tests/omega/pass/calls/machine_self_call_recursion_compile"
HOST_TARGETS = {
    ("Darwin", "arm64"): "macos_arm64",
    ("Linux", "x86_64"): "linux_x86_64",
    ("Linux", "aarch64"): "linux_arm64",
    ("Windows", "AMD64"): "windows_x86_64",
}


class DevelopmentRun(unittest.TestCase):
    omega = None
    target: str

    @classmethod
    def setUpClass(cls):
        if cls.omega is None:
            raise unittest.SkipTest("invoke this CLI integration test with --omega <binary>")

    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="omega-development-run-")
        self.addCleanup(directory.cleanup)
        self.project = Path(directory.name)
        for name in ("main.omg", "build.omg"):
            shutil.copyfile(FIXTURE / name, self.project / name)
        self.environment = dict(os.environ, OMEGA_LIBRARY_ROOT=str(REPOSITORY / "source/library"))

    def invoke(self, *arguments):
        return subprocess.run(
            [str(self.omega), *map(str, arguments)],
            cwd=self.project,
            env=self.environment,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=120,
        )

    def assert_exit(self, result, expected):
        self.assertEqual(result.returncode, expected, result.stdout + result.stderr)

    def accept_project(self):
        result = self.invoke("update", "--project", self.project, "--target", self.target, "--offline")
        self.assert_exit(result, 0)
        self.assertTrue((self.project / "omega.lock").is_file(), result.stdout + result.stderr)

    def test_unlocked_run_executes_without_creating_acceptance(self):
        result = self.invoke("run", "main.omg")
        self.assert_exit(result, 0)
        self.assertIn("native exit 0", result.stdout + result.stderr)
        self.assertFalse((self.project / "omega.lock").exists())
        self.assertFalse((self.project / "build/package-manager").exists())

    def independent_review(self, generated, sealed=True, second_component=False):
        fixture = REPOSITORY / "tests/omega/pass/facts/opaque_independent_closed_carrier_through_other_machine_exit"
        shutil.copytree(fixture, self.project, dirs_exist_ok=True,
                        ignore=shutil.ignore_patterns("build", "omega.lock"))
        packages = 2
        if second_component:
            shutil.copytree(self.project / "producer", self.project / "other")
            package_build = self.project / "other/build.omg"
            package_build.write_text(package_build.read_text().replace(
                '"opaque_address_producer"', '"other_address_producer"'))
            build = self.project / "build.omg"
            source = build.read_text()
            closing = source.rfind("}")
            build.write_text(source[:closing] + '''
 Build::depend_as(builder, "other", Source::Path { location: "other" });
 Build::select_representation<other::InternetAddress, other::AddressRepresentation>(builder);
 Build::select_provider<other::AddressBridge, other::AddressBridgeProvider>(builder, CompositionMode::Independent);
''' + source[closing:])
            packages += 1
        if generated:
            build = self.project / "build.omg"
            source = build.read_text()
            closing = source.rfind("}")
            source = source[:closing] + '''
 let generated: BuildPath = BuildOutput::resolve(&builder.output, "review_generated.omg");
 let writer: i32 = BuildOutput::create(&mut builder.output, generated, 0);
 _ = BuildOutput::write(&mut builder.output, writer, "const REVIEW_VALUE: u8 = 7;\\n");
 _ = BuildOutput::close(&mut builder.output, writer);
 BuildOutput::include_source(&mut builder.output, generated);
''' + source[closing:]
            build.write_text(source)
            if not sealed:
                build.write_text(source.replace(
                    " _ = BuildOutput::close(&mut builder.output, writer);\n", ""))
        if sealed:
            runtime = self.invoke("run", "--both", "main.omg")
            self.assert_exit(runtime, 0)
            self.assertIn("native exit 0", runtime.stdout + runtime.stderr)
            self.assertIn("interp exit: 0", runtime.stdout + runtime.stderr)
        self.environment["OMEGA_REVIEW_TIMINGS"] = "1"
        result = self.invoke("update", "--project", self.project,
                             "--target", self.target, "--offline")
        self.assertFalse((self.project / "omega.lock").exists())
        if not sealed:
            self.assert_exit(result, 1)
            self.assertIn("staging entry is not sealed", result.stderr)
            return
        self.assert_exit(result, 3)  # Dependency review awaits an explicit decision.
        self.assertIn(f"checked {packages} source packages", result.stdout)
        passes = re.findall(r"note=review_pass (\w+) occurrences=(\d+) compiled=(\d+) "
                            r"carried=(\d+).*?preliminary_checks=(\d+).*?settled_checks=(\d+)",
                            result.stderr)
        self.assertEqual(passes, [("discovery", str(packages), str(packages), "0",
                                  str(packages), str(2 * packages))],
                         result.stdout + result.stderr)
        resumed = self.invoke("update", "--project", self.project, "--resume", "--offline")
        self.assert_exit(resumed, 3)
        resumed_passes = re.findall(
            r"note=review_pass (\w+) occurrences=(\d+) compiled=(\d+) carried=(\d+)"
            r".*?preliminary_checks=(\d+).*?settled_checks=(\d+)",
            resumed.stderr)
        self.assertTrue(resumed_passes, resumed.stdout + resumed.stderr)
        for _, occurrences, compiled, carried, preliminary, settled in resumed_passes:
            self.assertEqual(int(compiled) + int(carried), int(occurrences),
                             resumed.stdout + resumed.stderr)
            self.assertLessEqual(int(preliminary), int(compiled), resumed.stderr)
            self.assertLessEqual(int(settled), 2 * int(compiled), resumed.stderr)

    def test_independent_review_completes_each_occurrence_once(self):
        self.independent_review(generated=False)

    def test_generated_independent_review_completes_each_occurrence_once(self):
        self.independent_review(generated=True)

    def test_independent_review_rejects_unsealed_generated_source(self):
        self.independent_review(generated=True, sealed=False)

    def test_component_cache_refresh_retains_completed_sibling_work(self):
        self.independent_review(generated=True, second_component=True)

    def test_unlocked_compile_uses_the_same_native_route(self):
        result = self.invoke("--build-dir", self.project / "output", "--target", self.target, "main.omg")
        self.assert_exit(result, 0)
        self.assertFalse((self.project / "omega.lock").exists())

    def test_unlocked_both_compares_the_retained_product(self):
        result = self.invoke("run", "--both", "main.omg")
        self.assert_exit(result, 0)
        self.assertIn("native exit 0", result.stdout + result.stderr)
        self.assertIn("interp exit: 0", result.stdout + result.stderr)
        self.assertFalse((self.project / "omega.lock").exists())

    def test_both_replays_multiple_inputs_from_one_compilation(self):
        inputs = self.project / "inputs"
        inputs.mkdir()
        for name, content in (("first", b"one\n"), ("second", b"two\n")):
            (inputs / f"{name}.stdin").write_bytes(content)
        result = self.invoke("run", "--both", "--inputs", inputs,
                             "--outputs", self.project / "outputs", "main.omg")
        self.assert_exit(result, 0)
        for name in ("first", "second"):
            self.assertIn(f"case {name}: native exit 0; interp exit 0", result.stderr)
        self.assertFalse((self.project / "omega.lock").exists())

    def test_run_preserves_private_entry_initialization_refusals(self):
        result = self.invoke("run", "--both", "--entry-ram", "1=2900000003000000", "main.omg")
        self.assert_exit(result, 200)
        self.assertIn("private entry RAM initializer answers no declared entry position", result.stderr)
        self.assertNotIn("native exit", result.stderr)

    def inspect_source(self, output):
        return self.invoke("inspect-terminal", "--machine", "Main::main",
                           "--output", output, "main.omg")

    def test_inspection_artifact_does_not_need_its_source(self):
        artifact = self.project / "inspection.oterm"
        result = self.inspect_source(artifact)
        self.assert_exit(result, 0)
        identity = [line for line in result.stdout.splitlines()
                    if line.startswith("terminal program_fingerprint=")]
        self.assertEqual(len(identity), 1, result.stdout)
        self.assertTrue(artifact.is_file())
        self.assertFalse((self.project / "build").exists())
        self.assertFalse((self.project / "omega.lock").exists())
        (self.project / "main.omg").unlink()
        (self.project / "build.omg").unlink()
        loaded = self.invoke("inspect-terminal", "--artifact", artifact)
        self.assert_exit(loaded, 0)
        self.assertIn(identity[0], loaded.stdout)
        artifact.write_bytes(artifact.read_bytes() + b"\x00")
        corrupt = self.invoke("inspect-terminal", "--artifact", artifact)
        self.assert_exit(corrupt, 1)
        self.assertIn("cannot inspect Terminal artifact", corrupt.stderr)

    def test_standalone_inspection_keeps_targetless_source(self):
        (self.project / "build.omg").unlink()
        result = self.inspect_source(self.project / "standalone.oterm")
        self.assert_exit(result, 0)
        self.assertIn("terminal program_fingerprint=", result.stdout)
        self.assertFalse((self.project / "omega.lock").exists())

    def test_missing_inspection_machine_does_not_publish(self):
        artifact = self.project / "absent.oterm"
        result = self.invoke("inspect-terminal", "--machine", "Main::absent",
                             "--output", artifact, "main.omg")
        self.assert_exit(result, 1)
        self.assertIn("Main::absent", result.stderr)
        self.assertFalse(artifact.exists())

    def test_inspection_preserves_accepted_source_identity(self):
        self.accept_project()
        lock = (self.project / "omega.lock").read_bytes()
        artifact = self.project / "accepted.oterm"
        self.assert_exit(self.inspect_source(artifact), 0)
        with (self.project / "main.omg").open("a", encoding="utf-8") as source:
            source.write("\n// Requires new source acceptance.\n")
        rejected = self.inspect_source(self.project / "stale.oterm")
        self.assert_exit(rejected, 1)
        self.assertIn("omega update", rejected.stderr)
        self.assertEqual((self.project / "omega.lock").read_bytes(), lock)
        self.assertFalse((self.project / "stale.oterm").exists())

    def test_check_does_not_claim_or_publish_the_output_directory(self):
        output = self.project / "check-output"
        result = self.invoke("--check", "--build-dir", output, "--target", self.target, "main.omg")
        self.assert_exit(result, 0)
        self.assertFalse(output.exists())
        self.assertFalse((self.project / "omega.lock").exists())

    def test_batch_keeps_a_directory_refusal_beside_a_runnable_sibling(self):
        rejected = "windows_x86_64" if self.target != "windows_x86_64" else "linux_x86_64"
        output = self.project / "batch-output"
        occupied = output / rejected / ".omega-occupant"
        occupied.parent.mkdir(parents=True)
        marker = "omega-build-directory-occupant 1\nheld by this test\n"
        occupied.write_text(marker, encoding="utf-8")
        report = self.project / "batch-report.txt"
        result = self.invoke("--build-dir", output, "--target", rejected, "--target", self.target,
                             "--report-file", report, "main.omg")
        self.assert_exit(result, 1)
        self.assertIn(f"target {rejected} failed", result.stdout)
        self.assertIn(f"target {self.target}:", result.stdout)
        self.assertEqual(occupied.read_text(encoding="utf-8"), marker)
        self.assertFalse((output / self.target / ".omega-occupant").exists())
        paths = [line.removeprefix("published native output to ")
                 for line in result.stdout.splitlines() if line.startswith("published native output to ")]
        self.assertEqual(len(paths), 1, result.stdout + result.stderr)
        executable = Path(paths[0])
        self.assertTrue(executable.resolve().is_relative_to((output / self.target).resolve()))
        observed = subprocess.run([str(executable)], cwd=self.project, env=self.environment,
                                  capture_output=True, text=True, timeout=30)
        self.assert_exit(observed, 0)
        published = report.read_text(encoding="utf-8")
        self.assertIn(f"target: {rejected}\nfailed:", published)
        self.assertIn(f"target: {self.target}\n", published)
        self.assertFalse((self.project / "omega.lock").exists())

    def test_invalid_batch_target_refuses_before_output_admission(self):
        output = self.project / "invalid-output"
        result = self.invoke("--build-dir", output, "--target", self.target,
                             "--target", "not_a_target", "main.omg")
        self.assert_exit(result, 1)
        self.assertFalse(output.exists())

    def test_batch_keeps_accepted_target_when_its_sibling_has_no_lock_section(self):
        self.accept_project()
        lock = (self.project / "omega.lock").read_bytes()
        rejected = "windows_x86_64" if self.target != "windows_x86_64" else "linux_x86_64"
        for check_only in (True, False):
            with self.subTest(check_only=check_only):
                output = self.project / "build" / ("checked-batch" if check_only else "native-batch")
                result = self.invoke(*(["--check"] if check_only else []),
                                     "--build-dir", output, "--target", rejected,
                                     "--target", self.target, "main.omg")
                self.assert_exit(result, 1)
                self.assertIn(f"target {rejected} failed", result.stdout)
                self.assertIn("cannot prepare accepted omega.lock", result.stdout + result.stderr)
                self.assertIn(f"target {self.target}:", result.stdout)
                self.assertNotIn(f"target {self.target} failed", result.stdout,
                                 result.stdout + result.stderr)
                self.assertEqual((self.project / "omega.lock").read_bytes(), lock)
                self.assertFalse(list(output.glob(".omega-package-build-*")))
                paths = [line.removeprefix("published native output to ")
                         for line in result.stdout.splitlines()
                         if line.startswith("published native output to ")]
                if check_only:
                    self.assertFalse(paths)
                    self.assertFalse(output.exists())
                else:
                    self.assertEqual(len(paths), 1, result.stdout + result.stderr)
                    executable = Path(paths[0])
                    self.assertTrue(executable.resolve().is_relative_to(
                        (output / self.target).resolve()))
                    observed = subprocess.run([str(executable)], cwd=self.project,
                                              env=self.environment, capture_output=True,
                                              text=True, timeout=30)
                    self.assert_exit(observed, 0)

    def test_artifact_only_build_publishes_its_sealed_output(self):
        fixture = REPOSITORY / "tests/omega/pass/build/artifact_only_required_output_completion"
        for name in ("main.omg", "build.omg"):
            shutil.copyfile(fixture / name, self.project / name)
        result = self.invoke("--build-dir", self.project / "artifact-output", "--target", self.target, "main.omg")
        self.assert_exit(result, 0)
        prefix = "published completed build outputs to "
        paths = [line.removeprefix(prefix) for line in result.stdout.splitlines() if line.startswith(prefix)]
        self.assertEqual(len(paths), 1, result.stdout + result.stderr)
        published = Path(paths[0])
        self.assertTrue((published / "manifest.bin").is_file())
        self.assertEqual((published / "files/manifest.txt").read_bytes(), b"omega artifact\n")
        self.assertNotIn("published native output", result.stdout)

    def test_artifact_only_build_does_not_publish_an_unsealed_output(self):
        fixture = REPOSITORY / "tests/omega/fail/build/artifact_only_unsealed_output_refused"
        for name in ("main.omg", "build.omg"):
            shutil.copyfile(fixture / name, self.project / name)
        result = self.invoke("--build-dir", self.project / "artifact-output", "--target", self.target, "main.omg")
        self.assert_exit(result, 1)
        self.assertIn("staged output never sealed", result.stdout + result.stderr)
        self.assertNotIn("published completed build outputs", result.stdout)

    def test_unreadable_lock_is_not_an_unlocked_project(self):
        lock = self.project / "omega.lock"
        lock.write_bytes(b"not an omega lock\n")
        result = self.invoke("run", "main.omg")
        self.assert_exit(result, 200)
        self.assertIn("cannot prepare accepted omega.lock", result.stdout + result.stderr)
        self.assertEqual(lock.read_bytes(), b"not an omega lock\n")

    def test_dependency_binding_is_not_accepted_by_development_run(self):
        fixture = REPOSITORY / "tests/omega/pass/text/runtime_number_to_decimal_exit"
        shutil.copyfile(fixture / "main.omg", self.project / "main.omg")
        build = (fixture / "build.omg").read_text(encoding="utf-8")
        build = build.replace("../../../../../source/library/std", (REPOSITORY / "source/library/std").as_posix())
        with (self.project / "build.omg").open("w", encoding="utf-8", newline="\n") as source:
            source.write(build)
        result = self.invoke("run", "main.omg")
        self.assert_exit(result, 200)
        self.assertIn("needs an accepted program-entry binding", result.stdout + result.stderr)
        self.assertFalse((self.project / "omega.lock").exists())

    def test_current_lock_still_runs(self):
        self.accept_project()
        lock = (self.project / "omega.lock").read_bytes()
        result = self.invoke("run", "main.omg")
        self.assert_exit(result, 0)
        self.assertIn("native exit 0", result.stdout + result.stderr)
        self.assertEqual((self.project / "omega.lock").read_bytes(), lock)

    def test_source_edit_does_not_fall_back_to_development(self):
        self.accept_project()
        lock = (self.project / "omega.lock").read_bytes()
        with (self.project / "main.omg").open("a", encoding="utf-8", newline="\n") as source:
            source.write("\n// A source edit requires explicit review of this locked project.\n")
        result = self.invoke("run", "main.omg")
        self.assert_exit(result, 200)
        self.assertIn("omega update", result.stdout + result.stderr)
        self.assertNotIn("native exit", result.stdout + result.stderr)
        self.assertEqual((self.project / "omega.lock").read_bytes(), lock)

    def test_missing_target_does_not_fall_back_to_development(self):
        self.accept_project()
        other_target = "linux_x86_64" if self.target != "linux_x86_64" else "macos_arm64"
        result = self.invoke("run", "--target", other_target, "main.omg")
        self.assert_exit(result, 200)
        self.assertIn("cannot prepare accepted omega.lock", result.stdout + result.stderr)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--omega", type=Path, required=True)
    arguments, remaining = parser.parse_known_args()
    DevelopmentRun.omega = arguments.omega.resolve(strict=True)
    DevelopmentRun.target = HOST_TARGETS.get((platform.system(), platform.machine()))
    if DevelopmentRun.target is None:
        parser.error("this host has no supported native execution target")
    unittest.main(argv=[__file__, *remaining])
