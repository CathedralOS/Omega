#!/usr/bin/env python3
"""One existing owner's context continuation without fabricated managed history.

Prepare validates recorded-HEAD contract context; reserve requires a fresh actual
thread read and precedes the sole external invocation. No create, rollout credit,
clean-candidate acceptance or historical delivery is inferred by this journal.
"""

import argparse
import copy
import hashlib
import json
from pathlib import Path
import sys
import time
import uuid

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import coordination
import qualification
from cloud import canonical_ancestor, fresh, now
from recovery_state import RecoveryError, atomic_json, exclusive, read_json

require = qualification.require
SCHEMA = "legacy-spec-continuation-1"


def context(value):
    repository = Path(value["repository"]).resolve()
    packet_path = Path(value["owner_packet"]).resolve()
    packet_bytes = packet_path.read_bytes()
    require(hashlib.sha256(packet_bytes).hexdigest() == value["owner_packet_sha256"], "owner packet bytes changed")
    owner = json.loads(packet_bytes)
    require(owner.get("schema") == "qualification-handoff-1" and owner.get("role") in {"implementation", "continuation"}
            and owner.get("canonical_context"),
            "legacy compiler continuation requires the owner's actual focused semantic packet")
    require(qualification.git(repository, "rev-parse", "HEAD") == owner["candidate"], "recorded owner HEAD differs from context checkout")
    mappings = [{key: item for key, item in row.items() if key not in {"revision", "blob"}}
                for row in owner["canonical_context"]]
    selected = [row["contract"] for row in mappings]
    require(not qualification.git(repository, "status", "--porcelain", "--untracked-files=all", "--", *selected),
            "selected spec files are dirty")
    packet = qualification.work_handoff(repository, dict(semantic_contracts=mappings, checks=owner.get("checks", [])),
                                        owner["source_revision"], "continuation")
    require(packet["canonical_context"] == owner["canonical_context"]
            and packet["context_fingerprint"] == owner.get("context_fingerprint"), "owner context identities are stale or corrupt")
    # This reports only the context checkout, which may be a parent snapshot.
    return packet, qualification.git(repository, "status", "--porcelain", "--untracked-files=all")


def render_request(value, packet, operation, execution):
    prompt = qualification.work_context_prompt(packet) + "\nLegacy continuation of existing outcome: " + value["outcome"]
    prompt += ("\nHistorical controller/create/delivery state is unavailable. This bootstrap adds no new semantic authority, "
               "managed rollout credit or final-candidate acceptance. Recorded HEAD and changed_paths describe committed source only. "
               "The owner's reported worktree status is " + value["owner_worktree_status"] + ". Preserve all existing changes and "
               "pending owner decisions. Reconcile owned processes and recovery before starting commands; do not duplicate a build. "
               "Continue the same authorized outcome through its existing checks, independent review and normal landing.\n"
               "Acknowledge operation " + operation + " / execution " + execution + " in the bounded turn result.\n")
    return dict(action="follow_up", tool="cloud_threads.send_message", operation=operation, execution_id=execution,
                   thread=value["thread"], work_context=packet, prompt=prompt,
                   runtime_arguments=dict(threadId=value["thread"], model="gpt-6.1-sol", thinking="medium", prompt=prompt))


def prepare(path, value):
    value = copy.deepcopy(value)
    value["repository"] = str(Path(value["repository"]).resolve())
    value["owner_packet"] = str(Path(value["owner_packet"]).resolve())
    require(isinstance(value.get("thread"), str) and value["thread"].strip()
            and value.get("binding_evidence") and value.get("authorization_evidence")
            and value.get("outcome"), "explicit existing thread binding and existing-outcome authorization required")
    require(value.get("owner_worktree_status") in {"uncommitted", "clean", "unknown"}, "actual owner worktree status or explicit unknown required")
    packet, status = context(value)
    packet["source_scope"] = "recorded_HEAD_contract_context_only"
    deadline = time.monotonic() + 30
    url = coordination.push_url(value["repository"], "origin", deadline=deadline)
    accepted = coordination.remote_refs(value["repository"], url, "refs/heads/main", deadline=deadline).get("refs/heads/main")
    require(accepted, "accepted main authority unavailable")
    coordination.fetch_object(value["repository"], url, accepted, reference="refs/heads/main", deadline=deadline)
    require(canonical_ancestor(value["repository"], packet["source_revision"], accepted),
            "owner packet source is not accepted canonical main authority")
    request = render_request(value, packet, uuid.uuid4().hex, uuid.uuid4().hex)
    record = dict(schema=SCHEMA, revision=0, phase="prepared", input=copy.deepcopy(value), request=request,
                  accepted_main=accepted, prepared_utc=now(), historical_state="unavailable",
                  prior_context_retained=False, managed_credit=False, final_candidate_acceptance=False,
                  context_checkout_status=status, owner_worktree_status=value["owner_worktree_status"], result=None)
    with exclusive(str(path) + ".lock"):
        require(not path.exists(), "legacy journal already exists; preserve and inspect it, never reset")
        atomic_json(path, record)
    return record


def load(path):
    record = read_json(path)
    require(record.get("schema") == SCHEMA and type(record.get("revision")) is int, "unsupported legacy journal")
    return record


def mutate(path, expected, command, value):
    with exclusive(str(path) + ".lock"):
        record = load(path)
        require(record["revision"] == expected, "stale legacy revision; reread, do not overwrite")
        if command == "reserve":
            require(record["phase"] == "prepared", "already reserved; uncertain invocation must be reconciled, never replayed")
            require(value.get("tool") == "cloud_threads.read" and value.get("evidence") and fresh(value), "fresh actual supported thread read required")
            raw = value["structuredContent"]
            turn = raw.get("latestTurn") or {}
            require(raw.get("threadId") == record["request"]["thread"] and turn.get("id")
                    and turn.get("status") == "completed" and turn.get("error") is None,
                    "exact existing completed thread required; preserve running/approval blockers")
            packet, status = context(record["input"])
            packet["source_scope"] = "recorded_HEAD_contract_context_only"
            require(render_request(record["input"], packet, record["request"]["operation"], record["request"]["execution_id"])
                    == record["request"], "prepared context/prompt changed; do not dispatch stale context")
            record.update(phase="reserved", observation=copy.deepcopy(value), reserved_utc=now(), context_checkout_status=status)
        else:
            require(record["phase"] == "reserved" and value.get("operation") == record["request"]["operation"]
                    and value.get("execution_id") == record["request"]["execution_id"] and value.get("evidence")
                    and value.get("effect") in {"applied", "not_applied", "unknown"}, "exact actual reserved invocation result required")
            if value["effect"] == "applied":
                require(value.get("result", {}).get("threadId") == record["request"]["thread"], "actual enqueue result thread mismatch")
            record.update(phase="recorded", result=copy.deepcopy(value))
        record["revision"] += 1
        atomic_json(path, record)
        return record


def main(arguments=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("prepare", "reserve", "record-result", "status"))
    parser.add_argument("--state", type=Path, required=True)
    parser.add_argument("--revision", type=int)
    parser.add_argument("--input", type=Path)
    options = parser.parse_args(arguments)
    try:
        if options.command == "status":
            record = load(options.state)
        else:
            require(options.input, "actual input JSON required")
            value = read_json(options.input)
            if options.command == "prepare":
                record = prepare(options.state, value)
            else:
                require(options.revision is not None, "current journal --revision required")
                record = mutate(options.state, options.revision, options.command, value)
        print(json.dumps(dict(revision=record["revision"], phase=record["phase"],
                             action="invoke_once" if options.command == "reserve" else "inspect_only",
                             request=record["request"], historical_state=record["historical_state"],
                             owner_worktree_status=record["owner_worktree_status"], managed_credit=False,
                             final_candidate_acceptance=False), indent=2))
        return 0
    except (RecoveryError, coordination.CoordinationError, OSError, KeyError, TypeError, ValueError) as error:
        print(json.dumps(dict(error=str(error), action="preserve-and-reconcile")))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
