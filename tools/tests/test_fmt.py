#!/usr/bin/env python3
"""Workspace fmt driver tests; stdlib only, no cargo or rustfmt needed."""

import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import unittest
from unittest import mock


FMT = Path(__file__).resolve().parents[1] / "fmt.py"


def load_fmt():
    specification = importlib.util.spec_from_file_location("fmt_under_test", FMT)
    module = importlib.util.module_from_spec(specification)
    previous = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        specification.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


def metadata(packages, member_ids=None):
    return json.dumps({
        "workspace_members": (
            member_ids if member_ids is not None
            else [package["id"] for package in packages]),
        "packages": packages,
    })


def package(identifier, targets):
    return {"id": identifier, "targets": targets}


def target(kind, src_path, edition="2024"):
    return {"kind": [kind], "name": src_path, "src_path": src_path,
            "edition": edition}


class Completed:
    def __init__(self, stdout="", returncode=0):
        self.stdout = stdout
        self.returncode = returncode


class FakeProcess:
    """Dispatches on argv[0]: `cargo metadata` answers canned JSON, every
    `rustfmt` invocation is recorded and returns the mapped exit code."""

    def __init__(self, metadata_json, rustfmt_exit=0):
        self.metadata_json = metadata_json
        self.rustfmt_exit = rustfmt_exit
        self.invocations = []

    def run(self, argv, cwd=None, capture_output=False, text=False,
            check=False, **kwargs):
        if argv[0] == "cargo":
            return Completed(stdout=self.metadata_json)
        self.invocations.append(argv)
        return Completed(returncode=self.rustfmt_exit)


class WorkspaceTargets(unittest.TestCase):
    def setUp(self):
        self.module = load_fmt()

    def run_targets(self, packages, member_ids=None):
        fake = FakeProcess(metadata(packages, member_ids))
        with mock.patch.object(self.module.subprocess, "run", fake.run):
            return self.module.workspace_targets("root")

    def test_groups_files_by_edition_and_filters_members(self):
        groups = self.run_targets([
            package("member-a", [
                target("lib", "/w/a/src/lib.rs"),
                target("test", "/w/a/tests/suite.rs", edition="2021"),
                target("dylib", "/w/a/src/plugin.rs"),
            ]),
            package("non-member", [target("lib", "/w/b/src/lib.rs")]),
        ], member_ids=["member-a"])
        self.assertEqual(groups, {
            "2024": ["/w/a/src/lib.rs"],
            "2021": ["/w/a/tests/suite.rs"],
        })

    def test_covers_the_kinds_cargo_fmt_formats(self):
        kinds = ["lib", "bin", "test", "bench", "example", "custom-build",
                 "proc-macro"]
        groups = self.run_targets([
            package("member", [target(kind, f"/w/t/{kind}.rs") for kind in kinds]),
        ])
        self.assertEqual(groups["2024"],
                         [f"/w/t/{kind}.rs" for kind in sorted(kinds)])

    def test_deduplicates_shared_src_paths(self):
        groups = self.run_targets([
            package("member", [
                target("lib", "/w/a/src/lib.rs"),
                target("test", "/w/a/src/lib.rs"),
            ]),
        ])
        self.assertEqual(groups["2024"], ["/w/a/src/lib.rs"])


class Chunking(unittest.TestCase):
    def setUp(self):
        self.module = load_fmt()

    def test_single_invocation_when_under_budget(self):
        files = ["/w/a.rs", "/w/b.rs"]
        result = self.module.chunked_invocations(["rustfmt"], files)
        self.assertEqual(result, [["rustfmt", "/w/a.rs", "/w/b.rs"]])

    def test_splits_at_budget_and_never_emits_empty_invocation(self):
        self.module.ARGV_BUDGET = 40
        files = ["abcdefghij", "klmnopqrst", "uvwxyz1234"]
        result = self.module.chunked_invocations(["rustfmt"], files)
        for invocation in result:
            self.assertGreater(len(invocation), 1)
        flattened = [arg for call in result for arg in call[1:]]
        self.assertEqual(flattened, files)
        self.assertGreater(len(result), 1)

    def test_oversized_single_file_still_runs(self):
        self.module.ARGV_BUDGET = 5
        result = self.module.chunked_invocations(["rustfmt"], ["x" * 100])
        self.assertEqual(result, [["rustfmt", "x" * 100]])


class Main(unittest.TestCase):
    def setUp(self):
        self.module = load_fmt()

    def run_main(self, argv, fake):
        with mock.patch.object(self.module.subprocess, "run", fake.run), \
                mock.patch.object(sys, "argv", ["fmt.py"] + argv):
            return self.module.main()

    def test_check_flag_and_edition_reach_rustfmt(self):
        fake = FakeProcess(metadata([
            package("member", [target("lib", "/w/a.rs")]),
        ]))
        self.assertEqual(self.run_main(["--check"], fake), 0)
        self.assertEqual(fake.invocations,
                         [["rustfmt", "--edition", "2024", "--check", "/w/a.rs"]])

    def test_failing_chunk_fails_run_but_runs_every_chunk(self):
        self.module.ARGV_BUDGET = 1
        fake = FakeProcess(metadata([
            package("member", [
                target("lib", "/w/a.rs"),
                target("test", "/w/b.rs"),
            ]),
        ]), rustfmt_exit=1)
        self.assertEqual(self.run_main([], fake), 1)
        self.assertEqual(len(fake.invocations), 2)

    def test_double_dash_passthrough_reaches_rustfmt(self):
        fake = FakeProcess(metadata([
            package("member", [target("lib", "/w/a.rs")]),
        ]))
        self.assertEqual(
            self.run_main(["--check", "--", "--emit", "stdout"], fake), 0)
        self.assertIn("--emit", fake.invocations[0])

    def test_metadata_failure_returns_one(self):
        def failing(argv, **kwargs):
            raise subprocess.CalledProcessError(1, argv)
        with mock.patch.object(self.module.subprocess, "run", failing), \
                mock.patch.object(sys, "argv", ["fmt.py"]):
            self.assertEqual(self.module.main(), 1)


if __name__ == "__main__":
    unittest.main()
