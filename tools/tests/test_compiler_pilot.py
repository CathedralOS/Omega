"""Refusal controls for the CI adapter; compiler behavior stays in the corpus."""

from contextlib import contextmanager
from copy import deepcopy
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "ci"))
import compiler_pilot as pilot
import compiler_cleanup as cleanup
import compiler_retained_foreign as retained


class CompilerPilotTests(unittest.TestCase):
    def setUp(self):
        # Real Git export and shallow-history controls have their own tests.
        export = patch.object(pilot.compiler_source_export, "export", return_value=[])
        self.source_export = export.start()
        self.addCleanup(export.stop)

    def test_source_export_refusal_stops_before_compiler_build_or_observations(self):
        self.source_export.side_effect = ValueError("incomplete source history")
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            with patch.object(pilot, "command", side_effect=lambda argv: "" if "status" in argv else "a" * 40), \
                    patch.dict(os.environ, {"GITHUB_SHA": "a" * 40}), \
                    patch.object(pilot.corpus_gate, "host_target", return_value="linux_x86_64"), \
                    patch.object(pilot, "prepare_producer") as build, \
                    patch.object(pilot.subprocess, "run") as consume:
                summary = {}
                with self.assertRaisesRegex(ValueError, "incomplete source history"):
                    pilot.execute(output, summary, positives=("pass/group/example",), negatives=())
            build.assert_not_called()
            consume.assert_not_called()
            self.assertEqual(summary["stages"], [dict(name="retain measured source", passed=False,
                                                    error="incomplete source history",
                                                    seconds=summary["stages"][0]["seconds"])])

    def retained_producer(self, root, *, runner=True):
        directory = root / "retained"
        directory.mkdir()
        cli = directory / "omega"
        cli.write_bytes(b"retained optimized CLI")
        corpus = directory / "runner" if runner else None
        if corpus is not None:
            corpus.write_bytes(b"retained optimized runner")
        return pilot.CompilerProducer("a" * 40, "b" * 40, "linux_x86_64", "rustc test toolchain",
                                      cli, corpus, self.retained_profile(cli),
                                      self.retained_profile(corpus) if corpus is not None else None)

    def retained_profile(self, binary):
        return dict(sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                    profile=dict(schema="omega-compiler-build-profile-v1", opt_level="1"))

    @contextmanager
    def producer_environment(self, root, producer):
        consumer = producer.consumer_revision or producer.revision
        tree = producer.tree if producer.consumer_revision is None else "c" * 40
        answers = {
            ("git", "rev-parse", "HEAD"): consumer,
            ("git", "rev-parse", "HEAD^{tree}"): tree,
            ("git", "rev-parse", consumer + "^{tree}"): tree,
            ("git", "status", "--porcelain"): "",
            ("rustc", "-Vv"): producer.toolchain,
        }
        with patch.object(pilot, "ROOT", root), \
                patch.object(pilot, "command", side_effect=lambda args: answers[tuple(args)]), \
                patch.dict(os.environ, {"GITHUB_SHA": consumer}), \
                patch.object(pilot.corpus_gate, "host_target", return_value=producer.target), \
                patch.object(pilot.corpus_gate, "require_optimized", side_effect=self.retained_profile):
            yield

    def test_harness_check_failure_retains_receipts_before_any_executable_build(self):
        import build_slot
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = self.retained_producer(root)
            environment = {"CARGO_TARGET_DIR": str(root / "harness-target")}

            def refuse_harness(arguments, **options):
                options["stdout"].write("checking corpus_runner\n")
                options["stderr"].write("error[E0004]: non-exhaustive patterns\n")
                return Mock(returncode=101)

            with self.producer_environment(root, source), \
                    patch.object(pilot.shutil, "which", return_value="/tools/mbx"), \
                    patch.object(pilot.corpus_gate, "runner_build_environment", return_value=environment), \
                    patch.object(build_slot, "hold") as slot, \
                    patch.object(pilot.subprocess, "run", side_effect=refuse_harness) as run, \
                    patch.object(pilot.corpus_gate, "build_runner") as build_runner, \
                    patch.object(pilot, "execute") as execute:
                self.assertEqual(pilot.run_suite("failed", ("pass/example/exit",), (), phase="prepare"), 1)
            run.assert_called_once()
            self.assertEqual(run.call_args.args[0],
                             ["mbx", "check", "--locked", "-p", "compiler", "--test", "corpus_runner"])
            self.assertEqual(run.call_args.kwargs["cwd"], root)
            self.assertEqual(run.call_args.kwargs["env"], environment)
            self.assertEqual(run.call_args.kwargs["stdin"], pilot.subprocess.DEVNULL)
            slot.assert_called_once_with(root)
            build_runner.assert_not_called()
            execute.assert_not_called()
            output = root / "build/failed"
            preparation = json.loads((output / "preparation.json").read_text())
            self.assertFalse(preparation["passed"])
            self.assertIn("corpus harness check failed (101)", preparation["error"])
            self.assertFalse((output / "producer.json").exists())
            receipts, = output.glob("producer-*")
            self.assertEqual({path.name for path in receipts.iterdir()},
                             {"runner-check.stdout", "runner-check.stderr"})
            self.assertIn("E0004", (receipts / "runner-check.stderr").read_text())
            self.assertEqual((receipts / "runner-check.stdout").read_text(), "checking corpus_runner\n")

    def test_fresh_preparation_checks_only_requested_harness_before_cli_build(self):
        import build_slot
        for need_runner in (False, True):
            with self.subTest(need_runner=need_runner), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                source = self.retained_producer(root)
                Path(str(source.runner) + ".compiler-profile.json").write_text("{}")
                stages = []

                def prepare_command(arguments, **options):
                    if arguments[1] == "check":
                        stages.append("check harness")
                    else:
                        stages.append("build CLI")
                        options["stdout"].write(json.dumps(dict(reason="compiler-artifact",
                            target=dict(name="omega"), executable=str(source.cli))) + "\n")
                        options["stdout"].write(json.dumps(dict(reason="build-finished", success=True)) + "\n")
                    return Mock(returncode=0)

                def build_harness():
                    stages.append("build harness")
                    return source.runner

                with self.producer_environment(root, source), \
                        patch.object(pilot.shutil, "which", return_value=None), \
                        patch.object(build_slot, "hold") as slot, \
                        patch.object(pilot.subprocess, "run", side_effect=prepare_command) as run, \
                        patch.object(pilot.corpus_gate, "build_runner", side_effect=build_harness):
                    prepared = pilot.prepare_producer(root / "prepared", need_runner=need_runner)
                    prepared.verify()
                self.assertEqual(stages, ["check harness", "build CLI", "build harness"]
                                 if need_runner else ["build CLI"])
                self.assertEqual(slot.call_count, 2 if need_runner else 1)
                self.assertTrue(all(call.args[0][0] == "cargo" for call in run.call_args_list))
                self.assertEqual(prepared.cli_profile, source.cli_profile)
                self.assertEqual(prepared.runner_profile, source.runner_profile if need_runner else None)
                self.assertEqual(prepared.runner is not None, need_runner)
                self.assertEqual((prepared.cli.parent / "runner-check.stdout").exists(), need_runner)
                for binary, profile in prepared._artifacts():
                    recorded = json.loads(Path(str(binary) + ".verified-profile.json").read_text())
                    self.assertEqual(recorded["source_revision"], source.revision)
                    self.assertEqual(recorded["observation"], profile)

    def test_prepared_manifest_round_trip_checks_both_and_cli_only_producers(self):
        for runner in (False, True):
            with self.subTest(runner=runner), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                producer = self.retained_producer(root, runner=runner)
                manifest = root / "producer.json"
                record = dict(schema="omega-compiler-producer-v1", producer=producer.record())
                pilot.write_new_record(manifest, record)
                with self.producer_environment(root, producer):
                    self.assertEqual(pilot.load_producer(manifest), producer)
                with self.assertRaises(FileExistsError):
                    pilot.write_new_record(manifest, record)

    def test_manifest_refuses_changed_source_host_toolchain_profile_and_runner_custody(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            producer = self.retained_producer(root)
            manifest = root / "producer.json"
            original = dict(schema="omega-compiler-producer-v1", producer=producer.record())
            changes = (
                ("revision", "c" * 40), ("tree", "c" * 40), ("target", "macos_arm64"),
                ("toolchain", "another rustc"), ("runner", None), ("runner_profile", None),
                ("cli_profile", dict(sha256="c" * 64, opt_level="1")),
                ("cli_profile", {}), ("cli", "relative/omega"),
            )
            with self.producer_environment(root, producer):
                for field, value in changes:
                    record = deepcopy(original)
                    record["producer"][field] = value
                    manifest.write_text(json.dumps(record))
                    with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                        pilot.load_producer(manifest)
                manifest.write_text(json.dumps(original))
                producer.cli.write_bytes(b"different compiler")
                with self.assertRaisesRegex(ValueError, "identity/profile changed"):
                    pilot.load_producer(manifest)
                producer.cli.unlink()
                with self.assertRaises(OSError):
                    pilot.load_producer(manifest)

    def test_prepare_only_builds_once_and_consumer_failure_preserves_preparation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            producer = self.retained_producer(root)
            with self.producer_environment(root, producer), \
                    patch.object(pilot, "prepare_producer", return_value=producer) as build, \
                    patch.object(pilot, "execute") as execute:
                self.assertEqual(pilot.run_suite("split", ("pass/example/exit",), (), phase="prepare"), 0)
                output = root / "build/split"
                build.assert_called_once_with(output, need_runner=True)
                execute.assert_not_called()
                preparation = (output / "preparation.json").read_bytes()
                manifest = (output / "producer.json").read_bytes()
                self.assertTrue(json.loads(preparation)["passed"])
                self.assertEqual(pilot.run_suite("split", ("pass/example/exit",), (), phase="prepare"), 1)
                build.assert_called_once()
                build.reset_mock()
                execute.side_effect = ValueError("retained checking refusal")
                self.assertEqual(pilot.run_suite("split", ("pass/example/exit",), (), phase="consume"), 1)
                build.assert_not_called()
                self.assertEqual(execute.call_args.kwargs["producer"], producer)
                self.assertFalse(json.loads((output / "summary.json").read_text())["passed"])
                self.assertEqual((output / "preparation.json").read_bytes(), preparation)
                self.assertEqual((output / "producer.json").read_bytes(), manifest)

    def test_invalid_preparation_request_never_builds_and_keeps_its_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(pilot, "ROOT", root), \
                    patch.object(pilot, "prepare_producer") as build, \
                    patch.object(pilot, "execute") as execute:
                self.assertEqual(pilot.run_suite("invalid", (), (), fixtures="../escape", phase="prepare"), 1)
                build.assert_not_called()
                execute.assert_not_called()
            output = root / "build/invalid"
            self.assertFalse(json.loads((output / "preparation.json").read_text())["passed"])
            self.assertFalse((output / "producer.json").exists())

    def test_failed_preparation_retains_error_and_consume_never_falls_back_to_build(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            producer = self.retained_producer(root)
            with self.producer_environment(root, producer), \
                    patch.object(pilot, "prepare_producer", side_effect=ValueError("CLI build refused")) as build, \
                    patch.object(pilot, "execute") as execute:
                self.assertEqual(pilot.run_suite("failed", ("pass/example/exit",), (), phase="prepare"), 1)
                build.assert_called_once()
                build.reset_mock()
                output = root / "build/failed"
                preparation = (output / "preparation.json").read_bytes()
                self.assertEqual(json.loads(preparation)["error"], "CLI build refused")
                self.assertEqual(pilot.run_suite("failed", ("pass/example/exit",), (), phase="consume"), 1)
                build.assert_not_called()
                execute.assert_not_called()
                self.assertEqual((output / "preparation.json").read_bytes(), preparation)

    def test_consume_requires_the_runner_for_corpus_without_building_one(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            producer = self.retained_producer(root, runner=False)
            output = root / "build/cli-only"
            output.mkdir(parents=True)
            pilot.write_new_record(output / "producer.json",
                                   dict(schema="omega-compiler-producer-v1", producer=producer.record()))
            with self.producer_environment(root, producer), \
                    patch.object(pilot, "prepare_producer") as build, \
                    patch.object(pilot.subprocess, "run") as run:
                self.assertEqual(pilot.run_suite("cli-only", ("pass/example/exit",), (), phase="consume"), 1)
                build.assert_not_called()
                run.assert_not_called()
            summary = json.loads((output / "summary.json").read_text())
            self.assertIn("requires a retained optimized corpus runner", summary["error"])

    def test_command_phase_defaults_to_all_and_forwards_explicit_selection(self):
        with patch.object(pilot, "run_suite", return_value=0) as suite:
            self.assertEqual(pilot.main([]), 0)
            self.assertEqual(suite.call_args.kwargs["phase"], "all")
            for phase in ("prepare", "consume"):
                self.assertEqual(pilot.main(["--phase", phase, "--fixtures", "pass/example/exit"]), 0)
                self.assertEqual(suite.call_args.kwargs["phase"], phase)
                self.assertEqual(suite.call_args.kwargs["fixtures"], "pass/example/exit")

    def test_consume_refuses_unoptimized_or_changed_binary_without_building(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            producer = self.retained_producer(root)
            output = root / "build/prepared"
            output.mkdir(parents=True)
            pilot.write_new_record(output / "producer.json",
                                   dict(schema="omega-compiler-producer-v1", producer=producer.record()))
            with self.producer_environment(root, producer), \
                    patch.object(pilot, "prepare_producer") as build, \
                    patch.object(pilot, "execute") as execute:
                with patch.object(pilot.corpus_gate, "require_optimized", side_effect=ValueError("unoptimized")):
                    self.assertEqual(pilot.run_suite("prepared", ("pass/example/exit",), (), phase="consume"), 1)
                producer.runner.write_bytes(b"changed runner")
                self.assertEqual(pilot.run_suite("prepared", ("pass/example/exit",), (), phase="consume"), 1)
                build.assert_not_called()
                execute.assert_not_called()


    def test_prepare_export_refusal_retains_failure_without_building(self):
        self.source_export.side_effect = ValueError("incomplete source history")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            producer = self.retained_producer(root)
            with self.producer_environment(root, producer), \
                    patch.object(pilot, "prepare_producer") as build, \
                    patch.object(pilot, "execute") as execute:
                self.assertEqual(pilot.run_suite("export-failure", ("pass/example/exit",), (), phase="prepare"), 1)
                build.assert_not_called()
                execute.assert_not_called()
            output = root / "build/export-failure"
            self.assertFalse((output / "producer.json").exists())
            self.assertIn("incomplete source history", json.loads((output / "preparation.json").read_text())["error"])

    def test_split_replay_preserves_identity_and_preparation_without_building(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            producer = self.retained_producer(root)
            inputs = {name: None for name in pilot.compiler_replay_plan.COMPILER_INPUTS}
            producer = pilot.CompilerProducer(
                producer.revision, producer.tree, producer.target, producer.toolchain,
                producer.cli, producer.runner, producer.cli_profile, producer.runner_profile,
                "d" * 40, inputs,
                dict(run=123, cache_key="retained-cache", plan="replay.json", transport="existing workflow cache"))
            plan = pilot.compiler_replay_plan.ReplayPlan("replay.json", dict(
                revision=producer.revision, tree=producer.tree, target=producer.target,
                toolchain=producer.toolchain, input_objects=inputs, origin_run=123, cache_key="retained-cache",
                executables=dict(cli=producer.cli_profile, runner=producer.runner_profile)),
                ("pass/example/exit",), True, (), ())
            order = []
            self.source_export.side_effect = lambda *args: order.append("source") or []
            def capture(*args):
                order.append("capture")
                return producer
            with self.producer_environment(root, producer), \
                    patch.object(pilot.compiler_replay_plan, "load", return_value=plan), \
                    patch.object(pilot.compiler_replay_plan, "check_inputs") as inputs_checked, \
                    patch.object(pilot, "select_fixtures", return_value=(("pass/example/exit",), ())), \
                    patch.object(pilot, "retained_producer", side_effect=capture) as retained, \
                    patch.object(pilot, "prepare_producer") as build, \
                    patch.object(pilot, "execute", side_effect=ValueError("actual consumer refusal")) as execute:
                self.assertEqual(pilot.run_suite("replay", (), (), fixtures="@replay.json", phase="prepare"), 0)
                output = root / "build/replay"
                preparation = (output / "preparation.json").read_bytes()
                manifest = (output / "producer.json").read_bytes()
                self.assertEqual(order, ["source", "capture"])
                self.source_export.assert_called_once_with(output, (producer.consumer_revision, producer.revision))
                self.assertEqual(json.loads(manifest)["producer"]["revision"], producer.revision)
                self.assertEqual(json.loads(manifest)["producer"]["consumer_revision"], producer.consumer_revision)
                self.assertEqual(pilot.run_suite("replay", (), (), fixtures="@replay.json", phase="consume"), 1)
                build.assert_not_called()
                retained.assert_called_once()
                self.assertEqual(execute.call_args.kwargs["producer"], producer)
                self.assertEqual(execute.call_args.kwargs["checking_only"], ("pass/example/exit",))
                self.assertFalse(execute.call_args.kwargs["positives"])
                self.assertEqual((output / "preparation.json").read_bytes(), preparation)
                self.assertEqual((output / "producer.json").read_bytes(), manifest)
                inputs_checked.assert_called()
                # A different requested binary cannot replace the prepared one.
                different = deepcopy(plan.producer)
                different["executables"]["cli"]["sha256"] = "f" * 64
                changed = pilot.compiler_replay_plan.ReplayPlan(plan.path, different, plan.fixtures, True, (), ())
                with patch.object(pilot.compiler_replay_plan, "load", return_value=changed):
                    execute.reset_mock()
                    self.assertEqual(pilot.run_suite("replay", (), (), fixtures="@replay.json", phase="consume"), 1)
                    execute.assert_not_called()
                build.assert_not_called()

    def test_ordinary_cli_copy_loop_retains_the_actual_project_directory(self):
        sample = "samples/cli/systems/journal"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.sample_root(root, sample)
            library = root / "source/library/std/last.omg"
            library.parent.mkdir(parents=True)
            library.write_text("data Last {}")
            dependency = "pass/packages/input_library"
            self.fixture_root(root, dependency)
            dependency_path = root / "tests/omega" / dependency
            (dependency_path / "build.omg").write_text("library Build")
            paths = [sample + "/main.omg", sample + "/build.omg", "source/library/std/last.omg",
                     "tests/omega/" + dependency + "/main.omg",
                     "tests/omega/" + dependency + "/build.omg"]
            output = root / "observation"
            producer = Mock(cli=Path("/retained/omega"), target="windows_x86_64")
            producer.record.return_value = {"revision": "a" * 40}
            for index, diagnostics in enumerate(({}, {"OMEGA_PROVIDER_DEBUG": "1",
                                                       "OMEGA_COMPILER_CHECK_SITES": "1"})):
                output = root / f"observation-{index}"
                expected_project = output / "workspace" / sample

                def refuse_update(arguments, **kwargs):
                    self.assertEqual(arguments[1:4], ["update", "--project", expected_project])
                    self.assertTrue((expected_project / "build.omg").is_file())
                    self.assertTrue((output / "workspace/source/library/std/last.omg").is_file())
                    self.assertEqual(
                        (output / "workspace/tests/omega" / dependency / "main.omg").read_bytes(),
                        (dependency_path / "main.omg").read_bytes())
                    for name in ("OMEGA_PROVIDER_DEBUG", "OMEGA_COMPILER_CHECK_SITES"):
                        self.assertEqual(kwargs["env"].get(name), diagnostics.get(name))
                    return Mock(returncode=1)

                with self.subTest(diagnostics=diagnostics), patch.dict(os.environ, diagnostics, clear=True), \
                        patch.object(pilot.ordinary_cli_fixture.subprocess, "check_output", return_value="\n".join(paths)), \
                        patch.object(pilot.ordinary_cli_fixture.subprocess, "run", side_effect=refuse_update) as run:
                    row = pilot.ordinary_cli_fixture.observe(
                        root, producer, sample, output, dependency_projects=(dependency,))
                    self.assertEqual(row["dependency_projects"], ["tests/omega/" + dependency])
                    run.assert_called_once()
                    self.assertEqual(row["stages"][0]["arguments"][3], str(expected_project))
                    self.assertFalse(row["passed"])

    def test_ordinary_cli_uses_exact_product_sample_and_its_authored_exit(self):
        sample = "samples/cli/systems/journal"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.sample_root(root, sample, sample + "/nested")
            project = root / sample
            (project / "main.omg").write_text("// Expected exit: 7.\nmachine main() {}")
            with patch.object(pilot, "ROOT", root):
                self.assertEqual(pilot.select_cli_fixtures(sample), (sample,))
                self.assertEqual(pilot.ordinary_cli_fixture.expected_exit(root, sample, "windows_x86_64"), 7)
                self.assertEqual(pilot.ordinary_cli_fixture.relative_project(root, sample), Path(sample))
                for invalid in (sample + "/../journal", sample + "," + sample,
                                "/" + sample, sample.replace("/", "\\")):
                    with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                        pilot.select_cli_fixtures(invalid)

    def test_ordinary_cli_product_samples_refuse_nonruntime_roles_and_extra_oracles(self):
        sample = "samples/cli/systems/journal"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.sample_root(root, sample)
            project = root / sample
            with patch.object(pilot, "ROOT", root):
                for role in ("intended-invalid", "library", "check-only"):
                    (project / "sample.json").write_text(json.dumps({"role": role}))
                    with self.subTest(role=role), self.assertRaises(ValueError):
                        pilot.select_cli_fixtures(sample)
                (project / "other.omg").write_text("machine other() {}")
                (project / "sample.json").write_text(json.dumps({"entry": "other.omg"}))
                with self.assertRaises(ValueError):
                    pilot.select_cli_fixtures(sample)
                (project / "sample.json").unlink()
                (project / "expected_stdout.txt").write_text("must be checked")
                with self.assertRaises(ValueError):
                    pilot.select_cli_fixtures(sample)
                (project / "expected_stdout.txt").unlink()
                (project / "main.omg").write_text("machine main() {}")
                with self.assertRaises(ValueError):
                    pilot.select_cli_fixtures(sample)

    def test_checking_only_retains_exact_roster_without_execution_defaults(self):
        positive, negative = "pass/copy/value", "fail/copy/value"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture_root(root, positive, negative)
            with patch.object(pilot, "ROOT", root), patch.object(pilot, "execute") as execute:
                request = positive + "," + negative
                self.assertEqual(pilot.run_suite("checks", pilot.POSITIVES, pilot.NEGATIVES,
                                                fixtures=request, check_only=True), 0)
                self.assertEqual(execute.call_args.kwargs["positives"], ())
                self.assertEqual(execute.call_args.kwargs["checking_only"], (positive,))
                self.assertEqual(execute.call_args.kwargs["negatives"], (negative,))
                for selectors in ({}, {"fixtures": request, "samples": "samples/cli/facts/copy"},
                                  {"fixtures": request, "cli_fixtures": positive}):
                    execute.reset_mock()
                    self.assertEqual(pilot.run_suite("refused", pilot.POSITIVES, pilot.NEGATIVES,
                                                    check_only=True, **selectors), 1)
                    execute.assert_not_called()

    def fixture_root(self, root, *fixtures):
        for fixture in fixtures:
            directory = root / "tests" / "omega" / fixture
            directory.mkdir(parents=True, exist_ok=True)
            (directory / "main.omg").write_text("machine main() {}", encoding="utf-8")

    def sample_root(self, root, *samples):
        for sample in samples:
            directory = root / sample
            directory.mkdir(parents=True, exist_ok=True)
            (directory / "main.omg").write_text("// Expected exit: 0\nmachine main() {}")
            (directory / "build.omg").write_text("ProgramEntry")

    def test_ordinary_cli_selection_is_exact_and_never_replaces_refusal_with_smoke(self):
        fixture = "pass/filesystem/normal_exit"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture_root(root, fixture)
            project = root / "tests/omega" / fixture
            (project / "build.omg").write_text("ProgramEntry")
            (project / "expected_native_exit.txt").write_text("70")
            with patch.object(pilot, "ROOT", root), patch.object(pilot, "execute") as execute:
                self.assertEqual(pilot.run_suite("cli", pilot.POSITIVES, pilot.NEGATIVES,
                                                cli_fixtures=fixture), 0)
                self.assertEqual(execute.call_args.kwargs["positives"], ())
                self.assertEqual(execute.call_args.kwargs["cli_fixtures"], (fixture,))
                execute.reset_mock()
                (project / "expected_stdout.windows_x86_64.txt").write_text("must be checked")
                self.assertEqual(pilot.run_suite("bad", pilot.POSITIVES, pilot.NEGATIVES,
                                                cli_fixtures=fixture), 1)
                execute.assert_not_called()

    def test_ordinary_cli_inspection_oracles_are_preserved_or_refused_before_build(self):
        with tempfile.TemporaryDirectory() as temporary:
            project = Path(temporary)
            oracle = project / "expected_terminal_inspections.txt"
            oracle.write_text("Main::main\nOther::read\n")
            self.assertEqual(pilot.ordinary_cli_fixture.inspection_machines(project),
                             ("Main::main", "Other::read"))
            for unsupported in ("", "Main::main debug", "Main::main runtime portable filesystem",
                                "Main::main native-layout-controls"):
                oracle.write_text(unsupported)
                with self.subTest(oracle=unsupported), self.assertRaises(ValueError):
                    pilot.ordinary_cli_fixture.inspection_machines(project)

    def test_named_inspection_requires_machine_and_same_reloaded_identity(self):
        validate = pilot.ordinary_cli_fixture.validate_inspection
        identity = "a" * 64
        source = f"terminal selected_machine=Main::main entry=MachineId(1)\nterminal program_fingerprint={identity}\n"
        restored = f"terminal selected_machine=artifact entry=MachineId(1)\nterminal program_fingerprint={identity}\n"
        self.assertEqual(validate("Main::main", source, restored), identity)
        for wrong_source, wrong_artifact in ((source.replace("Main::main", "Other::main"), restored),
                                            (source, restored.replace(identity, "b" * 64)),
                                            (source, ""), (source + source, restored), (source, source)):
            with self.subTest(source=wrong_source, artifact=wrong_artifact), self.assertRaises(ValueError):
                validate("Main::main", wrong_source, wrong_artifact)

    def test_ordinary_cli_requires_explicit_terminal_exit_including_nonzero_oracles(self):
        validate = pilot.ordinary_cli_fixture.validate_exits
        validate(70, "native exit 70\ninterp exit: 70\n", 70)
        for code, stderr in ((0, "native exit 0\ninterp: DECLINED\n"),
                             (0, "native exit 0\n"),
                             (0, "native exit 0\ninterp exit: 1\n"),
                             (0, "native exit 0\ninterp exit: 0\ninterp: TRAP\n"),
                             (1, "native exit 0\ninterp exit: 0\n")):
            with self.subTest(code=code, stderr=stderr), self.assertRaisesRegex(ValueError, "agreement"):
                validate(code, stderr, 0)

    def test_fact_selection_includes_nested_control_but_refuses_filter_collision(self):
        sample = "samples/cli/facts/layout/local"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.sample_root(root, sample, sample + "/invalid")
            with patch.object(pilot, "ROOT", root):
                self.assertEqual(pilot.select_samples(sample), (sample, sample + "/invalid"))
                self.sample_root(root, sample + "_extra")
                with self.assertRaisesRegex(ValueError, "unrequested"):
                    pilot.select_samples(sample)

    def test_invalid_fact_selection_never_builds_or_runs_smoke(self):
        sample = "samples/cli/facts/layout/local"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.sample_root(root, sample)
            (root / "samples/cli/facts/layout/no_entry").mkdir()
            (root / "samples/cli/facts/layout/no_entry/build.omg").write_text("ProgramEntry")
            if os.name != "nt":
                (root / "samples/cli/facts/escape").symlink_to(root.parent, target_is_directory=True)
            with patch.object(pilot, "ROOT", root), patch.object(pilot, "execute") as execute, \
                    patch.object(pilot, "prepare_producer") as build:
                for request in (17, "", " ", sample + ",", sample + "," + sample, "/" + sample,
                                "samples/cli/other", "samples/cli/facts/layout/../local",
                                "samples/cli/facts//layout/local", "samples/cli/facts/./layout/local",
                                "samples\\cli\\facts\\layout\\local", "samples/cli/facts/missing",
                                "samples/cli/facts/layout/no_entry", "samples/cli/facts/escape"):
                    with self.subTest(request=request):
                        self.assertEqual(pilot.run_suite("refused", pilot.POSITIVES, pilot.NEGATIVES,
                                                        samples=request), 1)
                execute.assert_not_called()
                build.assert_not_called()

    def test_sample_only_replaces_defaults_and_explicit_fixtures_can_share_it(self):
        sample = "samples/cli/facts/layout/local"
        fixture = "pass/layout/local"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.sample_root(root, sample)
            self.fixture_root(root, fixture)
            with patch.object(pilot, "ROOT", root), patch.object(pilot, "execute") as execute:
                self.assertEqual(pilot.run_suite("samples", pilot.POSITIVES, pilot.NEGATIVES,
                                                samples=sample), 0)
                self.assertEqual(execute.call_args.kwargs["positives"], ())
                self.assertEqual(execute.call_args.kwargs["negatives"], ())
                self.assertEqual(execute.call_args.kwargs["samples"], (sample,))
                self.assertEqual(pilot.run_suite("both", pilot.POSITIVES, pilot.NEGATIVES,
                                                fixtures=fixture, samples=sample), 0)
                self.assertEqual(execute.call_args.kwargs["positives"], (fixture,))
        with patch.dict(os.environ, {pilot.SAMPLE_ENVIRONMENT: sample}), \
                patch.object(pilot, "run_suite", return_value=0) as suite:
            pilot.main([])
            self.assertEqual(suite.call_args.kwargs["samples"], sample)
            pilot.main(["--samples", ""])
            self.assertEqual(suite.call_args.kwargs["samples"], "")

    def test_sample_consumer_uses_existing_cli_and_refuses_vacuous_or_unscored_evidence(self):
        sample = "samples/cli/facts/layout/local"
        accepted = [dict(sample=sample, state="pass"),
                    dict(sample=sample + "/invalid", state="rejected", role="intended-invalid")]
        producer = Mock(consumer_revision=None, revision="a" * 40, target="linux_x86_64", cli=Path("/retained/omega"),
                        runner=None, runner_profile=None)
        for rows, code in ((accepted, 0), ([], 0), (accepted[:1], 0),
                           ([accepted[0], accepted[0]], 0), (accepted, 1),
                           ([dict(sample=sample, state="no-oracle"), accepted[1]], 0),
                           ([dict(sample=sample, state="checked"), accepted[1]], 0),
                           ([dict(sample=sample, state="pass"), dict(accepted[1], state="rejection-failed")], 0)):
            with self.subTest(rows=rows, code=code), tempfile.TemporaryDirectory() as temporary:
                output = Path(temporary)
                def consume(arguments, **kwargs):
                    self.assertEqual(arguments[1:4], ["tools/sample_native_gate.py", "--cli", str(producer.cli)])
                    self.assertIn("--facts", arguments)
                    self.assertEqual(arguments[arguments.index("--jobs") + 1], "2")
                    self.assertEqual(arguments[arguments.index("--target") + 1], producer.target)
                    self.assertEqual(arguments[arguments.index("--filter") + 1], sample + "," + sample + "/invalid")
                    Path(arguments[arguments.index("--write-results") + 1]).write_text(json.dumps(rows))
                    return Mock(returncode=code)
                with patch.object(pilot, "command", side_effect=lambda argv: "" if "status" in argv else "a" * 40), \
                        patch.dict(os.environ, {"GITHUB_SHA": "a" * 40}), \
                        patch.object(pilot.corpus_gate, "host_target", return_value=producer.target), \
                        patch.object(pilot.subprocess, "run", side_effect=consume) as run, \
                        patch.object(pilot, "prepare_producer") as build:
                    arguments = dict(positives=(), negatives=(), producer=producer,
                                     samples=(sample, sample + "/invalid"))
                    if rows == accepted and code == 0:
                        summary = {}
                        pilot.execute(output, summary, **arguments)
                        self.assertEqual(summary["sample_observations"], accepted)
                    else:
                        with self.assertRaisesRegex(ValueError, "runtime/oracle/rejection"):
                            pilot.execute(output, {}, **arguments)
                    run.assert_called_once()
                    build.assert_not_called()

    def test_consumer_selection_prepares_only_required_executables(self):
        sample = "samples/cli/facts/layout/local"
        fixture = "fail/example/intended_rejection"
        for samples, negatives in (((sample,), ()), ((sample,), (fixture,)), ((), (fixture,))):
            with self.subTest(samples=samples, negatives=negatives), tempfile.TemporaryDirectory() as temporary:
                output = Path(temporary)
                producer = Mock(consumer_revision=None, revision="a" * 40, target="linux_x86_64", cli=output / "omega",
                                runner=output / "runner" if negatives else None,
                                runner_profile={"sha256": "b" * 64} if negatives else None)
                def consume(arguments, **kwargs):
                    if "--write-results" in arguments:
                        Path(arguments[arguments.index("--write-results") + 1]).write_text(
                            json.dumps([dict(sample=sample, state="pass")]))
                    else:
                        report = self.report()
                        report["observations"] = [dict(fixture=fixture, status="rejected", expected_satisfied=True)]
                        Path(arguments[arguments.index("--json") + 1]).write_text(json.dumps(report))
                    return Mock(returncode=0)
                with patch.object(pilot, "command", side_effect=lambda argv: "" if "status" in argv else "a" * 40), \
                        patch.dict(os.environ, {"GITHUB_SHA": "a" * 40}), \
                        patch.object(pilot.corpus_gate, "host_target", return_value=producer.target), \
                        patch.object(pilot, "observation_golden", return_value=output / "golden"), \
                        patch.object(pilot.subprocess, "run", side_effect=consume) as run, \
                        patch.object(pilot, "prepare_producer", return_value=producer) as build:
                    summary = {}
                    pilot.execute(output, summary, positives=(), negatives=negatives, samples=samples)
                build.assert_called_once_with(output, need_runner=bool(negatives))
                self.assertEqual(run.call_count, bool(samples) + bool(negatives))
                self.assertEqual(producer.verify.call_count, 2 + bool(samples) + bool(negatives))
                self.assertEqual("runner_sha256" in summary, bool(negatives))

    def test_failed_corpus_consumers_still_measure_cli_and_samples_and_keep_run_red(self):
        fixture = "pass/example/normal_exit"
        sample = "samples/cli/facts/layout/local"
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            producer = Mock(consumer_revision=None, revision="a" * 40, target="linux_x86_64",
                            runner_profile={"sha256": "b" * 64})

            def consume(arguments, **kwargs):
                if "--write-results" in arguments:
                    Path(arguments[arguments.index("--write-results") + 1]).write_text(
                        json.dumps([dict(sample=sample, state="pass")]))
                else:
                    Path(arguments[arguments.index("--json") + 1]).write_text("{}")
                return Mock(returncode=0)

            with patch.object(pilot, "command", side_effect=lambda argv: "" if "status" in argv else "a" * 40), \
                    patch.dict(os.environ, {"GITHUB_SHA": "a" * 40}), \
                    patch.object(pilot.corpus_gate, "host_target", return_value=producer.target), \
                    patch.object(pilot, "observation_golden", return_value=output / "golden"), \
                    patch.object(pilot.subprocess, "run", side_effect=consume) as run, \
                    patch.object(pilot, "validate_report", side_effect=[ValueError("checking timeout"), ValueError("runtime timeout")]), \
                    patch.object(pilot.ordinary_cli_fixture, "observe", return_value={"passed": True}) as ordinary:
                summary = {}
                with self.assertRaisesRegex(ValueError, "checking timeout.*runtime timeout"):
                    pilot.execute(output, summary, positives=(fixture,), negatives=(),
                                  producer=producer, samples=(sample,), cli_fixtures=(fixture,))
            self.assertEqual(run.call_count, 3)
            ordinary.assert_called_once()
            self.assertEqual([row["passed"] for row in summary["stages"]],
                             [True, True, False, False, True, True])
            self.assertEqual(summary["stages"][2]["error"], "checking timeout")
            self.assertEqual(summary["stages"][3]["error"], "runtime timeout")
            self.assertEqual(summary["sample_observations"], [dict(sample=sample, state="pass")])

    def test_changed_producer_stops_before_another_consumer(self):
        fixture = "pass/example/normal_exit"
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            producer = Mock(consumer_revision=None, revision="a" * 40, target="linux_x86_64",
                            runner_profile={"sha256": "b" * 64})
            producer.verify.side_effect = [None, ValueError("compiler bytes changed")]

            def consume(arguments, **kwargs):
                Path(arguments[arguments.index("--json") + 1]).write_text("{}")
                return Mock(returncode=0)

            with patch.object(pilot, "command", side_effect=lambda argv: "" if "status" in argv else "a" * 40), \
                    patch.dict(os.environ, {"GITHUB_SHA": "a" * 40}), \
                    patch.object(pilot.corpus_gate, "host_target", return_value=producer.target), \
                    patch.object(pilot, "observation_golden", return_value=output / "golden"), \
                    patch.object(pilot.subprocess, "run", side_effect=consume) as run, \
                    patch.object(pilot, "validate_report", side_effect=ValueError("checking timeout")), \
                    patch.object(pilot.ordinary_cli_fixture, "observe") as ordinary:
                with self.assertRaisesRegex(ValueError, "compiler bytes changed"):
                    pilot.execute(output, {}, positives=(fixture,), negatives=(), producer=producer,
                                  cli_fixtures=(fixture,))
            run.assert_called_once()
            ordinary.assert_not_called()

    def test_ordinary_cli_checks_through_package_admission_without_corpus(self):
        fixture = "pass/filesystem/write_all_full_device_exit"
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            producer = Mock(consumer_revision=None, revision="a" * 40, target="linux_x86_64", runner=None, runner_profile=None)
            with patch.object(pilot, "command", side_effect=lambda argv: "" if "status" in argv else "a" * 40), \
                    patch.dict(os.environ, {"GITHUB_SHA": "a" * 40}), \
                    patch.object(pilot.corpus_gate, "host_target", return_value=producer.target), \
                    patch.object(pilot.subprocess, "run") as run, \
                    patch.object(pilot, "prepare_producer", return_value=producer) as build, \
                    patch.object(pilot.ordinary_cli_fixture, "observe", return_value={"passed": True}) as observe:
                pilot.execute(output, {}, positives=(), negatives=(), cli_fixtures=(fixture,))
            build.assert_called_once_with(output, need_runner=False)
            observe.assert_called_once_with(pilot.ROOT, producer, fixture, output / "ordinary-cli" / fixture)
            run.assert_not_called()

    def test_cli_only_producer_cannot_execute_corpus_fixtures(self):
        producer = Mock(consumer_revision=None, revision="a" * 40, target="linux_x86_64", runner=None, runner_profile=None)
        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(pilot, "command", side_effect=lambda argv: "" if "status" in argv else "a" * 40), \
                patch.dict(os.environ, {"GITHUB_SHA": "a" * 40}), \
                patch.object(pilot.corpus_gate, "host_target", return_value=producer.target), \
                patch.object(pilot.subprocess, "run") as run:
            with self.assertRaisesRegex(ValueError, "requires a retained optimized corpus runner"):
                pilot.execute(Path(temporary), {}, positives=(), negatives=("fail/example/reject",), producer=producer)
            run.assert_not_called()

    def test_exact_fixture_ids_split_runtime_and_rejection_rosters(self):
        positive = "pass/float/named_conversion"
        negative = "fail/float/unproved_conversion"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture_root(root, positive, negative)
            with patch.object(pilot, "ROOT", root):
                self.assertEqual(pilot.select_fixtures(f" {negative}, {positive} "),
                                 ((positive,), (negative,)))

    def test_run_fixture_keeps_exact_selection_and_requires_both_engines(self):
        fixture = "run/borrows/conditional_origins"
        negative = "fail/borrows/local_escape"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture_root(root, fixture, negative)
            (root / "tests/omega" / fixture / "expected_native_exit.txt").write_text("0\n")
            with patch.object(pilot, "ROOT", root), \
                    patch.object(pilot, "prepare_producer") as build:
                self.assertEqual(pilot.select_fixtures(fixture + "," + negative),
                                 ((fixture,), (negative,)))
                report = self.report(both=True)
                report["observations"] = [dict(fixture=fixture, status="built",
                    facts={"exit": "0", "interpreter-exit": "0", "interpreter": "agrees"})]
                pilot.validate_report(report, (fixture,), both=True,
                                      revision="a" * 40, target="linux_x86_64")
                report["observations"][0]["facts"]["interpreter"] = "unchecked"
                with self.assertRaisesRegex(ValueError, "missing native/Terminal agreement"):
                    pilot.validate_report(report, (fixture,), both=True,
                                          revision="a" * 40, target="linux_x86_64")
                self.fixture_root(root, fixture + "_extra")
                with self.assertRaisesRegex(ValueError, "unrequested fixtures"):
                    pilot.select_fixtures(fixture)
                build.assert_not_called()

    def test_nested_fact_ids_keep_exact_selection_and_refusals(self):
        positive = "pass/facts/invocation_lifetime/local"
        negative = "fail/facts/invocation_lifetime/local"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture_root(root, positive, negative)
            with patch.object(pilot, "ROOT", root), \
                    patch.object(pilot, "prepare_producer") as build:
                self.assertEqual(pilot.select_fixtures(positive + "," + negative),
                                 ((positive,), (negative,)))
                for invalid in ("pass/facts/invocation_lifetime/../local",
                                "pass/facts//invocation_lifetime/local",
                                "pass/facts/invocation_lifetime/missing"):
                    with self.subTest(request=invalid), self.assertRaises(ValueError):
                        pilot.select_fixtures(invalid)
                self.fixture_root(root, positive + "_extra")
                with self.assertRaisesRegex(ValueError, "unrequested fixtures"):
                    pilot.select_fixtures(positive)
                self.assertEqual(pilot.select_fixtures(positive + "," + positive + "_extra"),
                                 ((positive, positive + "_extra"), ()))
                build.assert_not_called()

    def test_malformed_or_missing_exact_id_never_builds_or_runs_defaults(self):
        fixture = "pass/float/named_conversion"
        invalid = ("", " ", ",", fixture + ",", fixture + "," + fixture,
                   "float", "run/float/named_conversion", "/" + fixture,
                   "pass/float/../named_conversion", "pass//float/named_conversion",
                   "pass/float/./named_conversion", "pass\\float\\named_conversion",
                   "tests/omega/" + fixture, "pass/float/missing")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture_root(root, fixture)
            with patch.object(pilot, "ROOT", root), \
                    patch.object(pilot, "execute") as execute, \
                    patch.object(pilot, "prepare_producer") as build:
                for request in invalid:
                    with self.subTest(request=request):
                        self.assertEqual(pilot.run_suite(
                            "refused", pilot.POSITIVES, pilot.NEGATIVES, fixtures=request), 1)
                        summary = json.loads((root / "build/refused/summary.json").read_text())
                        self.assertFalse(summary["passed"])
                        self.assertTrue(summary["error"])
                execute.assert_not_called()
                build.assert_not_called()

    def test_exact_selector_refuses_substring_collision_before_build(self):
        fixture = "pass/float/named_conversion"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture_root(root, fixture, fixture + "_extra")
            with patch.object(pilot, "ROOT", root), \
                    patch.object(pilot, "prepare_producer") as build:
                with self.assertRaisesRegex(ValueError, "unrequested fixtures"):
                    pilot.select_fixtures(fixture)
                self.assertEqual(pilot.select_fixtures(fixture + "," + fixture + "_extra"),
                                 ((fixture, fixture + "_extra"), ()))
                build.assert_not_called()

    def test_existing_directory_without_main_is_not_a_fixture(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "tests/omega/pass/float/not_a_fixture").mkdir(parents=True)
            with patch.object(pilot, "ROOT", root):
                with self.assertRaisesRegex(ValueError, "main.omg"):
                    pilot.select_fixtures("pass/float/not_a_fixture")

    def test_cli_and_environment_requests_reach_the_same_suite(self):
        with patch.dict(os.environ, {pilot.FIXTURE_ENVIRONMENT: "pass/float/from_environment"}), \
                patch.object(pilot, "run_suite", return_value=0) as suite:
            self.assertEqual(pilot.main([]), 0)
            self.assertEqual(suite.call_args.kwargs["fixtures"], "pass/float/from_environment")
            self.assertEqual(pilot.main(["--fixtures", "fail/float/from_cli"]), 0)
            self.assertEqual(suite.call_args.kwargs["fixtures"], "fail/float/from_cli")
            self.assertEqual(pilot.main(["--fixtures", ""]), 0)
            self.assertEqual(suite.call_args.kwargs["fixtures"], "")
        with patch.dict(os.environ, {pilot.FIXTURE_ENVIRONMENT: ""}), \
                patch.object(pilot, "run_suite", return_value=0) as suite:
            self.assertEqual(pilot.main([]), 0)
            self.assertEqual(suite.call_args.args, ("compiler-pilot", pilot.POSITIVES, pilot.NEGATIVES))
            self.assertIsNone(suite.call_args.kwargs["fixtures"])

    def test_nonempty_bad_environment_request_never_falls_back(self):
        with tempfile.TemporaryDirectory() as temporary:
            with patch.object(pilot, "ROOT", Path(temporary)), \
                    patch.dict(os.environ, {pilot.FIXTURE_ENVIRONMENT: "pass/float/missing,"}), \
                    patch.object(pilot, "execute") as execute:
                self.assertEqual(pilot.main([]), 1)
                execute.assert_not_called()

    def test_failed_retained_cohort_does_not_hide_later_measurements_or_turn_green(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            facts = (("shared", "shared", "shared", (), ()),
                     ("mutable", "mutable", "mutable", (), ()),
                     ("nested/arrays", "array_cases", "nested", (), ()))
            with patch.object(retained, "ROOT", root), \
                    patch.object(retained, "FACTS", facts), \
                    patch.dict(os.environ), \
                    patch.object(retained, "qualify_cohort", side_effect=[
                        ValueError("native timeout"), OSError("provider unavailable"),
                        root / "nested/omega"] ) as qualify, \
                    patch.object(retained, "unoffered_descriptor"), \
                    patch.object(pilot, "command", return_value="a" * 40), \
                    patch.object(pilot.corpus_gate, "host_target", return_value="windows_x86_64"):
                self.assertEqual(retained.main(), 1)
            self.assertEqual([call.args[2] for call in qualify.call_args_list],
                             ["shared", "mutable", "nested/arrays"])
            report = json.loads((root / "build/compiler-retained-foreign/summary.json").read_text())
            self.assertFalse(report["passed"])
            self.assertEqual([row["passed"] for row in report["observations"]], [False, False, True])
            self.assertEqual(report["observations"][0]["error"], "native timeout")
            self.assertEqual(report["observations"][1]["error"], "provider unavailable")

    def test_recording_uses_the_corpus_command_and_keeps_the_published_golden(self):
        for intended_rejection in (True, False):
            with self.subTest(intended_rejection=intended_rejection), tempfile.TemporaryDirectory() as temporary:
                output = Path(temporary)
                baseline = output / "published.txt"
                baseline.write_text("published results remain unchanged\n")
                recorded = output / "recorded-corpus-outcomes.txt"
                producer = Mock(consumer_revision=None, revision="a" * 40, target="linux_x86_64",
                                runner_profile={"sha256": "b" * 64})
                fixture = "fail/example/intended_rejection"

                def corpus(arguments, **kwargs):
                    self.assertIn("--record", arguments)
                    self.assertNotIn("--both", arguments)
                    self.assertEqual(arguments[arguments.index("--filter") + 1], fixture)
                    self.assertEqual(arguments[arguments.index("--golden") + 1], str(recorded))
                    self.assertEqual(kwargs["env"]["OMEGA_CORPUS_TIMINGS"], str(baseline))
                    report = self.report()
                    report["observations"] = [dict(fixture=fixture, status="rejected",
                                                   expected_satisfied=intended_rejection)]
                    Path(arguments[arguments.index("--json") + 1]).write_text(json.dumps(report))
                    recorded.write_text("runner-produced record\n")
                    return Mock(returncode=0)

                with patch.object(pilot, "command", side_effect=lambda argv: "" if "status" in argv else "a" * 40), \
                     patch.dict(os.environ, {"GITHUB_SHA": "a" * 40, "OMEGA_COMPILER_PILOT_RECORD_RESULTS": "true"}), \
                     patch.object(pilot.corpus_gate, "host_target", return_value="linux_x86_64"), \
                     patch.object(pilot, "observation_golden", return_value=baseline), \
                     patch.object(pilot.subprocess, "run", side_effect=corpus) as execute:
                    summary = {}
                    if intended_rejection:
                        pilot.execute(output, summary, positives=(), negatives=(fixture,), producer=producer)
                        self.assertEqual(summary["recorded_corpus"], str(recorded))
                    else:
                        with self.assertRaisesRegex(ValueError, "missing intended rejection"):
                            pilot.execute(output, summary, positives=(), negatives=(fixture,), producer=producer)
                        self.assertNotIn("recorded_corpus", summary)
                    execute.assert_called_once()
                    with self.assertRaisesRegex(ValueError, "already exists"):
                        pilot.execute(output, {}, positives=(), negatives=(fixture,), producer=producer)
                    execute.assert_called_once()
                self.assertEqual(baseline.read_text(), "published results remain unchanged\n")

    def test_recording_without_corpus_fixtures_refuses_before_building(self):
        with tempfile.TemporaryDirectory() as temporary, \
             patch.dict(os.environ, {"OMEGA_COMPILER_PILOT_RECORD_RESULTS": "true"}), \
             patch.object(pilot, "prepare_producer") as build, \
             patch.object(pilot.subprocess, "run") as consume:
            with self.assertRaisesRegex(ValueError, "exact corpus fixture selection"):
                pilot.execute(Path(temporary), {}, positives=(), negatives=(),
                              cli_fixtures=("pass/example/exit",))
            build.assert_not_called()
            consume.assert_not_called()

    def test_recording_without_an_explicit_selection_does_not_build_the_smoke(self):
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(pilot, "ROOT", Path(temporary)), \
             patch.dict(os.environ, {"OMEGA_COMPILER_PILOT_RECORD_RESULTS": "true"}), \
             patch.object(pilot, "prepare_producer") as build, \
             patch.object(pilot, "execute") as consume:
            self.assertEqual(pilot.run_suite("record", pilot.POSITIVES, pilot.NEGATIVES, phase="prepare"), 1)
            build.assert_not_called()
            consume.assert_not_called()
            report = json.loads((Path(temporary) / "build/record/preparation.json").read_text())
            self.assertIn("exact fixture IDs or a replay plan", report["error"])

    def test_rejection_only_run_never_launches_an_empty_runtime_filter(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            producer = Mock(consumer_revision=None, revision="a" * 40, target="linux_x86_64",
                            runner_profile={"sha256": "b" * 64})
            fixture = "fail/example/intended_rejection"
            def corpus(arguments, **kwargs):
                self.assertEqual(arguments[arguments.index("--filter") + 1], fixture)
                self.assertNotIn("--both", arguments)
                report = self.report()
                report["observations"] = [dict(fixture=fixture, status="rejected", expected_satisfied=True)]
                Path(arguments[arguments.index("--json") + 1]).write_text(json.dumps(report))
                return Mock(returncode=0)
            with patch.object(pilot, "command", side_effect=lambda argv: "" if "status" in argv else "a" * 40), \
                 patch.dict(os.environ, {"GITHUB_SHA": "a" * 40}), \
                 patch.object(pilot.corpus_gate, "host_target", return_value="linux_x86_64"), \
                 patch.object(pilot, "observation_golden", return_value=output / "golden"), \
                 patch.object(pilot.subprocess, "run", side_effect=corpus) as execute, \
                 patch.object(pilot, "prepare_producer") as build:
                summary = {}
                pilot.execute(output, summary, positives=(), negatives=(fixture,), producer=producer)
                execute.assert_called_once()
                self.assertNotIn("native and Terminal execution", [stage["name"] for stage in summary["stages"]])
                with self.assertRaisesRegex(ValueError, "at least one fixture"):
                    pilot.execute(output, {}, positives=(), negatives=())
                build.assert_not_called()

    def test_failed_checking_still_measures_execution_and_stays_red(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            producer = Mock(consumer_revision=None, revision="a" * 40, target="linux_x86_64",
                            runner_profile={"sha256": "b" * 64})
            legs = []
            def corpus(arguments, **kwargs):
                both = "--both" in arguments
                legs.append("both" if both else "checking")
                report = self.report(both=both)
                if not both:
                    report["oracle_failures"] = [{"fixture": pilot.NEGATIVES[0]}]
                Path(arguments[arguments.index("--json") + 1]).write_text(json.dumps(report))
                return Mock(returncode=1)
            with patch.object(pilot, "command", side_effect=lambda argv: "" if "status" in argv else "a" * 40), \
                 patch.dict(os.environ, {"GITHUB_SHA": "a" * 40}), \
                 patch.object(pilot.corpus_gate, "host_target", return_value="linux_x86_64"), \
                 patch.object(pilot, "observation_golden", return_value=output / "golden"), \
                 patch.object(pilot.subprocess, "run", side_effect=corpus):
                with self.assertRaisesRegex(ValueError, "oracle"):
                    pilot.execute(output, {}, producer=producer)
            self.assertEqual(legs, ["checking", "both"])

    def test_failed_consumers_do_not_skip_later_ordinary_or_sample_observations(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            producer = Mock(consumer_revision=None, revision="a" * 40, target="linux_x86_64",
                            runner_profile={"sha256": "b" * 64})
            sample = "samples/cli/facts/example/local"
            observed = []

            def consume(arguments, **kwargs):
                if "--write-results" in arguments:
                    observed.append("sample")
                    Path(arguments[arguments.index("--write-results") + 1]).write_text(
                        json.dumps([dict(sample=sample, state="pass")]))
                    return Mock(returncode=0)
                observed.append("checking")
                return Mock(returncode=3)

            rows = [dict(fixture="pass/group/first", passed=False, error="source refusal"),
                    dict(fixture="pass/group/second", passed=True)]
            with patch.object(pilot, "command", side_effect=lambda argv: "" if "status" in argv else "a" * 40), \
                    patch.dict(os.environ, {"GITHUB_SHA": "a" * 40}), \
                    patch.object(pilot.corpus_gate, "host_target", return_value="linux_x86_64"), \
                    patch.object(pilot, "observation_golden", return_value=output / "golden"), \
                    patch.object(pilot.subprocess, "run", side_effect=consume), \
                    patch.object(pilot.ordinary_cli_fixture, "observe", side_effect=rows) as ordinary:
                summary = {}
                with self.assertRaisesRegex(ValueError, "source refusal"):
                    pilot.execute(output, summary, positives=(), negatives=("fail/group/control",),
                                  cli_fixtures=("pass/group/first", "pass/group/second"),
                                  samples=(sample,), producer=producer)
            self.assertEqual(ordinary.call_count, 2)
            self.assertEqual(observed, ["checking", "sample"])
            self.assertEqual(len(summary["consumer_errors"]), 2)
            self.assertTrue(summary["stages"][-1]["passed"])

    def test_first_host_keeps_unknown_history_separate_from_other_host_results(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            corpus = root / "tests/omega"
            corpus.mkdir(parents=True)
            other = corpus / "corpus_both_macos_arm64.txt"
            other.write_text("an existing host's measured rows\n", encoding="utf-8")
            output = root / "build"
            output.mkdir()
            with patch.object(pilot, "ROOT", root):
                baseline = pilot.observation_golden(output, both=True, target="linux_arm64")
                with self.assertRaisesRegex(ValueError, "published corpus baseline"):
                    pilot.observation_golden(output, both=False, target="linux_arm64")
            self.assertEqual(baseline.parent, output)
            self.assertTrue(all(line.startswith("#") for line in baseline.read_text().splitlines()))
            self.assertFalse((corpus / "corpus_both_linux_arm64.txt").exists())
            self.assertEqual(other.read_text(), "an existing host's measured rows\n")
            # Missing historical rows never permit a failed authored oracle.
            report = self.report(True)
            report["oracle_failures"] = ["expected native 0, observed timeout"]
            with self.assertRaises(ValueError):
                self.validate(report, True)

    def test_existing_matching_host_baseline_is_preserved(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            baseline = root / "tests/omega/corpus_both_linux_arm64.txt"
            baseline.parent.mkdir(parents=True)
            original = b"existing matching-host observations\n"
            baseline.write_bytes(original)
            with patch.object(pilot, "ROOT", root):
                self.assertEqual(pilot.observation_golden(
                    root / "absent-output", both=True, target="linux_arm64"), baseline)
            self.assertEqual(baseline.read_bytes(), original)
            self.assertFalse((root / "absent-output").exists())

    def report(self, both=False):
        fixtures = pilot.POSITIVES if both else pilot.POSITIVES + pilot.NEGATIVES
        return dict(schema="omega-corpus-verdict-v1", revision="a" * 40, dirty=False,
                    exit_code=0, oracle_failures=[], infrastructure_error=None,
                    leg="both" if both else "checked", target="linux_x86_64",
                    observations=[dict(fixture=name,
                                       status="rejected" if name.startswith("fail/") else "built",
                                       expected_satisfied=True if name.startswith("fail/") else None,
                                       facts={"exit": "70", "interpreter-exit": "70", "interpreter": "agrees"})
                                  for name in fixtures])

    def validate(self, report, both=False):
        pilot.validate_report(report, pilot.POSITIVES if both else pilot.POSITIVES + pilot.NEGATIVES,
                              both=both, revision="a" * 40, target="linux_x86_64")

    def test_complete_runtime_and_rejections_pass(self):
        self.validate(self.report())
        self.validate(self.report(True), True)

    def test_target_publication_control_needs_its_real_reason_and_no_execution(self):
        fixture = "pass/providers/missing_target_supply"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            directory = root / "tests/omega" / fixture
            directory.mkdir(parents=True)
            for target in ("macos_arm64", "linux_x86_64"):
                (directory / f"expected_native_rejection.{target}.txt").write_text(f"missing supply on {target}\n")
            for both in (False, True):
                target = "linux_x86_64" if both else "macos_arm64"
                facts = {"interpreter": "unchecked"} if both else {"check-target": target}
                row = dict(fixture=fixture, tier="pass", status="rejected", expected_satisfied=True,
                           facts=dict(facts, **{"native-rejection": "match"}),
                           diagnostics=[f"missing supply on {target}"])
                report = self.report(both)
                report["observations"] = [row]
                with patch.object(pilot, "ROOT", root):
                    pilot.validate_report(report, (fixture,), both=both, revision="a" * 40, target="linux_x86_64")
                    row["diagnostics"] = ["unrelated refusal"]
                    with self.assertRaises(ValueError):
                        pilot.validate_report(report, (fixture,), both=both, revision="a" * 40, target="linux_x86_64")

    def test_golden_movement_is_not_a_failed_oracle(self):
        report = self.report()
        report["exit_code"] = 1
        self.validate(report)

    def test_wrong_source_or_failed_gate_refuses(self):
        for key, value in (("schema", "other"), ("revision", "b" * 40), ("dirty", True),
                           ("exit_code", 3), ("exit_code", 4), ("oracle_failures", ["wrong exit"]),
                           ("infrastructure_error", "missing compiler")):
            report = self.report()
            report[key] = value
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                self.validate(report)

    def test_missing_duplicate_or_extra_observation_refuses(self):
        for operation in (lambda rows: rows.pop(), lambda rows: rows.append(deepcopy(rows[0])),
                          lambda rows: rows[0].update(fixture="pass/unrelated")):
            report = self.report()
            operation(report["observations"])
            with self.assertRaises(ValueError):
                self.validate(report)

    def test_wrong_reason_or_accepted_invalid_source_refuses(self):
        for key, value in (("expected_satisfied", False), ("expected_satisfied", None),
                           ("status", "checked")):
            report = self.report()
            report["observations"][-1][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.validate(report)

    def test_no_runtime_or_disagreement_refuses(self):
        for facts in ({}, {"exit": "70"},
                      {"exit": "70", "interpreter-exit": "71", "interpreter": "agrees"},
                      {"exit": "70", "interpreter-exit": "70", "interpreter": "declined"},
                      {"exit": "signal 5", "interpreter-exit": "signal 5", "interpreter": "agrees"},
                      {"exit": "timeout", "interpreter-exit": "timeout", "interpreter": "agrees"},
                      {"exit": 70, "interpreter-exit": 70, "interpreter": "agrees"}):
            report = self.report(True)
            report["observations"][0]["facts"] = facts
            with self.subTest(facts=facts), self.assertRaises(ValueError):
                self.validate(report, True)

    def test_wrong_engine_or_target_refuses(self):
        for key, value in (("leg", "native"), ("target", "macos_arm64")):
            report = self.report(True)
            report[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.validate(report, True)

    def test_authored_traps_require_matching_native_fault_and_typed_terminal_trap(self):
        fixture = "pass/proofs/runtime_round_trip_trap"
        targets = {
            "linux_x86_64": {"exit": "signal", "signal": "4"},
            "linux_arm64": {"exit": "signal", "signal": "5"},
            "macos_arm64": {"exit": "signal", "signal": "5"},
            "windows_x86_64": {"exit": "-1073741795"},
        }
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture_root(root, fixture)
            oracle = root / "tests/omega" / fixture / "expected_native_exit.txt"
            oracle.write_text("trap\n")
            for target, native in targets.items():
                report = self.report(True)
                report["target"] = target
                report["observations"] = [dict(fixture=fixture, status="built", facts={
                    **native, "interpreter": "trapped"})]
                with patch.object(pilot, "ROOT", root):
                    def validate(value):
                        pilot.validate_report(value, (fixture,), both=True,
                                              revision="a" * 40, target=target)
                    with self.subTest(target=target):
                        validate(report)
                        for facts in ({"exit": "0", "interpreter": "trapped"},
                                      {"exit": "signal", "signal": "11", "interpreter": "trapped"},
                                      {**native, "interpreter": "declined"},
                                      {**native, "interpreter": "trapped", "interpreter-exit": "0"},
                                      {**native, "interpreter": "trapped", "interpreter-signal": "5"}):
                            missing = deepcopy(report)
                            missing["observations"][0]["facts"] = facts
                            with self.subTest(facts=facts), self.assertRaises(ValueError):
                                validate(missing)
                        oracle.write_text("0\n")
                        with self.assertRaises(ValueError):
                            validate(report)
                        oracle.write_text("trap\n")

    def test_cleanup_report_requires_both_authored_output_observations(self):
        fixture = "pass/drops/runtime_projected_nominal_cleanup_exit"
        report = self.report(True)
        report["observations"] = [dict(fixture=fixture, status="built", facts={
            "exit": "0", "interpreter-exit": "0", "interpreter": "agrees",
            "stdout": "match", "interpreter-stdout": "match"})]

        def validate(value):
            pilot.validate_report(value, (fixture,), both=True,
                                  revision="a" * 40, target="linux_x86_64")

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture_root(root, fixture)
            (root / "tests/omega" / fixture / "expected_stdout.txt").write_text("cleanup\n")
            with patch.object(pilot, "ROOT", root):
                validate(report)
                for key in ("stdout", "interpreter-stdout"):
                    for value in (None, "mismatch"):
                        missing = deepcopy(report)
                        missing["observations"][0]["facts"][key] = value
                        with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                            validate(missing)

    def test_cleanup_cannot_lose_a_transport_or_intended_rejection(self):
        report = self.report()
        report["observations"] = [dict(
            fixture=fixture, status="checked" if fixture.startswith("pass/") else "rejected",
            expected_satisfied=fixture.startswith("fail/"))
            for fixture in cleanup.POSITIVES + cleanup.NEGATIVES]

        def validate(value):
            pilot.validate_report(value, cleanup.POSITIVES + cleanup.NEGATIVES,
                                  both=False, revision="a" * 40, target="linux_x86_64")

        validate(report)
        for fixture in (cleanup.POSITIVES[-1], cleanup.NEGATIVES[-1]):
            missing = deepcopy(report)
            missing["observations"] = [row for row in missing["observations"]
                                       if row["fixture"] != fixture]
            with self.subTest(fixture=fixture), self.assertRaises(ValueError):
                validate(missing)


    def test_ordinary_cli_honors_both_numeric_engine_oracles_before_build(self):
        fixture = "pass/borrows/saved_reference"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture_root(root, fixture)
            project = root / "tests/omega" / fixture
            (project / "build.omg").write_text("ProgramEntry")
            (project / "expected_native_exit.txt").write_text("70")
            interpreter = project / "expected_interpreter_exit.txt"
            interpreter.write_text("70")
            with patch.object(pilot, "ROOT", root), patch.object(pilot, "execute") as execute:
                self.assertEqual(pilot.select_cli_fixtures(fixture), (fixture,))
                self.assertEqual(pilot.ordinary_cli_fixture.expected_exit(root, fixture, "linux_x86_64"), 70)
                for unsupported in ("", "trap", "70 trailing text", "71"):
                    interpreter.write_text(unsupported)
                    with self.subTest(oracle=unsupported):
                        self.assertEqual(pilot.run_suite("bad", pilot.POSITIVES, pilot.NEGATIVES,
                                                        cli_fixtures=fixture), 1)
                    execute.assert_not_called()
                interpreter.write_text("70")
                (project / "expected_native_exit.linux_x86_64.txt").write_text("0")
                with self.assertRaisesRegex(ValueError, "matching native and interpreter"):
                    pilot.ordinary_cli_fixture.expected_exit(root, fixture, "linux_x86_64")

    def test_ordinary_cli_sample_header_must_match_its_interpreter_oracle(self):
        sample = "samples/cli/systems/journal"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.sample_root(root, sample)
            project = root / sample
            (project / "main.omg").write_text("// Expected exit: 7\nmachine main() {}")
            interpreter = project / "expected_interpreter_exit.txt"
            interpreter.write_text("7")
            self.assertEqual(pilot.ordinary_cli_fixture.expected_exit(root, sample, "linux_x86_64"), 7)
            interpreter.write_text("0")
            with self.assertRaisesRegex(ValueError, "matching native and interpreter"):
                pilot.ordinary_cli_fixture.expected_exit(root, sample, "linux_x86_64")


if __name__ == "__main__":
    unittest.main()
