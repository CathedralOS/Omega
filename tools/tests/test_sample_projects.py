"""Project discovery and gate verdicts, without accepting real packages."""

import contextlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import sample_projects as projects
import sample_gate as check_gate
import sample_native_gate as native_gate


class ProjectGateTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        # Discovery resolves directories; on macOS the temporary directory
        # lives behind the /var -> /private/var link, so compare resolved paths.
        self.root = Path(self.temporary.name).resolve()
        for module in (projects, check_gate, native_gate):
            replacement = patch.object(module, "ROOT", self.root)
            replacement.start()
            self.addCleanup(replacement.stop)
        replacement = patch.object(check_gate, "SAMPLES", self.root / "samples")
        replacement.start()
        self.addCleanup(replacement.stop)
        profile = patch.object(native_gate, "require_optimized", return_value={"profile": {"opt_level": "1"}})
        profile.start()
        self.addCleanup(profile.stop)

    def project(self, path, build="Build::application(builder, \"study\"); Build::bind(builder, Target::ProgramEntry, Main::main);", source="// Expected exit: 0\n"):
        directory = self.root / "samples" / path
        directory.mkdir(parents=True)
        (directory / "build.omg").write_text(build)
        (directory / "main.omg").write_text(source)
        return directory

    def negative(self):
        directory = self.project("program/controls/intended_invalid")
        (directory / "expected.txt").write_text("index `4` is within length 4\n")
        return directory

    def test_nested_projects_and_library_consumer_are_independent(self):
        parent = self.project("group")
        consumer = self.project("group/consumer", 'Build::depend(builder, Source::Path { location: "source/library/numerics" }); Build::bind(builder, Target::ProgramEntry, Main::main);')
        invalid = self.negative()
        library = self.project("program/controls/intended_invalid/dependency", 'Build::package(builder, "kernel");')
        rows = projects.discover(self.root / "samples", self.root)
        self.assertEqual({row.directory: row.role for row in rows},
                         {parent: "program", consumer: "library-consumer", invalid: "intended-invalid", library: "library"})
        self.assertEqual(set(check_gate.sample_roots([])), {parent, consumer, invalid})
        self.assertEqual({str(Path(row).resolve()) for row in native_gate.discover(self.root / "samples")},
                         {str(parent), str(consumer), str(invalid)})

    def test_external_submodule_and_build_output_are_not_discovered(self):
        self.root.joinpath(".gitmodules").write_text('[submodule "squalr"]\npath = samples/apps/squalr\nurl = https://example.invalid/pinned\n')
        self.project("apps/squalr")
        retained = self.project("apps/squalr_tools")
        self.project("apps/squalr_tools/build/generated")
        self.assertEqual([row.directory for row in projects.discover(self.root / "samples", self.root)], [retained])

    def test_role_override_and_non_main_entry_preserve_layout(self):
        directory = self.project("group/invalid_but_valid_example")
        (directory / "main.omg").rename(directory / "run.omg")
        (directory / "sample.json").write_text(json.dumps(dict(role="example", entry="run.omg")))
        row = projects.project_at(directory, self.root)
        self.assertEqual((row.role, row.entry.name), ("example", "run.omg"))
        (directory / "sample.json").write_text(json.dumps(dict(entry="../other.omg")))
        with self.assertRaisesRegex(ValueError, "local Omega source"):
            projects.project_at(directory, self.root)

    def test_filtered_gate_does_not_read_unrelated_partial_metadata(self):
        retained = self.project("selected")
        unrelated = self.project("peer")
        (unrelated / "sample.json").write_text('{"role":')
        self.assertEqual(check_gate.sample_roots(["selected"]), [retained])
        self.assertEqual([Path(path).resolve() for path in native_gate.discover(self.root / "samples", ["selected"])], [retained])

    def test_check_gate_only_passes_intended_rejection_identity(self):
        directory = self.negative()
        for code, output, accepted in [(1, "error: index `4` is within length 4", True),
                                        (0, "index `4` is within length 4", False),
                                        (1, "package acceptance is missing", False),
                                        (-11, "index `4` is within length 4", False)]:
            with self.subTest(code=code, output=output), patch.object(check_gate.subprocess, "Popen") as launch:
                process = launch.return_value.__enter__.return_value
                process.returncode = code
                process.communicate.return_value = ("", output)
                row = check_gate.check_one(Path("omega"), self.root / "scratch", directory)
                self.assertEqual(row["checked"], accepted)
                self.assertEqual(row["rejected"], accepted)

    def test_native_negative_never_accepts_packages_or_runs(self):
        directory = self.negative()
        with patch.object(native_gate, "accept_review") as review, patch.object(native_gate, "run", return_value=subprocess.CompletedProcess([], 1, "", "index `4` is within length 4")) as runner:
            row = native_gate.measure("omega", str(directory), "linux_x86_64", 10)
        self.assertEqual(row["state"], "rejected")
        review.assert_not_called()
        self.assertEqual(runner.call_args.args[0][1], "--check")

    def test_missing_expectation_wrong_reason_crash_and_timeout_fail(self):
        directory = self.negative()
        for result in (subprocess.CompletedProcess([], 1, "", "custody denied"),
                       subprocess.CompletedProcess([], 139, "", "index `4` is within length 4"), None):
            with self.subTest(result=result), patch.object(native_gate, "run", return_value=result):
                self.assertIn(native_gate.measure("omega", str(directory), "linux_x86_64", 10)["state"], ("rejection-failed", "timeout"))
        (directory / "expected.txt").unlink()
        with patch.object(native_gate, "run", return_value=subprocess.CompletedProcess([], 1, "", "any refusal")):
            row = native_gate.measure("omega", str(directory), "linux_x86_64", 10)
        self.assertEqual(row["state"], "rejection-failed")
        self.assertIn("missing authored", row["detail"])
        with patch.object(check_gate.subprocess, "Popen") as launch, patch.object(check_gate, "kill_process_tree"):
            process = launch.return_value.__enter__.return_value
            process.communicate.side_effect = [subprocess.TimeoutExpired("omega", 10), ("", "")]
            self.assertFalse(check_gate.check_one(Path("omega"), self.root / "scratch", directory, 10)["checked"])

    def test_failed_positive_check_only_is_red_without_native_score(self):
        directory = self.project("probe", 'builder.freestanding = true;')
        with patch.object(native_gate, "accept_review", return_value=(None, "")), patch.object(native_gate, "run", return_value=subprocess.CompletedProcess([], 1, "", "error: source rejected")):
            row = native_gate.measure("omega", str(directory), "linux_x86_64", 10)
        self.assertEqual(row["state"], "check-fail")
        with patch.object(sys, "argv", ["sample_native_gate.py", "--cli", "omega", "--history-dir", ""]), patch.object(native_gate, "discover", return_value=[str(directory)]), patch.object(native_gate, "measure", return_value=row), contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(native_gate.main(), 1)

    def test_unscored_no_oracle_and_freestanding_reporting_stays(self):
        directory = self.project("interactive", source="machine main() {}\n")
        with patch.object(native_gate, "accept_review", return_value=(None, "")), patch.object(native_gate, "run", return_value=subprocess.CompletedProcess([], 0, "", "")) as runner:
            self.assertEqual(native_gate.measure("omega", str(directory), "linux_x86_64", 10)["state"], "no-oracle")
            self.assertNotIn("run", runner.call_args.args[0])
        with patch.object(native_gate, "NOT_HOST_RUNNABLE", (str(directory),)), patch.object(native_gate, "accept_review") as review:
            self.assertEqual(native_gate.measure("omega", str(directory), "linux_x86_64", 10)["state"], "not-host-runnable")
            review.assert_not_called()

    def test_two_authored_fragments_are_both_required(self):
        directory = self.negative()
        (directory / "sample.json").write_text(json.dumps(dict(role="intended-invalid", rejection_contains=["callee obligation", "structurally false"])))
        project = projects.project_at(directory, self.root)
        self.assertFalse(projects.rejection_verdict(project, 1, "callee obligation")[0])
        self.assertTrue(projects.rejection_verdict(project, 1, "callee obligation: structurally false")[0])

    def test_invocation_control_requires_run_admission_refusal(self):
        directory = self.negative()
        (directory / "entry_ram.txt").write_text("0=00\n")
        for code, accepted in [(200, True), (1, False), (202, False), (139, False), (0, False)]:
            with self.subTest(code=code), patch.object(native_gate, "run", return_value=subprocess.CompletedProcess([], code, "", "index `4` is within length 4")):
                row = native_gate.measure("omega", str(directory), "linux_x86_64", 10)
                self.assertEqual(row["state"] == "rejected", accepted)

    def test_authored_alternatives_casefold_and_stderr_scope(self):
        directory = self.negative()
        (directory / "sample.json").write_text(json.dumps(dict(
            role="intended-invalid", rejection_contains=["index"],
            rejection_any_contains=["bound", "prove"], rejection_casefold=True)))
        project = projects.project_at(directory, self.root)
        for output, expected in [("INDEX cannot PROVE", True), ("index BOUND", True),
                                 ("index custody denied", False), ("cannot prove package admission", False)]:
            self.assertEqual(projects.rejection_verdict(project, 1, output)[0], expected)
        refusal = subprocess.CompletedProcess([], 1, "INDEX cannot PROVE", "package custody denied")
        with patch.object(check_gate.subprocess, "Popen") as launch:
            process = launch.return_value.__enter__.return_value
            process.returncode = refusal.returncode
            process.communicate.return_value = (refusal.stdout, refusal.stderr)
            self.assertFalse(check_gate.check_one(Path("omega"), self.root / "scratch", directory)["checked"])
        with patch.object(native_gate, "run", return_value=refusal), patch.object(native_gate, "accept_review") as review:
            self.assertEqual(native_gate.measure("omega", str(directory), "linux_x86_64", 10)["state"], "rejection-failed")
            review.assert_not_called()
        (directory / "sample.json").write_text(json.dumps(dict(rejection_any_contains=[""])))
        with self.assertRaisesRegex(ValueError, "nonempty literal"):
            projects.project_at(directory, self.root)
        (directory / "sample.json").write_text(json.dumps(dict(rejection_casefold="yes")))
        with self.assertRaisesRegex(ValueError, "boolean"):
            projects.project_at(directory, self.root)


if __name__ == "__main__":
    unittest.main()
