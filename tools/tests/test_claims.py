#!/usr/bin/env python3
"""Local Git protocol tests; no GitHub access or non-standard Python packages."""

import json
from datetime import datetime, timedelta, timezone
import importlib.util
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import time
from types import SimpleNamespace
import unittest

import worktree_testkit as kit
from unittest import mock


CLAIMS = Path(__file__).resolve().parents[1] / "claims.py"
CLAIMS_REF = "refs/coordination/omega-claims/main"
MAIN_REF = "refs/heads/main"
BOARD = "- **ALPHA-ITEM.** First item.\n- **BETA-ITEM.** Second item.\n- **GAMMA-ITEM.** Third item.\n"


def load_claims():
    return kit.load_module(CLAIMS, "claims_under_test")

class PathTests(unittest.TestCase):
    def test_paths_overlap_covers_exact_and_parent_child(self):
        module = load_claims()
        self.assertTrue(module.paths_overlap("a/b", "a/b"))
        self.assertTrue(module.paths_overlap("a/b", "a/b/c"))
        self.assertTrue(module.paths_overlap("a/b/c", "a/b"))
        self.assertFalse(module.paths_overlap("a/b", "a/c"))
        self.assertFalse(module.paths_overlap("a/b", "a/bc"))

    def test_paths_overlap_explodes_comma_joined_fields(self):
        module = load_claims()
        self.assertTrue(module.paths_overlap("a/b,a/c", "a/b/deep"))
        self.assertTrue(module.paths_overlap("a/b/deep", "a/b,a/c"))
        self.assertTrue(module.paths_overlap("a/b, a/c", "a/c"))
        self.assertFalse(module.paths_overlap("a/b,a/c", "a/d"))
        self.assertFalse(module.paths_overlap("a/b,a/c", "a/bc"))

    def test_task_boards_never_overlap_claimed_paths(self):
        module = load_claims()
        for board in (*module.BOARDS, "samples/app/TASKS.md", "./tasks.md"):
            with self.subTest(board=board):
                self.assertFalse(module.paths_overlap(board, board))
                self.assertFalse(module.paths_overlap("samples", board))
                self.assertFalse(module.paths_overlap(board, "samples"))
        self.assertTrue(module.paths_overlap("TASKS.md,src/x", "src/x/file.rs"))

    def test_normalize_path_rejects_absolute_and_parent_escape(self):
        module = load_claims()
        self.assertEqual(module.normalize_path("src\\windows\\style"), "src/windows/style")
        self.assertEqual(module.normalize_path(" ./leading/dot "), "leading/dot")
        for bad in ("/abs/path", "C:/abs/path", "../escape", "a/../../escape", "."):
            with self.subTest(bad=bad):
                self.assertRaises(module.ClaimsError, module.normalize_path, bad)

    def test_normalize_path_rejects_comma_joined_field(self):
        module = load_claims()
        self.assertRaises(module.ClaimsError, module.normalize_path, "a/b,a/c")
        self.assertRaises(module.ClaimsError, module.normalize_path, "a/b a/c")

    def test_paths_overlap_explodes_whitespace_joined_fields(self):
        module = load_claims()
        self.assertTrue(module.paths_overlap("a/b a/c", "a/c/deep"))
        self.assertTrue(module.paths_overlap("a/b\ta/c,a/d", "a/d"))
        self.assertFalse(module.paths_overlap("a/b a/c", "a/bc"))

    def test_directory_fence_tolerates_unprobeable_names(self):
        """A joined list recorded literally can exceed the OS name limit."""
        module = load_claims()
        name = " ".join(["segment/" + "x" * 200 + ".rs"] * 4)
        with tempfile.TemporaryDirectory() as repository:
            self.assertFalse(module.is_directory_fence(repository, name))


class NoteRetentionTests(unittest.TestCase):
    """Every write rewrites the whole registry, so trim_notes bounds it."""

    def setUp(self):
        self.claims = load_claims()
        self.now = datetime.now(timezone.utc)

    def stamp(self, days_ago):
        return (self.now - timedelta(days=days_ago)).isoformat()

    def note(self, ticket, days_ago, swept=False):
        return {"ticket": ticket, "item": "ITEM", "owner": "owner", "text": "t",
                "created_utc": self.stamp(days_ago),
                "swept_utc": self.stamp(days_ago) if swept else None}

    def test_live_notes_stay_and_finished_tickets_keep_their_newest_recent_note(self):
        live, done, old = "a" * 32, "b" * 32, "c" * 32
        record = {"claims": [{"ticket": live, "expires_utc": (self.now + timedelta(hours=1)).isoformat()},
                             {"ticket": done, "expires_utc": self.stamp(1)}],
                  "notes": [self.note(live, 40), self.note(live, 0.1),
                            self.note(done, 2), self.note(done, 1),
                            self.note(old, 10)]}
        self.claims.trim_notes(record)
        kept = [(note["ticket"], note["created_utc"]) for note in record["notes"]]
        self.assertEqual(kept, [(live, self.stamp(40)), (live, self.stamp(0.1)),
                                (done, self.stamp(1))])

    def test_swept_history_keeps_its_own_bound(self):
        ticket = "d" * 32
        record = {"claims": [], "notes": [self.note(ticket, 0, swept=True)
                                         for _ in range(self.claims.SWEPT_NOTE_RETENTION + 5)]}
        self.claims.trim_notes(record)
        self.assertEqual(len(record["notes"]), self.claims.SWEPT_NOTE_RETENTION)


class RegistrationBudgetTests(unittest.TestCase):
    def registry(self):
        module = load_claims()
        registry = module.Claims.__new__(module.Claims)
        registry.attempted_claims = []
        registry.repository = "."
        registry.push_url = "test-remote"
        registry.deadline = time.monotonic() + 15
        registry.git = mock.Mock(return_value=SimpleNamespace(returncode=1, stdout=""))
        return module, registry

    def options(self, fresh=True):
        return SimpleNamespace(command="claim", provenance={}, ticket="a" * 32, owner="worker", item="ITEM",
                               paths=["src/file.rs"], board=None, note=None,
                               lease_minutes=15, allow_overlap=False, fresh_ticket=fresh)

    def snapshot(self):
        return {"version": "b" * 40, "record": {"claims": [], "notes": []}}

    def test_new_ticket_never_searches_historical_messages(self):
        _, registry = self.registry()
        snapshot = self.snapshot()
        result = registry.prepare_claim(self.options(), snapshot, [])
        self.assertTrue(result["proceed"])
        registry.git.assert_not_called()

    def test_external_retired_ticket_still_rejects(self):
        module, registry = self.registry()
        registry.git.return_value = SimpleNamespace(returncode=0, stdout="removed")
        with self.assertRaises(module.ClaimsError):
            registry.prepare_claim(self.options(fresh=False), self.snapshot(), [])
        self.assertEqual(registry.git.call_args.args[0], "log")

    def test_uncertain_accepted_then_removed_claim_cannot_resurrect(self):
        module, registry = self.registry()
        registry.attempted_claims = ["c" * 40]
        registry.git.return_value = SimpleNamespace(returncode=0, stdout="")
        with self.assertRaises(module.ClaimsError):
            registry.prepare_claim(self.options(), self.snapshot(), [])
        self.assertEqual(registry.git.call_args.args[:2], ("merge-base", "--is-ancestor"))

    def test_lost_push_race_only_checks_this_invocations_candidates(self):
        _, registry = self.registry()
        registry.attempted_claims = ["c" * 40]
        self.assertTrue(registry.prepare_claim(self.options(), self.snapshot(), [])["proceed"])
        self.assertEqual(registry.git.call_count, 1)

    def test_failed_ancestry_check_cannot_recreate_claim(self):
        module, registry = self.registry()
        registry.attempted_claims = ["c" * 40]
        registry.git.return_value = SimpleNamespace(returncode=128, stdout="")
        with self.assertRaises(module.ClaimsError):
            registry.prepare_claim(self.options(), self.snapshot(), [])
        self.assertEqual(registry.git.call_args.args[0], "merge-base")

    def test_existing_snapshot_is_not_refetched(self):
        module, registry = self.registry()
        registry.git.return_value = SimpleNamespace(returncode=0, stdout="")
        with mock.patch.object(module.coordination, "fetch_object") as fetch, \
                mock.patch.object(module.coordination, "remember_object") as remember:
            registry.fetch_version("b" * 40)
        fetch.assert_not_called()
        remember.assert_called_once_with(".", "test-remote", CLAIMS_REF, "b" * 40,
                                         deadline=registry.deadline)

    def test_cache_write_obeys_the_existing_deadline(self):
        module = load_claims()
        with mock.patch.object(module.coordination.subprocess, "Popen") as run:
            with self.assertRaises(module.coordination.CoordinationTimeout):
                module.coordination.remember_object(
                    ".", "test-remote", CLAIMS_REF, "b" * 40,
                    deadline=time.monotonic() - 1)
        run.assert_not_called()

    def test_git_timeout_is_an_uncertain_outcome_not_success(self):
        module = load_claims()
        process = mock.Mock(pid=12345, args=["git", "push"])
        process.wait.side_effect = [subprocess.TimeoutExpired("git", 0.1), 0]
        with mock.patch.object(module.coordination.subprocess, "Popen", return_value=process), \
                mock.patch.object(module.coordination.os, "killpg", create=True), \
                mock.patch.object(module.coordination.subprocess, "run"), \
                mock.patch.object(module.coordination.time, "monotonic", return_value=0.0):
            with self.assertRaises(module.coordination.CoordinationTimeout):
                module.coordination.git(".", "push", deadline=time.monotonic() + 0.1)
        first_wait = process.wait.call_args_list[0]
        self.assertGreater(first_wait.kwargs["timeout"], 0)
        self.assertLessEqual(first_wait.kwargs["timeout"], 0.1)

    def test_deadline_applies_before_launch_and_sleep(self):
        module, registry = self.registry()
        registry.deadline = time.monotonic() - 1
        with mock.patch.object(module.coordination.subprocess, "Popen") as run:
            with self.assertRaises(module.coordination.CoordinationTimeout):
                module.coordination.git(".", "status", deadline=registry.deadline)
        run.assert_not_called()
        with self.assertRaises(module.coordination.CoordinationTimeout):
            registry.pause(1, 1, 1)

    def test_transport_child_cannot_hold_timeout_output_open(self):
        module = load_claims()
        popen = subprocess.Popen
        script = ("import subprocess, sys, time; "
                  "subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(3)']); "
                  "time.sleep(3)")

        def stalled_transport(arguments, **options):
            return popen([sys.executable, "-c", script], **options)

        started = time.monotonic()
        with mock.patch.object(module.coordination.subprocess, "Popen", side_effect=stalled_transport):
            with self.assertRaises(module.coordination.CoordinationTimeout):
                module.coordination.git(".", "status", deadline=started + 0.2)
        self.assertLess(time.monotonic() - started, 2)

    def test_timed_out_claim_keeps_scope_and_ticket_locally(self):
        module = load_claims()
        with tempfile.TemporaryDirectory() as repository:
            with mock.patch.object(module, "Claims", side_effect=module.coordination.CoordinationTimeout("test")), \
                    mock.patch.object(module, "emit") as emit:
                result = module.main(["--repository", repository, "claim", "--item", "ITEM",
                                      "--owner", "worker", "--path", "src/file.rs"])
            self.assertEqual(result, 3)
            report = emit.call_args.args[0]
            self.assertEqual(report["state"], "unavailable")
            pending = json.loads(Path(report["pending_file"]).read_text(encoding="utf-8"))
            self.assertEqual(pending["ticket"], report["ticket"])
            self.assertEqual(pending["paths"], ["src/file.rs"])
            self.assertTrue(pending["may_have_published"])


class ClaimsTests(unittest.TestCase):
    def setUp(self):
        (self.root, self.remote, self.a,
         self.b) = kit.seed_remote_pair(
            self, "omega claims test ", "Claims Test")
        (self.a / "TASKS.md").write_text(BOARD, encoding="utf-8")
        self.git(self.a, "add", "TASKS.md")
        self.initial = self.commit(self.a, "initial")
        kit.clone_remote_pair(
            self, self.remote, self.a, self.b, "Claims Test")

    def git(self, directory, *arguments, input_text=""):
        return kit.git_input(self, directory, *arguments, input_text=input_text)

    def commit(self, directory, message):
        self.git(directory, "commit", "--allow-empty", "-m", message)
        return self.git(directory, "rev-parse", "HEAD")

    def start(self, directory, *arguments):
        return kit.start_tool(self, CLAIMS, directory, *arguments)

    def finish(self, process):
        return kit.finish_tool(self, process, "claims")

    def run_claims(self, directory, *arguments, expected=0):
        code, output, error = self.finish(self.start(directory, *arguments))
        self.assertEqual(code, expected, error + output)
        return json.loads(output) if output else error

    def claim(self, directory, item="ALPHA-ITEM", owner="writer", *extra, expected=0):
        return self.run_claims(directory, "claim", "--item", item, "--board", "TASKS.md",
                               "--owner", owner, *extra, expected=expected)

    def rewrite_record(self, record):
        previous = self.git(self.a, "ls-remote", "--refs", str(self.remote),
                            CLAIMS_REF).split("\t")[0]
        identity = self.git(self.remote, "commit-tree", previous + "^{tree}", "-p", previous,
                            "-F", "-", input_text=json.dumps(record))
        self.git(self.remote, "update-ref", CLAIMS_REF, identity, previous)
        return identity

    def remote_record(self):
        return json.loads(self.git(self.remote, "show", "-s", "--format=%B", CLAIMS_REF))

    def test_cold_acquisition_extends_only_the_automatic_budget(self):
        module = load_claims()
        self.claim(self.a)
        version = self.git(self.remote, "rev-parse", CLAIMS_REF)
        for explicit in (None, 15):
            with self.subTest(explicit=explicit):
                reader = self.root / ("cold-default" if explicit is None else "cold-explicit")
                self.git(self.root, "clone", "--no-local", str(self.remote), str(reader))
                registry = module.Claims(reader, "origin", explicit)
                original_fetch = module.coordination.fetch_object
                def elapsed_acquisition(*arguments, **keywords):
                    # Account for a transfer taking 20 seconds without sleeping.
                    # The default bootstrap must survive; the explicit budget
                    # must expire. Complete normal acquisition for the survivor.
                    registry.deadline -= 20
                    module.coordination.remaining_seconds(registry.deadline)
                    keywords["deadline"] = registry.deadline
                    return original_fetch(*arguments, **keywords)
                with mock.patch.object(module.coordination, "fetch_object", side_effect=elapsed_acquisition):
                    if explicit is None:
                        registry.fetch_version(version)
                        self.assertEqual(self.git(reader, "rev-parse", module.coordination.cache_ref(str(self.remote), CLAIMS_REF)), version)
                        warm = module.Claims(reader, "origin")
                        self.assertLessEqual(warm.deadline - warm.started, 15)
                    else:
                        with self.assertRaises(module.coordination.CoordinationTimeout):
                            registry.fetch_version(version)

    def test_cached_reader_gets_acquisition_budget_for_a_new_tip(self):
        module = load_claims()
        self.claim(self.a)
        initial = module.Claims(self.b, "origin").snapshot()
        self.claim(self.a, "BETA-ITEM", "another session")
        version = self.git(self.remote, "rev-parse", CLAIMS_REF)
        self.assertNotEqual(initial["version"], version)
        registry = module.Claims(self.b, "origin")
        original_fetch = module.coordination.fetch_object
        def elapsed_acquisition(*arguments, **keywords):
            registry.deadline -= 20
            module.coordination.remaining_seconds(registry.deadline)
            keywords["deadline"] = registry.deadline
            return original_fetch(*arguments, **keywords)
        with mock.patch.object(module.coordination, "fetch_object", side_effect=elapsed_acquisition):
            registry.fetch_version(version)
        self.assertEqual(self.git(self.b, "rev-parse", module.coordination.cache_ref(str(self.remote), CLAIMS_REF)), version)

    def test_existing_tip_warms_local_cache_without_fetch_or_remote_write(self):
        module = load_claims()
        self.claim(self.a)
        before = self.git(self.remote, "show-ref")
        registry = module.Claims(self.a, "origin")
        with mock.patch.object(module.coordination, "fetch_object") as fetch:
            snapshot = registry.snapshot()
        fetch.assert_not_called()
        cache = module.coordination.cache_ref(str(self.remote), CLAIMS_REF)
        self.assertEqual(self.git(self.a, "rev-parse", cache), snapshot["version"])
        self.assertEqual(self.git(self.remote, "show-ref"), before)

    def test_missing_and_advancing_tips_refresh_local_cache(self):
        module = load_claims()
        first = self.claim(self.a)
        cache = module.coordination.cache_ref(str(self.remote), CLAIMS_REF)
        for item in ("ALPHA-ITEM", "BETA-ITEM"):
            if item == "BETA-ITEM":
                self.claim(self.a, item, "second owner")
            version = self.git(self.remote, "rev-parse", CLAIMS_REF)
            missing = subprocess.run(
                ["git", "-C", str(self.b), "cat-file", "-e", version],
                capture_output=True, env=self.environment, timeout=5)
            self.assertNotEqual(missing.returncode, 0)
            before = self.git(self.remote, "show-ref")
            status = self.run_claims(self.b, "status")
            self.assertEqual(status["version"], version)
            self.assertIn(first["ticket"], [claim["ticket"] for claim in status["claims"]])
            self.assertEqual(self.git(self.b, "rev-parse", cache), version)
            self.assertEqual(self.git(self.remote, "show-ref"), before)

    def test_cache_is_scoped_to_remote_and_protocol_reference(self):
        module = load_claims()
        self.claim(self.a)
        other = self.root / "other remote.git"
        self.git(self.root, "clone", "--bare", str(self.remote), str(other))
        self.git(other, "fetch", str(self.remote), f"{CLAIMS_REF}:{CLAIMS_REF}")
        self.git(self.a, "remote", "add", "other", str(other))
        first = module.Claims(self.a, "origin").snapshot()
        second = module.Claims(self.a, "other").snapshot()
        first_cache = module.coordination.cache_ref(str(self.remote), CLAIMS_REF)
        second_cache = module.coordination.cache_ref(str(other), CLAIMS_REF)
        self.assertNotEqual(first_cache, second_cache)
        self.assertNotEqual(first_cache, module.coordination.cache_ref(
            str(self.remote), "refs/coordination/omega-landing/main"))
        self.assertEqual(self.git(self.a, "rev-parse", first_cache), first["version"])
        self.assertEqual(self.git(self.a, "rev-parse", second_cache), second["version"])

    def test_cache_lock_collision_preserves_successful_snapshot(self):
        module = load_claims()
        self.claim(self.a)
        registry = module.Claims(self.a, "origin")
        first = registry.snapshot()
        cache = module.coordination.cache_ref(str(self.remote), CLAIMS_REF)
        self.claim(self.a, "BETA-ITEM", "second owner")
        before = self.git(self.remote, "show-ref")
        lock = Path(self.git(self.a, "rev-parse", "--git-path", cache + ".lock"))
        if not lock.is_absolute():
            lock = self.a / lock
        lock.write_text("concurrent local cache writer\n", encoding="utf-8")
        second = module.Claims(self.a, "origin").snapshot()
        self.assertNotEqual(first["version"], second["version"])
        self.assertEqual(len(second["record"]["claims"]), 2)
        self.assertEqual(self.git(self.a, "rev-parse", cache), first["version"])
        self.assertEqual(self.git(self.remote, "show-ref"), before)
        self.assertTrue(lock.exists())

    def test_status_available_then_claim_release_round_trip(self):
        self.assertEqual(self.run_claims(self.a, "status")["state"], "available")
        claimed = self.claim(self.a, owner="A / café", )
        ticket = claimed["ticket"]
        status = self.run_claims(self.b, "status")
        self.assertEqual(status["state"], "claimed")
        entry = status["claims"][0]
        self.assertEqual((entry["item"], entry["board"], entry["owner"]),
                         ("ALPHA-ITEM", "TASKS.md", "A / café"))
        self.assertEqual(self.run_claims(self.b, "release", "--ticket", ticket)["state"],
                         "released")
        self.assertEqual(self.run_claims(self.a, "status")["state"], "available")

    def test_claim_validates_board_item_and_lease_bounds(self):
        self.claim(self.a, item="NOPE", expected=1)
        self.run_claims(self.a, "claim", "--item", "ALPHA-ITEM", "--board", "TASKS.md",
                        "--owner", "A", "--lease-minutes", "1", expected=1)
        self.run_claims(self.a, "claim", "--item", "ALPHA-ITEM", "--board", "TASKS.md",
                        "--owner", "A", "--lease-minutes", "2000", expected=1)
        self.assertEqual(self.run_claims(self.a, "status")["state"], "available")

    def test_same_item_claims_coexist_and_release_independently(self):
        # Claims make work visible; they never refuse it (AGENTS.md).
        claimed = self.claim(self.a, owner="A")
        second = self.run_claims(self.b, "claim", "--item", "ALPHA-ITEM", "--owner", "B")
        self.assertEqual(second["state"], "claimed")
        self.assertEqual(second["overlaps"][0]["owner"], "A")
        self.assertEqual(second["overlaps"][0]["reason"], "same item")
        self.assertEqual(len(self.run_claims(self.a, "status")["claims"]), 2)
        self.run_claims(self.a, "release", "--ticket", claimed["ticket"])
        claims = self.run_claims(self.a, "status")["claims"]
        self.assertEqual([claim["owner"] for claim in claims], ["B"])

    def test_task_board_path_claims_are_rejected_even_with_overlap(self):
        for board in ("TASKS.md", "TASKS_BOOTSTRAP.md", "TASKS_OPTIMIZER.md",
                      "./TASKS.md", "samples\\app\\TASKS.md", "tasks.md",
                      "src/x,TASKS.md"):
            with self.subTest(board=board):
                error = self.claim(self.a, "ALPHA-ITEM", "A", "--path", board,
                                   "--allow-overlap", expected=1)
                self.assertIn("is shared and cannot be claimed", error)
        self.assertEqual(self.run_claims(self.a, "status")["state"], "available")

    def test_legacy_board_claim_does_not_block_edits_or_other_items(self):
        self.claim(self.a, "ALPHA-ITEM", "A", "--path", "src/x")
        record = self.remote_record()
        record["claims"][0]["paths"] = ["TASKS.md,src/x"]
        self.rewrite_record(record)
        (self.b / "TASKS.md").write_text(BOARD + "\nBoard update.\n", encoding="utf-8")
        report = self.run_claims(self.b, "audit", "--worktree", str(self.b),
                                 "--owner", "B")
        self.assertEqual(report["changed"], ["TASKS.md"])
        self.assertEqual(report["uncovered"], [])
        self.assertEqual(report["violations"], [])
        self.claim(self.b, "BETA-ITEM", "B")
        self.claim(self.b, "ALPHA-ITEM", "B")
        self.claim(self.b, "GAMMA-ITEM", "B", "--path", "src/x/file.rs")

    def test_claim_splits_comma_joined_path_values(self):
        self.claim(self.a, "ALPHA-ITEM", "A", "--path", "src/x,src/y",
                   "--path", "src/z")
        entry = self.run_claims(self.b, "status")["claims"][0]
        self.assertEqual(entry["paths"], ["src/x", "src/y", "src/z"])
        conflict = self.run_claims(self.b, "claim", "--item", "BETA-ITEM",
                                   "--board", "TASKS.md",
                                   "--owner", "B", "--path", "src/y/deep")
        self.assertEqual(conflict["overlaps"][0]["reason"], "overlapping paths")
        self.assertEqual(conflict["overlaps"][0]["shared_paths"], ["src/y/deep"])

    def test_claim_splits_whitespace_joined_path_values(self):
        self.claim(self.a, "ALPHA-ITEM", "A", "--path", "src/x src/y")
        entry = self.run_claims(self.b, "status")["claims"][0]
        self.assertEqual(entry["paths"], ["src/x", "src/y"])

    def test_status_survives_an_unprobeable_legacy_path(self):
        """A space-joined path recorded literally must not crash status."""
        self.claim(self.a, "ALPHA-ITEM", "A", "--path", "src/x")
        record = self.remote_record()
        joined = " ".join(["src/" + "x" * 200 + ".rs"] * 4)
        record["claims"][0]["paths"] = [joined]
        record["claims"][0]["board"] = None
        self.rewrite_record(record)
        status = self.run_claims(self.b, "status")
        self.assertEqual(status["state"], "claimed")

    def test_claim_rejects_empty_path_segments(self):
        for bad in ("src/x,,src/y", "src/x,", ",src/x"):
            with self.subTest(bad=bad):
                self.claim(self.a, "ALPHA-ITEM", "A", "--path", bad, expected=1)
        self.assertEqual(self.run_claims(self.a, "status")["state"], "available")

    def test_legacy_comma_joined_claim_reports_overlap(self):
        """A claim recorded before comma-splitting keeps its full coverage."""
        self.claim(self.a, "ALPHA-ITEM", "A", "--path", "src/x")
        record = self.remote_record()
        record["claims"][0]["paths"] = ["src/x,src/y"]
        self.rewrite_record(record)
        conflict = self.run_claims(self.b, "claim", "--item", "BETA-ITEM",
                                   "--board", "TASKS.md",
                                   "--owner", "B", "--path", "src/y/deep")
        self.assertEqual(conflict["state"], "claimed")
        self.assertEqual(conflict["overlaps"][0]["shared_paths"], ["src/y/deep"])

    def test_path_overlap_is_advisory_with_or_without_legacy_flag(self):
        self.claim(self.a, "ALPHA-ITEM", "A", "--path", "src/x")
        conflict = self.run_claims(self.b, "claim", "--item", "BETA-ITEM", "--board",
                                   "TASKS.md", "--owner", "B",
                                   "--path", "src/x/y")
        self.assertEqual(conflict["overlaps"][0]["reason"], "overlapping paths")
        self.assertEqual(conflict["overlaps"][0]["shared_paths"], ["src/x/y"])
        allowed = self.run_claims(self.b, "claim", "--item", "GAMMA-ITEM", "--board",
                                  "TASKS.md", "--owner", "B",
                                  "--path", "src/x/deep", "--allow-overlap")
        self.assertEqual(allowed["overlaps"][0]["item"], "ALPHA-ITEM")

    def test_directory_paths_are_estimates_even_without_board(self):
        (self.a / "src" / "dir").mkdir(parents=True)
        first = self.run_claims(self.a, "claim", "--item", "OFF-BOARD",
                                "--owner", "A", "--path", "src/dir")
        second = self.claim(self.b, "BETA-ITEM", "B", "--path", "src/dir/new.rs")
        self.assertEqual(second["overlaps"][0]["ticket"], first["ticket"])

    def test_second_claim_on_an_item_is_recorded_with_its_overlap(self):
        self.claim(self.a, owner="A")
        second = self.run_claims(self.b, "claim", "--item", "ALPHA-ITEM",
                                 "--owner", "B", "--allow-overlap")
        self.assertEqual(second["state"], "claimed")
        self.assertEqual(second["overlaps"][0]["reason"], "same item")
        self.assertEqual(len(self.run_claims(self.a, "status")["claims"]), 2)

    def test_file_claim_stays_off_board(self):
        (self.a / "notes.txt").write_text("x\n", encoding="utf-8")
        for path in ("notes.txt", "src/new_file.rs"):
            with self.subTest(path=path):
                claimed = self.run_claims(self.a, "claim", "--item",
                                          f"OFF-BOARD-{path}", "--owner", "A",
                                          "--path", path)
                self.assertEqual(claimed["state"], "claimed")

    def test_status_keeps_legacy_directory_claims_without_fences(self):
        claimed = self.claim(self.a, "ALPHA-ITEM", "A", "--path", "src/dir")
        record = self.remote_record()
        record["claims"][0]["board"] = None
        record["claims"][0]["paths"] = ["src/dir", "src/pinned.rs"]
        self.rewrite_record(record)
        status = self.run_claims(self.b, "status")
        self.assertEqual(status["off_board_fences"], [])
        self.assertEqual(status["off_board_paths"],
                         [{"ticket": claimed["ticket"], "item": "ALPHA-ITEM",
                           "owner": "A", "paths": ["src/dir"],
                           "expires_utc": status["claims"][0]["expires_utc"]}])

    def test_idempotent_retry_and_ticket_ownership(self):
        claimed = self.claim(self.a, owner="A")
        again = self.run_claims(self.a, "claim", "--item", "ALPHA-ITEM", "--owner", "A",
                                "--ticket", claimed["ticket"])
        self.assertTrue(again["idempotent"])
        self.run_claims(self.a, "claim", "--item", "ALPHA-ITEM", "--owner", "different",
                        "--ticket", claimed["ticket"], expected=1)
        self.assertEqual(len(self.run_claims(self.a, "status")["claims"]), 1)

    def test_renew_extends_lease_and_unknown_ticket_fails(self):
        claimed = self.claim(self.a, "ALPHA-ITEM", "A", "--lease-minutes", "15")
        renewed = self.run_claims(self.a, "renew", "--ticket", claimed["ticket"],
                                  "--lease-minutes", "60")
        self.assertGreater(datetime.fromisoformat(renewed["expires_utc"]),
                           datetime.fromisoformat(claimed["expires_utc"]))
        self.run_claims(self.b, "renew", "--ticket", "0" * 32, expected=1)
        self.run_claims(self.b, "release", "--ticket", "0" * 32, expected=1)

    def test_expired_claim_stays_unresolved_but_ordinary_claim_is_advisory(self):
        claimed = self.claim(self.a, owner="A")
        record = self.remote_record()
        record["claims"][0]["expires_utc"] = (
            datetime.now(timezone.utc) - timedelta(minutes=1)).isoformat()
        self.rewrite_record(record)
        status = self.run_claims(self.b, "status")
        self.assertEqual(status["state"], "unresolved")
        self.assertEqual(status["expired"][0]["ticket"], claimed["ticket"])
        reclaimed = self.claim(self.b, owner="B")
        self.assertNotEqual(reclaimed["ticket"], claimed["ticket"])
        record = self.remote_record()
        self.assertEqual([entry["ticket"] for entry in record["claims"]],
                         [claimed["ticket"], reclaimed["ticket"]])
        self.run_claims(self.a, "release", "--ticket", claimed["ticket"])

    def test_recover_requires_reason_and_removes_other_owner(self):
        claimed = self.claim(self.a, owner="A")
        self.run_claims(self.b, "recover", "--ticket", claimed["ticket"],
                        "--reason", " ", expected=1)
        self.assertEqual(self.run_claims(self.b, "recover", "--ticket", claimed["ticket"],
                                         "--reason", "A confirmed stopped")["state"],
                         "recovered")
        self.assertEqual(self.run_claims(self.a, "status")["state"], "available")
        self.run_claims(self.a, "claim", "--item", "ALPHA-ITEM", "--owner", "A",
                        "--ticket", claimed["ticket"], expected=1)

    def test_concurrent_automatic_reservations_exclude_same_outcome(self):
        processes = [self.start(directory, "reserve", "--item", "ALPHA-ITEM", "--owner", owner)
                     for directory, owner in [(self.a, "A"), (self.b, "B")]]
        results = [self.finish(process) for process in processes]
        self.assertEqual(sorted(code for code, _, _ in results), [0, 2])
        self.assertEqual(len(self.remote_record()["claims"]), 1)

    def test_existing_ticket_cannot_bypass_peer_ownership_for_automatic_reserve(self):
        first = self.claim(self.a, owner="A")
        self.claim(self.b, owner="B")
        self.run_claims(self.a, "reserve", "--ticket", first["ticket"], "--owner", "A",
                        "--board", "TASKS.md", "--item", "ALPHA-ITEM", expected=2)
        self.assertEqual(len(self.remote_record()["claims"]), 2)

    def test_session_namespaces_normalize_without_folding_opaque_handles(self):
        processes = [self.start(directory, "reserve", "--item", item, "--owner", item,
                                "--session-id", session)
                     for directory, item, session in [(self.a, "ALPHA-ITEM", "Codex:One"),
                                                       (self.b, "BETA-ITEM", "codex:One")]]
        self.assertEqual(sorted(self.finish(p)[0] for p in processes), [0, 2])
        self.run_claims(self.a, "reserve", "--item", "OTHER", "--owner", "C",
                        "--session-id", "codex:one")
        self.run_claims(self.a, "reserve", "--item", "UNKNOWN", "--owner", "D",
                        "--session-id", "unrecognized:One", expected=1)
        record = self.remote_record()
        record["claims"][0]["provenance"]["session_id"] = "CODEX:One"
        self.rewrite_record(record)
        self.run_claims(self.a, "reserve", "--item", "THIRD", "--owner", "E",
                        "--session-id", "codex:One", expected=2)

    def test_distinct_outcomes_can_share_paths_but_not_session(self):
        first = self.run_claims(self.a, "reserve", "--item", "ALPHA-ITEM", "--owner", "A",
                                "--path", "src/shared", "--session-id", "codex:one")
        self.run_claims(self.b, "reserve", "--item", "BETA-ITEM", "--owner", "B",
                        "--path", "src/shared", "--session-id", "codex:one", expected=2)
        self.run_claims(self.b, "reserve", "--item", "BETA-ITEM", "--owner", "B",
                        "--path", "src/shared", "--session-id", "cursor:two")
        self.assertEqual(len(self.remote_record()["claims"]), 2)
        self.run_claims(self.a, "reserve", "--ticket", first["ticket"], "--item", "OTHER",
                        "--owner", "A", expected=1)

    def test_automatic_reservation_blocks_expired_live_or_legacy_alias_claims(self):
        held = self.run_claims(self.a, "claim", "--item", "samples/cli/facts/example/local/main.omg",
                               "--owner", "Zac / Devin")
        record = self.remote_record()
        record["claims"][0]["expires_utc"] = (datetime.now(timezone.utc) - timedelta(days=2)).isoformat()
        self.rewrite_record(record)
        self.claim(self.b, item="BETA-ITEM")  # Unrelated writes must not erase stale ownership.
        self.run_claims(self.b, "reserve", "--item", "samples/cli/facts/example", "--owner", "automatic", expected=2)
        self.assertEqual(len(self.remote_record()["claims"]), 2)
        self.run_claims(self.a, "renew", "--ticket", held["ticket"])
        self.run_claims(self.b, "reserve", "--item", "samples/cli/facts/example", "--owner", "automatic", expected=2)
        self.run_claims(self.a, "recover", "--ticket", held["ticket"],
                        "--reason", "Owner confirmed stopped; preserved branch and session checkpoint")
        self.run_claims(self.b, "reserve", "--item", "samples/cli/facts/example", "--owner", "automatic")

    def test_concurrent_claims_retain_both_arrivals(self):
        for round_number in range(3):
            with self.subTest(round=round_number):
                a = self.start(self.a, "claim", "--item", "ALPHA-ITEM", "--board",
                               "TASKS.md", "--owner", "A", "--path", "src/shared")
                b = self.start(self.b, "claim", "--item", "BETA-ITEM", "--board",
                               "TASKS.md", "--owner", "B", "--path", "src/shared")
                results = [self.finish(a), self.finish(b)]
                self.assertEqual([row[0] for row in results], [0, 0], results)
                tickets = [entry["ticket"] for entry in self.run_claims(self.a, "status")["claims"]]
                self.assertEqual(len(set(tickets)), 2)
                for ticket in tickets:
                    self.run_claims(self.a, "release", "--ticket", ticket)

    def test_concurrent_claims_on_one_item_both_register(self):
        first = self.start(self.a, "claim", "--item", "ALPHA-ITEM", "--owner", "A")
        second = self.start(self.b, "claim", "--item", "ALPHA-ITEM", "--owner", "B",
                            "--allow-overlap")
        results = [self.finish(first), self.finish(second)]
        self.assertEqual(sorted(row[0] for row in results), [0, 0], results)
        self.assertEqual(len(self.run_claims(self.a, "status")["claims"]), 2)

    def test_audit_reports_advisory_overlaps_without_rejecting(self):
        self.claim(self.a, "ALPHA-ITEM", "A", "--path", "src/x")
        self.claim(self.b, "BETA-ITEM", "B", "--path", "src/y")
        for relative in ("src/x/mine.txt", "src/y/theirs.txt", "src/z/free.txt"):
            path = self.a / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("changed\n", encoding="utf-8")
        report = self.run_claims(self.a, "audit", "--worktree", str(self.a),
                                 "--owner", "A")
        self.assertEqual(report["uncovered"],
                         ["src/y/theirs.txt", "src/z/free.txt"])
        self.assertEqual(report["violations"], [])
        self.assertEqual(report["overlaps"],
                         [{"path": "src/y/theirs.txt", "item": "BETA-ITEM",
                           "claimed_by": "B",
                           "ticket": report["overlaps"][0]["ticket"],
                           "expires_utc": report["overlaps"][0]["expires_utc"]}])
        clean = self.run_claims(self.a, "audit", "--worktree", str(self.b),
                                "--owner", "B")
        self.assertEqual((clean["changed"], clean["uncovered"],
                          clean["violations"]), ([], [], []))

    def test_available_lists_unclaimed_board_items(self):
        self.claim(self.a, item="ALPHA-ITEM", owner="A")
        available = self.run_claims(self.b, "available", "--board", "TASKS.md")
        self.assertEqual(available["unclaimed"], ["BETA-ITEM", "GAMMA-ITEM"])
        self.assertEqual(available["claimed"], ["ALPHA-ITEM"])

    def test_notes_attach_to_live_claim_and_survive_release(self):
        claimed = self.claim(self.a, owner="A")
        noted = self.run_claims(self.a, "note", "--ticket", claimed["ticket"],
                                "--text", "already resolved upstream; verified at abc123")
        self.assertEqual(noted["state"], "noted")
        self.assertEqual(noted["item"], "ALPHA-ITEM")
        notes = self.run_claims(self.b, "notes")
        self.assertEqual(notes["count"], 1)
        self.assertEqual(notes["notes"][0]["item"], "ALPHA-ITEM")
        self.assertEqual(notes["notes"][0]["owner"], "A")
        self.assertIsNone(notes["notes"][0]["swept_utc"])
        self.assertEqual(self.run_claims(self.b, "status")["notes_pending"], 1)
        self.run_claims(self.a, "release", "--ticket", claimed["ticket"])
        self.assertEqual(self.run_claims(self.b, "notes")["count"], 1)
        self.assertEqual(self.run_claims(self.b, "sweep")["swept"], 1)
        self.assertEqual(self.run_claims(self.a, "notes")["count"], 0)
        self.assertEqual(self.run_claims(self.a, "status")["notes_pending"], 0)
        swept = self.remote_record()["notes"]
        self.assertEqual(len(swept), 1)
        self.assertIsNotNone(swept[0]["swept_utc"])

    def test_note_requires_live_claim_and_bounded_text(self):
        self.run_claims(self.a, "note", "--ticket", "0" * 32, "--text", "x",
                        expected=1)
        claimed = self.claim(self.a, owner="A")
        self.run_claims(self.a, "note", "--ticket", claimed["ticket"],
                        "--text", "   ", expected=1)
        self.run_claims(self.a, "note", "--ticket", claimed["ticket"],
                        "--text", "x" * 2001, expected=1)

    def test_notes_filter_by_ticket_owner_and_swept(self):
        first = self.claim(self.a, owner="wave one")
        second = self.claim(self.b, "BETA-ITEM", "wave two")
        self.run_claims(self.a, "note", "--ticket", first["ticket"],
                        "--text", "finding one")
        self.run_claims(self.b, "note", "--ticket", second["ticket"],
                        "--text", "finding two")
        everything = self.run_claims(self.b, "notes")
        self.assertEqual(everything["count"], 2)
        by_prefix = self.run_claims(self.b, "notes", "--ticket",
                                    first["ticket"][:6])
        self.assertEqual([note["text"] for note in by_prefix["notes"]],
                         ["finding one"])
        self.assertEqual(by_prefix["notes"][0]["ticket"], first["ticket"])
        by_owner = self.run_claims(self.b, "notes", "--owner", "wave two")
        self.assertEqual([note["text"] for note in by_owner["notes"]],
                         ["finding two"])
        self.assertEqual(self.run_claims(self.b, "notes", "--ticket",
                                         first["ticket"], "--owner",
                                         "wave two")["count"], 0)
        self.run_claims(self.b, "sweep")
        self.assertEqual(self.run_claims(self.b, "notes")["count"], 0)
        swept = self.run_claims(self.b, "notes", "--all")
        self.assertEqual(swept["count"], 2)
        self.assertTrue(all(note["swept_utc"] for note in swept["notes"]))
        self.assertEqual(self.run_claims(
            self.b, "notes", "--all", "--ticket", second["ticket"])["count"],
            1)

    def test_ambiguous_ticket_prefix_is_refused(self):
        first, second = "abcd" + "1" * 28, "abcd" + "2" * 28
        self.claim(self.a, "ALPHA-ITEM", "A", "--ticket", first)
        self.claim(self.b, "BETA-ITEM", "B", "--ticket", second)
        self.run_claims(self.a, "note", "--ticket", first, "--text", "one")
        refused = self.run_claims(self.b, "notes", "--ticket", "abcd",
                                  expected=1)
        self.assertIn("matches 2 tickets", refused)
        # The second ticket has no notes, yet it still makes the prefix
        # ambiguous: a filter must not silently pick whichever inbox is
        # non-empty.
        self.run_claims(self.b, "notes", "--ticket", "abcd", "--owner", "A",
                        expected=1)
        exact = self.run_claims(self.b, "notes", "--ticket", "abcd1")
        self.assertEqual([note["text"] for note in exact["notes"]], ["one"])

    def test_notes_survive_record_rewrite_and_concurrent_claims(self):
        claimed = self.claim(self.a, owner="A")
        self.run_claims(self.a, "note", "--ticket", claimed["ticket"],
                        "--text", "finding one")
        self.claim(self.b, "BETA-ITEM", "B")
        notes = self.run_claims(self.b, "notes")
        self.assertEqual(notes["count"], 1)
        self.assertEqual(notes["notes"][0]["text"], "finding one")

    def test_transport_stall_bounds_installed_without_overriding(self):
        """Remote calls inherit bounded, non-interactive transports; an
        operator's own environment always wins."""
        module = load_claims()
        names = ("GIT_HTTP_LOW_SPEED_LIMIT", "GIT_HTTP_LOW_SPEED_TIME",
                 "GIT_SSH_COMMAND")
        saved = {name: os.environ.pop(name, None) for name in names}
        try:
            module.Claims(str(self.a), "origin")
            self.assertTrue(all(os.environ.get(name) for name in names))
            self.assertIn("BatchMode=yes", os.environ["GIT_SSH_COMMAND"])
            os.environ["GIT_HTTP_LOW_SPEED_TIME"] = "7"
            module.Claims(str(self.a), "origin")
            self.assertEqual(os.environ["GIT_HTTP_LOW_SPEED_TIME"], "7")
        finally:
            for name, value in saved.items():
                if value is None:
                    os.environ.pop(name, None)
                else:
                    os.environ[name] = value

    def test_unknown_record_format_is_not_replaced(self):
        tree = self.git(self.a, "mktree")
        identity = self.git(self.a, "commit-tree", tree, "-F", "-",
                            input_text=json.dumps({"protocol": "future-protocol"}))
        self.git(self.a, "push", "origin", identity + ":" + CLAIMS_REF)
        self.run_claims(self.b, "status", expected=1)
        self.claim(self.b, expected=1)
        self.assertEqual(self.git(self.a, "ls-remote", "--refs", str(self.remote),
                                  CLAIMS_REF).split("\t")[0], identity)


if __name__ == "__main__":
    unittest.main()
