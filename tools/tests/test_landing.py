#!/usr/bin/env python3
"""Local Git protocol tests; no GitHub access or non-standard Python packages."""

import json
from datetime import datetime, timedelta, timezone
import contextlib
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
from unittest import mock
import uuid


LANDING = Path(__file__).resolve().parents[1] / "landing.py"
CLAIM_REF = "refs/coordination/omega-landing/main"
MAIN_REF = "refs/heads/main"


def load_landing():
    specification = importlib.util.spec_from_file_location("landing_under_test", LANDING)
    module = importlib.util.module_from_spec(specification)
    previous = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        specification.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


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


class LandingTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="omega landing test ")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.remote = self.root / "remote.git"
        self.a, self.b = self.root / "writer a", self.root / "writer b"
        self.environment = {key: value for key, value in os.environ.items()
                            if not key.startswith("GIT_")}
        self.environment.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull,
                                GIT_TERMINAL_PROMPT="0")
        self.git(self.root, "init", "--bare", str(self.remote))
        self.configure(self.remote)
        self.git(self.root, "init", "-b", "main", str(self.a))
        self.configure(self.a)
        self.initial = self.commit(self.a, "initial")
        self.git(self.a, "remote", "add", "origin", str(self.remote))
        self.git(self.a, "push", "origin", "HEAD:" + MAIN_REF)
        self.git(self.root, "clone", "--branch", "main", str(self.remote), str(self.b))
        self.configure(self.b)

    def configure(self, directory):
        for name, value in (("user.name", "Landing Test"), ("user.email", "test@example.invalid"),
                            ("commit.gpgSign", "false"), ("core.autocrlf", "false")):
            self.git(directory, "config", name, value)

    def git(self, directory, *arguments, input_text=""):
        result = subprocess.run(["git", "-C", str(directory), *arguments], input=input_text,
                                capture_output=True, text=True, encoding="utf-8",
                                env=self.environment, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout.strip()

    def commit(self, directory, message):
        self.git(directory, "commit", "--allow-empty", "-m", message)
        return self.git(directory, "rev-parse", "HEAD")

    def start(self, directory, *arguments):
        return subprocess.Popen([sys.executable, str(LANDING), "--repository", str(directory),
                                 *arguments], stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                text=True, encoding="utf-8", env=self.environment)

    def finish(self, process):
        try:
            output, error = process.communicate(timeout=30)
        except subprocess.TimeoutExpired:
            process.kill()
            process.communicate()
            self.fail("Local landing command timed out")
        return process.returncode, output, error

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

    def test_invalid_arguments_are_errors_not_busy(self):
        for arguments in (("unknown-command",), ("release", "--claim", "not-an-id"),
                          ("status", "--remote", "--all")):
            with self.subTest(arguments=arguments):
                self.run_landing(self.a, *arguments, expected=1)
        self.assert_remote(self.initial)


if __name__ == "__main__":
    unittest.main()
