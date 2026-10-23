#!/usr/bin/env python3
"""Local Git protocol tests; no GitHub access or non-standard Python packages."""

import json
import copy
from types import SimpleNamespace
from datetime import datetime, timedelta, timezone
import contextlib
import errno
import importlib.util
import io
import os
from pathlib import Path
import shlex
import stat
import subprocess
import sys
import tempfile
import unittest

import worktree_testkit as kit
from unittest import mock
import uuid


LANDING = Path(__file__).resolve().parents[1] / "landing.py"
CLAIM_REF = "refs/coordination/omega-landing/main"
MAIN_REF = "refs/heads/main"


def load_landing():
    return kit.load_module(LANDING, "landing_under_test")

class DeadlineTests(unittest.TestCase):
    def test_expiry_is_inclusive_at_exact_deadline(self):
        module = load_landing()
        deadline = datetime(2026, 9, 5, tzinfo=timezone.utc)
        record = {"entries": [{}], "head_expires_utc": deadline.isoformat()}

        class Clock(datetime):
            instant = deadline

            @classmethod
            def now(cls, timezone_value=None):
                return cls.instant

        with mock.patch.object(module, "datetime", Clock):
            Clock.instant = deadline - timedelta(microseconds=1)
            self.assertFalse(module.expired(record))
            Clock.instant = deadline
            self.assertTrue(module.expired(record))
            Clock.instant = deadline + timedelta(microseconds=1)
            self.assertTrue(module.expired(record))


class AcquisitionTests(unittest.TestCase):
    """Deterministic clocks and persisted records; never contact the live queue."""
    def setUp(self):
        self.module = load_landing()
        self.expiry = datetime(2026, 10, 12, tzinfo=timezone.utc)
        self.instant = self.expiry - timedelta(seconds=20)
        case = self
        class Clock(datetime):
            @classmethod
            def now(cls, timezone_value=None):
                return case.instant
        for patcher in (mock.patch.object(self.module, "datetime", Clock),
                        mock.patch.object(self.module.time, "monotonic", side_effect=lambda: self.instant.timestamp())):
            patcher.start()
            self.addCleanup(patcher.stop)
        self.ticket, self.base = "a" * 32, "b" * 40
        self.persisted = {"version": "c" * 40, "main": self.base, "record": {
            "protocol": "omega-landing-v3", "entries": [{"ticket": self.ticket, "owner": "test"}],
            "active": {"ticket": self.ticket, "base": self.base},
            "head_promoted_utc": (self.expiry - timedelta(seconds=180)).isoformat(),
            "head_expires_utc": self.expiry.isoformat()}}
        self.client = object.__new__(self.module.Landing)
        self.client.snapshot = mock.Mock(side_effect=lambda: copy.deepcopy(self.persisted))
        self.client.clean_head = mock.Mock()
        self.client.fetch_version = mock.Mock()
        self.client.queue_object = mock.Mock(return_value="d" * 40)
        self.client.publish = mock.Mock(return_value=True)
        self.options = SimpleNamespace(command="claim", ticket=self.ticket, wait_seconds=0)
        self.responses = []
        emitter = mock.patch.object(self.module, "emit", side_effect=self.responses.append)
        emitter.start()
        self.addCleanup(emitter.stop)

    def claim(self):
        return self.client.coordinate(self.options)

    def expire(self, *arguments):
        self.instant = self.expiry

    def test_existing_fetch_and_final_observation_cannot_cross_expiry(self):
        for boundary in ("fetch", "observation"):
            with self.subTest(boundary=boundary):
                self.instant = self.expiry - timedelta(seconds=20)
                self.client.deadline = None
                self.responses.clear()
                self.client.fetch_version.side_effect = self.expire if boundary == "fetch" else None
                def observe():
                    if boundary == "observation" and self.client.fetch_version.called:
                        self.expire()
                    return copy.deepcopy(self.persisted)
                self.client.fetch_version.reset_mock()
                self.client.snapshot.side_effect = observe
                with self.assertRaises(self.module.LandingError):
                    self.claim()
                self.assertEqual(self.responses, [])

    def test_new_claim_expiring_during_preparation_does_not_push(self):
        self.persisted["record"]["active"] = None
        self.client.clean_head.side_effect = self.expire
        with self.assertRaises(self.module.LandingError):
            self.claim()
        self.client.publish.assert_not_called()
        self.client.queue_object.assert_not_called()
        self.assertEqual(self.responses, [])

    def test_push_crossing_expiry_cannot_report_new_reservation(self):
        self.persisted["record"]["active"] = None
        def publish(*arguments):
            self.persisted["record"]["active"] = {"ticket": self.ticket, "base": self.base}
            self.expire()
            return True
        self.client.publish.side_effect = publish
        with self.assertRaises(self.module.LandingError):
            self.claim()
        self.assertEqual(self.responses, [])

    def test_changed_persisted_base_is_refused(self):
        self.client.fetch_version.side_effect = lambda *arguments: self.persisted["record"]["active"].update(base="e" * 40)
        with self.assertRaises(self.module.LandingError):
            self.claim()
        self.assertEqual(self.responses, [])

    def test_unchanged_head_accepts_tail_arrival_and_preserves_expiry(self):
        def append(*arguments):
            self.persisted["record"]["entries"].append({"ticket": "f" * 32, "owner": "tail"})
            self.persisted["version"] = "e" * 40
        self.client.fetch_version.side_effect = append
        self.assertEqual(self.claim(), 0)
        self.assertEqual(self.responses[0]["base"], self.base)
        self.assertEqual(self.responses[0]["expires_utc"], self.expiry.isoformat())
        self.client.publish.assert_not_called()

    def test_setup_receives_budget_before_remote_discovery(self):
        with mock.patch.object(self.module.coordination, "push_url", return_value="remote") as discover:
            self.module.Landing("repository", "origin", deadline=123)
        self.assertEqual(discover.call_args.kwargs["deadline"], 123)

    def test_timed_out_fetch_is_not_retried_by_ref(self):
        client = object.__new__(self.module.Landing)
        client.repository, client.push_url, client.deadline = Path("repository"), "remote", 123
        client.git = mock.Mock(return_value=subprocess.CompletedProcess([], 1))
        with mock.patch.object(self.module.coordination, "fetch_object", side_effect=self.module.coordination.CoordinationTimeout("expired")) as fetch:
            with self.assertRaises(self.module.coordination.CoordinationTimeout):
                client.fetch_version(self.base)
        self.assertEqual(client.git.call_count, 1)
        self.assertEqual(fetch.call_args.kwargs["deadline"], 123)

    def test_confirmed_publication_starts_separate_notes_budget(self):
        self.options.command = "publish"
        self.options.claim = self.ticket
        self.options.base = self.base
        self.options.candidate = "e" * 40
        self.options.provenance = "model=test"
        self.client.publication_check = mock.Mock()
        def push(*arguments):
            self.expire()
            return True
        self.client.publish.side_effect = push
        def notes(*arguments):
            self.assertEqual(self.client.deadline, self.instant.timestamp() + self.module.LEASE_SECONDS)
            return "not recorded: notes unavailable"
        self.client.record_provenance = mock.Mock(side_effect=notes)
        self.assertEqual(self.client.coordinate(self.options), 0)
        self.assertEqual(self.responses[0]["state"], "published")
        self.assertTrue(self.responses[0]["provenance"].startswith("not recorded:"))

    def test_provenance_deadline_cannot_turn_landed_main_into_failure(self):
        client = object.__new__(self.module.Landing)
        client.repository, client.push_url = Path("repository"), "remote"
        client.deadline = self.instant.timestamp() - 1
        with mock.patch.object(self.module.coordination, "remote_refs", return_value={}):
            result = client.record_provenance(self.base, ["owner: test"])
        self.assertTrue(result.startswith("not recorded:"))

    def test_push_timeout_stays_uncertain_and_is_not_retried(self):
        client = object.__new__(self.module.Landing)
        client.repository, client.push_url, client.deadline = Path("repository"), "remote", 123
        with mock.patch.object(self.module.coordination, "push_atomic", side_effect=self.module.coordination.CoordinationTimeout("remote mutation may have landed")) as push:
            with self.assertRaisesRegex(self.module.coordination.CoordinationTimeout, "may have landed"):
                client.publish("c" * 40, "d" * 40)
        self.assertEqual(push.call_count, 1)
        self.assertEqual(push.call_args.kwargs["deadline"], 123)


class FetchCacheTests(unittest.TestCase):
    def test_existing_tip_is_not_refetched_and_remains_reachable(self):
        module = load_landing()
        landing = object.__new__(module.Landing)
        landing.repository = Path("repository")
        landing.push_url = "remote"
        landing.git = mock.Mock(return_value=subprocess.CompletedProcess([], 0))
        identity = "a" * 40
        with mock.patch.object(module.coordination, "fetch_object") as fetch, \
                mock.patch.object(module.coordination, "remember_object") as remember:
            landing.fetch_version(identity)
        fetch.assert_not_called()
        landing.git.assert_called_once_with("cat-file", "-e", identity, allow_failure=True)
        remember.assert_called_once_with(landing.repository, "remote", module.CLAIM_REF, identity, deadline=None)


class LandingTests(unittest.TestCase):
    def setUp(self):
        (self.root, self.remote, self.a,
         self.b) = kit.seed_remote_pair(
            self, "omega landing test ", "Landing Test")
        self.initial = self.commit(self.a, "initial")
        kit.clone_remote_pair(
            self, self.remote, self.a, self.b, "Landing Test")

    def git(self, directory, *arguments, input_text=""):
        return kit.git_input(self, directory, *arguments, input_text=input_text)

    def commit(self, directory, message):
        counter = getattr(self, "_commits", 0) + 1
        self._commits = counter
        marker = directory / f"work-{counter}.txt"
        marker.write_text(message + "\n", encoding="utf-8")
        self.git(directory, "add", marker.name)
        self.git(directory, "commit", "-m", message)
        return self.git(directory, "rev-parse", "HEAD")

    def start(self, directory, *arguments):
        return kit.start_tool(self, LANDING, directory, *arguments)

    def finish(self, process):
        return kit.finish_tool(self, process, "landing")

    def run_landing(self, directory, *arguments, expected=0):
        code, output, error = self.finish(self.start(directory, *arguments))
        self.assertEqual(code, expected, error + output)
        return json.loads(output) if output else error

    def claim(self, directory, owner="writer"):
        ticket = self.enqueue(directory, owner)
        return self.run_landing(directory, "claim", "--ticket", ticket)

    def enqueue(self, directory, owner="writer"):
        return self.run_landing(directory, "enqueue", "--owner", owner)["ticket"]

    def state(self):
        return self.run_landing(self.a, "status")

    def publish(self, directory, claim, candidate, expected=0):
        return self.run_landing(directory, "publish", "--claim", claim["claim"],
                                "--base", claim["base"], "--candidate", candidate,
                                expected=expected)

    def assert_remote(self, main, claim=None, remote=None):
        remote = remote or self.remote
        self.assertEqual(self.git(remote, "rev-parse", MAIN_REF), main)
        observed = self.git(self.a, "ls-remote", "--refs", str(remote), CLAIM_REF)
        active = None
        if observed:
            record = json.loads(self.git(remote, "show", "-s", "--format=%B", CLAIM_REF))
            active = (record["active"]["ticket"] if record.get("active") else None) if record["protocol"] in ("omega-landing-v2", "omega-landing-v3") else observed.split("\t")[0]
        self.assertEqual(active, claim)

    def hook(self, directory, name, body):
        hooks = directory / "hooks"
        hooks.mkdir(exist_ok=True)
        path = hooks / name
        path.write_bytes(("#!/bin/sh\n" + body + "\n").encode("utf-8"))
        path.chmod(path.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
        return path

    def test_claim_requires_clean_checkpoint_and_identifiable_owner(self):
        self.assertEqual(self.run_landing(self.a, "status")["state"], "available")
        self.run_landing(self.a, "enqueue", expected=1)
        self.run_landing(self.a, "claim", "--owner", "no ticket", expected=1)
        unfinished = self.a / "unfinished.txt"
        unfinished.write_text("unfinished", encoding="utf-8")
        self.run_landing(self.a, "enqueue", "--owner", "A", expected=1)
        unfinished.unlink()
        self.assert_remote(self.initial)

    def test_owner_round_trip_and_busy_status(self):
        claim = self.claim(self.a, "A / caf\u00e9")
        self.assertEqual(claim["base"], self.initial)
        ticket = self.enqueue(self.b, "B")
        busy = self.run_landing(self.b, "claim", "--ticket", ticket, expected=2)
        self.assertEqual(busy["active"]["ticket"], claim["claim"])
        self.assertEqual(busy["queue"][0]["owner"], "A / caf\u00e9")
        self.run_landing(self.b, "release", "--claim", self.initial, expected=1)
        self.assert_remote(self.initial, claim["claim"])

    def test_publish_requires_changed_clean_exact_head(self):
        claim = self.claim(self.a)
        self.publish(self.a, claim, self.initial, expected=1)
        candidate = self.commit(self.a, "candidate")
        self.publish(self.a, claim, self.initial, expected=1)
        unfinished = self.a / "unfinished.txt"
        unfinished.write_text("unfinished", encoding="utf-8")
        self.publish(self.a, claim, candidate, expected=1)
        unfinished.unlink()
        self.assert_remote(self.initial, claim["claim"])
        self.assertEqual(self.publish(self.a, claim, candidate)["state"], "published")
        self.assert_remote(candidate)

    def test_publish_refuses_empty_candidate(self):
        claim = self.claim(self.a)
        self.git(self.a, "commit", "--allow-empty", "-m", "ledger entry")
        empty = self.git(self.a, "rev-parse", "HEAD")
        self.publish(self.a, claim, empty, expected=1)
        self.assert_remote(self.initial, claim["claim"])

    def test_publish_refuses_board_only_candidate_without_flag(self):
        claim = self.claim(self.a)
        (self.a / "TASKS.md").write_text("- **ITEM.** row\n", encoding="utf-8")
        self.git(self.a, "add", "TASKS.md")
        self.git(self.a, "commit", "-m", "board sweep")
        board = self.git(self.a, "rev-parse", "HEAD")
        self.publish(self.a, claim, board, expected=1)
        self.assert_remote(self.initial, claim["claim"])
        published = self.run_landing(self.a, "publish", "--claim", claim["claim"],
                                     "--base", claim["base"], "--candidate", board,
                                     "--board-update")
        self.assertEqual(published["state"], "published")
        self.assert_remote(board)

    def test_board_update_flag_is_publish_only(self):
        self.run_landing(self.a, "enqueue", "--owner", "A", "--board-update",
                         expected=1)

    def test_recovery_fences_former_owner_and_requires_reason(self):
        claim = self.claim(self.a)
        candidate = self.commit(self.a, "candidate")
        self.run_landing(self.a, "recover", "--claim", claim["claim"], expected=1)
        self.run_landing(self.b, "recover", "--claim", claim["claim"],
                         "--reason", "A confirmed stopped in this test")
        replacement = self.claim(self.b)
        self.publish(self.a, claim, candidate, expected=1)
        self.run_landing(self.a, "release", "--claim", claim["claim"], expected=1)
        self.assert_remote(self.initial, replacement["claim"])

    def test_server_rejection_rolls_back_main_and_release_together(self):
        claim = self.claim(self.a)
        candidate = self.commit(self.a, "candidate")
        hook = self.hook(self.remote, "pre-receive",
                         'while read old new ref; do if [ "$ref" = refs/heads/main ]; then exit 1; fi; done')
        self.publish(self.a, claim, candidate, expected=1)
        self.assert_remote(self.initial, claim["claim"])
        hook.unlink()
        self.publish(self.a, claim, candidate)
        self.assert_remote(candidate)

    def test_divergent_and_merge_commits_cannot_publish(self):
        first = self.claim(self.a)
        candidate_a = self.commit(self.a, "A")
        self.publish(self.a, first, candidate_a)
        candidate_b = self.commit(self.b, "B")
        claim = self.claim(self.b)
        self.publish(self.b, claim, candidate_b, expected=1)
        self.git(self.b, "rebase", candidate_a)
        candidate_b = self.git(self.b, "rev-parse", "HEAD")
        side = self.git(self.b, "commit-tree", "HEAD^{tree}", "-p", candidate_a, "-m", "side")
        merge = self.git(self.b, "commit-tree", "HEAD^{tree}", "-p", candidate_b, "-p", side, "-m", "merge")
        self.git(self.b, "switch", "--detach", merge)
        self.publish(self.b, claim, merge, expected=1)
        self.assert_remote(candidate_a, claim["claim"])

    def test_replacement_during_publication_fences_stale_owner_atomically(self):
        claim = self.claim(self.a)
        candidate = self.commit(self.a, "candidate")
        version = self.state()["version"]
        record = json.loads(self.git(self.remote, "show", "-s", "--format=%B", version))
        record.update(entries=[], active=None)
        self.replace_on_push(record, version)
        self.publish(self.a, claim, candidate, expected=1)
        self.assert_remote(self.initial)

    def replace_on_push(self, record, version):
        if not record["entries"]:
            record.update(head_promoted_utc=None, head_expires_utc=None)
        replacement = self.git(self.remote, "commit-tree", version + "^{tree}", "-p", version,
                               "-F", "-", input_text=json.dumps(record))
        hooks = self.root / "race"
        hooks.mkdir()
        marker = shlex.quote((self.root / "race-fired").as_posix())
        self.hook(hooks, "pre-push", f"if [ ! -f {marker} ]; then touch {marker}; git --git-dir=" +
                  shlex.quote(self.remote.as_posix()) + f" update-ref {CLAIM_REF} {replacement} {version} || exit 1; fi")
        self.git(self.a, "config", "core.hooksPath", str(hooks / "hooks"))

    def test_concurrent_enqueues_retain_both_arrivals_and_only_head_can_claim(self):
        for round_number in range(3):
            with self.subTest(round=round_number):
                a = self.start(self.a, "enqueue", "--owner", "A")
                b = self.start(self.b, "enqueue", "--owner", "B")
                results = [self.finish(a), self.finish(b)]
                self.assertEqual([row[0] for row in results], [0, 0], results)
                tickets = [entry["ticket"] for entry in self.state()["queue"]]
                self.assertEqual(len(set(tickets)), 2)
                self.run_landing(self.b, "claim", "--ticket", tickets[1], expected=2)
                for ticket in tickets:
                    self.run_landing(self.a, "claim", "--ticket", ticket)
                    self.assert_remote(self.initial, ticket)
                    self.run_landing(self.a, "release", "--claim", ticket)

    def test_uncoordinated_main_update_is_not_overwritten(self):
        claim = self.claim(self.a)
        candidate = self.commit(self.a, "A")
        outsider = self.commit(self.b, "B bypasses protocol")
        self.git(self.b, "push", "origin", "HEAD:" + MAIN_REF)
        self.publish(self.a, claim, candidate, expected=1)
        self.assert_remote(outsider, claim["claim"])

    def test_observation_uses_push_url_and_multiple_destinations_reject(self):
        alternate = self.root / "alternate.git"
        self.git(self.root, "clone", "--bare", str(self.remote), str(alternate))
        fetch_only = self.commit(self.b, "fetch destination moves")
        self.git(self.b, "push", "origin", "HEAD:" + MAIN_REF)
        self.git(self.a, "config", "remote.origin.pushurl", str(alternate))
        claim = self.claim(self.a)
        self.assertEqual(claim["base"], self.initial)
        self.assert_remote(fetch_only)
        self.assert_remote(self.initial, claim["claim"], alternate)
        self.assertEqual(self.run_landing(self.a, "status")["active"]["ticket"], claim["claim"])
        self.run_landing(self.a, "release", "--claim", claim["claim"])
        self.git(self.a, "config", "--add", "remote.origin.pushurl", str(self.remote))
        self.run_landing(self.a, "enqueue", "--owner", "A", expected=1)
        self.assert_remote(self.initial, remote=alternate)
        self.assert_remote(fetch_only)

    def test_existing_v1_record_is_readable_and_unknown_format_is_not_replaced(self):
        tree = self.git(self.a, "mktree")
        # The timestamp and JSON fields match the retired helper's wire shape.
        record = {"protocol": "omega-landing-v1", "owner": "prior writer",
                  "nonce": "0" * 32, "created_utc": "2026-09-05T00:00:00.0000000+00:00"}
        identity = self.git(self.a, "commit-tree", tree, "-F", "-", input_text=json.dumps(record))
        self.git(self.a, "push", "origin", identity + ":" + CLAIM_REF)
        self.assertEqual(self.run_landing(self.b, "status")["owner"], "prior writer")
        self.assertEqual(self.state()["state"], "legacy_reserved")
        self.run_landing(self.a, "release", "--claim", identity)
        record["protocol"] = "future-protocol"
        identity = self.git(self.a, "commit-tree", tree, "-F", "-", input_text=json.dumps(record))
        self.git(self.a, "push", "origin", identity + ":" + CLAIM_REF)
        self.run_landing(self.b, "status", expected=1)
        self.run_landing(self.b, "enqueue", "--owner", "B", expected=1)
        self.assert_remote(self.initial, identity)

    def test_fifo_wait_cancel_idempotence_and_retired_tickets(self):
        a, b, c = self.enqueue(self.a, "A"), self.enqueue(self.b, "B"), self.enqueue(self.b, "C")
        self.assertEqual([entry["ticket"] for entry in self.state()["queue"]], [a, b, c])
        self.run_landing(self.a, "enqueue", "--owner", "A", "--ticket", a)
        self.run_landing(self.a, "enqueue", "--owner", "different", "--ticket", a, expected=1)
        self.run_landing(self.b, "claim", "--ticket", c, "--wait-seconds", "1", "--poll-seconds", "1", expected=2)
        self.assertEqual(len(self.state()["queue"]), 3)
        claim = self.run_landing(self.a, "claim", "--ticket", a)
        self.assertEqual(self.run_landing(self.a, "claim", "--ticket", a), claim)
        self.run_landing(self.a, "cancel", "--ticket", a, expected=1)
        self.run_landing(self.b, "cancel", "--ticket", c)
        candidate = self.commit(self.a, "A published")
        waiting = self.start(self.b, "claim", "--ticket", b, "--wait-seconds", "20", "--poll-seconds", "1")
        self.publish(self.a, claim, candidate)
        code, output, error = self.finish(waiting)
        self.assertEqual(code, 0, error)
        self.assertEqual(json.loads(output)["base"], candidate)
        self.assert_remote(candidate, b)
        self.run_landing(self.a, "enqueue", "--owner", "A", "--ticket", a, expected=1)
        self.run_landing(self.a, "enqueue", "--owner", "C", "--ticket", c, expected=1)
        self.run_landing(self.b, "release", "--claim", b)
        self.assertEqual(self.state()["state"], "available")
        self.assertTrue(self.state()["version"])

    def test_enqueue_during_publication_preserves_arrival_without_repeating_build(self):
        claim = self.claim(self.a)
        candidate = self.commit(self.a, "candidate")
        version = self.state()["version"]
        record = json.loads(self.git(self.remote, "show", "-s", "--format=%B", version))
        arriving = uuid.uuid4().hex
        record["entries"].append({"ticket": arriving, "owner": "arriving", "created_utc": "2026-09-05T00:00:00Z"})
        self.replace_on_push(record, version)
        self.publish(self.a, claim, candidate)
        self.assert_remote(candidate)
        self.assertEqual([entry["ticket"] for entry in self.state()["queue"]], [arriving])

    def test_cancelling_waiting_head_advances_next_ticket(self):
        a, b = self.enqueue(self.a), self.enqueue(self.b)
        self.run_landing(self.b, "claim", "--ticket", b, expected=2)
        self.run_landing(self.a, "cancel", "--ticket", a, "--reason", "owner stopped")
        self.run_landing(self.b, "claim", "--ticket", b)
        self.assert_remote(self.initial, b)

    def rewrite_queue(self, record):
        previous = self.state()["version"]
        identity = self.git(self.remote, "commit-tree", previous + "^{tree}", "-p", previous,
                            "-F", "-", input_text=json.dumps(record))
        self.git(self.remote, "update-ref", CLAIM_REF, identity, previous)
        return identity

    def expire_head(self):
        record = json.loads(self.git(self.remote, "show", "-s", "--format=%B", CLAIM_REF))
        started = datetime.now(timezone.utc) - timedelta(seconds=181)
        record.update(head_promoted_utc=started.isoformat(),
                      head_expires_utc=(started + timedelta(seconds=180)).isoformat())
        self.rewrite_queue(record)

    def test_promotion_starts_three_minutes_and_claim_does_not_renew(self):
        a = self.enqueue(self.a, "A")
        initial = self.state()
        started = datetime.fromisoformat(initial["head_promoted_utc"])
        expires = datetime.fromisoformat(initial["head_expires_utc"])
        self.assertEqual(expires - started, timedelta(seconds=180))
        claim = self.run_landing(self.a, "claim", "--ticket", a)
        self.assertEqual(claim["expires_utc"], initial["head_expires_utc"])
        b = self.enqueue(self.b, "B")
        self.run_landing(self.a, "claim", "--ticket", a)
        self.run_landing(self.a, "enqueue", "--ticket", a, "--owner", "A")
        self.assertEqual(self.state()["head_expires_utc"], initial["head_expires_utc"])
        self.run_landing(self.a, "release", "--claim", a)
        promoted = self.state()
        self.assertEqual(promoted["queue"][0]["ticket"], b)
        self.assertGreater(datetime.fromisoformat(promoted["head_promoted_utc"]), started)
        self.assertEqual(datetime.fromisoformat(promoted["head_expires_utc"]) -
                         datetime.fromisoformat(promoted["head_promoted_utc"]), timedelta(seconds=180))

    def test_expired_active_owner_is_fenced_and_next_ticket_gets_full_lease(self):
        claim = self.claim(self.a, "A")
        b = self.enqueue(self.b, "B")
        candidate = self.commit(self.a, "too late")
        self.expire_head()
        self.assertEqual(self.state()["state"], "expired")
        next_claim = self.run_landing(self.b, "claim", "--ticket", b)
        self.assertGreater(datetime.fromisoformat(next_claim["expires_utc"]), datetime.now(timezone.utc))
        self.publish(self.a, claim, candidate, expected=1)
        self.run_landing(self.a, "release", "--claim", claim["claim"], expected=1)
        self.run_landing(self.a, "enqueue", "--owner", "A", "--ticket", claim["claim"], expected=1)
        self.assert_remote(self.initial, b)

    def test_absent_unclaimed_head_expires_and_does_not_block_next_writer(self):
        self.enqueue(self.a, "absent head")
        b = self.enqueue(self.b, "B")
        self.expire_head()
        self.run_landing(self.b, "claim", "--ticket", b)
        self.assert_remote(self.initial, b)
        self.assertEqual(len(self.state()["queue"]), 1)

    def test_expired_owner_cannot_publish_even_without_a_competitor(self):
        claim = self.claim(self.a)
        candidate = self.commit(self.a, "late")
        self.expire_head()
        self.publish(self.a, claim, candidate, expected=1)
        self.assert_remote(self.initial)
        self.assertEqual(self.state()["queue"], [])

    def test_longer_than_three_minute_record_rejects(self):
        self.enqueue(self.a)
        record = json.loads(self.git(self.remote, "show", "-s", "--format=%B", CLAIM_REF))
        started = datetime.fromisoformat(record["head_promoted_utc"])
        record["head_expires_utc"] = (started + timedelta(seconds=181)).isoformat()
        identity = self.rewrite_queue(record)
        self.run_landing(self.a, "status", expected=1)
        self.assertEqual(self.git(self.remote, "rev-parse", CLAIM_REF), identity)

    def test_v2_upgrade_preserves_order_and_bounds_existing_head(self):
        a, b = uuid.uuid4().hex, uuid.uuid4().hex
        record = {"protocol": "omega-landing-v2", "entries": [
            {"ticket": a, "owner": "A", "created_utc": "2026-09-05T00:00:00Z"},
            {"ticket": b, "owner": "B", "created_utc": "2026-09-05T00:00:00Z"}],
            "active": {"ticket": a, "base": self.initial}}
        tree = self.git(self.a, "mktree")
        identity = self.git(self.a, "commit-tree", tree, "-F", "-", input_text=json.dumps(record))
        self.git(self.a, "push", "origin", identity + ":" + CLAIM_REF)
        self.run_landing(self.b, "claim", "--ticket", b, expected=2)
        state = self.state()
        self.assertEqual([entry["ticket"] for entry in state["queue"]], [a, b])
        self.assertEqual(state["active"]["ticket"], a)
        self.assertEqual(datetime.fromisoformat(state["head_expires_utc"]) -
                         datetime.fromisoformat(state["head_promoted_utc"]), timedelta(seconds=180))

    def test_v1_holder_is_migrated_to_a_timed_head_not_left_indefinite(self):
        tree = self.git(self.a, "mktree")
        identity = self.git(self.a, "commit-tree", tree, "-F", "-", input_text=json.dumps(
            {"protocol": "omega-landing-v1", "owner": "legacy owner"}))
        self.git(self.a, "push", "origin", identity + ":" + CLAIM_REF)
        b = self.enqueue(self.b, "B")
        state = self.state()
        self.assertEqual(state["queue"][0]["legacy_claim"], identity)
        self.assertEqual(state["queue"][0]["owner"], "legacy owner")
        self.assertEqual(state["queue"][1]["ticket"], b)
        self.assertEqual(datetime.fromisoformat(state["head_expires_utc"]) -
                         datetime.fromisoformat(state["head_promoted_utc"]), timedelta(seconds=180))
        self.expire_head()
        self.run_landing(self.b, "claim", "--ticket", b)
        self.assert_remote(self.initial, b)

    def test_publication_rechecks_deadline_after_preparing_git_update(self):
        module = load_landing()
        claim = self.claim(self.a)
        candidate = self.commit(self.a, "candidate")
        deadline = datetime.fromisoformat(claim["expires_utc"])
        original = module.Landing.queue_object

        class Clock(datetime):
            instant = datetime.now(timezone.utc)

            @classmethod
            def now(cls, timezone_value=None):
                return cls.instant

        def prepare_then_expire(instance, record, parent):
            result = original(instance, record, parent)
            Clock.instant = deadline
            return result

        with mock.patch.object(module, "datetime", Clock), \
                mock.patch.object(module.Landing, "queue_object", prepare_then_expire), \
                contextlib.redirect_stderr(io.StringIO()) as errors:
            result = module.main(["--repository", str(self.a), "publish", "--claim", claim["claim"],
                                  "--base", claim["base"], "--candidate", candidate])
        self.assertEqual(result, 1)
        self.assertIn("expired before push", errors.getvalue())
        self.assert_remote(self.initial, claim["claim"])

    def test_publish_records_owner_and_provenance_as_a_note(self):
        first = self.claim(self.a, owner="Jarod / swarm")
        first_candidate = self.commit(self.a, "first")
        published = self.run_landing(self.a, "publish", "--claim", first["claim"], "--base", first["base"],
                                     "--candidate", first_candidate,
                                     "--provenance", "agent=devin model=swe-2-medium")
        self.assertEqual(published["provenance"], "recorded")
        # A second lander on another machine adds its note beside the first.
        self.git(self.b, "pull", "--ff-only", "origin", "main")
        second = self.claim(self.b, owner="Zac / swarm")
        second_candidate = self.commit(self.b, "second")
        self.assertEqual(self.publish(self.b, second, second_candidate)["provenance"], "recorded")
        self.assert_remote(second_candidate)
        first_note = self.git(self.remote, "notes", "--ref", "provenance", "show", first_candidate)
        self.assertIn("owner: Jarod / swarm", first_note)
        self.assertIn("provenance: agent=devin model=swe-2-medium", first_note)
        second_note = self.git(self.remote, "notes", "--ref", "provenance", "show", second_candidate)
        self.assertIn("owner: Zac / swarm", second_note)
        self.assertNotIn("provenance:", second_note)
        # The commit message itself stays free of tool attribution.
        self.assertEqual(self.git(self.remote, "log", "-1", "--format=%B", first_candidate), "first")

    def test_missing_model_warns_but_still_lands(self):
        claim = self.claim(self.a)
        candidate = self.commit(self.a, "unattributed")
        code, output, error = self.finish(self.start(
            self.a, "publish", "--claim", claim["claim"], "--base", claim["base"], "--candidate", candidate,
            "--provenance", "agent=devin"))
        self.assertEqual(code, 0, error)
        self.assertIn("warning", error)
        self.assertIn("provenance_warning", json.loads(output))
        self.assert_remote(candidate)
        named = self.claim(self.a)
        second = self.commit(self.a, "attributed")
        code, output, error = self.finish(self.start(
            self.a, "publish", "--claim", named["claim"], "--base", named["base"], "--candidate", second,
            "--provenance", "agent=devin model=swe-2-medium"))
        self.assertEqual(code, 0, error)
        self.assertNotIn("warning", error)
        self.assertNotIn("provenance_warning", json.loads(output))

    def test_provenance_scratch_survives_another_worktree_writer(self):
        module = load_landing()
        candidate = self.commit(self.a, "published candidate")
        self.git(self.a, "push", "origin", "HEAD:refs/heads/main")
        peer_path = self.root / "peer worktree"
        self.git(self.a, "worktree", "add", "--detach", str(peer_path), self.initial)
        writer = module.Landing(self.a, "origin")
        peer = module.Landing(peer_path, "origin")
        self.assertEqual(writer.record_provenance(self.initial, ["owner: seed"]), "recorded")
        seeded = self.git(self.remote, "rev-parse", module.PROVENANCE_REF)
        shared_scratch = "refs/notes/landing-provenance-scratch"
        original_git = writer.git

        def start_peer_after_note(*arguments, **keywords):
            result = original_git(*arguments, **keywords)
            if "notes" in arguments and "add" in arguments:
                # A peer starts after this writer produced its note but before
                # it reads the scratch tip. Worktrees share these local refs.
                peer.git("update-ref", shared_scratch, seeded)
            return result

        with mock.patch.object(writer, "git", side_effect=start_peer_after_note):
            observed = writer.record_provenance(candidate, ["owner: candidate"])
        self.assertEqual(observed, "recorded")
        self.assertEqual(self.git(self.remote, "notes", "--ref", "provenance", "show", candidate),
                         "owner: candidate")
        # The finishing writer also must not delete the peer's in-flight ref.
        self.assertEqual(peer.git("rev-parse", shared_scratch).stdout, seeded)

    def test_published_main_survives_provenance_filesystem_errors(self):
        for boundary in ("note creation", "scratch cleanup"):
            with self.subTest(boundary=boundary):
                module = load_landing()
                claim = self.claim(self.a)
                candidate = self.commit(self.a, boundary)
                original_git = module.Landing.git
                note_created = False

                def fail_advisory_write(client, *arguments, **keywords):
                    nonlocal note_created
                    if "notes" in arguments and "add" in arguments:
                        if boundary == "note creation":
                            raise OSError(errno.ENOSPC, "No space left on device")
                        result = original_git(client, *arguments, **keywords)
                        note_created = True
                        return result
                    if boundary == "scratch cleanup" and note_created and arguments[:2] == ("update-ref", "-d"):
                        raise OSError(errno.ENOSPC, "No space left on device")
                    return original_git(client, *arguments, **keywords)

                output, errors = io.StringIO(), io.StringIO()
                with mock.patch.object(module.Landing, "git", fail_advisory_write), \
                        mock.patch.object(module, "pause") as pause, \
                        contextlib.redirect_stdout(output), contextlib.redirect_stderr(errors):
                    result = module.main([
                        "--repository", str(self.a), "publish", "--claim", claim["claim"],
                        "--base", claim["base"], "--candidate", candidate,
                        "--provenance", "agent=test model=test"])
                self.assert_remote(candidate)
                self.assertEqual(result, 0, errors.getvalue())
                observed = json.loads(output.getvalue())
                self.assertEqual(observed["state"], "published")
                pause.assert_not_called()
                if boundary == "note creation":
                    self.assertIn("not recorded:", observed["provenance"])
                    self.assertIn("No space left on device", observed["provenance"])
                else:
                    self.assertEqual(observed["provenance"], "recorded")
                    self.assertIn("model=test", self.git(
                        self.remote, "notes", "--ref", "provenance", "show", candidate))

    def test_filesystem_error_before_publication_still_fails(self):
        module = load_landing()
        claim = self.claim(self.a)
        candidate = self.commit(self.a, "unpublished candidate")
        with mock.patch.object(module.Landing, "clean_head", side_effect=OSError(
                errno.ENOSPC, "No space left on device")), \
                contextlib.redirect_stderr(io.StringIO()) as errors:
            result = module.main([
                "--repository", str(self.a), "publish", "--claim", claim["claim"],
                "--base", claim["base"], "--candidate", candidate])
        self.assertEqual(result, 1)
        self.assertIn("No space left on device", errors.getvalue())
        self.assert_remote(self.initial, claim["claim"])

    def test_provenance_applies_only_to_publish(self):
        self.run_landing(self.a, "enqueue", "--owner", "writer", "--provenance", "model=x", expected=1)
        claim = self.claim(self.a)
        candidate = self.commit(self.a, "candidate")
        self.run_landing(self.a, "publish", "--claim", claim["claim"], "--base", claim["base"],
                         "--candidate", candidate, "--provenance", "x" * 501, expected=1)
        self.assert_remote(self.initial, claim["claim"])

    def test_invalid_arguments_are_errors_not_busy(self):
        for arguments in (("unknown-command",), ("release", "--claim", "not-an-id"),
                          ("status", "--remote", "--all")):
            with self.subTest(arguments=arguments):
                self.run_landing(self.a, *arguments, expected=1)
        self.assert_remote(self.initial)


class ReceiptTests(LandingTests):
    """`verify` receipts gate publish for Rust-touching candidates; the check
    subprocess is mocked so the suite never runs mbx/cargo."""

    def rust_crate(self, package="crate-one"):
        manifest = self.a / "crate" / "Cargo.toml"
        manifest.parent.mkdir(parents=True, exist_ok=True)
        manifest.write_text(f'[package]\nname = "{package}"\nversion = "0.0.0"\n',
                            encoding="utf-8")
        source = manifest.parent / "src" / "lib.rs"
        source.parent.mkdir(exist_ok=True)
        source.write_text("pub fn answer() -> i32 { 1 }\n", encoding="utf-8")
        self.git(self.a, "add", "-A")
        self.git(self.a, "commit", "-m", "add crate")
        return self.git(self.a, "rev-parse", "HEAD")

    def receipt(self, module, candidate, packages, exit_code=0):
        return module.write_receipt(
            self.a, {"schema": module.RECEIPT_SCHEMA, "candidate": candidate,
                     "base": self.initial, "packages": sorted(packages),
                     "command": ["mbx", "check"], "exit_code": exit_code,
                     "utc": "2026-01-01T00:00:00+00:00"})

    def test_affected_packages_follow_manifests(self):
        module = load_landing()
        self.assertEqual(
            module.affected_packages(self.a, self.initial,
                                     self.commit(self.a, "notes only")),
            set())
        crate = self.rust_crate()
        self.assertEqual(module.affected_packages(self.a, self.initial, crate),
                         {"crate-one", "omega"})
        root = self.a / "Cargo.lock"
        root.write_text("version = 3\n", encoding="utf-8")
        self.git(self.a, "add", "Cargo.lock")
        self.git(self.a, "commit", "-m", "lockfile")
        lock = self.git(self.a, "rev-parse", "HEAD")
        self.assertEqual(module.affected_packages(self.a, crate, lock),
                         {"omega"})

    def test_affected_packages_walk_deleted_file_parents(self):
        module = load_landing()
        crate = self.rust_crate()
        self.git(self.a, "rm", "crate/src/lib.rs")
        self.git(self.a, "commit", "-m", "drop lib")
        deleted = self.git(self.a, "rev-parse", "HEAD")
        self.assertEqual(module.affected_packages(self.a, crate, deleted),
                         {"crate-one", "omega"})

    def check_recorder(self, exit_code, calls):
        """A subprocess.run side_effect that answers `mbx`/`cargo check`
        calls itself and delegates everything else (the git invocations) to
        the real run."""
        real_run = subprocess.run

        def run(command, *arguments, **keyword):
            if command[0] in ("mbx", "cargo"):
                calls.append(command)
                return subprocess.CompletedProcess(command, exit_code)
            return real_run(command, *arguments, **keyword)

        return run

    def test_verify_runs_the_check_and_writes_the_receipt(self):
        module = load_landing()
        candidate = self.rust_crate()
        calls = []
        with mock.patch.object(module.shutil, "which", return_value="/bin/mbx"), \
                mock.patch.object(module.subprocess, "run",
                                  side_effect=self.check_recorder(0, calls)):
            self.assertEqual(module.verify(self.a, self.initial), 0)
        command = calls[0]
        self.assertEqual(command[:2], ["mbx", "check"])
        self.assertIn("crate-one", command)
        self.assertIn("omega", command)
        receipt = json.loads((self.a / "build" / "landing_receipts"
                              / f"{candidate}.json").read_text())
        self.assertEqual(receipt["schema"], module.RECEIPT_SCHEMA)
        self.assertEqual(receipt["candidate"], candidate)
        self.assertEqual(receipt["exit_code"], 0)

    def test_verify_records_a_failing_check(self):
        module = load_landing()
        candidate = self.rust_crate()
        calls = []
        with mock.patch.object(module.shutil, "which", return_value=None), \
                mock.patch.object(module.subprocess, "run",
                                  side_effect=self.check_recorder(101, calls)):
            self.assertEqual(module.verify(self.a, self.initial), 1)
        receipt = json.loads((self.a / "build" / "landing_receipts"
                              / f"{candidate}.json").read_text())
        self.assertEqual(receipt["exit_code"], 101)
        self.assertEqual(calls[0][0], "cargo")

    def rebased_qualified_receipt(self, module):
        with (self.a / ".git" / "info" / "exclude").open("a", encoding="utf-8") as exclude:
            exclude.write("build/\n")
        original = self.rust_crate()
        path = self.receipt(module, original, {"crate-one", "omega"})
        prior = json.loads(path.read_text(encoding="utf-8"))
        qualification_path = self.a / "build" / "qualification.json"
        prior["qualification"] = {"path": str(qualification_path), "candidate": original}
        module.write_receipt(self.a, prior)
        self.git(self.a, "reset", "--hard", self.initial)
        base = self.commit(self.a, "incoming fixture evidence")
        candidate = self.rust_crate()
        return original, base, candidate, qualification_path

    def test_verify_reuses_build_after_historical_qualification_moves(self):
        module = load_landing()
        original, base, candidate, path = self.rebased_qualified_receipt(module)
        current = {"path": str(path), "candidate": candidate, "base": base}
        observations = []

        def validate(repository, selected_path, selected_candidate, selected_base):
            observations.append((selected_candidate, selected_base))
            if selected_candidate == original:
                raise module.qualification.RecoveryError("historical final review moved")
            return current

        calls = []
        with mock.patch.object(module.qualification, "active_path", return_value=path), \
                mock.patch.object(module.qualification, "validate", side_effect=validate), \
                mock.patch.object(module.shutil, "which", return_value="/bin/mbx"), \
                mock.patch.object(module.subprocess, "run",
                                  side_effect=self.check_recorder(0, calls)):
            self.assertEqual(module.verify(self.a, base), 0)
        self.assertEqual(calls, [])
        self.assertEqual(observations, [(candidate, base), (candidate, base)])
        receipt = json.loads(module.receipt_path(self.a, candidate).read_text(encoding="utf-8"))
        self.assertEqual(receipt["reused_build_from"], original)
        self.assertEqual(receipt["qualification"], current)

    def test_build_reuse_still_rejects_changed_rust_inputs(self):
        module = load_landing()
        _, base, _, _ = self.rebased_qualified_receipt(module)
        source = self.a / "crate" / "src" / "lib.rs"
        source.write_text("pub fn answer() -> i32 { 2 }\n", encoding="utf-8")
        self.git(self.a, "add", "crate/src/lib.rs")
        self.git(self.a, "commit", "--amend", "--no-edit")
        candidate = self.git(self.a, "rev-parse", "HEAD")
        workspaces = module.affected_workspaces(self.a, base, candidate)
        self.assertIsNone(module.reusable_build_receipt(
            self.a, base, candidate, workspaces, {"crate-one", "omega"}))

    def test_current_qualification_still_blocks_verify_and_publication(self):
        module = load_landing()
        original, base, candidate, path = self.rebased_qualified_receipt(module)
        calls = []
        with mock.patch.object(module.qualification, "active_path", return_value=path), \
                mock.patch.object(module.qualification, "validate", side_effect=
                                  module.qualification.RecoveryError("current final review stale")), \
                mock.patch.object(module.subprocess, "run",
                                  side_effect=self.check_recorder(0, calls)):
            with self.assertRaisesRegex(module.LandingError, "current final review stale"):
                module.verify(self.a, base)
            self.assertEqual(calls, [])
            self.assertFalse(module.receipt_path(self.a, candidate).exists())
            prior = json.loads(module.receipt_path(self.a, original).read_text(encoding="utf-8"))
            prior.update(candidate=candidate, base=base,
                         qualification={"path": str(path), "candidate": candidate})
            module.write_receipt(self.a, prior)
            client = module.Landing(self.a, "origin")
            options = SimpleNamespace(base=base, candidate=candidate, board_update=False)
            snapshot = {"main": base, "record": {"active": {"base": base}}}
            with mock.patch.object(module, "expired", return_value=False):
                with self.assertRaisesRegex(module.LandingError, "current final review stale"):
                    client.publication_check(options, snapshot)

    def standalone_crate(self, package="crate-one"):
        self.rust_crate(package)
        manifest = self.a / "crate" / "Cargo.toml"
        with manifest.open("a", encoding="utf-8") as output:
            output.write("\n[workspace]\n")
        self.git(self.a, "add", "crate/Cargo.toml")
        self.git(self.a, "commit", "-m", "independent workspace")
        return self.git(self.a, "rev-parse", "HEAD")

    def test_verify_checks_standalone_workspace_and_requires_its_receipt(self):
        module = load_landing()
        candidate = self.standalone_crate()
        calls = []
        with mock.patch.object(module.shutil, "which", return_value="/bin/mbx"), \
                mock.patch.object(module.subprocess, "run",
                                  side_effect=self.check_recorder(0, calls)):
            self.assertEqual(module.verify(self.a, self.initial), 0)
        self.assertEqual(calls[0], ["mbx", "check", "-p", "omega"])
        self.assertEqual(calls[1], ["mbx", "check", "--manifest-path",
                                   str((self.a / "crate" / "Cargo.toml").resolve()), "-p", "crate-one"])
        workspaces = module.affected_workspaces(self.a, self.initial, candidate)
        affected = {"crate-one", "omega"}
        self.assertIsNone(module.receipt_failure(self.a, candidate, affected, workspaces))
        # Same flat names in an old receipt cannot stand in for a check in the
        # independent package namespace (including same-name root packages).
        self.receipt(module, candidate, affected)
        self.assertIn("workspace", module.receipt_failure(self.a, candidate, affected, workspaces))

    def test_standalone_failure_prevents_passing_receipt(self):
        module = load_landing()
        candidate = self.standalone_crate()
        calls = []
        recorder = self.check_recorder(0, calls)

        def run(command, *arguments, **keyword):
            result = recorder(command, *arguments, **keyword)
            if "--manifest-path" in command:
                return subprocess.CompletedProcess(command, 101)
            return result

        with mock.patch.object(module.subprocess, "run", side_effect=run):
            self.assertEqual(module.verify(self.a, self.initial), 1)
        receipt = json.loads(module.receipt_path(self.a, candidate).read_text())
        self.assertEqual(receipt["exit_code"], 101)
        self.assertEqual([check["exit_code"] for check in receipt["checks"]], [0, 101])

    def test_same_named_standalone_package_needs_its_own_check(self):
        module = load_landing()
        candidate = self.standalone_crate("omega")
        workspaces = module.affected_workspaces(self.a, self.initial, candidate)
        self.assertEqual(workspaces, {".": {"omega"}, "crate/Cargo.toml": {"omega"}})
        self.receipt(module, candidate, {"omega"})
        self.assertIn("workspace", module.receipt_failure(self.a, candidate, {"omega"}, workspaces))

    def test_nested_lockfile_and_root_lock_keep_independent_coverage(self):
        module = load_landing()
        base = self.standalone_crate()
        (self.a / "Cargo.lock").write_text("version = 4\n", encoding="utf-8")
        (self.a / "crate" / "Cargo.lock").write_text("version = 4\n", encoding="utf-8")
        self.git(self.a, "add", "Cargo.lock", "crate/Cargo.lock")
        self.git(self.a, "commit", "-m", "lock dependencies")
        candidate = self.git(self.a, "rev-parse", "HEAD")
        self.assertEqual(module.affected_workspaces(self.a, base, candidate),
                         {".": {"omega"}, "crate/Cargo.toml": {"crate-one"}})

    def test_verify_skips_the_check_when_nothing_rust_changed(self):
        module = load_landing()
        candidate = self.commit(self.a, "notes only")
        calls = []
        with mock.patch.object(module.subprocess, "run",
                               side_effect=self.check_recorder(0, calls)):
            self.assertEqual(module.verify(self.a, self.initial), 0)
        self.assertEqual(calls, [])
        receipt = json.loads((self.a / "build" / "landing_receipts"
                              / f"{candidate}.json").read_text())
        self.assertEqual(receipt["packages"], [])
        self.assertIsNone(receipt["command"])

    def test_verify_refuses_a_base_that_is_not_an_ancestor(self):
        module = load_landing()
        # Branch a sibling Rust change off initial, then return to a
        # Python-only candidate: the sibling is not in HEAD's history.
        sibling = self.rust_crate()
        self.git(self.a, "reset", "--hard", self.initial)
        candidate = self.commit(self.a, "notes only")
        calls = []
        with mock.patch.object(module.subprocess, "run",
                               side_effect=self.check_recorder(0, calls)):
            with self.assertRaises(module.LandingError) as refusal:
                module.verify(self.a, sibling)
        self.assertIn("not an ancestor", str(refusal.exception))
        self.assertEqual(calls, [])
        self.assertFalse((self.a / "build" / "landing_receipts"
                          / f"{candidate}.json").exists())

    def test_publish_refuses_a_rust_candidate_without_a_passing_receipt(self):
        module = load_landing()
        # The temp repository has no .gitignore; keep receipts invisible to
        # clean_head the same way the real repository's /build/ rule does.
        with open(self.a / ".git" / "info" / "exclude", "a",
                  encoding="utf-8") as exclude:
            exclude.write("build/\n")
        claim = self.claim(self.a)
        candidate = self.rust_crate()
        refusal = self.publish(self.a, claim, candidate, expected=1)
        self.assertIn("verify", str(refusal))
        self.assert_remote(self.initial, claim["claim"])
        # A receipt for a different sha, a failing check, or one missing an
        # affected package are all refused; only an exact match publishes.
        self.receipt(module, "0" * 40, {"crate-one", "omega"})
        self.publish(self.a, claim, candidate, expected=1)
        self.receipt(module, candidate, {"crate-one", "omega"}, exit_code=101)
        self.publish(self.a, claim, candidate, expected=1)
        self.receipt(module, candidate, {"omega"})
        self.publish(self.a, claim, candidate, expected=1)
        self.receipt(module, candidate, {"crate-one", "omega"})
        self.assertEqual(self.publish(self.a, claim, candidate)["state"],
                         "published")
        self.assert_remote(candidate)


if __name__ == "__main__":
    unittest.main()
