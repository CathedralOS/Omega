"""Retained cache bytes cannot bypass source, profile or selection checks."""

import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from ci import compiler_pilot as pilot
from ci import compiler_replay_plan as replay


class ReplayPlanTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.git("init", "-q")
        self.git("config", "user.email", "test@example.invalid")
        self.git("config", "user.name", "Replay test")
        for name in ("omega-rust/compiler.rs", "source/library/core.omg"):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("original")
        self.git("add", ".")
        self.git("commit", "-qm", "source")
        self.source = self.git("rev-parse", "HEAD")
        self.binary = b"retained executable"
        self.identity = dict(sha256=hashlib.sha256(self.binary).hexdigest(),
                             profile=dict(schema="omega-compiler-build-profile-v1", opt_level="1"))
        self.data = dict(schema=replay.SCHEMA, source_only=True,
                         fixtures=["pass/group/example"], samples=[], cli_fixtures=[], producer=dict(
                             revision=self.source, tree=self.git("rev-parse", "HEAD^{tree}"),
                             target="linux_x86_64", toolchain="recorded rustc", origin_run=123,
                             cache_key="original-cache", input_objects=replay.input_objects(self.root, self.source),
                             executables=dict(cli=self.identity, runner=self.identity)))

    def git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.root, text=True).strip()

    def plan(self):
        (self.root / "replay.json").write_text(json.dumps(self.data))
        self.git("add", "replay.json")
        return replay.load(self.root, "@replay.json")

    def test_oracle_changes_keep_original_producer_but_compiler_and_sdk_changes_refuse(self):
        (self.root / "tests").mkdir()
        (self.root / "tests/expected.txt").write_text("new expectation")
        self.git("add", "tests")
        self.git("commit", "-qm", "oracle")
        plan = self.plan()
        self.assertEqual(plan.producer["revision"], self.source)
        for name in ("omega-rust/compiler.rs", "source/library/core.omg"):
            with self.subTest(name=name):
                path = self.root / name
                path.write_text("changed")
                self.git("add", name)
                self.git("commit", "-qm", "changed input")
                with self.assertRaisesRegex(ValueError, "changed since"):
                    self.plan()
                self.git("reset", "--hard", self.source)

    def test_narrowed_input_roster_and_false_origin_refuse(self):
        del self.data["producer"]["input_objects"]["source/library"]
        with self.assertRaisesRegex(ValueError, "original source|complete compiler"):
            self.plan()
        self.data["producer"]["input_objects"] = replay.input_objects(self.root, self.source)
        self.data["producer"]["tree"] = "0" * 40
        with self.assertRaisesRegex(ValueError, "tree differs"):
            self.plan()

    def test_untracked_and_escaping_plan_paths_refuse(self):
        (self.root / "untracked.json").write_text(json.dumps(self.data))
        for name in ("@untracked.json", "@../outside.json", "@/absolute.json", "@replay.json/../other.json"):
            with self.subTest(name=name), self.assertRaises(ValueError):
                replay.load(self.root, name)

    def test_each_selector_is_one_literal_path_not_another_plan(self):
        for selector in ("@another.json", "pass/group/a,pass/group/b", " pass/group/a"):
            with self.subTest(selector=selector):
                self.data["fixtures"] = [selector]
                with self.assertRaisesRegex(ValueError, "selectors"):
                    self.plan()

    def test_missing_or_different_cache_bytes_never_build(self):
        plan = self.plan()
        output = self.root / "output"
        output.mkdir()
        cache = self.root / "target/debug"
        cache.mkdir(parents=True)
        with patch.dict(os.environ, CARGO_TARGET_DIR=str(self.root / "target")), \
                patch.object(pilot, "prepare_producer") as build:
            with self.assertRaisesRegex(ValueError, "missing; no rebuild"):
                replay.materialize(self.root, output, plan, self.profile)
            (cache / "omega").write_bytes(b"other executable")
            with patch.object(self, "profile") as query:
                with self.assertRaisesRegex(ValueError, "differs.*no rebuild"):
                    replay.materialize(self.root, output, plan, query)
                query.assert_not_called()
            build.assert_not_called()

    def profile(self, path):
        return dict(binary=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                    profile=self.identity["profile"])

    def test_cache_copies_are_authenticated_before_and_after_capture(self):
        plan = self.plan()
        output = self.root / "output"
        output.mkdir()
        cache = self.root / "target/debug"
        cache.mkdir(parents=True)
        (cache / "omega").write_bytes(self.binary)
        (cache / "deps").mkdir()
        (cache / "deps/corpus_runner-0123456789abcdef").write_bytes(self.binary)
        (cache / "deps/corpus_runner-fedcba9876543210").write_bytes(b"unrelated runner")
        with patch.dict(os.environ, CARGO_TARGET_DIR=str(self.root / "target")):
            paths, profiles = replay.materialize(self.root, output, plan, self.profile)
        self.assertEqual(paths["cli"].read_bytes(), self.binary)
        self.assertEqual(profiles["runner"]["sha256"], self.identity["sha256"])
        self.assertEqual(paths["runner"].name, "corpus_runner-0123456789abcdef")
        self.assertNotEqual(paths["cli"], cache / "omega")

    def test_replay_selection_uses_the_retained_producer_without_building(self):
        self.data.update(samples=["samples/cli/facts/example/local"],
                         cli_fixtures=["pass/group/runtime_exit"])
        plan = self.plan()
        retained = object()
        with patch.object(pilot, "ROOT", self.root), \
                patch.object(pilot.compiler_replay_plan, "load", return_value=plan), \
                patch.object(pilot, "select_fixtures", return_value=(("positive",), ("negative",))), \
                patch.object(pilot, "select_samples", return_value=("sample",)), \
                patch.object(pilot, "select_cli_fixtures", return_value=("ordinary",)), \
                patch.object(pilot, "retained_producer", return_value=retained), \
                patch.object(pilot, "prepare_producer") as build, \
                patch.object(pilot, "execute") as execute:
            self.assertEqual(pilot.run_suite("replay", (), (), fixtures="@replay.json"), 0)
            build.assert_not_called()
            arguments = execute.call_args.kwargs
            self.assertEqual(arguments["checking_only"], ("positive",))
            self.assertEqual(arguments["positives"], ())
            self.assertEqual(arguments["negatives"], ("negative",))
            self.assertEqual(arguments["samples"], ("sample",))
            self.assertEqual(arguments["cli_fixtures"], ("ordinary",))
            self.assertIs(arguments["producer"], retained)

    def test_cargo_unit_output_runner_is_authenticated_by_its_original_digest(self):
        plan = self.plan()
        output = self.root / "output"
        output.mkdir()
        cache = self.root / "target/debug"
        cache.mkdir(parents=True)
        (cache / "omega").write_bytes(self.binary)
        runner = cache / "build/compiler/75d244958a7c5dee/out/corpus_runner"
        runner.parent.mkdir(parents=True)
        runner.write_bytes(self.binary)
        other = cache / "build/compiler/0000000000000000/out/corpus_runner"
        other.parent.mkdir(parents=True)
        other.write_bytes(b"unrelated runner")
        with patch.dict(os.environ, CARGO_TARGET_DIR=str(self.root / "target")):
            paths, profiles = replay.materialize(self.root, output, plan, self.profile)
        manifest = json.loads((paths["runner"].parent / "cache-inputs.json").read_text())
        self.assertEqual(manifest["runner"]["path"], str(runner))
        self.assertEqual(profiles["runner"]["sha256"], self.identity["sha256"])

    def test_missing_retained_cache_stays_red_without_a_build_fallback(self):
        plan = self.plan()
        with patch.object(pilot, "ROOT", self.root), \
                patch.object(pilot.compiler_replay_plan, "load", return_value=plan), \
                patch.object(pilot, "select_fixtures", return_value=(("positive",), ())), \
                patch.object(pilot, "retained_producer", side_effect=ValueError("cache missing")), \
                patch.object(pilot, "prepare_producer") as build, \
                patch.object(pilot, "execute") as execute:
            self.assertEqual(pilot.run_suite("refused", (), (), fixtures="@replay.json"), 1)
            build.assert_not_called()
            execute.assert_not_called()
            report = json.loads((self.root / "build/refused/summary.json").read_text())
            self.assertFalse(report["passed"])
            self.assertIn("cache missing", report["error"])


if __name__ == "__main__":
    unittest.main()
