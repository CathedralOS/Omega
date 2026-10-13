#!/usr/bin/env python3
"""Actual CLI migration, Git authority and dirty-owner preservation; no cloud calls."""

import contextlib
import hashlib
import io
import json
from pathlib import Path
import sys
import unittest

import worktree_testkit as kit

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS / "swarm"))
legacy = kit.load_module(TOOLS / "swarm/legacy_context.py", "legacy_context_under_test")


class LegacyTests(unittest.TestCase):
    def setUp(self):
        self.root, self.repository = kit.seed_repository(self, "legacy-context-")
        spec = self.repository / "wiki/spec/rule.md"
        spec.parent.mkdir(parents=True)
        spec.write_text("The exact existing canonical rule.\n")
        (self.repository / ".gitignore").write_text("/build/\n")
        kit.git(self, self.repository, "add", ".")
        kit.git(self, self.repository, "commit", "-m", "tools: canonical fixture")
        self.base = kit.git(self, self.repository, "rev-parse", "HEAD")
        remote = self.root / "accepted.git"
        kit.git(self, self.root, "init", "--bare", str(remote))
        kit.git(self, self.repository, "remote", "add", "origin", str(remote))
        kit.git(self, self.repository, "push", "origin", "HEAD:refs/heads/main")
        config = dict(semantic_contracts=[dict(implementation=["file.txt"], contract="wiki/spec/rule.md",
            classification="missing_implementation", reason="existing rule", passages=["The exact existing canonical rule."])])
        packet = legacy.qualification.work_handoff(self.repository, config, self.base, "continuation")
        self.packet = self.root / "packet.json"
        self.packet.write_text(json.dumps(packet, indent=2) + "\n")
        self.state = self.root / "legacy.json"
        self.input = dict(repository=str(self.repository), owner_packet=str(self.packet),
            owner_packet_sha256=legacy.qualification.file_digest(self.packet), thread="actual-existing-thread",
            binding_evidence="parent mapped the existing owner thread", authorization_evidence="continue same authorized outcome",
            outcome="same held compiler repair", owner_worktree_status="uncommitted")

    def command(self, command, value=None, revision=None):
        args = [command, "--state", str(self.state)]
        if value is not None:
            path = self.root / (command + "-input.json")
            path.write_text(json.dumps(value))
            args += ["--input", str(path)]
        if revision is not None:
            args += ["--revision", str(revision)]
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            code = legacy.main(args)
        return code, json.loads(output.getvalue())

    def read_receipt(self, **updates):
        value = dict(tool="cloud_threads.read", observed_utc=legacy.now(), evidence="actual fresh read receipt",
            structuredContent=dict(threadId="actual-existing-thread", latestTurn=dict(id="owner-report-turn", status="completed", error=None), items=[], nextCursor=None))
        value.update(updates)
        return value

    def test_legacy_cli_preserves_uncommitted_owner_and_never_invents_history(self):
        (self.repository / "file.txt").write_text("uncommitted repair remains here\n")
        before = (self.repository / "file.txt").read_bytes()
        code, prepared = self.command("prepare", self.input)
        self.assertEqual(code, 0)
        self.assertEqual(prepared["action"], "inspect_only")
        self.assertEqual(prepared["historical_state"], "unavailable")
        self.assertFalse(prepared["managed_credit"])
        self.assertFalse(prepared["final_candidate_acceptance"])
        request = prepared["request"]
        self.assertFalse(request["work_context"]["prior_context_retained"])
        self.assertEqual(request["work_context"]["changed_paths"], [])
        self.assertIn("uncommitted", request["prompt"])
        self.assertIn("The exact existing canonical rule.", request["prompt"])
        self.assertEqual(request["runtime_arguments"]["threadId"], "actual-existing-thread")
        self.assertEqual(request["runtime_arguments"]["prompt"], request["prompt"])
        code, reserved = self.command("reserve", self.read_receipt(), 0)
        self.assertEqual(code, 0)
        self.assertEqual(reserved["action"], "invoke_once")
        self.assertEqual((self.repository / "file.txt").read_bytes(), before)
        self.assertEqual(self.command("reserve", self.read_receipt(), 1)[0], 1)
        result = dict(operation=request["operation"], execution_id=request["execution_id"], effect="applied",
            evidence="actual existing-thread enqueue result", result=dict(threadId="actual-existing-thread"))
        self.assertEqual(self.command("record-result", result, 1)[0], 0)
        code, status = self.command("status")
        self.assertEqual(code, 0)
        self.assertEqual(status["phase"], "recorded")
        self.assertEqual(status["action"], "inspect_only")

    def test_unknown_thread_and_running_turn_refuse_reservation(self):
        self.assertEqual(self.command("prepare", self.input)[0], 0)
        for thread, status, error in (("different-thread", "completed", None),
                                      ("actual-existing-thread", "running", None),
                                      ("actual-existing-thread", "completed", "approval denied")):
            value = self.read_receipt()
            value["structuredContent"].update(threadId=thread, latestTurn=dict(id="turn", status=status, error=error))
            self.assertEqual(self.command("reserve", value, 0)[0], 1)
        self.assertEqual(legacy.load(self.state)["phase"], "prepared")

    def test_stale_spec_and_changed_packet_refuse_without_overwriting_journal(self):
        self.assertEqual(self.command("prepare", self.input)[0], 0)
        self.assertEqual(self.command("prepare", self.input)[0], 1)
        (self.repository / "wiki/spec/rule.md").write_text("Dirty replacement.\n")
        self.assertEqual(self.command("reserve", self.read_receipt(), 0)[0], 1)
        self.assertEqual(legacy.load(self.state)["revision"], 0)
        self.packet.write_text("{}")
        self.assertEqual(self.command("reserve", self.read_receipt(), 0)[0], 1)

    def test_candidate_only_spec_is_not_canonical_authority(self):
        (self.repository / "wiki/spec/rule.md").write_text("An invented candidate rule.\n")
        kit.git(self, self.repository, "add", ".")
        kit.git(self, self.repository, "commit", "-m", "tools: unaccepted spec")
        candidate = kit.git(self, self.repository, "rev-parse", "HEAD")
        config = dict(semantic_contracts=[dict(implementation=["file.txt"], contract="wiki/spec/rule.md",
            classification="missing_implementation", reason="claimed new rule", passages=["An invented candidate rule."])])
        packet = legacy.qualification.work_handoff(self.repository, config, candidate, "continuation")
        self.packet.write_text(json.dumps(packet))
        self.input["owner_packet_sha256"] = legacy.qualification.file_digest(self.packet)
        code, result = self.command("prepare", self.input)
        self.assertEqual(code, 1)
        self.assertIn("not accepted canonical main", result["error"])
        self.assertFalse(self.state.exists())

    def test_old_retention_claims_do_not_suppress_reinjection(self):
        packet = json.loads(self.packet.read_text())
        packet.update(prior_context_retained=True, context_refresh=False, reinject_context=[])
        self.packet.write_text(json.dumps(packet))
        self.input["owner_packet_sha256"] = legacy.qualification.file_digest(self.packet)
        code, result = self.command("prepare", self.input)
        self.assertEqual(code, 0)
        self.assertFalse(result["request"]["work_context"]["prior_context_retained"])
        self.assertIn("The exact existing canonical rule.", result["request"]["prompt"])

    def test_missing_context_and_review_packets_cannot_adopt_implementation(self):
        packet = json.loads(self.packet.read_text())
        packet["role"] = "review"
        self.packet.write_text(json.dumps(packet))
        self.input["owner_packet_sha256"] = legacy.qualification.file_digest(self.packet)
        self.assertEqual(self.command("prepare", self.input)[0], 1)
        packet.update(role="continuation", canonical_context=[])
        self.packet.write_text(json.dumps(packet))
        self.input["owner_packet_sha256"] = legacy.qualification.file_digest(self.packet)
        self.assertEqual(self.command("prepare", self.input)[0], 1)

    def test_stale_read_wrong_result_and_lost_result_do_not_allow_replay(self):
        self.assertEqual(self.command("prepare", self.input)[0], 0)
        self.assertEqual(self.command("reserve", self.read_receipt(observed_utc="2000-01-01T00:00:00+00:00"), 0)[0], 1)
        code, result = self.command("reserve", self.read_receipt(), 0)
        self.assertEqual(code, 0)
        request = result["request"]
        wrong = dict(operation=request["operation"], execution_id=request["execution_id"], effect="applied",
            evidence="actual tool reply", result=dict(threadId="other-thread"))
        self.assertEqual(self.command("record-result", wrong, 1)[0], 1)
        unknown = dict(wrong, effect="unknown", result={})
        self.assertEqual(self.command("record-result", unknown, 1)[0], 0)
        self.assertEqual(self.command("reserve", self.read_receipt(), 2)[0], 1)

    def test_two_reported_legacy_owners_bind_exact_thread_without_completion_credit(self):
        # Real reported thread identities; source and runtime receipts here are
        # synthetic fixture evidence, not validation of either owner's work.
        cases = [("01a13341-1bbc-74c0-9924-a57c203760c2", "uncommitted",
                  "Held mathematical-codomain repair; pending edge decisions unchanged"),
                 ("01a132f2-cd2e-718a-9090-bc8cfee8b0fe", "unknown",
                  "Physical lending/accessor execution unfinished; old ab1061 sample is not completion")]
        for index, (thread, status, outcome) in enumerate(cases):
            with self.subTest(thread=thread):
                self.state = self.root / ("owner-" + str(index) + ".json")
                value = dict(self.input, thread=thread, owner_worktree_status=status, outcome=outcome)
                self.assertEqual(self.command("prepare", value)[0], 0)
                read = self.read_receipt();read["structuredContent"]["threadId"] = thread
                code, result = self.command("reserve", read, 0)
                self.assertEqual(code, 0)
                self.assertEqual(result["request"]["runtime_arguments"]["threadId"], thread)
                self.assertFalse(result["managed_credit"])
                self.assertFalse(result["final_candidate_acceptance"])
                self.assertIn(outcome, result["request"]["prompt"])

    def test_saved_prompt_cannot_drop_context_or_change_runtime_recipient(self):
        self.assertEqual(self.command("prepare", self.input)[0], 0)
        original = legacy.load(self.state)
        for field, value in (("prompt", "context omitted"), ("threadId", "wrong-thread")):
            edited = json.loads(json.dumps(original))
            if field == "prompt":
                edited["request"]["prompt"] = value
                edited["request"]["runtime_arguments"]["prompt"] = value
            else:
                edited["request"]["runtime_arguments"][field] = value
            legacy.atomic_json(self.state, edited)
            self.assertEqual(self.command("reserve", self.read_receipt(), 0)[0], 1)
            self.assertEqual(legacy.load(self.state)["phase"], "prepared")


if __name__ == "__main__":
    unittest.main()
