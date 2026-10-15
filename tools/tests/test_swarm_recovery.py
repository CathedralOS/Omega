#!/usr/bin/env python3
"""Local fault injection and explicit fake adapters, not cloud transport tests."""

import copy
from datetime import datetime, timedelta, timezone
import io
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import tempfile
import time
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "swarm"))
import recovery
import recovery_artifacts as artifacts
from recovery_state import RecoveryError, Store, atomic_json


def lane(name="atomic"):
    return dict(id=name, assignment=name + "-leg1", worktree="executor-local/" + name,
                branch="swarm/" + name, base="a" * 40, head="b" * 40,
                reviewed_main="a" * 40, worker_handle="handle-" + name,
                mode="implementation", checkpoint=dict(status="incomplete", summary="WIP; native unrun"),
                resources=dict(target="owned-target", cache="shared-cache; preserve"))


class FakeAdapter:
    """No network. Simulates observations and a runtime losing a resume reply."""
    def __init__(self):
        self.available = True
        self.worker = "idle"
        self.calls = []
        self.counter = 0
        self.effects = {}

    def observe(self, record):
        self.counter += 1
        return dict(schema=1, id="seen-" + str(self.counter), revision=record["revision"],
                    executor=record["executor"], generation=record["generation"],
                    observed_utc=datetime.now(timezone.utc).isoformat(),
                    availability="available" if self.available else "disconnected",
                    lanes={key: dict(assignment=row["assignment"], worker_handle=row["worker_handle"],
                                     worker_state=self.worker, processes_stopped=self.worker != "running",
                                     workspace=dict(exists=True, head=row["head"], clean=False, primary=False),
                                     main=row["reviewed_main"], ownership="clear", claims=[], landing_tickets=[],
                                     resources_owned=True, operations=copy.deepcopy(self.effects))
                           for key, row in record["lanes"].items()})

    def resume(self, handle, operation):
        self.calls.append((handle, operation))
        self.worker = "running"
        self.available = False  # delivery happened, response was lost


def coordinator_step(store, adapter):
    """Coordinator consumes actual next_action; intent precedes its adapter call."""
    current = store.load()
    observation = adapter.observe(current)
    if not recovery.fresh(current, observation):
        return recovery.plan(current, observation)
    updated = store.update(current["revision"], lambda r: recovery.reconcile(r, observation))
    decision = recovery.plan(updated)
    for row in decision["lanes"]:
        if row["action"] == "offer_resume":
            updated = store.update(updated["revision"], lambda r: recovery.intent(
                r, row["lane"], row["resume_intent"], "mutation"))
            adapter.resume(updated["lanes"][row["lane"]]["worker_handle"], row["resume_intent"])
    return decision


class RecoveryTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix="swarm recovery ")
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name)
        self.store = Store(self.root / "recovery.json")
        self.store.initialize("host-a", "boot-a")
        self.store.update(0, lambda r: recovery.register(r, lane()))
        self.adapter = FakeAdapter()

    def record(self):
        return self.store.load()

    def update(self, function):
        return self.store.update(self.record()["revision"], function)

    def observed(self):
        return self.adapter.observe(self.record())

    def action(self, observation=None):
        return recovery.plan(self.record(), observation or self.observed())["lanes"][0]["action"]

    def test_compact_records_keep_values_and_atomic_failure_guards(self):
        record = self.record()
        pretty = self.store.path.read_bytes()
        atomic_json(self.store.path, record, indent=None)
        compact = self.store.path.read_bytes()
        self.assertEqual(self.record(), record)
        self.assertLess(len(compact), len(pretty))
        with self.assertRaises(ValueError):
            atomic_json(self.store.path, dict(record, invalid=float("nan")), indent=None)
        self.assertEqual(self.store.path.read_bytes(), compact)
        with mock.patch("recovery_state.os.replace", side_effect=OSError("fault")):
            with self.assertRaises(OSError):
                atomic_json(self.store.path, dict(record, changed=True), indent=None)
        self.assertEqual(self.store.path.read_bytes(), compact)
        self.assertEqual(list(self.root.glob(".recovery.json.*")), [])
        atomic_json(self.store.path, record)
        self.assertEqual(self.store.path.read_bytes(), pretty)

    def test_atomic_payload_digest_binds_only_successful_exact_bytes(self):
        record = self.record()
        digest = atomic_json(self.store.path, record, indent=None, with_digest=True)
        before = self.store.path.read_bytes()
        self.assertEqual(digest, hashlib.sha256(before).hexdigest())
        self.assertEqual(json.loads(before), record)
        with mock.patch("recovery_state.os.replace", side_effect=OSError("digest fault")):
            with self.assertRaisesRegex(OSError, "digest fault"):
                atomic_json(self.store.path, dict(record, changed=True),
                            indent=None, with_digest=True)
        self.assertEqual(self.store.path.read_bytes(), before)
        self.assertIsNone(atomic_json(self.store.path, record))

    def test_interrupted_replace_keeps_old_complete_revision(self):
        before = self.store.path.read_bytes()
        with mock.patch("recovery_state.os.replace", side_effect=OSError("fault")):
            with self.assertRaises(OSError):
                self.update(lambda r: recovery.checkpoint(r, "atomic", dict(status="blocked", summary="later")))
        self.assertEqual(before, self.store.path.read_bytes())
        self.assertEqual(list(self.root.glob(".recovery.json.*")), [])

    def test_crash_after_replace_releases_lock_and_leaves_new_revision(self):
        script = """import os,sys
sys.path.insert(0,sys.argv[1])
import recovery_state as s
original=s.os.replace
def crash(a,b):
 original(a,b)
 os._exit(17)
s.os.replace=crash
s.Store(sys.argv[2]).update(1,lambda r:r.update(crash_marker=True))
"""
        result = subprocess.run([sys.executable, "-c", script, str(ROOT / "tools/swarm"),
                                 str(self.store.path)], timeout=10)
        self.assertEqual(result.returncode, 17)
        self.assertTrue(self.record()["crash_marker"])
        self.assertEqual(self.record()["revision"], 2)
        self.update(lambda r: r.update(restarted=True))
        self.assertTrue(self.record()["restarted"])

    def test_duplicate_init_register_and_reconcile_are_idempotent(self):
        self.store.initialize("host-a", "boot-a")
        self.update(lambda r: recovery.register(r, lane()))
        self.assertEqual(self.record()["revision"], 1)
        observation = self.observed()
        self.update(lambda r: recovery.reconcile(r, observation))
        self.update(lambda r: recovery.reconcile(r, observation))
        self.assertEqual(self.record()["revision"], 2)
        changed = copy.deepcopy(observation)
        changed["availability"] = "disconnected"
        with self.assertRaises(RecoveryError):
            self.update(lambda r: recovery.reconcile(r, changed))

    def test_stale_writer_and_future_schema_refused(self):
        with self.assertRaises(RecoveryError):
            self.store.update(0, lambda r: r.clear())
        bad = self.record()
        bad["schema"] = 99
        atomic_json(self.store.path, bad)
        with self.assertRaises(RecoveryError):
            self.store.load()

    def test_checkpoint_invalidates_observation_and_explicit_previous_revision_is_stale(self):
        observation = self.observed()
        self.update(lambda r: recovery.reconcile(r, observation))
        self.assertEqual(recovery.plan(self.record())["lanes"][0]["action"], "offer_resume")
        self.assertEqual(self.action(observation), "reconcile_executor")
        self.update(lambda r: recovery.checkpoint(r, "atomic", dict(status="incomplete", summary="later")))
        self.assertIsNone(self.record()["observation"])
        self.assertEqual(recovery.plan(self.record())["lanes"][0]["action"], "reconcile_executor")

    def test_build_slots_are_executor_scoped_and_not_pid_scoped(self):
        self.update(lambda r: recovery.register(r, lane("second")))
        self.update(lambda r: recovery.intent(r, "atomic", "compile-1", "build"))
        with self.assertRaises(RecoveryError):
            self.update(lambda r: recovery.intent(r, "second", "compile-2", "build"))
        other = Store(self.root / "other-host.json")
        other.initialize("host-b", "boot-b")
        other.update(0, lambda r: recovery.register(r, lane()))
        other.update(1, lambda r: recovery.intent(r, "atomic", "compile-1", "build"))
        self.assertIsNotNone(other.load()["build_slot"])
        with self.assertRaises(RecoveryError):
            self.store.initialize("host-b", "boot-b")

    def test_stale_wrong_host_and_missing_observations_never_release_slot(self):
        self.update(lambda r: recovery.intent(r, "atomic", "compile-1", "build"))
        for field, value in [("executor", "host-b"), ("generation", "boot-b"),
                             ("observed_utc", (datetime.now(timezone.utc) - timedelta(minutes=5)).isoformat())]:
            observation = self.observed()
            observation[field] = value
            with self.assertRaises(RecoveryError):
                self.update(lambda r: recovery.reconcile(r, observation))
            self.assertEqual(self.action(observation), "reconcile_executor")
            self.assertIsNotNone(self.record()["build_slot"])
        self.assertEqual(recovery.plan(self.record())["lanes"][0]["action"], "reconcile_executor")

    def test_slot_release_requires_exact_operation_and_stopped_children(self):
        self.update(lambda r: recovery.intent(r, "atomic", "compile-1", "build"))
        for name, children in [("wrong-id", True), ("compile-1", False), ("compile-1", True)]:
            observation = self.observed()
            observation["lanes"]["atomic"]["operations"][name] = dict(
                process="stopped", children_stopped=children, exit=101, evidence="adapter: process tree/result")
            self.update(lambda r: recovery.reconcile(r, observation))
            self.assertEqual(self.record()["build_slot"] is None, name == "compile-1" and children)
        self.assertEqual(self.record()["lanes"]["atomic"]["operations"]["compile-1"]["outcome"], "failed")

    def test_unknown_mutation_requires_read_only_proof_even_after_client_exit(self):
        self.update(lambda r: recovery.intent(r, "atomic", "publish-1", "mutation"))
        self.update(lambda r: recovery.finished(r, "atomic", "publish-1",
                                                dict(exit=0, process="stopped", result="exited")))
        self.assertEqual(self.action(), "reconcile_operation")
        with self.assertRaises(RecoveryError):
            self.update(lambda r: recovery.intent(r, "atomic", "publish-2", "mutation"))
        self.update(lambda r: recovery.intent(r, "atomic", "registry-read", "read"))
        self.update(lambda r: recovery.finished(r, "atomic", "registry-read",
                                                dict(exit=0, process="stopped", result="exited")))
        observation = self.observed()
        observation["lanes"]["atomic"]["operations"]["publish-1"] = dict(
            process="stopped", children_stopped=True, effect="applied", evidence="registry: exact candidate ancestor")
        self.update(lambda r: recovery.reconcile(r, observation))
        self.assertEqual(self.record()["lanes"]["atomic"]["operations"]["publish-1"]["outcome"], "applied")

    def test_coordinator_disconnect_then_resume_does_not_duplicate_dispatch(self):
        first = coordinator_step(self.store, self.adapter)
        self.assertEqual(first["lanes"][0]["action"], "offer_resume")
        self.assertEqual(len(self.adapter.calls), 1)
        self.assertEqual(coordinator_step(self.store, self.adapter)["lanes"][0]["action"], "reconcile_executor")
        self.adapter.available = True
        self.assertEqual(coordinator_step(self.store, self.adapter)["lanes"][0]["action"], "reconcile_operation")
        self.adapter.effects["resume-atomic-leg1"] = dict(process="stopped", children_stopped=True,
                                                       effect="applied", evidence="runtime: assignment acknowledged")
        self.assertEqual(coordinator_step(self.store, self.adapter)["lanes"][0]["action"], "wait_running")
        self.adapter.worker = "idle"
        self.assertEqual(coordinator_step(self.store, self.adapter)["lanes"][0]["action"], "review_resume_result")
        self.assertEqual(len(self.adapter.calls), 1)

    def test_missing_workspace_unknown_worker_and_main_drift(self):
        observation = self.observed()
        observation["lanes"]["atomic"]["workspace"]["exists"] = False
        self.assertEqual(self.action(observation), "preserve_missing_workspace")
        observation["lanes"]["atomic"]["worker_state"] = "unknown"
        self.assertEqual(self.action(observation), "reconcile_processes")
        observation = self.observed()
        observation["lanes"]["atomic"]["main"] = "c" * 40
        self.assertEqual(self.action(observation), "review_upstream")
        observation["lanes"]["atomic"]["assignment"] = "old-assignment"
        self.assertEqual(self.action(observation), "reconcile_worker")

    def artifact(self):
        (self.root / "candidate.patch").write_bytes(b"incomplete patch\n")
        return artifacts.pack(self.root, ["candidate.patch"], self.root / "recovery.tar.gz",
                              "a" * 40, recovery.checkpoint_identity(self.record(), "atomic"))

    def test_confirmed_backup_is_exact_and_becomes_stale_with_checkpoint(self):
        artifact = self.artifact()
        self.update(lambda r: recovery.backup(r, "atomic", artifact))
        self.assertFalse(recovery.protected(self.record(), "atomic"))
        self.update(lambda r: recovery.intent(r, "atomic", "upload-1", "mutation"))
        receipt = dict(status="succeeded", operation="upload-1", route="fake-approved-route",
                       file_id="saved-identity", version=0, sha256=artifact["sha256"], bytes=artifact["bytes"])
        invalid = dict(receipt, sha256="0" * 64)
        with self.assertRaises(RecoveryError):
            self.update(lambda r: recovery.backup(r, "atomic", artifact, invalid))
        self.update(lambda r: recovery.backup(r, "atomic", artifact, receipt))
        self.assertTrue(recovery.protected(self.record(), "atomic"))
        self.update(lambda r: recovery.checkpoint(r, "atomic", dict(status="incomplete", summary="new edits")))
        self.assertFalse(recovery.protected(self.record(), "atomic"))

    def test_corrupt_and_truncated_archive_are_not_backups(self):
        artifact = self.artifact()
        archive = self.root / "recovery.tar.gz"
        original = archive.read_bytes()
        for corrupt in [original[:25], original[:-30], original + b"changed"]:
            archive.write_bytes(corrupt)
            with self.assertRaises(RecoveryError):
                artifacts.verify(archive, artifact["sha256"])
        archive.write_bytes(original[:-8])
        with self.assertRaises(RecoveryError):
            artifacts.verify(archive)

    def test_incomplete_duplicate_and_unsafe_members_refused(self):
        self.artifact()
        archive = self.root / "recovery.tar.gz"
        with tarfile.open(archive) as valid:
            manifest = valid.extractfile("manifest.json").read()
        for names in [["manifest.json"], ["manifest.json", "manifest.json"], ["../escape"]]:
            with tarfile.open(archive, "w:gz") as broken:
                for name in names:
                    info = tarfile.TarInfo(name)
                    info.size = len(manifest)
                    broken.addfile(info, io.BytesIO(manifest))
            with self.assertRaises(RecoveryError):
                artifacts.verify(archive)

    def test_secret_paths_and_overwriting_saved_bundle_refused(self):
        (self.root / ".env").write_text("do not collect")
        with self.assertRaises(RecoveryError):
            artifacts.pack(self.root, [".env"], self.root / "bad.tar.gz", "base", "checkpoint")
        self.artifact()
        with self.assertRaises(FileExistsError):
            artifacts.pack(self.root, ["candidate.patch"], self.root / "recovery.tar.gz", "base", "checkpoint")

    def test_cleanup_requires_owned_landed_quiescent_complete_inventory(self):
        self.update(lambda r: recovery.checkpoint(r, "atomic", dict(status="validated", summary="focused gates recorded")))
        observation = self.observed()
        row = observation["lanes"]["atomic"]
        row.update(landed_head="b" * 40, landed_evidence="git: exact HEAD ancestor of fetched main")
        row["workspace"]["clean"] = True
        def cleanup(value):
            return recovery.plan(self.record(), value)["lanes"][0]["cleanup"]
        self.assertEqual(cleanup(observation), "eligible-owned-landed")
        self.assertEqual(recovery.plan(self.record(), observation)["lanes"][0]["action"], "review_cleanup")
        for key, value in [("worker_state", "running"), ("worker_state", "unknown"),
                           ("processes_stopped", False), ("claims", ["live"]),
                           ("landing_tickets", ["pending"]), ("resources_owned", False),
                           ("landed_head", "other"), ("landed_evidence", "")]:
            changed = copy.deepcopy(observation)
            changed["lanes"]["atomic"][key] = value
            self.assertEqual(cleanup(changed), "preserve", key)
        self.update(lambda r: recovery.checkpoint(r, "atomic", dict(status="blocked", summary="unbacked WIP")))
        self.assertEqual(cleanup(observation), "preserve")

    def test_bounded_local_command_timeout_preserves_uncertainty(self):
        result = recovery.bounded_command([sys.executable, "-c", "import time;time.sleep(20)"],
                                          self.root, self.root / "timeout.log", 0.1)
        self.assertEqual(result["process"], "unknown")
        self.assertEqual(result["result"], "interrupted")
        self.assertIsNone(result["exit"])

    def test_cli_records_intent_before_command_and_refuses_repeat(self):
        args = [sys.executable, str(ROOT / "tools/swarm/recovery.py"), "--state", str(self.store.path),
                "--revision", "1", "run", "--lane", "atomic", "--operation", "local-1", "--kind", "read",
                "--timeout", "5", "--cwd", str(self.root), "--", sys.executable, "-c", "print('local fixture')"]
        first = subprocess.run(args, capture_output=True, text=True, timeout=10)
        self.assertEqual(first.returncode, 0, first.stdout + first.stderr)
        self.assertEqual(self.record()["lanes"]["atomic"]["operations"]["local-1"]["exit"], 0)
        args[args.index("--revision") + 1] = str(self.record()["revision"])
        second = subprocess.run(args, capture_output=True, text=True, timeout=10)
        self.assertEqual(second.returncode, 1)
        self.assertIn("already recorded", second.stdout)

    def run_args(self, store, name, operation, kind="read", code="pass", timeout="5"):
        return [sys.executable, str(ROOT / "tools/swarm/recovery.py"), "--state", str(store.path),
                "--revision", str(store.load()["revision"]), "run", "--lane", name,
                "--operation", operation, "--kind", kind, "--timeout", timeout,
                "--cwd", str(self.root), "--", sys.executable, "-c", code]

    def test_concurrent_lane_update_does_not_lose_build_completion(self):
        self.update(lambda r: recovery.register(r, lane("peer")))
        code = "from pathlib import Path; import time; Path('ready').touch(); " + \
               "exec(\"while not Path('go').exists(): time.sleep(0.01)\"); Path('once').write_text('ran')"
        process = subprocess.Popen(self.run_args(self.store, "atomic", "build-1", "build", code),
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            deadline = time.monotonic() + 5
            while not (self.root / "ready").exists() and time.monotonic() < deadline:
                time.sleep(0.01)
            self.assertTrue((self.root / "ready").exists())
            self.update(lambda r: recovery.checkpoint(r, "peer", dict(status="blocked", summary="peer progress")))
            (self.root / "go").touch()
            stdout, stderr = process.communicate(timeout=10)
            self.assertEqual(process.returncode, 0, stdout + stderr)
        finally:
            if process.poll() is None:
                process.kill()
                process.communicate()
        op = self.record()["lanes"]["atomic"]["operations"]["build-1"]
        self.assertEqual((op["exit"], op["outcome"], op["local_result"]), (0, "succeeded", "exited"))
        self.assertIsInstance(op["pid"], int)
        self.assertIsNotNone(self.record()["build_slot"])
        revision = self.record()["revision"]
        recovery.complete(self.store, "atomic", "build-1")
        self.assertEqual(self.record()["revision"], revision)
        self.adapter.effects["build-1"] = dict(process="stopped", children_stopped=True, evidence="fake exact process tree")
        self.update(lambda r: recovery.reconcile(r, self.adapter.observe(r)))
        self.assertIsNone(self.record()["build_slot"])
        self.update(lambda r: recovery.intent(r, "atomic", "build-2", "build"))

    def test_durable_completion_survives_merge_failure_and_refuses_changed_identity(self):
        self.update(lambda r: recovery.intent(r, "atomic", "read-1", "read"))
        identity = recovery.execution_identity(self.record(), "atomic", "read-1")
        _, path = recovery.execution_paths(self.store, identity)
        atomic_json(path, dict(schema=1, identity=identity,
                    result=dict(result="exited", exit=7, process="stopped", pid=123)))
        with mock.patch.object(self.store, "update_guarded", side_effect=RecoveryError("busy")):
            with self.assertRaises(RecoveryError):
                recovery.complete(self.store, "atomic", "read-1")
        self.assertTrue(path.is_file())
        recovery.complete(self.store, "atomic", "read-1")
        self.assertEqual(self.record()["lanes"]["atomic"]["operations"]["read-1"]["exit"], 7)
        envelope = json.loads(path.read_text()); envelope["identity"]["generation"] = "another"
        atomic_json(path, envelope)
        with self.assertRaises(RecoveryError):
            recovery.complete(self.store, "atomic", "read-1")

    def test_timeout_without_exit_resolves_only_local_quiescence(self):
        for kind in ("build", "mutation"):
            with self.subTest(kind=kind):
                op = kind + "-timeout"
                result = subprocess.run(self.run_args(self.store, "atomic", op, kind,
                                                     "import time; time.sleep(20)", "0.05"),
                                        capture_output=True, text=True, timeout=10)
                self.assertEqual(result.returncode, 0, result.stdout)
                self.adapter.effects[op] = dict(process="stopped", children_stopped=True, evidence="exact stopped tree")
                self.update(lambda r: recovery.reconcile(r, self.adapter.observe(r)))
                value = self.record()["lanes"]["atomic"]["operations"][op]
                self.assertIsNone(value["exit"])
                self.assertEqual(value["outcome"], "unknown" if kind == "mutation" else "stopped_unknown_result")
                if kind == "mutation":
                    with self.assertRaises(RecoveryError):
                        self.update(lambda r: recovery.intent(r, "atomic", "no-retry", "mutation"))
                else:
                    self.assertIsNone(self.record()["build_slot"])
                    retry = subprocess.run(self.run_args(self.store, "atomic", "intentional-new-build", "build"),
                                           capture_output=True, text=True, timeout=10)
                    self.assertEqual(retry.returncode, 0, retry.stdout)
                    self.adapter.effects["intentional-new-build"] = dict(
                        process="stopped", children_stopped=True, evidence="new exact stopped tree")
                    self.update(lambda r: recovery.reconcile(r, self.adapter.observe(r)))

    def test_log_and_result_namespaces_cover_lane_and_ledger(self):
        self.update(lambda r: recovery.register(r, lane("peer")))
        second = Store(self.root / "second.json"); second.initialize("host-b", "boot-b")
        second.update(0, lambda r: recovery.register(r, lane()))
        paths = []
        for store, name in ((self.store, "atomic"), (self.store, "peer"), (second, "atomic")):
            result = subprocess.run(self.run_args(store, name, "inspect-1"), capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stdout)
            identity = recovery.execution_identity(store.load(), name, "inspect-1")
            log, receipt = recovery.execution_paths(store, identity)
            self.assertTrue(log.exists() and receipt.exists()); paths.append(log)
        self.assertEqual(len(set(paths)), 3)

    def test_known_pre_dispatch_failures_are_not_uncertain_effects(self):
        log = self.root / "exists.log"; log.touch()
        with mock.patch.object(recovery.subprocess, "Popen") as launch:
            result = recovery.bounded_command(["never"], self.root, log, 1)
            launch.assert_not_called()
        self.assertEqual(result["result"], "not_started")
        result = recovery.bounded_command([str(self.root / "absent-executable")], self.root,
                                           self.root / "new.log", 1)
        self.update(lambda r: recovery.intent(r, "atomic", "never-started", "mutation"))
        identity = recovery.execution_identity(self.record(), "atomic", "never-started")
        _, path = recovery.execution_paths(self.store, identity)
        atomic_json(path, dict(schema=1, identity=identity, result=result))
        recovery.complete(self.store, "atomic", "never-started")
        self.assertEqual(self.record()["lanes"]["atomic"]["operations"]["never-started"]["outcome"], "not_started")
        self.update(lambda r: recovery.intent(r, "atomic", "new-attempt", "mutation"))

    def test_backup_cannot_revive_by_omitting_head_or_cross_lanes(self):
        original = copy.deepcopy(self.record()["lanes"]["atomic"]["checkpoint"])
        artifact = self.artifact()
        self.update(lambda r: recovery.intent(r, "atomic", "upload", "mutation"))
        receipt = dict(operation="upload", status="succeeded", file_id="fake", version=0,
                       route="fake", sha256=artifact["sha256"], bytes=artifact["bytes"])
        self.update(lambda r: recovery.backup(r, "atomic", artifact, receipt))
        self.assertTrue(recovery.protected(self.record(), "atomic"))
        self.update(lambda r: recovery.register(r, lane("peer")))
        with self.assertRaises(RecoveryError):
            self.update(lambda r: recovery.backup(r, "peer", artifact))
        self.update(lambda r: recovery.checkpoint(r, "atomic", dict(status="incomplete", summary="new", head="c" * 40)))
        self.update(lambda r: recovery.checkpoint(r, "atomic", original))
        self.assertEqual(self.record()["lanes"]["atomic"]["head"], "c" * 40)
        self.assertFalse(recovery.protected(self.record(), "atomic"))
        with self.assertRaises(RecoveryError):
            self.update(lambda r: recovery.backup(r, "atomic", artifact, receipt))

    def test_concatenated_tar_and_nonpadding_trailing_content_rejected(self):
        self.artifact()
        path = self.root / "recovery.tar.gz"
        valid = gzip.decompress(path.read_bytes())
        tail = io.BytesIO()
        with tarfile.open(fileobj=tail, mode="w") as tar:
            info = tarfile.TarInfo("../unsafe"); info.size = 1; tar.addfile(info, io.BytesIO(b"x"))
        for suffix in (tail.getvalue(), b"unchecked bytes"):
            path.write_bytes(gzip.compress(valid + suffix))
            with self.assertRaises(RecoveryError): artifacts.verify(path)
        path.write_bytes(gzip.compress(valid + bytes(1024)))
        artifacts.verify(path)

    def test_fill_ledger_uses_atomic_writer(self):
        import fill
        with mock.patch.object(fill, "atomic_json") as writer:
            fill.save_ledger(self.root, "wave", {"slots": {}})
        writer.assert_called_once()


class SkillContracts(unittest.TestCase):
    def test_frontmatter_and_local_references(self):
        # No installed skill-creator validator is available in this executor.
        # Validate this repository's simple frontmatter and Markdown references.
        paths = [ROOT / ".agents/skills/dot-swarm/SKILL.md",
                 ROOT / ".agents/skills/devin-swarm/SKILL.md",
                 ROOT / ".agents/skills/codex-swarm/SKILL.md",
                 ROOT / ".agents/skills/cursor-swarm/SKILL.md",
                 ROOT / ".agents/skills/dot-swarm/references/recovery.md"]
        for path in paths:
            text = path.read_text(encoding="utf-8")
            if path.name == "SKILL.md":
                self.assertTrue(text.startswith("---\n"))
                header = text.split("---\n", 2)[1]
                fields = dict(line.split(":", 1) for line in header.splitlines() if line)
                self.assertEqual(set(fields), {"name", "description"})
                self.assertEqual(fields["name"].strip(), path.parent.name)
                self.assertTrue(fields["description"].strip())
            for target in re.findall(r"\]\(([^)]+)\)", text):
                if "://" in target or target.startswith("#"):
                    continue
                self.assertTrue((path.parent / target.split("#")[0]).exists(), (path, target))

    def test_dot_ui_metadata_preserved(self):
        content = (ROOT / ".agents/skills/dot-swarm/agents/openai.yaml").read_text()
        self.assertIn('display_name: "dot swarm"', content)
        self.assertIn('default_prompt: "Use $dot-swarm to coordinate a bounded Omega wave for the program or board scope I specify."', content)


if __name__ == "__main__":
    unittest.main()
