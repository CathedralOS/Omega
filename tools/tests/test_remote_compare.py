#!/usr/bin/env python3
"""`--compare --remote` dispatches the hosted pilot at base and head with one
fixture list and diffs their rows; no network, Git remote or compiler build."""

import contextlib
import io
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import corpus_gate
import remote_compare

HEAD = "a" * 40
BASE = "b" * 40
LANE = "work/lane"


def row(fixture, status, expected=None, facts=None):
    return {"fixture": fixture, "tier": fixture.split("/", 1)[0], "status": status,
            "millis": 1, "expected_satisfied": expected, "facts": facts or {},
            "diagnostics": [], "recorded_at": None}


def verdict(revision, rows):
    return {"schema": "omega-corpus-verdict-v1", "revision": revision, "observations": rows}


class FakeRemote:
    """Stands in for `git` and `gh` at `remote_compare.run`.

    `measure(revision, fixtures, checking_only)` returns the reports a pilot
    run at that revision leaves, by file name."""

    def __init__(self, fixtures, measure, branches=None, base_fixtures=None,
                 artifact="compiler-pilot-records-{host}-{sha}-1"):
        self.fixtures = {HEAD: fixtures, BASE: base_fixtures or fixtures}
        self.measure = measure
        self.branches = {LANE: HEAD} if branches is None else branches
        self.artifact = artifact
        self.runs = {}
        self.calls = []

    def __call__(self, *arguments, input=None):
        self.calls.append(arguments)
        if arguments[:3] == ("git", "rev-parse", "HEAD"):
            return HEAD
        if arguments[:3] == ("git", "rev-parse", "--verify"):
            name = arguments[3][len("refs/remotes/origin/"):]
            if name not in self.branches:
                raise remote_compare.RemoteFailure("unknown revision")
            return self.branches[name]
        if arguments[:2] == ("git", "rev-parse"):
            return self.branches[arguments[2][len("origin/"):]]
        if arguments[:2] == ("git", "ls-tree"):
            return "\n".join(f"tests/omega/{fixture}/main.omg"
                             for fixture in self.fixtures[arguments[4]])
        if arguments[:2] == ("git", "for-each-ref"):
            return "\n".join(self.branches)
        if arguments[:3] == ("git", "rev-list", "--count"):
            return "0"
        if arguments[:2] == ("git", "push"):
            if arguments[3] != "--delete":
                revision, ref = arguments[3].split(":refs/heads/")
                self.branches[ref] = revision
            return ""
        if arguments[:3] == ("gh", "workflow", "run"):
            ref = arguments[arguments.index("--ref") + 1]
            run_id = str(100 + len(self.runs))
            self.runs[run_id] = dict(ref=ref, sha=self.branches[ref], inputs=json.loads(input))
            return f"https://github.com/example/repo/actions/runs/{run_id}"
        if arguments[:3] == ("gh", "run", "view"):
            if "url" in arguments:
                return f"https://github.com/example/repo/actions/runs/{arguments[3]}"
            return json.dumps({"status": "completed", "conclusion": "success"})
        if arguments[:2] == ("gh", "api"):
            run = self.runs[arguments[2].split("/")[-2]]
            return self.artifact.format(host=run["inputs"]["native_host"], sha=run["sha"])
        if arguments[:3] == ("gh", "run", "download"):
            run = self.runs[arguments[3]]
            target = Path(arguments[arguments.index("-D") + 1])
            target.mkdir(parents=True)
            fixtures = run["inputs"]["fixtures"].split(",")
            for name, rows in self.measure(run["sha"], fixtures,
                                           run["inputs"]["checking_only"] == "true").items():
                (target / name).write_text(json.dumps(verdict(run["sha"], rows)))
            return ""
        raise AssertionError(arguments)

    def dispatched(self):
        return [(run["sha"], run["inputs"]) for run in self.runs.values()]


def rejects_on_base_only(revision, fixtures, checking_only):
    """The fail fixture stops rejecting at HEAD; the pass fixture checks on both."""
    rows = [row(fixture, "checked") if fixture.startswith("pass/")
            else row(fixture, "rejected", expected=True) if revision == BASE
            else row(fixture, "checked") for fixture in fixtures]
    return {"checking.json": rows}


class RemoteCompare(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        for fixture in ("pass/calls/forward", "fail/calls/refused", "run/calls/window"):
            (self.root / "tests/omega" / fixture).mkdir(parents=True)
        for patcher in (patch.object(corpus_gate, "ROOT", self.root),
                        patch.object(corpus_gate, "dirty_paths", return_value=[]),
                        patch.object(remote_compare.time, "sleep")):
            patcher.start()
            self.addCleanup(patcher.stop)

    def compare(self, fake, *, host="linux_x86_64", checking_only=True,
                filter_text="calls/forward,calls/refused", branch=""):
        options = SimpleNamespace(json=str(self.root / "verdict.json"))
        output = io.StringIO()
        with patch.object(remote_compare, "run", fake), \
             contextlib.redirect_stdout(output), contextlib.redirect_stderr(io.StringIO()):
            code = remote_compare.compare(options, BASE, host, checking_only,
                                          filter_text, branch)
        return code, output.getvalue()

    def test_one_fixture_list_dispatches_at_base_and_head_and_a_loss_exits_one(self):
        fixtures = ["fail/calls/refused", "pass/calls/forward"]
        fake = FakeRemote(fixtures, rejects_on_base_only)
        code, output = self.compare(fake)
        self.assertEqual(code, 1)
        self.assertEqual(fake.dispatched(), [
            (sha, {"fixtures": ",".join(fixtures), "native_host": "linux_x86_64",
                   "checking_only": "true"}) for sha in (BASE, HEAD)])
        self.assertIn("- checking: fail/calls/refused: rejected -> checked", output)
        self.assertIn("2 lost", output)
        document = json.loads((self.root / "verdict.json").read_text())
        self.assertEqual(document["remote"]["runs"]["base"],
                         "https://github.com/example/repo/actions/runs/100")
        # The base ref exists only while its run is created and measured.
        self.assertEqual([call[4] for call in fake.calls if call[3:4] == ("--delete",)],
                         [remote_compare.BASE_BRANCH + BASE[:12]])

    def test_cached_base_rows_dispatch_only_the_head(self):
        fixtures = ["fail/calls/refused", "pass/calls/forward"]
        self.compare(FakeRemote(fixtures, rejects_on_base_only))
        again = FakeRemote(fixtures, rejects_on_base_only)
        code, _ = self.compare(again)
        self.assertEqual(code, 1)
        self.assertEqual([sha for sha, _ in again.dispatched()], [HEAD])

    def test_unpushed_head_is_refused_before_any_push_or_dispatch(self):
        fake = FakeRemote(["pass/calls/forward"], rejects_on_base_only, branches={})
        with self.assertRaisesRegex(remote_compare.RemoteUsage, "none is pushed"):
            self.compare(fake)
        self.assertFalse([call for call in fake.calls if call[:2] in (("git", "push"), ("gh", "workflow"))])
        with self.assertRaisesRegex(remote_compare.RemoteUsage, "not a pushed work branch"):
            self.compare(fake, branch="work/elsewhere")

    def test_head_ahead_of_its_branch_is_pushed_there_first(self):
        fake = FakeRemote(["pass/calls/forward"], rejects_on_base_only, branches={LANE: "c" * 40})
        self.compare(fake)
        pushes = [call[3] for call in fake.calls if call[:2] == ("git", "push")]
        self.assertEqual(pushes[0], f"{HEAD}:refs/heads/{LANE}")
        self.assertEqual(fake.runs["101"]["ref"], LANE)

    def test_run_tier_fixtures_stay_local(self):
        fake = FakeRemote(["pass/calls/forward"], rejects_on_base_only)
        with self.assertRaisesRegex(remote_compare.RemoteUsage, "run/calls/window"):
            self.compare(fake, filter_text="calls/")
        self.assertFalse(fake.runs)

    def test_a_fixture_new_at_head_is_dispatched_only_there_and_gains(self):
        fake = FakeRemote(["fail/calls/refused", "pass/calls/forward"],
                          lambda revision, fixtures, _: {"checking.json": [
                              row(fixture, "rejected", expected=True) if fixture.startswith("fail/")
                              else row(fixture, "checked") for fixture in fixtures]},
                          base_fixtures=["fail/calls/refused"])
        code, output = self.compare(fake)
        self.assertEqual(code, 0)
        self.assertEqual(fake.runs["100"]["inputs"]["fixtures"], "fail/calls/refused")
        self.assertIn("+ checking: pass/calls/forward: new checked", output)

    def test_both_leg_runs_on_the_matching_host_and_compares_execution(self):
        def measure(revision, fixtures, checking_only):
            self.assertFalse(checking_only)
            exit_code = "0" if revision == BASE else "3"
            return {"checking.json": [row(fixture, "checked") for fixture in fixtures],
                    "both.json": [row(fixture, "built", facts={"exit": exit_code})
                                  for fixture in fixtures if fixture.startswith("pass/")]}
        fake = FakeRemote(["pass/calls/forward"], measure)
        code, output = self.compare(fake, host="macos_arm64", checking_only=False,
                                    filter_text="calls/forward")
        self.assertEqual(code, 1)
        self.assertEqual({run["inputs"]["native_host"] for run in fake.runs.values()},
                         {"macos_arm64"})
        self.assertIn("- both: pass/calls/forward", output)

    def test_a_base_older_than_the_records_artifact_reads_the_full_one(self):
        fake = FakeRemote(["pass/calls/forward"], rejects_on_base_only,
                          artifact="compiler-pilot-{host}-{sha}-1")
        code, _ = self.compare(fake, filter_text="calls/forward")
        self.assertEqual(code, 0)
        downloads = [call[5] for call in fake.calls if call[:3] == ("gh", "run", "download")]
        self.assertEqual(downloads, [f"compiler-pilot-linux_x86_64-{BASE}-1",
                                     f"compiler-pilot-linux_x86_64-{HEAD}-1"])

    def test_a_run_without_rows_is_not_measured(self):
        fake = FakeRemote(["pass/calls/forward"],
                          lambda revision, fixtures, _: {} if revision == HEAD
                          else {"checking.json": [row(fixtures[0], "checked")]})
        with self.assertRaisesRegex(remote_compare.RemoteFailure, "head run .* no corpus rows"):
            self.compare(fake, filter_text="calls/forward")

    def test_dirty_tree_and_foreign_host_are_refused(self):
        fake = FakeRemote(["pass/calls/forward"], rejects_on_base_only)
        with self.assertRaisesRegex(remote_compare.RemoteUsage, "macos_x86_64"):
            self.compare(fake, host="macos_x86_64")
        with patch.object(corpus_gate, "dirty_paths", return_value=["tools/x.py"]), \
             self.assertRaisesRegex(remote_compare.RemoteUsage, "commit"):
            self.compare(fake)
        self.assertFalse(fake.runs)


class LocalCompareBudget(unittest.TestCase):
    """A local compare past LOCAL_COMPARE_BUDGET fixtures refuses without
    --local; --remote sends it to hosted CI instead."""

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "golden.txt").write_text("", encoding="utf-8")
        for index in range(3):
            (self.root / "tests/omega/pass/calls" / f"case_{index}").mkdir(parents=True)

    def run_gate(self, *extra):
        argv = ["corpus_gate", "--golden", str(self.root / "golden.txt"),
                "--filter", "calls/", *extra]
        errors = io.StringIO()
        with patch.object(corpus_gate, "ROOT", self.root), \
             patch.object(corpus_gate, "LOCAL_COMPARE_BUDGET", 2), \
             patch.object(corpus_gate, "git_out", return_value=BASE), \
             patch.object(corpus_gate, "build_runner") as build, \
             patch.object(corpus_gate, "run_runner", return_value=[]), \
             patch.object(corpus_gate, "compare_with_revision", return_value=0), \
             patch.object(remote_compare, "compare", return_value=0) as remote, \
             patch.dict(corpus_gate.os.environ, {}, clear=True), \
             patch.object(sys, "argv", argv), \
             contextlib.redirect_stderr(errors), contextlib.redirect_stdout(io.StringIO()):
            code = corpus_gate.main()
        return code, build.called, remote.called, errors.getvalue()

    def test_large_local_compare_is_refused_before_building(self):
        code, built, remote, errors = self.run_gate("--compare", "origin/main")
        self.assertEqual((code, built, remote), (2, False, False))
        self.assertIn("--remote", errors)

    def test_local_flag_admits_the_local_compare(self):
        code, built, remote, _ = self.run_gate("--compare", "origin/main", "--local")
        self.assertEqual((code, built, remote), (0, True, False))

    def test_remote_compare_builds_nothing_here(self):
        code, built, remote, _ = self.run_gate("--compare", "origin/main", "--remote")
        self.assertEqual((code, built, remote), (0, False, True))

    def test_remote_needs_a_base_and_exact_fixtures(self):
        self.assertEqual(self.run_gate("--remote")[0], 2)
        code, _, remote, errors = self.run_gate("--compare", "origin/main", "--remote",
                                                "--runner", "corpus_runner")
        self.assertEqual((code, remote), (2, False))
        self.assertIn("--runner", errors)


if __name__ == "__main__":
    unittest.main()
