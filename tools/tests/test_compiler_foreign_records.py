"""Reject incomplete rows, stale producers and hidden cohort failures before credit."""
import hashlib
import json
import os
from pathlib import Path
import sys
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch
from contextlib import nullcontext

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "ci"))
import compiler_fact_rows as rows
import compiler_foreign_records as foreign
import compiler_pilot as pilot


class OwnedForeignCohortTests(unittest.TestCase):
    def fact(self, root):
        fact = root / "samples/cli/facts/example"
        fact.mkdir(parents=True)
        main = "// Expected exit: 0\nmachine run_main() {}\n"
        for name in ("main.omg", "build.omg"):
            (fact / name).write_text(main)
        for place in rows.PLACES:
            for tier, suffix, source in (("pass", "exit", fact / place),
                                         ("fail", "rejected", fact / place / "invalid")):
                source.mkdir(parents=True)
                fixture = root / f"tests/omega/{tier}/facts/example_through_{place}_{suffix}"
                fixture.mkdir(parents=True)
                for folder in (source, fixture):
                    (folder / "main.omg").write_text(main if tier == "pass" else "broken copy obligation\n")
                    (folder / "build.omg").write_text("authored package\n")
                    if tier == "fail":
                        (folder / "expected.txt").write_text("holds no value\n")
        return fact

    def test_selection_is_explicit_and_default_cohort_stays_available(self):
        self.assertEqual(foreign.select_cohorts("nested, arrays,nested"), ("nested", "arrays"))
        self.assertEqual(foreign.select_cohorts("all"), ("scalar-opaque", "nested", "arrays"))
        for selection in ("", " , ", "unknown", "nested,unknown", "nested; echo wrong"):
            with self.subTest(selection=selection), self.assertRaises(ValueError):
                foreign.select_cohorts(selection)

    def test_missing_twin_and_changed_pin_refuse_discovery(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fact = self.fact(root)
            positive, negative = rows.fixtures(root, fact, "example")
            self.assertEqual((len(positive), len(negative)), (9, 9))
            twin = root / "tests/omega" / negative[0] / "main.omg"
            twin.write_text("unrelated invalid source\n")
            with self.assertRaisesRegex(ValueError, "pinned input differs"):
                rows.fixtures(root, fact, "example")
            twin.unlink()
            with self.assertRaisesRegex(ValueError, "required authored place/twin"):
                rows.fixtures(root, fact, "example")

    def test_root_cannot_silently_lose_its_runtime_oracle(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fact = self.fact(root)
            (fact / "main.omg").write_text("machine run_main() {}\n")
            with self.assertRaisesRegex(ValueError, "zero-exit oracle"):
                rows.fixtures(root, fact, "example")

    def test_preparation_and_measurement_never_add_evidence_to_accepted_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fact = self.fact(root)
            output = root / "evidence"
            producer = Mock(cli=root / "producer/omega", target="linux_x86_64")
            producer.record.return_value = dict(revision="a" * 40)
            accepted = {}

            def review(cli, project, target, timeout):
                project = Path(project)
                (project / "omega.lock").write_text("exact reviewed lock\n")
                accepted[project] = {p.name for p in project.iterdir()}
                return None, "accepted package source inventory\n"

            def execute(arguments, recording, stem, supplied=b""):
                recording.mkdir(parents=True, exist_ok=True)
                if stem == "run":
                    kept = root / "kept" / recording.name
                    kept.mkdir(parents=True)
                    (kept / "program.oterm").write_bytes(b"canonical artifact")
                    stderr = f"native exit 0\ninterp exit: 0\nbuild dir kept: {kept}\n".encode()
                else:
                    stderr = b""
                (recording / (stem + ".stdout")).write_bytes(b"")
                (recording / (stem + ".stderr")).write_bytes(stderr)
                return subprocess.CompletedProcess(arguments, 0, b"", stderr)

            with patch.object(rows.sample_native_gate, "accept_review", side_effect=review), \
                 patch.object(rows, "execute", side_effect=execute):
                inventory = rows.prepare(output, fact, producer)
                self.assertEqual(len(accepted), 10)
                self.assertTrue(all({p.name for p in project.iterdir()} == names
                                    for project, names in accepted.items()))
                report = rows.measure(output, inventory, producer)
            self.assertTrue(report["passed"])
            self.assertTrue(all({p.name for p in project.iterdir()} == names
                                for project, names in accepted.items()))
            self.assertEqual(len(list((output / "sample-logs").glob("*/products/program.oterm"))), 10)

    def test_mutating_frozen_source_or_lock_refuses(self):
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "main.omg"
            lock = Path(temporary) / "omega.lock"
            source.write_text("frozen source\n")
            lock.write_text("exact reviewed lock\n")
            row = dict(authored_inputs={str(source): rows.digest(source)}, inputs={str(lock): rows.digest(lock)})
            rows.validate_inputs(row)
            lock.write_text("another producer's lock\n")
            with self.assertRaisesRegex(ValueError, "frozen authored/replay input changed"):
                rows.validate_inputs(row)

    def run_cohorts(self, root, failures):
        producer = Mock()
        producer.record.return_value = dict(revision="a" * 40)
        with patch.object(pilot, "ROOT", root), patch.dict(os.environ), \
             patch.object(pilot, "command", return_value="a" * 40), \
             patch.object(pilot.corpus_gate, "host_target", return_value="linux_x86_64"), \
             patch.object(pilot, "prepare_producer", return_value=producer) as build, \
             patch.object(rows, "fixtures", return_value=(("positive",), ("negative",))), \
             patch.object(foreign, "prepare_provider", return_value=root / "provider/library"), \
             patch.object(foreign, "owned_row", side_effect=failures) as qualify:
            status = foreign.main(["--cohorts", "nested,arrays"])
        report = json.loads((root / "build/compiler-foreign-records/latest.json").read_text())
        return status, report, build.call_count, qualify.call_count

    def test_failed_cohort_never_hides_later_measurements_or_turns_green(self):
        with tempfile.TemporaryDirectory() as temporary:
            status, report, builds, measured = self.run_cohorts(Path(temporary), [ValueError("native timeout"), None])
        self.assertEqual((status, builds, measured), (1, 1, 2))
        self.assertEqual([row["passed"] for row in report["cohorts"]], [False, True])
        self.assertEqual(report["cohorts"][0]["error"], "native timeout")
        self.assertFalse(report["passed"])

    def test_complete_cohorts_share_exactly_one_producer(self):
        with tempfile.TemporaryDirectory() as temporary:
            status, report, builds, measured = self.run_cohorts(Path(temporary), [None, None])
        self.assertEqual((status, builds, measured), (0, 1, 2))
        self.assertTrue(report["passed"])

    def test_provider_environment_is_restored_after_failure(self):
        library = Path("provider/library.dll").resolve()
        with patch.dict(os.environ, {"PATH": "original path"}), patch.object(foreign.os, "name", "nt"):
            with self.assertRaisesRegex(ValueError, "intended failure"):
                with foreign.provider_path(library):
                    self.assertEqual(os.environ["PATH"], str(library.parent) + os.pathsep + "original path")
                    raise ValueError("intended failure")
            self.assertEqual(os.environ["PATH"], "original path")

    def producer_build(self, root, *, refused=False):
        name = "omega.exe" if os.name == "nt" else "omega"
        cli = root / "bin" / name
        runner = root / "bin" / ("corpus_runner.exe" if os.name == "nt" else "corpus_runner")
        cli.parent.mkdir()
        cli.write_bytes(b"CLI with current profile protocol")
        runner.write_bytes(b"corpus runner")
        Path(str(runner) + ".compiler-profile.json").write_text("retained Cargo build receipt\n")
        cargo = "\n".join(json.dumps(row) for row in [
            dict(reason="compiler-artifact", target=dict(name="omega"), executable=str(cli)),
            dict(reason="build-finished", success=True)])
        result = subprocess.CompletedProcess(["cargo"], 0, cargo, "optimized build\n")

        def command(arguments):
            if arguments == ["git", "rev-parse", "HEAD"]:
                return "a" * 40
            if arguments == ["git", "rev-parse", "HEAD^{tree}"]:
                return "b" * 40
            return "" if "status" in arguments else "pinned rustc"

        def profile(path):
            if refused:
                raise ValueError("unoptimized or unknown CLI profile")
            return dict(binary=str(Path(path).absolute()), sha256=rows.digest(path),
                        profile=dict(schema="omega-compiler-build-profile-v1", opt_level="1"))

        with patch.object(pilot, "ROOT", root), patch.dict(os.environ), \
             patch.object(pilot, "command", side_effect=command), \
             patch.object(pilot.subprocess, "run", return_value=result), \
             patch("build_slot.hold", return_value=nullcontext()), \
             patch.object(pilot.corpus_gate, "build_runner", return_value=runner) as build_runner, \
             patch.object(pilot.corpus_gate, "host_target", return_value="linux_x86_64"), \
             patch.object(pilot.corpus_gate, "require_optimized", side_effect=profile), \
             patch.object(pilot.corpus_gate, "record_cargo_build") as historical_fingerprint:
            os.environ.pop("GITHUB_SHA", None)
            if refused:
                with self.assertRaisesRegex(ValueError, "unoptimized or unknown"):
                    pilot.prepare_producer(root / "evidence")
                build_runner.assert_not_called()
                producer = None
            else:
                producer = pilot.prepare_producer(root / "evidence")
            historical_fingerprint.assert_not_called()
        return producer

    def test_current_cli_profile_needs_no_historical_cargo_unit_sidecar(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            producer = self.producer_build(root)
            self.assertEqual(producer.revision, "a" * 40)
            self.assertTrue(producer.cli.is_file())
            self.assertTrue(Path(str(producer.cli) + ".verified-profile.json").is_file())
            self.assertFalse(Path(str(producer.cli) + ".compiler-profile.json").exists())

    def test_unknown_cli_profile_refuses_before_runner_or_program_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.assertIsNone(self.producer_build(root, refused=True))
            self.assertEqual(len(list((root / "evidence").glob("*/cli-build.jsonl"))), 1)

    def test_changed_retained_binary_refuses_even_when_profile_kind_is_same(self):
        with tempfile.TemporaryDirectory() as temporary:
            cli, runner = Path(temporary) / "omega", Path(temporary) / "corpus_runner"
            cli.write_bytes(b"first CLI")
            runner.write_bytes(b"first runner")
            def profile(path):
                return dict(binary=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                            profile=dict(schema="omega-compiler-build-profile-v1", opt_level="1"))
            producer = pilot.CompilerProducer("a" * 40, "b" * 40, "linux_x86_64", "pinned rustc", cli, runner, profile(cli), profile(runner))
            with patch.object(pilot, "command", side_effect=lambda argv: "a" * 40 if "rev-parse" in argv else ""), \
                 patch.object(pilot.corpus_gate, "require_optimized", side_effect=profile):
                producer.verify()
                runner.write_bytes(b"replacement runner")
                with self.assertRaisesRegex(ValueError, "identity/profile changed"):
                    producer.verify()


if __name__ == "__main__":
    unittest.main()
