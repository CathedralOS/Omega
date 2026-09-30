#!/usr/bin/env python3
"""Local recovery journal and decision planner for existing swarm helpers.

Adapters observe runtime handles, Git/claims/landing and artifact receipts. This
tool persists their explicit evidence; it never calls a cloud API, retries an
operation, deletes a resource, or grants publication/claim authority.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time

from recovery_state import RecoveryError, Store, read_json
import recovery_artifacts

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import coordination


def identifier(value):
    if not isinstance(value, str) or not re.fullmatch(r"[A-Za-z0-9_-]{1,100}", value):
        raise RecoveryError("expected a stable safe identifier")
    return value


def fingerprint(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, allow_nan=False).encode()).hexdigest()


def register(record, lane):
    required = {"id", "assignment", "worktree", "branch", "base", "head", "reviewed_main",
                "worker_handle", "mode", "checkpoint", "resources"}
    if not isinstance(lane, dict) or not required <= lane.keys():
        raise RecoveryError("incomplete lane record")
    identifier(lane["id"])
    identifier(lane["assignment"])
    if (lane["mode"] not in {"diagnostic", "implementation"}
            or not isinstance(lane["checkpoint"], dict)
            or lane["checkpoint"].get("status") not in {"incomplete", "validated", "blocked"}
            or not lane["checkpoint"].get("summary")):
        raise RecoveryError("explicit checkpoint status and summary required")
    if lane["id"] in record["lanes"]:
        existing = record["lanes"][lane["id"]]
        if any(existing.get(key) != value for key, value in lane.items()):
            raise RecoveryError("lane already exists; preserve it and checkpoint explicitly")
        return
    record["lanes"][lane["id"]] = dict(lane, operations={}, backup=None)


def checkpoint(record, lane, value):
    if value.get("status") not in {"incomplete", "validated", "blocked"} or not value.get("summary"):
        raise RecoveryError("explicit checkpoint status and summary required")
    record["lanes"][lane]["checkpoint"] = value
    for key in ("head", "reviewed_main"):
        if key in value:
            record["lanes"][lane][key] = value[key]
    # Receipts for older bytes remain inventory, never protection for new work.


def intent(record, lane, operation, kind):
    identifier(operation)
    if kind not in {"read", "mutation", "build"}:
        raise RecoveryError("unknown operation kind")
    row = record["lanes"][lane]
    if operation in row["operations"]:
        raise RecoveryError("operation already recorded; reconcile it, never replay blindly")
    if kind != "read" and any(o["outcome"] == "unknown" for o in row["operations"].values()):
        raise RecoveryError("prior outcome unknown; reconcile before another operation")
    if kind == "build":
        if record["build_slot"] is not None:
            raise RecoveryError("executor build slot occupied or uncertain")
        record["build_slot"] = {"lane": lane, "operation": operation}
    row["operations"][operation] = dict(kind=kind, outcome="unknown", exit=None,
                                        started_utc=coordination.now(), process="unknown")


def finished(record, lane, operation, result):
    row = record["lanes"][lane]["operations"][operation]
    row.update(exit=result["exit"], process=result["process"],
               local_result=result["result"], pid=result.get("pid"))
    # An exited client does not prove a remote mutation failed or succeeded.
    if row["kind"] != "mutation" and result["process"] == "stopped":
        row["outcome"] = "succeeded" if result["exit"] == 0 else "failed"
    # A parent exit alone cannot release a build slot; children need observation.


def fresh(record, observation, now=None):
    try:
        stamp = datetime.fromisoformat(observation["observed_utc"])
        age = ((now or datetime.now(timezone.utc)) - stamp).total_seconds()
        return (observation["schema"] == 1 and 0 <= age <= 120
                and observation["executor"] == record["executor"]
                and observation["generation"] == record["generation"]
                and observation["availability"] == "available")
    except (KeyError, TypeError, ValueError):
        return False


def lane_observation(lane, observation):
    row = observation.get("lanes", {}).get(lane["id"], {})
    if (row.get("assignment") != lane["assignment"]
            or row.get("worker_handle") != lane["worker_handle"]):
        return {}
    return row


def reconcile(record, observation, now=None):
    """Accept one fresh read-only adapter snapshot, idempotently.

    Evidence is attested by the adapter, not manufactured from a PID or mtime.
    The saved snapshot is useful history, not fresh permission on a later run.
    """
    identifier(observation.get("id"))
    previous = record.get("observation")
    if previous and previous["id"] == observation["id"]:
        if previous != observation:
            raise RecoveryError("observation ID reused with different evidence")
        return
    if observation.get("revision") != record["revision"]:
        raise RecoveryError("observation targets a stale revision")
    if not fresh(record, observation, now):
        raise RecoveryError("executor unavailable, different generation, or stale observation")
    for lane in record["lanes"].values():
        seen = lane_observation(lane, observation)
        for operation, value in lane["operations"].items():
            result = seen.get("operations", {}).get(operation, {})
            if not result.get("evidence"):
                continue
            stopped = result.get("process") == "stopped" and result.get("children_stopped") is True
            if stopped:
                value["process"] = "stopped"
                if value["kind"] == "mutation":
                    if result.get("effect") in {"applied", "not_applied"}:
                        value["outcome"] = result["effect"]
                elif type(result.get("exit")) is int:
                    value["exit"] = result["exit"]
                    value["outcome"] = "succeeded" if result["exit"] == 0 else "failed"
                if record["build_slot"] == {"lane": lane["id"], "operation": operation}:
                    record["build_slot"] = None
    record["observation"] = observation


def backup(record, lane, artifact, receipt=None):
    row = record["lanes"][lane]
    if artifact["checkpoint"] != fingerprint(row["checkpoint"]) or artifact["base"] != row["base"]:
        raise RecoveryError("artifact does not describe this checkpoint/base")
    if receipt is not None:
        # Adapter must correlate an actual successful approved upload result with
        # the exact local bytes, not a guessed URL or another executor's path.
        operation = row["operations"].get(receipt.get("operation"), {})
        if (operation.get("kind") != "mutation"
                or receipt.get("status") != "succeeded" or not receipt.get("file_id")
                or receipt.get("version") is None or not receipt.get("route")
                or receipt.get("sha256") != artifact["sha256"]
                or receipt.get("bytes") != artifact["bytes"]):
            raise RecoveryError("backup receipt missing or does not match verified bytes")
        receipt = {key: receipt[key] for key in
                   ("operation", "status", "file_id", "version", "route", "sha256", "bytes")}
    row["backup"] = dict(artifact, receipt=receipt,
                         status="confirmed-upload" if receipt is not None else "local-only")


def protected(lane):
    saved = lane.get("backup") or {}
    return (saved.get("status") == "confirmed-upload"
            and saved.get("checkpoint") == fingerprint(lane["checkpoint"])
            and saved.get("base") == lane["base"])


def plan(record, observation=None, now=None):
    persisted = observation is None
    observation = (record.get("observation") or {}) if persisted else observation
    available = (fresh(record, observation, now)
                 and observation.get("revision") == record["revision"] - int(persisted))
    rows = []
    for lane in record["lanes"].values():
        seen = lane_observation(lane, observation) if available else {}
        unknown = [key for key, operation in lane["operations"].items()
                   if operation["outcome"] == "unknown"]
        worker = seen.get("worker_state", "unknown")
        workspace = seen.get("workspace", {})
        resume_id = "resume-" + lane["assignment"]
        action = "reconcile_executor"
        if available:
            if not seen:
                action = "reconcile_worker"
            elif unknown:
                action = "reconcile_operation"
            elif worker == "running":
                action = "wait_running"
            elif worker not in {"idle", "stopped"} or seen.get("processes_stopped") is not True:
                action = "reconcile_processes"
            elif workspace.get("exists") is False:
                action = "materialize_verify_backup" if protected(lane) else "preserve_missing_workspace"
            elif workspace.get("exists") is not True or workspace.get("head") != lane["head"]:
                action = "review_worktree"
            elif seen.get("main") != lane["reviewed_main"]:
                action = "review_upstream"
            elif seen.get("ownership") != "clear":
                action = "reconcile_claims"
            elif record["build_slot"] is not None:
                action = "reconcile_build_slot"
            elif resume_id in lane["operations"]:
                action = "review_resume_result"
            else:
                action = "offer_resume"
        cleanup = (available and bool(seen) and worker in {"idle", "stopped"}
                   and lane["checkpoint"]["status"] == "validated"
                   and seen.get("processes_stopped") is True and not unknown
                   and record["build_slot"] is None and workspace.get("exists") is True
                   and workspace.get("head") == lane["head"]
                   and workspace.get("clean") is True and workspace.get("primary") is False
                   and seen.get("landed_head") == lane["head"]
                   and bool(seen.get("landed_evidence"))
                   and seen.get("claims") == [] and seen.get("landing_tickets") == []
                   and seen.get("resources_owned") is True)
        if cleanup:
            action = "review_cleanup"
        rows.append(dict(lane=lane["id"], action=action, resume_intent=resume_id,
                         unknown_operations=unknown,
                         backup="confirmed-upload" if protected(lane) else "unprotected-current-checkpoint",
                         checkpoint=lane["checkpoint"]["status"],
                         cleanup="eligible-owned-landed" if cleanup else "preserve",
                         resources=dict(worktree=lane["worktree"], branch=lane["branch"],
                                        worker_handle=lane["worker_handle"],
                                        claims=seen.get("claims", "unknown"),
                                        landing_tickets=seen.get("landing_tickets", "unknown"),
                                        declared=lane["resources"])))
    return dict(revision=record["revision"], executor=record["executor"], lanes=rows,
                build_slot=record["build_slot"], executes_actions=False,
                provider_lifecycle="provider-managed; no deletion or keep-alive capability inferred")


def bounded_command(arguments, cwd, log, timeout):
    """One invocation only; all timeouts retain uncertain child/mutation state.

    Never persist argv/environment: an adapter may carry private credentials.
    Logs stay local and require review before any backup. Windows kills only
    this parent; POSIX also signals its new process group. Detached children
    remain possible on either platform, so cancellation proves no quiescence.
    """
    if not arguments or not 0 < timeout <= 3600:
        raise RecoveryError("command needs argv and a bounded timeout (0, 3600]")
    started = time.monotonic()
    with Path(log).open("xb") as output:
        try:
            process = subprocess.Popen(arguments, cwd=cwd, stdin=subprocess.DEVNULL,
                                       stdout=output, stderr=subprocess.STDOUT,
                                       start_new_session=os.name != "nt")
        except OSError:
            return dict(result="not_started", exit=None, process="stopped", pid=None)
        try:
            code = process.wait(timeout=timeout)
        except (subprocess.TimeoutExpired, KeyboardInterrupt):
            if os.name != "nt":
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            else:
                process.kill()
            try:
                process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                pass
            return dict(result="interrupted", exit=None, process="unknown", pid=process.pid,
                        elapsed_seconds=time.monotonic() - started)
    return dict(result="exited", exit=code, process="stopped", pid=process.pid,
                elapsed_seconds=time.monotonic() - started)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--state", type=Path, required=True)
    parser.add_argument("--revision", type=int, help="required for every existing-state write")
    commands = parser.add_subparsers(dest="command", required=True)
    init = commands.add_parser("init")
    init.add_argument("--executor", required=True)
    init.add_argument("--generation", required=True)
    commands.add_parser("status")
    for name in ("register", "reconcile"):
        commands.add_parser(name).add_argument("--input", type=Path, required=True)
    check = commands.add_parser("checkpoint")
    check.add_argument("--lane", required=True)
    check.add_argument("--input", type=Path, required=True)
    for name in ("intent", "run"):
        command = commands.add_parser(name)
        command.add_argument("--lane", required=True)
        command.add_argument("--operation", required=True)
        command.add_argument("--kind", choices=("read", "mutation", "build"), required=True)
        if name == "run":
            command.add_argument("--timeout", type=float, required=True)
            command.add_argument("--cwd", type=Path, required=True)
            command.add_argument("argv", nargs=argparse.REMAINDER)
    archive = commands.add_parser("backup")
    archive.add_argument("--lane", required=True)
    archive.add_argument("--artifact", type=Path, required=True)
    archive.add_argument("--sha256", required=True)
    archive.add_argument("--receipt", type=Path)
    bundle = commands.add_parser("bundle")
    bundle.add_argument("--lane", required=True)
    bundle.add_argument("--root", type=Path, required=True)
    bundle.add_argument("--output", type=Path, required=True)
    bundle.add_argument("--file", action="append", required=True)
    args = parser.parse_args(argv)
    try:
        store = Store(args.state)
        if args.command == "init":
            result = store.initialize(args.executor, args.generation)
        elif args.command == "status":
            result = plan(store.load())
        else:
            if args.revision is None:
                raise RecoveryError("--revision required; read status first")
            if args.command == "register":
                result = store.update(args.revision, lambda r: register(r, read_json(args.input)))
            elif args.command == "checkpoint":
                result = store.update(args.revision, lambda r: checkpoint(r, args.lane, read_json(args.input)))
            elif args.command == "reconcile":
                result = store.update(args.revision, lambda r: reconcile(r, read_json(args.input)))
            elif args.command == "backup":
                artifact = recovery_artifacts.verify(args.artifact, args.sha256)
                receipt = read_json(args.receipt) if args.receipt else None
                result = store.update(args.revision, lambda r: backup(r, args.lane, artifact, receipt))
            elif args.command == "bundle":
                current = store.load()
                if current["revision"] != args.revision:
                    raise RecoveryError("stale revision before bundle")
                lane = current["lanes"][args.lane]
                artifact = recovery_artifacts.pack(args.root, args.file, args.output,
                                                   lane["base"], fingerprint(lane["checkpoint"]))
                result = store.update(args.revision, lambda r: backup(r, args.lane, artifact))
            else:
                result = store.update(args.revision, lambda r: intent(r, args.lane, args.operation, args.kind))
                if args.command == "run":
                    command = args.argv[1:] if args.argv[:1] == ["--"] else args.argv
                    log = args.state.parent / (args.operation + ".log")
                    completed = bounded_command(command, args.cwd, log, args.timeout)
                    result = store.update(result["revision"], lambda r: finished(r, args.lane, args.operation, completed))
        print(json.dumps(result, indent=2))
        return 0
    except (RecoveryError, OSError, KeyError, TypeError, ValueError) as error:
        print(json.dumps(dict(error=str(error), action="preserve-and-reconcile")))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
