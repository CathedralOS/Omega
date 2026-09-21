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
import unittest


CLAIMS = Path(__file__).resolve().parents[1] / "claims.py"
CLAIMS_REF = "refs/coordination/omega-claims/main"
MAIN_REF = "refs/heads/main"
BOARD = "- **ALPHA-ITEM.** First item.\n- **BETA-ITEM.** Second item.\n- **GAMMA-ITEM.** Third item.\n"


def load_claims():
    specification = importlib.util.spec_from_file_location("claims_under_test", CLAIMS)
    module = importlib.util.module_from_spec(specification)
    previous = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        specification.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


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


class ClaimsTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="omega claims test ")
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
        (self.a / "TASKS.md").write_text(BOARD, encoding="utf-8")
        self.git(self.a, "add", "TASKS.md")
        self.initial = self.commit(self.a, "initial")
        self.git(self.a, "remote", "add", "origin", str(self.remote))
        self.git(self.a, "push", "origin", "HEAD:" + MAIN_REF)
        self.git(self.root, "clone", "--branch", "main", str(self.remote), str(self.b))
        self.configure(self.b)

    def configure(self, directory):
        for name, value in (("user.name", "Claims Test"), ("user.email", "test@example.invalid"),
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
        return subprocess.Popen([sys.executable, str(CLAIMS), "--repository", str(directory),
                                 *arguments], stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                text=True, encoding="utf-8", env=self.environment)

    def finish(self, process):
        try:
            output, error = process.communicate(timeout=30)
        except subprocess.TimeoutExpired:
            process.kill()
            process.communicate()
            self.fail("Local claims command timed out")
        return process.returncode, output, error

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

    def test_same_item_conflict_blocks_until_release(self):
        claimed = self.claim(self.a, owner="A")
        conflict = self.run_claims(self.b, "claim", "--item", "ALPHA-ITEM",
                                   "--owner", "B", expected=2)
        self.assertEqual(conflict["state"], "conflict")
        self.assertEqual(conflict["conflicts"][0]["owner"], "A")
        self.claim(self.b, item="BETA-ITEM", owner="B")
        self.run_claims(self.a, "release", "--ticket", claimed["ticket"])
        self.claim(self.b, item="ALPHA-ITEM", owner="B")
        self.assertEqual(len(self.run_claims(self.a, "status")["claims"]), 2)

    def test_claim_splits_comma_joined_path_values(self):
        self.claim(self.a, "ALPHA-ITEM", "A", "--path", "src/x,src/y",
                   "--path", "src/z")
        entry = self.run_claims(self.b, "status")["claims"][0]
        self.assertEqual(entry["paths"], ["src/x", "src/y", "src/z"])
        conflict = self.run_claims(self.b, "claim", "--item", "BETA-ITEM",
                                   "--owner", "B", "--path", "src/y/deep",
                                   expected=2)
        self.assertEqual(conflict["conflicts"][0]["reason"], "overlapping paths")
        self.assertEqual(conflict["conflicts"][0]["shared_paths"], ["src/y/deep"])

    def test_claim_rejects_empty_path_segments(self):
        for bad in ("src/x,,src/y", "src/x,", ",src/x"):
            with self.subTest(bad=bad):
                self.claim(self.a, "ALPHA-ITEM", "A", "--path", bad, expected=1)
        self.assertEqual(self.run_claims(self.a, "status")["state"], "available")

    def test_legacy_comma_joined_claim_still_fences(self):
        """A claim recorded before comma-splitting keeps its full coverage."""
        self.claim(self.a, "ALPHA-ITEM", "A", "--path", "src/x")
        record = self.remote_record()
        record["claims"][0]["paths"] = ["src/x,src/y"]
        self.rewrite_record(record)
        conflict = self.run_claims(self.b, "claim", "--item", "BETA-ITEM",
                                   "--owner", "B", "--path", "src/y/deep",
                                   expected=2)
        self.assertEqual(conflict["state"], "conflict")
        self.assertEqual(conflict["conflicts"][0]["shared_paths"], ["src/y/deep"])

    def test_path_overlap_blocks_and_allow_overlap_records_exception(self):
        self.claim(self.a, "ALPHA-ITEM", "A", "--path", "src/x")
        conflict = self.run_claims(self.b, "claim", "--item", "BETA-ITEM", "--owner", "B",
                                   "--path", "src/x/y", expected=2)
        self.assertEqual(conflict["conflicts"][0]["reason"], "overlapping paths")
        self.assertEqual(conflict["conflicts"][0]["shared_paths"], ["src/x/y"])
        self.claim(self.b, "BETA-ITEM", "B", "--path", "src/z")
        allowed = self.run_claims(self.b, "claim", "--item", "GAMMA-ITEM", "--owner", "B",
                                  "--path", "src/x/deep", "--allow-overlap")
        self.assertEqual(allowed["overlaps"][0]["item"], "ALPHA-ITEM")

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

    def test_expired_claim_is_reaped_and_never_blocks(self):
        claimed = self.claim(self.a, owner="A")
        record = self.remote_record()
        record["claims"][0]["expires_utc"] = (
            datetime.now(timezone.utc) - timedelta(minutes=1)).isoformat()
        self.rewrite_record(record)
        status = self.run_claims(self.b, "status")
        self.assertEqual(status["state"], "available")
        self.assertEqual(status["expired"][0]["ticket"], claimed["ticket"])
        reclaimed = self.claim(self.b, owner="B")
        self.assertNotEqual(reclaimed["ticket"], claimed["ticket"])
        record = self.remote_record()
        self.assertEqual([entry["ticket"] for entry in record["claims"]],
                         [reclaimed["ticket"]])
        self.run_claims(self.a, "release", "--ticket", claimed["ticket"], expected=1)

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

    def test_concurrent_claims_retain_both_arrivals(self):
        for round_number in range(3):
            with self.subTest(round=round_number):
                a = self.start(self.a, "claim", "--item", "ALPHA-ITEM", "--board",
                               "TASKS.md", "--owner", "A")
                b = self.start(self.b, "claim", "--item", "BETA-ITEM", "--board",
                               "TASKS.md", "--owner", "B")
                results = [self.finish(a), self.finish(b)]
                self.assertEqual([row[0] for row in results], [0, 0], results)
                tickets = [entry["ticket"] for entry in self.run_claims(self.a, "status")["claims"]]
                self.assertEqual(len(set(tickets)), 2)
                for ticket in tickets:
                    self.run_claims(self.a, "release", "--ticket", ticket)

    def test_audit_reports_uncovered_and_violating_paths(self):
        self.claim(self.a, "ALPHA-ITEM", "A", "--path", "src/x")
        self.claim(self.b, "BETA-ITEM", "B", "--path", "src/y")
        for relative in ("src/x/mine.txt", "src/y/theirs.txt", "src/z/free.txt"):
            path = self.a / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("changed\n", encoding="utf-8")
        report = self.run_claims(self.a, "audit", "--worktree", str(self.a),
                                 "--owner", "A", expected=2)
        self.assertEqual(report["uncovered"],
                         ["src/y/theirs.txt", "src/z/free.txt"])
        self.assertEqual(report["violations"],
                         [{"path": "src/y/theirs.txt", "item": "BETA-ITEM",
                           "claimed_by": "B",
                           "ticket": report["violations"][0]["ticket"],
                           "expires_utc": report["violations"][0]["expires_utc"]}])
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
