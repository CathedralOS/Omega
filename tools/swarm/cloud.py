#!/usr/bin/env python3
"""Durable cloud assignments and tool-adapter requests in the recovery journal.

No transport/daemon is invented here. The parent invokes currently exposed tools
once after intent, then ingests sanitized correlated results. Uncertain mutations
require authoritative reads. Claims v1 tracks outcome ownership; the landing queue serializes publication.
"""

import argparse
import copy
from datetime import datetime, timezone
import json
import re
from pathlib import Path
import sys
import subprocess
import time
import uuid

from recovery_state import Store, RecoveryError, read_json
from recovery import identifier, fingerprint
from completion import require, sha, validate, git, inputs, artifact, program_failed

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import claims
import qualification
import coordination


def now():
    return datetime.now(timezone.utc).isoformat()


def fresh(value, clock=None):
    try:
        age = ((clock or datetime.now(timezone.utc)) - datetime.fromisoformat(value["observed_utc"])).total_seconds()
        return 0 <= age <= 120
    except (KeyError, TypeError, ValueError):
        return False


def paths(values):
    require(isinstance(values, list), "paths must be a list")
    return [claims.normalize_path(value) for value in values]


def initialize(record, capabilities):
    require("cloud" not in record, "cloud controller already initialized")
    require(capabilities.get("evidence") and isinstance(capabilities.get("tools"), dict),
            "inspect actual exposed tool schemas before declaring adapter capabilities")
    if capabilities.get("mode") == "observational-only":
        require(not record["lanes"] and record["build_slot"] is None,
                "observational bootstrap cannot adopt executor resources or recovery lanes")
        require(set(capabilities["tools"]) == {"read"} and capabilities["tools"].get("read")
                and capabilities.get("reporting", {}).get("mode") == "recorded-thread-items"
                and capabilities["reporting"].get("evidence"), "observational controller requires only actual read capability")
        record["cloud"] = dict(capabilities=capabilities, batches={}, assignments={}, handoffs={}, operations={})
        return
    require(capabilities.get("mode", "managed") == "managed", "unsupported controller mode")
    require(capabilities.get("reporting", {}).get("mode") == "bounded-turn-results"
            and type(capabilities["reporting"].get("max_turn_seconds")) is int
            and 0 < capabilities["reporting"]["max_turn_seconds"] <= 1800
            and capabilities["reporting"].get("evidence"),
            "parent-visible bounded turn/result reporting contract required")
    for action in ("create", "read", "follow_up"):
        require(capabilities["tools"].get(action), "missing runtime capability: " + action)
    record["cloud"] = dict(capabilities=capabilities, batches={}, assignments={}, handoffs={}, operations={})


def observe_existing(record, value):
    """Bind only a reported existing thread, without a compiler or host claim."""
    cloud = record["cloud"]
    require(cloud["capabilities"].get("mode") == "observational-only", "observational-only controller required")
    require(set(value) == {"id", "thread", "evidence"} and value.get("evidence")
            and isinstance(value.get("thread"), str) and value["thread"].strip(), "actual existing thread and binding evidence required")
    identifier(value["id"])
    existing = cloud["assignments"].get(value["id"])
    if existing:
        require(existing["thread"] == value["thread"] and existing["binding_evidence"] == value["evidence"],
                "conflicting observational binding")
        return
    require(not any(row["thread"] == value["thread"] for row in cloud["assignments"].values()),
            "duplicate observational thread binding")
    cloud["assignments"][value["id"]] = dict(id=value["id"], mode="observational-only", thread=value["thread"],
        binding_evidence=value["evidence"], environment=None, generation=None, runtime="unknown", checkpoint=None,
        observation=None, accepted=None)


def admit_preflight(record, value):
    """Authorize one inspection turn that establishes unknown executor evidence."""
    cloud = record["cloud"]
    require(cloud["capabilities"].get("mode") == "observational-only", "separate observational bootstrap required")
    require(set(value) == {"id", "observed_assignment", "observed_operation", "expected_turn", "tool",
                           "schema_evidence", "authorization_evidence", "lane"}
            and value.get("schema_evidence") and value.get("authorization_evidence"),
            "explicit actual preflight schema and parent authorization required")
    identifier(value["id"])
    require(value["tool"] == "cloud_threads.send_message" and value["lane"] == "frontend resolution",
            "supported supplied preflight tool/lane required")
    digest = fingerprint(value)
    old = cloud["assignments"].get(value["id"])
    if old:
        require(old.get("preflight_contract_digest") == digest, "conflicting preflight assignment")
        return
    observed = cloud["assignments"][value["observed_assignment"]]
    require(observed.get("mode") == "observational-only" and observed.get("observation"),
            "actual prior thread observation required")
    raw = observed["observation"]["structuredContent"]
    turn = raw.get("latestTurn") or {}
    require(raw["threadId"] == observed["thread"] and turn.get("id") == value["expected_turn"]
            and turn.get("status") == "completed" and turn.get("error") is None,
            "exact reported completed turn required before preflight")
    source = cloud["operations"][value["observed_operation"]]
    require(source["assignment"] == observed["id"] and source["request"]["action"] == "read"
            and source["effect"] == "applied" and source.get("receipt") and source["result"] == raw,
            "preflight source must be the exact recorded actual read")
    require(not observed.get("approval_required"), "preflight cannot clear a known approval denial")
    require(not any(row.get("mode") == "preflight-only" and row["thread"] == observed["thread"]
                    for row in cloud["assignments"].values()), "one-shot preflight already assigned to this thread")
    cloud["assignments"][value["id"]] = dict(id=value["id"], mode="preflight-only", thread=observed["thread"],
        binding_evidence=observed["binding_evidence"], environment=None, generation=None, runtime="unknown",
        checkpoint=None, observation=None, accepted=None, preflight_contract=copy.deepcopy(value), preflight_contract_digest=digest)


def preflight_prompt(operation, execution_id):
    return ("Read AGENTS.md and the current /advance skill. Read-only preflight/scouting on current main: inspect claims, "
        "actual executor/environment/generation, resource limits, processes, recovery operations and build-slot state. "
        "Report actual evidence and approval/authority blockers; preserve them. Retain the authorized outcome if one is "
        "already held; otherwise scout an unclaimed defect within the authorized frontend resolution lane. State its "
        "authored oracle, refusal controls and next gate. No builds, source edits, claim mutations or "
        "publication this turn. Preserve held work and actual live ownership. Stop with a preflight report "
        "acknowledging operation " + operation + " / execution " + execution_id + ". Implementation requires separate "
        "authorization, actual readiness and selected-contract admission. Retain the assignment's required independent "
        "review and publication scope; this inspection adds neither authority nor a reviewer or sign-off gate.")


def preflight_intent(record, row, action, operation, parameters):
    cloud = record["cloud"]
    require(action == "follow_up" and parameters == dict(model="gpt-6.1-sol", reasoning="medium", phase="preflight"),
            "preflight-only assignment cannot dispatch implementation or another action")
    require(not row.get("approval_required"), "preflight cannot clear a known approval denial")
    require(not any(op["assignment"] == row["id"] and op["request"]["action"] == "follow_up"
                    for op in cloud["operations"].values()), "one-shot preflight already has an intent; reconcile")
    value = row["preflight_contract"]
    source = cloud["operations"][value["observed_operation"]]
    observed = cloud["assignments"][value["observed_assignment"]]
    require(not observed.get("approval_required"), "preflight cannot clear a known approval denial")
    current = (observed.get("observation") or {}).get("structuredContent") or {}
    turn = current.get("latestTurn") or {}
    require(source["effect"] == "applied" and source["result"]["latestTurn"]["id"] == value["expected_turn"]
            and current.get("threadId") == row["thread"] and turn.get("id") == value["expected_turn"]
            and turn.get("status") == "completed" and turn.get("error") is None,
            "preflight source observation changed")
    execution_id = uuid.uuid4().hex
    prompt = preflight_prompt(operation, execution_id)
    request = dict(schema=1, operation=operation, execution_id=execution_id, assignment=row["id"], action=action,
        tool=value["tool"], thread=row["thread"], environment=None, generation=None, parameters=copy.deepcopy(parameters),
        runtime_arguments=dict(threadId=row["thread"], model="gpt-6.1-sol", thinking="medium", prompt=prompt),
        contract=dict(id=row["id"], mode="preflight-only", authorization=copy.deepcopy(value)),
        readiness_contract=None, readiness_gate=None, dependencies=[], reporting=cloud["capabilities"]["reporting"], created_utc=now())
    cloud["operations"][operation] = dict(request=request, assignment=row["id"], effect="unknown", result=None)
    return request


def admit(record, manifest, snapshot, *, rolling_from=None):
    cloud = record["cloud"]
    require(cloud.get("capabilities", {}).get("mode") != "observational-only", "observational controller cannot admit compiler work")
    identifier(manifest["id"])
    require(manifest["id"] not in cloud["batches"], "batch already admitted")
    size = manifest["size"]
    mode = manifest.get("mode", "staged")
    require(mode in {"staged", "non-promoting-pilot"} or mode == "rolling-successor" and rolling_from is not None,
            "explicit supported batch mode required")
    require(type(size) is int and (size in {2, 5, 10} if mode == "staged" else size == 1),
            "bounded 2/5/10 batch or explicit non-promoting pilot required")
    require(len(manifest["assignments"]) == size, "batch size differs from assignments")
    require(manifest.get("dependency_review") and manifest.get("publication") == "serialized-landing",
            "complete producer/consumer and shared-file review plus serialized landing required")
    require(fresh(snapshot) and snapshot.get("evidence") and snapshot.get("state") != "unavailable"
            and isinstance(snapshot.get("claims"), list), "fresh actual claims snapshot required")
    if rolling_from is not None:
        prior = cloud["assignments"][rolling_from]
        accepted = prior.get("accepted") or {}
        require(mode == "rolling-successor" and prior.get("binding") and not prior.get("successor")
                and accepted.get("result") == "landed" and accepted.get("cleanup") == "completed"
                and accepted.get("credit") == "managed" and accepted.get("user_rescue") is False,
                "rolling admission requires an unused managed settled slot")
    previous = manifest.get("previous")
    if mode == "staged" and size != 2:
        require(previous in cloud["batches"] and cloud["batches"][previous]["manifest"]["size"] == {5: 2, 10: 5}[size],
                "previous staged batch missing")
        require(batch_complete(record, previous), "previous batch has not completed without user rescue")
    else:
        require(previous is None, "canary must start staged rollout")
    rows = {row["id"]: copy.deepcopy(row) for row in manifest["assignments"]}
    require(len(rows) == size and not set(rows) & cloud["assignments"].keys(), "duplicate assignment identity")
    require(len({row["outcome"] for row in rows.values()}) == size, "each session needs a distinct real outcome")
    require(len({row["item"] for row in rows.values()}) == size, "one DRI per outcome item")
    for row in rows.values():
        row.pop("binding", None)  # Session provenance enters only through a verified binding.
        row.pop("mode", None)  # Read-only provenance enters only through observe-existing.
        for field in ("source_revision", "revisions", "preflight_history", "scouting", "readiness", "successor", "successor_receipt", "canonical_authority"):
            row.pop(field, None)
        identifier(row["id"])
        sha(row["base"])
        require(row.get("item") and row.get("owner") and row.get("outcome") and row.get("dependency_review") and row.get("acceptance"), "incomplete assignment")
        require(row.get("board") is None or row["board"] in claims.BOARDS, "supported board required")
        row["writes"] = paths(row["writes"])
        row["reads"] = paths(row["reads"])
        require(row["writes"], "implementation ownership required")
        require(type(row.get("max_continuations")) is int and 0 <= row["max_continuations"] <= 12,
                "explicit continuation bound must be 0..12")
        require(len({requirement["id"] for requirement in row["acceptance"]}) == len(row["acceptance"]),
                "duplicate acceptance requirement ID")
        require(type(row.get("retry_budget")) is int and 0 <= row["retry_budget"] <= 3, "retry budget must be 0..3")
        require({r["kind"] for r in row["acceptance"]} >= {"native", "interpreter", "negative", "tests"},
                "complete acceptance commands required before dispatch")
        for requirement in row["acceptance"]:
            require(requirement.get("command") and requirement.get("inputs") and requirement.get("verdict")
                    and type(requirement.get("exit")) is int, "incomplete acceptance requirement")
            paths(requirement["inputs"])
            if requirement["kind"] in {"native", "interpreter"}:
                require(isinstance(requirement.get("expected_result"), dict) and requirement["expected_result"],
                        "authored program result oracle required")
        for live in snapshot["claims"]:
            if live["ticket"] == row.get("claim_ticket"):
                require(live["owner"] == row.get("owner") and set(live["paths"]) == set(row["writes"])
                        and live.get("item") == row["item"] and live.get("board") == row.get("board"),
                        "existing own claim differs from exact assignment")
                continue
            require(live.get("item") != row["item"],
                    "outcome already owned; explicit handoff required")
        for active in cloud["assignments"].values():
            require(active.get("accepted") and active["accepted"].get("cleanup") == "completed"
                    or active["item"] != row["item"],
                    "inflight outcome already assigned")
    dependencies = manifest["dependencies"]
    ids = set()
    graph = {name: set() for name in rows}
    for dependency in dependencies:
        identifier(dependency["id"])
        require(dependency["id"] not in ids and not any(dependency["id"] == prior["id"]
                    for batch in cloud["batches"].values() for prior in batch["manifest"]["dependencies"]),
                "duplicate dependency ID")
        ids.add(dependency["id"])
        require(dependency["producer"] in rows and dependency["consumer"] in rows
                and dependency["producer"] != dependency["consumer"], "dependency needs exact producer/consumer")
        require(dependency["kind"] in {"producer-consumer", "digest", "golden", "shared"}
                and dependency.get("verification"), "dependency verification required")
        dependency_paths = paths(dependency["paths"])
        require(dependency_paths, "empty dependency paths")
        for path in dependency_paths:
            require(any(claims.paths_overlap(path, p) for p in rows[dependency["producer"]]["writes"])
                    and any(claims.paths_overlap(path, p) for p in rows[dependency["consumer"]]["reads"]),
                    "dependency must connect actual producer write to consumer read")
        graph[dependency["consumer"]].add(dependency["producer"])
    pending = copy.deepcopy(graph)
    while pending:
        ready = [key for key, values in pending.items() if not values & pending.keys()]
        require(ready, "dependency cycle; partition or designate integration owner")
        for key in ready:
            del pending[key]
    # Shared files do not imply a semantic dependency. The reviewed dependency
    # graph records required producer results, not exclusive path reservations.
    require(rolling_from is not None or all(batch_settled(record, batch) for batch in cloud["batches"]),
            "unfinished batch remains; do not overlap staged batches")
    cloud["batches"][manifest["id"]] = dict(manifest=copy.deepcopy(manifest), admitted_utc=now(), claims=snapshot)
    for row in rows.values():
        cloud["assignments"][row["id"]] = dict(row, batch=manifest["id"], thread=None, environment=None,
                generation=None, runtime="unknown", observation=None, checkpoint=None, attempts={}, accepted=None)


def successor(record, value):
    """Atomically assign one reviewed outcome to one accepted, settled slot."""
    cloud = record["cloud"]
    prior = cloud["assignments"][value["predecessor"]]
    digest = fingerprint(value)
    if prior.get("successor"):
        require(prior.get("successor_receipt") == digest, "conflicting successor for settled slot")
        return
    accepted = prior.get("accepted") or {}
    require(accepted.get("result") == "landed" and accepted.get("cleanup") == "completed"
            and accepted.get("credit") == "managed" and accepted.get("user_rescue") is False,
            "rolling reuse requires managed settlement")
    manifest = value["manifest"]
    require(manifest.get("mode") == "rolling-successor" and manifest.get("size") == 1,
            "rolling successor must replace exactly one slot without stage promotion")
    binding = value["binding"]
    receipt = binding["receipt"]
    require(receipt["prior"]["assignment"] == prior["id"]
            and (prior.get("binding") or {}).get("source") == receipt["source"],
            "successor must reuse same recorded slot and predecessor")
    require(len(manifest["assignments"]) == 1 and manifest["assignments"][0]["id"] == binding["assignment"],
            "successor binding differs from reviewed assignment")
    admit(record, manifest, value["claims"], rolling_from=prior["id"])
    bind_existing(record, binding)
    prior.update(successor=binding["assignment"], successor_receipt=digest)


def dependencies(record, assignment):
    cloud = record["cloud"]
    row = cloud["assignments"][assignment]
    return [d for d in cloud["batches"][row["batch"]]["manifest"]["dependencies"] if d["consumer"] == assignment]


def handoff(record, value):
    cloud = record["cloud"]
    producer = cloud["assignments"][value["producer"]]
    consumer = cloud["assignments"][value["consumer"]]
    dependency = next((d for d in dependencies(record, consumer["id"]) if d["id"] == value["dependency"]), None)
    require(dependency and dependency["producer"] == producer["id"], "handoff dependency mismatch")
    require(value.get("paths") == dependency["paths"] and value.get("verification") == dependency["verification"],
            "handoff path/verification differs from partition")
    sha(value["candidate"])
    require(value.get("producer_receipt") and value.get("consumer_receipt") and value.get("verification_evidence")
            and value.get("claim_evidence") and value.get("ownership") in {"single-writer", "reconciled"}
            and value.get("producer_ticket") and value.get("consumer_ticket"), "auditable bilateral handoff required")
    require(fresh(value), "stale handoff; observe exact claims/candidate again")
    require(producer.get("claim_ticket") == value["producer_ticket"]
            and consumer.get("claim_ticket") == value["consumer_ticket"], "handoff exact claim tickets mismatch")
    old = cloud["handoffs"].get(value["dependency"])
    require(old is None or old == value, "handoff already recorded; new dependency leg required")
    cloud["handoffs"][value["dependency"]] = value


def ownership(record, assignment, snapshot):
    row = record["cloud"]["assignments"][assignment]
    require(fresh(snapshot) and snapshot.get("evidence") and snapshot.get("state") != "unavailable",
            "fresh claims read required")
    live = next((c for c in snapshot["claims"] if c["ticket"] == snapshot["ticket"]), None)
    require(live and live["owner"] == row["owner"] and set(live["paths"]) == set(row["writes"])
            and live.get("item") == row["item"] and live.get("board") == row.get("board"),
            "exact owner/path claim receipt required")
    require(row.get("claim_ticket") in {None, live["ticket"]}, "claim identity changed; explicit new leg required")
    require(not any(c["ticket"] != live["ticket"] and
                c.get("item") == row["item"]
                for c in snapshot["claims"]), "outcome already owned")
    row["claim_ticket"] = live["ticket"]
    row["ownership_receipt"] = snapshot


def source_revision(row):
    return row.get("source_revision", row["base"])


def spec_context(record, value):
    """Configure focused context for an existing assignment, never dispatch."""
    row = record["cloud"]["assignments"][value["assignment"]]
    require(row.get("mode") not in {"observational-only", "preflight-only"} and not row.get("accepted"),
            "only active managed work takes a spec context")
    context = value["context"]
    allowed = {"work_classification", "semantic_contracts", "context_repository", "canonical_revision"}
    require(isinstance(context, dict) and not set(context) - allowed, "explicit supported work context fields required")
    qualification.work_classification(context)
    proposed = dict(row)
    for key in allowed:
        proposed.pop(key, None)
    proposed.update(copy.deepcopy(context))
    if qualification.work_classification(proposed)["kind"] == "semantic":
        repository = proposed.get("context_repository")
        require(repository and git(repository, "rev-parse", "HEAD") == source_revision(row),
                "spec context checkout must match the recorded assignment source revision")
        base = qualification.revision(repository, proposed.get("canonical_revision", row["base"]))
        authority = row.get("canonical_authority", {}).get("base", row["base"])
        if not canonical_ancestor(repository, base, authority):
            # Advance canonical authority only after observing accepted main at
            # the actual publication destination, never a caller's candidate.
            deadline = time.monotonic() + 30
            try:
                url = coordination.push_url(repository, "origin", deadline=deadline)
                accepted = coordination.remote_refs(repository, url, "refs/heads/main", deadline=deadline).get("refs/heads/main")
                require(accepted, "accepted main authority is unavailable")
                coordination.fetch_object(repository, url, accepted, reference="refs/heads/main", deadline=deadline)
                require(canonical_ancestor(repository, base, accepted),
                        "canonical revision is not accepted main authority; candidate spec edits are not authority")
            except coordination.CoordinationError as error:
                raise RecoveryError("cannot establish accepted main authority for canonical revision") from error
            proposed["canonical_authority"] = dict(base=accepted, remote="origin", observed_utc=now(),
                evidence="actual publication destination refs/heads/main read")
    dispatch_context(record, proposed, "implementation")
    for key in allowed:
        row.pop(key, None)
    row.update(copy.deepcopy(context))
    if proposed.get("canonical_authority"):
        row["canonical_authority"] = proposed["canonical_authority"]


def canonical_ancestor(repository, revision, authority):
    result = coordination.git(repository, "merge-base", "--is-ancestor", revision, authority, allow_failure=True)
    require(result.returncode in {0, 1}, "canonical authority object is unavailable")
    return result.returncode == 0


def dispatch_context(record, row, role):
    classification = qualification.work_classification(row)
    if classification["kind"] == "non_semantic":
        return dict(qualification.work_handoff(None, row, None, role),
                    candidate=source_revision(row), source_revision=row["base"])
    repository = row.get("context_repository")
    require(isinstance(repository, str) and repository, "semantic dispatch requires context_repository pinned to the assignment source")
    require(git(repository, "rev-parse", "HEAD") == source_revision(row),
            "spec context checkout must match the recorded assignment source revision")
    selected = [mapping["contract"] for mapping in row["semantic_contracts"]]
    require(not git(repository, "status", "--porcelain", "--untracked-files=all", "--", *selected),
            "selected canonical context files are dirty")
    base = qualification.revision(repository, row.get("canonical_revision", row["base"]))
    require(canonical_ancestor(repository, base, row.get("canonical_authority", {}).get("base", row["base"])),
            "canonical revision must derive from accepted base authority; candidate spec edits are not authority")
    git(repository, "merge-base", "--is-ancestor", base, source_revision(row))
    # A normal dispatch acknowledgment proves task receipt, not retained spec
    # context. Only an exact completed child receipt can justify omission.
    previous, retained = None, False
    for operation in reversed(list(record["cloud"]["operations"].values())):
        request = operation["request"]
        packet = request.get("work_context")
        acknowledgment = operation.get("acknowledgment", {})
        if (operation["assignment"] == row["id"] and operation["effect"] == "applied"
                and packet and packet.get("context_fingerprint")):
            previous = packet
            observation = row.get("observation") or {}
            retained = (acknowledgment.get("status") == "completed"
                        and acknowledgment.get("context_fingerprint") == packet["context_fingerprint"]
                        and acknowledgment.get("context_retained") is True
                        and acknowledgment.get("evidence") is not None
                        and fresh(observation)
                        and observation.get("context_fingerprint") == packet["context_fingerprint"]
                        and observation.get("context_retained") is True)
            break
    return qualification.work_handoff(repository, row, base, role, previous, retained)


def readiness_contract(record, assignment):
    row = record["cloud"]["assignments"][assignment]
    return dict({key: row[key] for key in
                 ("base", "outcome", "writes", "reads", "acceptance", "dependency_review")},
                source_revision=source_revision(row),
                work_context={key: row[key] for key in ("work_classification", "semantic_contracts", "canonical_revision") if key in row},
                dependencies=[d for d in record["cloud"]["batches"][row["batch"]]["manifest"]["dependencies"]
                              if assignment in {d["producer"], d["consumer"]}])


def ownership_identity(row, snapshot):
    # Lease timestamps and unrelated peers do not change relevant ownership.
    return sorted((dict(ticket=c["ticket"], owner=c["owner"], item=c.get("item"),
                        board=c.get("board"), paths=sorted(c["paths"]))
                   for c in snapshot["claims"] if c["ticket"] == row.get("claim_ticket")
                   or c.get("item") == row["item"]),
                  key=lambda c: c["ticket"])


def implementation_started(record, assignment):
    contract = fingerprint(readiness_contract(record, assignment))
    return any(o["assignment"] == assignment and o["effect"] == "applied"
               and o["request"].get("readiness_contract") == contract
               and o["request"]["action"] == "follow_up"
               and o["request"]["parameters"].get("phase") == "implementation"
               and o["request"]["parameters"].get("role") != "review"
               for o in record["cloud"]["operations"].values())


def readiness_state(record, assignment):
    row = record["cloud"]["assignments"][assignment]
    value = row.get("readiness")
    if not value:
        return "unscouted"
    if value["status"] != "ready":
        return value["status"]
    if (not fresh(value["receipt"]) or value["contract"] != fingerprint(readiness_contract(record, assignment))
            or not row.get("ownership_receipt") or not fresh(row["ownership_receipt"])
            or value["ownership"] != fingerprint(ownership_identity(row, row["ownership_receipt"]))
            or value["prerequisites"] != fingerprint(dict(mapping=[row["thread"], row["environment"], row["generation"]], preflight=row.get("preflight"),
                 handoffs={d["id"]: record["cloud"]["handoffs"].get(d["id"])
                           for d in dependencies(record, assignment)}))):
        return "stale"
    try:
        if git(value["repository"], "status", "--porcelain", "--untracked-files=all", "--", *value["receipt"]["inputs"]):
            return "stale"
        artifact(value["evidence_root"], value["receipt"]["review_artifact"])
        artifact(value["evidence_root"], value["receipt"]["before"]["artifact"])
        if inputs(value["repository"], value["receipt"]["source_revision"], value["receipt"]["inputs"]) != inputs(
                value["repository"], git(value["repository"], "rev-parse", "HEAD"), value["receipt"]["inputs"]):
            return "stale"
    except (RecoveryError, OSError, subprocess.TimeoutExpired):
        return "stale"
    return "ready"


def readiness(record, assignment, value, repository, root):
    """Record scouting, including non-admissions; never execute a reproduction."""
    row = record["cloud"]["assignments"][assignment]
    require(not row["accepted"], "accepted assignment cannot be scouted again")
    require(value.get("schema") == 1 and value.get("assignment") == assignment
            and value.get("environment") == row["environment"] and row["environment"],
            "readiness assignment/environment mismatch")
    identifier(value["id"])
    events = row.setdefault("scouting", [])
    prior = next((e for e in events if e["id"] == value["id"]), None)
    if prior:
        require(prior["receipt"] == value, "conflicting scouting receipt; use a new attempt ID")
        return
    started = datetime.fromisoformat(value["started_utc"])
    observed = datetime.fromisoformat(value["observed_utc"])
    elapsed = (observed - started).total_seconds()
    require(elapsed >= 0 and value.get("cache") in {"warm", "cold", "unknown"}
            and value.get("complexity") and value.get("assumptions"),
            "scouting chronology/cache/complexity and scope assumptions required")
    artifact(root, value["review_artifact"])
    require(value.get("scope") == readiness_contract(record, assignment),
            "reviewed readiness scope differs from assignment")
    source = sha(value["source_revision"])
    current = sha(value["current_revision"])
    required_paths = set(row["writes"] + row["reads"] +
                         [p for r in row["acceptance"] for p in r["inputs"]]
                         + [mapping["contract"] for mapping in row.get("semantic_contracts", [])])
    reviewed_paths = paths(value["inputs"])
    require(all(any(p == r or p.startswith(r + "/") for r in reviewed_paths)
                for p in required_paths), "readiness input closure misses declared scope")
    before = value.get("before")
    blocker = value.get("blocker")
    require(blocker is None or blocker.get("kind") in
            {"live-owner", "product-decision", "executor-unavailable", "denied-action", "dependency"}
            and blocker.get("evidence") and blocker.get("reason"), "named external blocker required")
    reason = None
    if blocker:
        status, reason = "blocked", blocker["kind"]
    else:
        require(isinstance(before, dict) and before.get("revision") == source
                and before.get("environment") == row["environment"]
                and before.get("kind") in {"native", "interpreter"}
                and type(before.get("exit")) is int, "actual before-program observation required")
        oracle = next((r for r in row["acceptance"] if r["kind"] == before["kind"]
                       and r["command"] == before.get("command")), None)
        require(oracle and before.get("expected_result") == oracle["expected_result"],
                "before observation differs from authored execution oracle")
        artifact(root, before["artifact"])
        require(before.get("classification") in {"valid-should-work", "invalid-source", "pending-approval", "compile-only"},
                "classify actual program validity and execution route")
        failed = program_failed(before, oracle)
        status = "ready" if before["classification"] == "valid-should-work" and failed else "not-reproduced"
        reason = before["classification"] if status != "ready" else "reproduced"
    # This checks known dependencies, not automatic semantic discovery.
    declared = {d["id"] for d in readiness_contract(record, assignment)["dependencies"]}
    require(isinstance(value.get("known_dependencies"), list), "known dependency review required")
    if set(value["known_dependencies"]) != declared:
        status, reason = "blocked", "undeclared-or-unreviewed-dependency"
    if not fresh(value) or inputs(repository, source, reviewed_paths) != inputs(repository, current, reviewed_paths):
        status, reason = "stale", "changed-or-stale-inputs"
    require(git(repository, "rev-parse", "HEAD") == current, "current source must match inspected checkout")
    # An unrelated source advance is neutral only when every reviewed input is unchanged.
    if (inputs(repository, source_revision(row), reviewed_paths) != inputs(repository, current, reviewed_paths)
            or git(repository, "status", "--porcelain", "--untracked-files=all", "--", *reviewed_paths)):
        status, reason = "stale", "assignment-inputs-changed"
    prerequisites = dict(mapping=[row["thread"], row["environment"], row["generation"]], preflight=row.get("preflight"),
                         handoffs={d["id"]: record["cloud"]["handoffs"].get(d["id"])
                                   for d in dependencies(record, assignment)})
    if status == "ready":
        if row.get("approval_required") or (row.get("checkpoint") or {}).get("phase") == "required_approval":
            status, reason = "blocked", "denied-action"
        elif not row.get("preflight"):
            status, reason = "blocked", "executor-unavailable"
        elif any(v is None for v in prerequisites["handoffs"].values()):
            status, reason = "blocked", "dependency"
        elif not row.get("ownership_receipt") or not fresh(row["ownership_receipt"]):
            status, reason = "blocked", "live-owner"
    event = dict(id=value["id"], status=status, reason=reason, elapsed_seconds=elapsed, receipt=copy.deepcopy(value))
    events.append(event)
    row["readiness"] = dict(event, repository=str(Path(repository).resolve()),
                            evidence_root=str(Path(root).resolve()), contract=fingerprint(readiness_contract(record, assignment)),
                            ownership=fingerprint(ownership_identity(row, row["ownership_receipt"]))
                            if row.get("ownership_receipt") else None,
                            prerequisites=fingerprint(prerequisites))


def preflight(record, assignment, value):
    row = record["cloud"]["assignments"][assignment]
    require(value.get("assignment") == assignment
            and (value.get("thread"), value.get("environment"), value.get("generation"))
            == (row["thread"], row["environment"], row["generation"]) and row["thread"],
            "preflight runtime mapping mismatch")
    require(value.get("repository") == "CathedralOS/Omega" and value.get("base") == row["base"]
            and value.get("source_revision") == source_revision(row) and value.get("toolchain")
            and value.get("execution_route") and value.get("evidence")
            and value.get("doctor_exit") == 0 and type(value.get("doctor_exit")) is int,
            "complete checkout/toolchain/approval preflight required")
    identity = value.get("identity", {})
    require(identity.get("name") == "Jarod Castillo"
            and identity.get("email") == "43973221+Smotto@users.noreply.github.com"
            and identity.get("scope") == "repository-local", "authorized repository-local identity required")
    resources = value.get("resources", {})
    for key in ("cpus", "memory_bytes", "disk_bytes", "build_jobs"):
        require(type(resources.get(key)) in {int, float} and resources[key] > 0,
                "actual executor resource measurements required")
    require(type(resources["build_jobs"]) is int
            and resources["build_jobs"] <= max(1, int(resources["cpus"])), "build jobs exceed measured CPU quota")
    smoke = value.get("smoke", {})
    require(smoke.get("native_exit") == 0 and type(smoke.get("native_exit")) is int
            and smoke.get("interpreter_exit") == 0 and type(smoke.get("interpreter_exit")) is int
            and smoke.get("command") and smoke.get("preparation") and smoke.get("evidence"),
            "normal package preparation and native/interpreter smoke required")
    backup = value.get("backup", {})
    require(backup.get("status") == "confirmed-upload" and backup.get("file_id")
            and backup.get("version") is not None and backup.get("route")
            and isinstance(backup.get("sha256"), str) and re.fullmatch(r"[0-9a-f]{64}", backup["sha256"])
            and type(backup.get("bytes")) is int and backup["bytes"] > 0,
            "confirmed recoverable preflight artifact required")
    require(row.get("preflight") in (None, value), "preflight changed; establish a new explicit assignment")
    row["preflight"] = value


def checkpoint(record, assignment, value):
    row = record["cloud"]["assignments"][assignment]
    require(value.get("phase") in {"preflight", "implementation", "validation", "publication", "waiting_dependency",
                                     "required_approval", "blocked", "candidate_ready", "landed"}, "explicit phase required")
    require(value.get("next_gate") and value.get("evidence") and fresh(value)
            and value.get("started_utc") and value.get("progress_utc") and "active_command" in value and "session" in value, "incomplete operational checkpoint")
    require(value["phase"] not in {"waiting_dependency", "required_approval", "blocked"} or value.get("reason"),
            "wait/approval/blocker reason required")
    started = datetime.fromisoformat(value["started_utc"])
    progress = datetime.fromisoformat(value["progress_utc"])
    observed = datetime.fromisoformat(value["observed_utc"])
    require(started <= progress <= observed, "checkpoint progress/start chronology invalid")
    row["checkpoint"] = value
    row.pop("cleanup_review", None)


def intent(record, assignment, action, operation, parameters, *, execution_id=None):
    cloud = record["cloud"]
    row = cloud["assignments"][assignment]
    identifier(operation)
    require(operation not in cloud["operations"], "operation already recorded; reconcile, never redispatch")
    if row.get("mode") == "preflight-only" and action != "read":
        return preflight_intent(record, row, action, operation, parameters)
    if row.get("mode") in {"observational-only", "preflight-only"}:
        require(action == "read", "observational assignment permits only reads")
        require(isinstance(parameters, dict) and not set(parameters) - {"limit", "cursor"}, "only supported read parameters required")
        require(type(parameters.get("limit", 20)) is int and 0 < parameters.get("limit", 20) <= 100,
                "bounded observation limit must be 1..100")
        require("cursor" not in parameters or isinstance(parameters["cursor"], str), "actual read cursor must be a string")
        require(operation not in cloud["operations"], "operation already recorded; reconcile, never redispatch")
        runtime_arguments = dict(threadId=row["thread"], limit=parameters.get("limit", 20))
        if "cursor" in parameters:
            runtime_arguments["cursor"] = parameters["cursor"]
        request = dict(schema=1, operation=operation, execution_id=uuid.uuid4().hex, assignment=assignment,
            action="read", tool=cloud["capabilities"]["tools"]["read"], thread=row["thread"], environment=None, generation=None,
            parameters=copy.deepcopy(parameters), runtime_arguments=runtime_arguments,
            contract=dict(id=assignment, mode=row["mode"], thread=row["thread"], binding_evidence=row["binding_evidence"]),
            readiness_contract=None, readiness_gate=None, dependencies=[], reporting=cloud["capabilities"]["reporting"], created_utc=now())
        cloud["operations"][operation] = dict(request=request, assignment=assignment, effect="unknown", result=None)
        return request
    require(isinstance(parameters, dict) and not set(parameters) - {"model", "reasoning", "phase", "message", "environment", "lifecycle_action", "role", "purpose"},
            "only normalized adapter parameters are accepted; no credentials or permission bypass fields")
    require(parameters.get("purpose", "work") in {"work", "status"}, "supported work/status purpose required")
    status_only = parameters.get("purpose") == "status"
    if status_only:
        require(action == "follow_up" and parameters.get("phase") == "preflight"
                and not parameters.get("message") and not parameters.get("role"),
                "status-only follow-up is a fixed bounded report, without work instructions or review role")
    require(action != "follow_up" or parameters.get("phase") in {"preflight", "implementation"},
            "continuation needs explicit preflight/implementation phase")
    require(operation not in cloud["operations"], "operation already recorded; reconcile, never redispatch")
    require(action in {"create", "read", "follow_up", "lifecycle"}, "unsupported adapter action")
    require(cloud["capabilities"]["tools"].get(action), "runtime does not expose this action")
    if action != "read":
        require(row["checkpoint"] is not None and fresh(row["checkpoint"]), "checkpoint before dispatch required")
        require(not row.get("approval_required") and row["checkpoint"]["phase"] != "required_approval",
                "actual approval blocker; do not retry")
        require(not any(o["assignment"] == assignment and o["effect"] == "unknown"
                        and o["request"]["action"] != "read"
                        for o in cloud["operations"].values()), "unknown mutation/read result; reconcile before dispatch")
        if action != "create" and parameters.get("phase") != "preflight":
            require(not any(d["id"] not in cloud["handoffs"] for d in dependencies(record, assignment)),
                    "waiting for dependency handoff")
        require(not row["accepted"], "accepted assignment cannot be resumed")
        attempts = row["attempts"].get(action, 0)
        failures = sum(op["effect"] == "not_applied" for op in cloud["operations"].values()
                       if op["assignment"] == assignment and op["request"]["action"] == action)
        require(failures <= row["retry_budget"], "retry budget exhausted")
        if action == "follow_up":
            continuations = sum(op["effect"] == "applied" for op in cloud["operations"].values()
                                if op["assignment"] == assignment and op["request"]["action"] == action)
            require(continuations < row["max_continuations"], "continuation budget exhausted")
        if action == "create":
            require(parameters.get("phase") == "preflight", "initial cloud turn must be bounded preflight")
            require(row["thread"] is None, "existing thread; continue exact assignment")
            require(parameters.get("model") == "gpt-6.1-sol" and parameters.get("reasoning") == "medium",
                    "explicit repository model/effort required")
        else:
            require(row["thread"] and row["environment"], "known runtime mapping required")
            require(row["observation"] and fresh(row["observation"]), "fresh runtime read required")
            if action == "follow_up":
                if parameters["phase"] == "implementation":
                    require(row.get("preflight"), "verified preflight receipt required before implementation continuation")
                    require(row.get("ownership_receipt") and fresh(row["ownership_receipt"]),
                            "fresh exact claims receipt required before implementation continuation")
                    require(implementation_started(record, assignment) or readiness_state(record, assignment) == "ready",
                            "fresh reproduced readiness receipt required before implementation continuation")
                require(not any(op["assignment"] == assignment and op["request"]["action"] in {"create", "follow_up"}
                                and op["effect"] == "applied" and not op.get("acknowledgment")
                                for op in cloud["operations"].values()),
                        "queued dispatch has no child acknowledgment; read existing task")
                require(row["runtime"] in {"idle", "completed", "stopped"}
                        and row["observation"].get("processes_stopped") is True, "running/uncertain processes; do not resume")
                recovery_clear(record, row, row["observation"],
                               allow_prior=parameters["phase"] == "preflight" and not row.get("preflight"))
        row["attempts"][action] = attempts + 1
    work_context = None
    prompt = None
    if action in {"create", "follow_up"}:
        if status_only:
            work_context = dict(schema="qualification-work-context-1", role="continuation",
                work_classification=dict(kind="non_semantic", reason="Fixed bounded status report only"))
            prompt = ("Return a bounded status report for existing assignment " + assignment
                      + ". Report current progress, active processes, checkpoint and actual blockers. "
                        "Do not edit source, start builds, implement new work or publish. This is a status-only turn.\n")
        else:
            role = parameters.get("role", "continuation" if action == "follow_up" else "implementation")
            work_context = dispatch_context(record, row, role)
            prompt = qualification.work_context_prompt(work_context) + "\nAssignment contract:\n" + json.dumps(
                {key: row[key] for key in ("id", "outcome", "acceptance", "writes", "reads")}, indent=2)
            if parameters.get("phase") == "preflight":
                prompt += "\nThis turn is bounded preflight inspection only; do not begin implementation.\n"
            if parameters.get("message"):
                require(isinstance(parameters["message"], str), "dispatch message must be text")
                prompt += "\nRequested task details:\n" + parameters["message"]
    request = dict(schema=1, operation=operation, execution_id=execution_id or uuid.uuid4().hex, assignment=assignment,
                   action=action, tool=cloud["capabilities"]["tools"][action], thread=row["thread"],
                   environment=row["environment"], generation=row["generation"], parameters=parameters,
                   contract=dict({key: row[key] for key in ("id", "base", "owner", "item", "outcome", "writes", "reads", "acceptance", "dependency_review")},
                                 board=row.get("board"), claim_ticket=row.get("claim_ticket"), source_revision=source_revision(row),
                                 retry_budget=row["retry_budget"], max_continuations=row["max_continuations"]),
                   readiness_contract=fingerprint(readiness_contract(record, assignment)),
                   readiness_gate=copy.deepcopy(row.get("readiness")) if action == "follow_up"
                       and parameters.get("phase") == "implementation" else None,
                   dependencies=dependencies(record, assignment),
                   reporting=cloud["capabilities"]["reporting"], created_utc=now())
    if work_context is not None:
        prompt += "\nAcknowledge this exact dispatch identity in the bounded turn result:\n" + json.dumps(
            {key: request[key] for key in ("operation", "execution_id", "assignment", "reporting")}, indent=2)
        request.update(work_context=work_context, prompt=prompt)
    cloud["operations"][operation] = dict(request=request, assignment=assignment, effect="unknown", result=None)
    return request


def step_status(record, assignment, step):
    entry = record["cloud"]["steps"][assignment][step]
    operation = record["cloud"]["operations"][entry["operation"]]
    awaiting_acknowledgment = operation["effect"] == "applied" and operation["request"]["action"] in {
        "create", "follow_up"} and not operation.get("acknowledgment")
    reconciled = operation["effect"] != "unknown" and operation.get("reconciliation")
    if reconciled and awaiting_acknowledgment:
        action = "read_reconcile"
    elif entry.get("completed"):
        action = "settled" if operation["effect"] != "unknown" else "read_reconcile"
    elif reconciled and entry.get("result") is None:
        action = "complete_reconciled_step"
    elif entry.get("result") is not None:
        action = "read_reconcile" if operation["request"]["action"] == "read" and (
            entry["saved_revision"] != record["revision"] or not fresh(entry["result"])) else "ingest_saved_result"
    elif entry.get("dispatched_utc"):
        action = "read_reconcile"
    else:
        action = "dispatch_prepared_step"
    return dict(assignment=assignment, step=step, operation=entry["operation"],
                execution_id=operation["request"]["execution_id"], action=action,
                effect=operation["effect"], checkpoint=copy.deepcopy(entry["checkpoint"]))


def prepare_step(record, value):
    """Save a logical step once; replay yields status, never another tool request."""
    identifier(value["step"])
    cloud = record["cloud"]
    steps = cloud.setdefault("steps", {}).setdefault(value["assignment"], {})
    digest = fingerprint(value)
    if value["step"] in steps:
        require(steps[value["step"]]["contract"] == digest, "conflicting step contract")
        return step_status(record, value["assignment"], value["step"])
    request = intent(record, value["assignment"], value["action"], value["operation"], value.get("parameters", {}))
    steps[value["step"]] = dict(operation=request["operation"], contract=digest,
                               checkpoint=copy.deepcopy(cloud["assignments"][value["assignment"]]["checkpoint"]))
    return step_status(record, value["assignment"], value["step"])


def dispatch_step(record, assignment, step):
    """Reserve the sole adapter invocation before returning the exact saved request.

    A crash after this write has uncertain effects, even if the tool was never
    reached. There is no retry permission without an authoritative read.
    """
    entry = record["cloud"]["steps"][assignment][step]
    require(not entry.get("dispatched_utc"), "step already dispatched; read and reconcile")
    operation = record["cloud"]["operations"][entry["operation"]]
    require(operation["effect"] == "unknown" and not operation.get("receipt"), "step already reconciled")
    request = operation["request"]
    # Run the existing admission guards against current state, removing only
    # this prepared intent in a scratch record. Nothing dispatches from scratch.
    scratch = copy.deepcopy(record)
    del scratch["cloud"]["operations"][entry["operation"]]
    checked = intent(scratch, assignment, request["action"], entry["operation"], request["parameters"],
                     execution_id=request["execution_id"])
    for field in ("tool", "thread", "environment", "generation", "contract", "readiness_contract", "readiness_gate", "dependencies", "reporting"):
        require(checked[field] == request[field], "prepared dispatch contract changed; reconcile")
    require(checked.get("work_context") == request.get("work_context")
            and checked.get("prompt") == request.get("prompt"),
            "prepared spec context/prompt changed; prepare a fresh reviewed turn")
    if "runtime_arguments" in request:
        if record["cloud"]["assignments"][assignment].get("mode") == "preflight-only" and request["action"] == "follow_up":
            checked["runtime_arguments"]["prompt"] = preflight_prompt(entry["operation"], request["execution_id"])
        require(checked["runtime_arguments"] == request["runtime_arguments"], "prepared observation arguments changed")
    entry["dispatched_utc"] = now()
    return dict(action="invoke_once", request=copy.deepcopy(request))


def save_step_result(record, assignment, step, envelope):
    """Commit the actual sanitized result before attempting its state transition."""
    entry = record["cloud"]["steps"][assignment][step]
    request = record["cloud"]["operations"][entry["operation"]]["request"]
    require(entry.get("dispatched_utc"), "step has no dispatch reservation")
    require(envelope.get("schema") == 1 and envelope.get("operation") == request["operation"]
            and envelope.get("execution_id") == request["execution_id"]
            and envelope.get("assignment") == assignment, "saved step result identity mismatch")
    if entry.get("result") is not None:
        require(entry["result"] == envelope, "conflicting saved step result")
        return
    require(request["action"] != "read" or envelope.get("revision") == record["revision"],
            "stale runtime observation")
    entry.update(result=copy.deepcopy(envelope), saved_revision=record["revision"] + 1)


def complete_step(record, assignment, step):
    entry = record["cloud"]["steps"][assignment][step]
    if entry.get("completed"):
        return
    operation = record["cloud"]["operations"][entry["operation"]]
    request = operation["request"]
    if entry.get("result") is None:
        require(operation["effect"] != "unknown" and operation.get("reconciliation"),
                "save actual result or reconcile exact operation before completing step")
        require(operation["effect"] != "applied" or request["action"] not in {"create", "follow_up"}
                or operation.get("acknowledgment"), "reconciled dispatch needs exact child acknowledgment")
        entry.update(completed=True, reconciliation=copy.deepcopy(operation["reconciliation"]))
        return
    envelope = copy.deepcopy(entry["result"])
    if request["action"] == "read":
        require(entry["saved_revision"] == record["revision"], "stale runtime observation; perform a fresh read")
        # Only our result-save commit advanced this observation's revision.
        # Preserve the original envelope in the step as exact adapter evidence.
        envelope["revision"] = record["revision"]
    ingest(record, envelope)
    entry["completed"] = True


def ingest(record, envelope):
    cloud = record["cloud"]
    op = cloud["operations"][envelope["operation"]]
    request = op["request"]
    require(envelope.get("schema") == 1 and envelope.get("execution_id") == request["execution_id"]
            and envelope.get("assignment") == request["assignment"], "adapter result identity mismatch")
    digest = fingerprint(envelope)
    if op.get("receipt"):
        require(op["receipt"] == digest, "conflicting duplicate adapter result")
        return
    row = cloud["assignments"][request["assignment"]]
    effect = envelope.get("effect")
    require(effect in {"applied", "not_applied", "unknown"} and envelope.get("evidence"), "adapter effect evidence required")
    result = envelope.get("result", {})
    if row.get("mode") == "preflight-only" and request["action"] == "follow_up":
        if effect == "applied":
            require(isinstance(result, dict) and result.get("threadId") == row["thread"], "preflight enqueue thread mismatch")
        op.update(effect=effect, result=copy.deepcopy(result), receipt=digest, envelope=copy.deepcopy(envelope))
        if envelope.get("approval_denied") is True:
            require(effect == "not_applied" and result.get("error") and result.get("action"), "exact approval denial required")
            row["approval_required"] = dict(operation=request["operation"], execution_id=request["execution_id"],
                                            action=result["action"], error=result["error"])
        return
    if row.get("mode") in {"observational-only", "preflight-only"}:
        require(request["action"] == "read" and fresh(envelope) and envelope.get("revision") == record["revision"],
                "stale observational read or unsupported action")
        if effect == "applied":
            require(isinstance(result, dict) and result.get("threadId") == row["thread"], "observational read thread mismatch")
            require({"parentThreadId", "latestTurn", "items", "nextCursor"} <= result.keys()
                    and isinstance(result["items"], list)
                    and (result["latestTurn"] is None or isinstance(result["latestTurn"], dict)
                         and {"id", "status", "error"} <= result["latestTurn"].keys()), "actual structured thread-read result required")
            row["observation"] = dict(structuredContent=copy.deepcopy(result), observed_utc=envelope["observed_utc"],
                                      evidence=envelope["evidence"], executor_readiness_verified=False)
        else:
            row["observation"] = None
        # Recorded turn status does not attest live processes or executor identity.
        op.update(effect=effect, result=copy.deepcopy(result), receipt=digest, envelope=copy.deepcopy(envelope))
        return
    if request["action"] == "read":
        require(fresh(envelope) and envelope.get("revision") == record["revision"], "stale runtime observation")
        if effect != "applied":
            row["runtime"] = "unknown"
            row["observation"] = None
            op.update(effect=effect, result=result, receipt=digest, envelope=envelope)
            return
        require(result.get("runtime") in {"running", "idle", "completed", "stopped", "unknown"}, "actual runtime state required")
        if row["thread"]:
            require((result.get("thread"), result.get("environment"), result.get("generation"))
                    == (row["thread"], row["environment"], row["generation"]), "stale runtime mapping")
        if row["thread"]:
            mapping(record, row, result)
        row["runtime"] = result["runtime"]
        row["observation"] = dict(result, observed_utc=envelope["observed_utc"])
        for name, proof in result.get("effects", {}).items():
            pending = cloud["operations"][name]
            require(pending["assignment"] == row["id"] and proof.get("execution_id") == pending["request"]["execution_id"]
                    and proof.get("evidence") and proof.get("effect") in {"applied", "not_applied", "unknown"},
                    "reconciliation does not match exact pending operation")
            if pending["effect"] != "unknown":
                require(pending["effect"] == proof["effect"], "conflicting authoritative effect")
            else:
                pending["effect"] = proof["effect"]
                pending["reconciliation"] = proof
            if pending["request"]["action"] == "create" and proof["effect"] == "applied":
                require((result.get("thread"), result.get("environment"), result.get("generation"))
                        == (proof.get("thread"), proof.get("environment"), proof.get("generation")),
                        "reconciled create differs from runtime observation")
                # The read can supply executor identity absent from the create
                # proof. Validate and retain both sources: checking only the
                # proof would discard the observed executor and hide collisions.
                mapping(record, row, result)
                mapping(record, row, proof)
        for acknowledgment in result.get("acknowledgments", []):
            acknowledged = cloud["operations"][acknowledgment["operation"]]
            require(acknowledged["assignment"] == row["id"]
                    and acknowledgment.get("execution_id") == acknowledged["request"]["execution_id"]
                    and acknowledged["request"]["action"] in {"create", "follow_up"}
                    and acknowledged["effect"] == "applied"
                    and acknowledgment.get("status") in {"started", "completed"}
                    and acknowledgment.get("evidence"), "exact child dispatch acknowledgment required")
            if acknowledgment.get("context_retained") is True:
                packet = acknowledged["request"].get("work_context") or {}
                require(packet.get("context_fingerprint")
                        and acknowledgment.get("context_fingerprint") == packet["context_fingerprint"],
                        "retained context acknowledgment must name the exact delivered fingerprint")
            acknowledged["acknowledgment"] = acknowledgment
        if "preflight" in result:
            preflight(record, row["id"], result["preflight"])
        if "report" in result:
            report = result["report"]
            require(report.get("assignment") == row["id"] and report.get("thread") == row["thread"]
                    and report.get("environment") == row["environment"] and report.get("evidence")
                    and type(report.get("sequence")) is int
                    and report["sequence"] > row.get("report_sequence", -1),
                    "missing, stale or uncorrelated task result report")
            checkpoint(record, row["id"], dict(report["checkpoint"], observed_utc=envelope["observed_utc"]))
            row["report_sequence"] = report["sequence"]
            row["report_receipt"] = report
    elif request["action"] == "create" and effect == "applied":
        mapping(record, row, result)
    # A late unknown client result cannot undo a reconciled effect.
    if op["effect"] != "unknown":
        require(effect in {"unknown", op["effect"]}, "conflicting reconciled result")
    else:
        op["effect"] = effect
    op.update(result=result, receipt=digest, envelope=envelope)
    if envelope.get("approval_denied") is True:
        require(effect == "not_applied" and result.get("error") and result.get("action"), "exact approval denial required")
        row["approval_required"] = dict(operation=request["operation"], execution_id=request["execution_id"],
                                        action=result["action"], error=result["error"])
        stamp = now()
        row["checkpoint"] = dict(phase="required_approval", reason=result, evidence=envelope["evidence"],
                observed_utc=stamp, progress_utc=stamp, started_utc=request["created_utc"], active_command=None, session=None, next_gate="parent approval")


def approval(record, assignment, receipt):
    row = record["cloud"]["assignments"][assignment]
    blocked = row.get("approval_required")
    require(blocked and receipt.get("operation") == blocked["operation"]
            and receipt.get("execution_id") == blocked["execution_id"]
            and receipt.get("action") == blocked["action"]
            and receipt.get("status") == "approved" and receipt.get("evidence")
            and receipt.get("route") and fresh(receipt), "actual exact-action approval receipt required")
    row["approval_receipt"] = receipt
    row["approval_required"] = None
    # Caller still needs a new truthful checkpoint and fresh runtime read.


def binding_evidence(root, value):
    artifact(root, value["artifact"])
    packet = read_json(Path(root) / value["artifact"]["path"])
    require(packet.get("normalized") == {k: v for k, v in value.items() if k != "artifact"}
            and packet.get("raw_result"), "binding evidence differs from normalized host/pool receipt")


def review_revision(record, assignment, value, repository, root):
    row = record["cloud"]["assignments"][assignment]
    identifier(value["id"])
    artifact(root, value["artifact"])
    old = record["cloud"]["operations"].get(value["id"])
    if old:
        require(old["request"]["action"] == "review-revision" and old["assignment"] == assignment
                and old["result"] == value, "conflicting reviewed revision operation")
        return
    require(value.get("schema") == 1 and value.get("assignment") == assignment and fresh(value)
            and value.get("reason") and value.get("assumptions"), "fresh concrete revision review required")
    require(not row["accepted"] and not row.get("approval_required")
            and (row.get("checkpoint") or {}).get("phase") != "required_approval", "accepted/approval revision blocker")
    require((value.get("thread"), value.get("environment"), value.get("generation"))
            == (row["thread"], row["environment"], row["generation"]) and row["thread"], "revision runtime mapping mismatch")
    require(row.get("observation") and fresh(row["observation"])
            and row["runtime"] in {"idle", "completed", "stopped"}
            and row["observation"].get("processes_stopped") is True, "settled revision read required")
    require(not any(o["assignment"] == assignment and (o["effect"] == "unknown"
                    or o["request"]["action"] in {"create", "follow_up"} and o["effect"] == "applied"
                    and not o.get("acknowledgment")) for o in record["cloud"]["operations"].values()),
            "revision operations/acknowledgments not reconciled")
    recovery_clear(record, row, row["observation"])
    require(row.get("ownership_receipt") and fresh(row["ownership_receipt"]), "fresh revision ownership required")
    require(value.get("scope") == readiness_contract(record, assignment), "revision scope differs from reviewed contract")
    previous, target = sha(value["from_revision"]), sha(value["to_revision"])
    require(previous == source_revision(row) and previous != target
            and git(repository, "rev-parse", "HEAD") == target, "revision source/head mismatch")
    git(repository, "merge-base", "--is-ancestor", previous, target)
    required = (row["writes"] + row["reads"] + [p for r in row["acceptance"] for p in r["inputs"]]
                + [mapping["contract"] for mapping in row.get("semantic_contracts", [])])
    reviewed = paths(value["inputs"])
    require(reviewed and all(any(p == r or p.startswith(r + "/") for r in reviewed) for p in required),
            "revision review misses input closure")
    require(not git(repository, "status", "--porcelain", "--untracked-files=all", "--", *reviewed),
            "revision checkout inputs are dirty")
    changed = git(repository, "diff", "--name-only", previous, target, "--", *reviewed).splitlines()
    require(sorted(paths(value["changed_paths"])) == sorted(changed), "revision changed paths differ from actual diff")
    edges = dependencies(record, assignment)
    require(value.get("dependencies") == {d["id"]: record["cloud"]["handoffs"].get(d["id"], {}).get("candidate")
                                         for d in edges}
            and all(d["id"] in record["cloud"]["handoffs"] for d in edges), "reviewed bilateral dependencies missing")
    for candidate in value["dependencies"].values():
        git(repository, "merge-base", "--is-ancestor", sha(candidate), target)
    row.setdefault("revisions", []).append(copy.deepcopy(value))
    if row.get("preflight"):
        row.setdefault("preflight_history", []).append(copy.deepcopy(row["preflight"]))
    row["preflight"] = None
    row["source_revision"] = target
    row.pop("cleanup_review", None)
    record["cloud"]["operations"][value["id"]] = dict(assignment=assignment, effect="applied", result=copy.deepcopy(value),
        request=dict(action="review-revision", assignment=assignment, operation=value["id"], created_utc=now()))


def bind_existing(record, value):
    """Adopt a settled real session without inventing a create or remote action."""
    cloud = record["cloud"]
    assignment, operation = value["assignment"], value["operation"]
    row = cloud["assignments"][assignment]
    identifier(operation)
    receipt = value["receipt"]
    artifact(value["evidence_root"], value["receipt_artifact"])
    require(read_json(Path(value["evidence_root"]) / value["receipt_artifact"]["path"]) == receipt,
            "binding artifact differs from receipt")
    digest = fingerprint(receipt)
    old = cloud["operations"].get(operation)
    if old:
        require(old["request"]["action"] == "bind-existing" and old["assignment"] == assignment
                and old.get("receipt") == digest, "conflicting binding operation")
        return
    require(receipt.get("schema") == 1 and receipt.get("assignment") == assignment,
            "binding assignment/schema mismatch")
    require(row["thread"] is None and not row["accepted"] and not row.get("binding"),
            "assignment already bound or accepted")
    require(not row.get("approval_required"), "binding cannot clear an assignment denial")
    require(not any(o["assignment"] == assignment for o in cloud["operations"].values()),
            "assignment already has operations; reconcile rather than replace provenance")
    source, creation, observed, prior = (receipt[k] for k in ("source", "creation", "observation", "prior"))
    require(set(source) == {"pool", "slot"} and source.get("pool")
            and type(source.get("slot")) is int and source["slot"] > 0,
            "recorded original pool/slot required")
    require(creation.get("tool") == cloud["capabilities"]["tools"]["create"]
            and creation.get("receipt") and creation.get("thread"), "actual original create provenance required")
    require(observed.get("tool") == cloud["capabilities"]["tools"]["read"]
            and observed.get("receipt") and fresh(observed), "fresh supported host read required")
    for evidence in (creation, observed, prior):
        binding_evidence(value["evidence_root"], evidence)
    require(observed.get("thread") == creation["thread"] and observed.get("environment")
            and observed.get("generation") and observed.get("executor") and observed.get("identity_evidence")
            and all(isinstance(observed[k], str) and observed[k].strip()
                    and observed[k] not in {"unknown", "unreported", "unavailable"}
                    for k in ("thread", "environment", "generation", "executor")),
            "observed session/executor incarnation must correlate with original create")
    for key in ("environment", "generation"):
        require(not creation.get(key) or creation[key] == observed[key], "original create identity mismatch")
    require(observed.get("runtime") in {"idle", "completed", "stopped"}
            and observed.get("processes_stopped") is True and observed.get("active_assignments") == [],
            "existing session still active or assignment ownership unknown")
    require(prior.get("assignment") and prior["assignment"] != assignment and prior.get("outcome")
            and prior["outcome"] != row["outcome"] and prior.get("disposition") in {"released", "accepted"}
            and "approval_required" in prior, "explicit different prior outcome settlement required")
    require(observed.get("prior_assignment") == prior["assignment"], "prior assignment observation mismatch")
    require(not any(o["assignment"] == prior["assignment"] and
                    (o["effect"] == "unknown" or o["request"]["action"] in {"create", "follow_up"}
                     and o["effect"] == "applied" and not o.get("acknowledgment"))
                    for o in cloud["operations"].values()), "prior operations/acknowledgments not reconciled")
    checkpoint({"cloud": {"assignments": {prior["assignment"]: {}}}}, prior["assignment"], prior["checkpoint"])
    require(prior["checkpoint"]["active_command"] is None and prior["checkpoint"]["session"] is None,
            "prior scouting/command has not settled")
    recovery_clear(record, dict(id=prior["assignment"], environment=observed["environment"],
                                generation=observed["generation"]), observed)
    resources = prior["resources"]
    require(fresh(resources) and resources.get("evidence")
            and resources.get("claims_state") in {"claimed", "available"}
            and resources.get("landing_state") in {"github-merge-queue", "available", "queued", "reserved", "expired", "legacy_queue", "legacy_reserved"}
            and resources.get("claims") == []
            and resources.get("landing_tickets") == [] and isinstance(resources.get("workspaces"), list)
            and resources["workspaces"], "prior resource ownership not reconciled")
    protected = paths(prior["protected_paths"])
    for workspace in resources["workspaces"]:
        require(workspace.get("path") and workspace.get("evidence") and workspace.get("owned") is True
                and workspace.get("state") in {"clean", "removed", "preserved"}, "prior workspace inventory required")
        if workspace["state"] == "preserved":
            backup = workspace.get("backup", {})
            require(backup.get("status") == "confirmed-upload" and backup.get("file_id")
                    and backup.get("version") is not None and backup.get("route")
                    and isinstance(backup.get("sha256"), str) and re.fullmatch(r"[0-9a-f]{64}", backup["sha256"])
                    and type(backup.get("bytes")) is int and backup["bytes"] > 0 and protected,
                    "preserved workspace needs confirmed backup and protected paths")
    known = cloud["assignments"].get(prior["assignment"])
    if known:
        require((known.get("thread"), known.get("environment"), known.get("generation"))
                == (observed["thread"], observed["environment"], observed["generation"])
                and known["outcome"] == prior["outcome"]
                and (known.get("accepted") or {}).get("result") == "landed"
                and known["accepted"].get("cleanup") == "completed"
                and known["accepted"].get("user_rescue") is False and prior["disposition"] == "accepted"
                and known.get("approval_required") == prior["approval_required"],
                "known prior assignment not accepted or denial history differs")
    else:
        require(prior["disposition"] == "released", "external settlement is not controller acceptance")
    if prior["approval_required"]:
        require(protected and prior["approval_required"].get("action") and prior["approval_required"].get("error"),
                "prior denial and affected paths must be preserved")
    for history in cloud["operations"].values():
        if history["request"]["action"] == "bind-existing" and history["result"]["observation"]["thread"] == observed["thread"]:
            protected.extend(history["result"]["prior"]["protected_paths"])
    require(not any(claims.paths_overlap(a, b) for a in row["writes"] for b in protected),
            "new outcome overlaps preserved or denied prior work")
    for other in cloud["assignments"].values():
        if other["id"] == assignment or other.get("accepted"):
            continue
        bound = other.get("binding", {})
        require(other.get("thread") != observed["thread"] and other.get("environment") != observed["environment"]
                and bound.get("executor") != observed["executor"] and bound.get("source") != source
                and other["outcome"] != row["outcome"], "duplicate live session/slot/executor/outcome binding")
    mapping(record, row, observed)
    # Keep the prior recovery identity until the first bounded preflight establishes
    # the new child lane. Implementation and acceptance never use that exemption.
    row.update(runtime=observed["runtime"], observation=dict(observed),
               binding=dict(operation=operation, source=source, executor=observed["executor"],
                            prior_assignment=prior["assignment"], receipt=digest))
    cloud["operations"][operation] = dict(assignment=assignment, effect="applied", receipt=digest,
        request=dict(action="bind-existing", assignment=assignment, operation=operation, created_utc=now()),
        result=copy.deepcopy(receipt), evidence_root=str(Path(value["evidence_root"]).resolve()),
        receipt_artifact=copy.deepcopy(value["receipt_artifact"]))


def mapping(record, row, result):
    require(result.get("thread") and result.get("environment") and result.get("generation"), "actual dispatch mapping required")
    for other in record["cloud"]["assignments"].values():
        if other["id"] == row["id"] or other.get("accepted"):
            continue
        require(other.get("thread") != result.get("thread")
                and other.get("environment") != result.get("environment")
                and (not result.get("executor") or result["executor"] !=
                     (other.get("executor") or (other.get("binding") or {}).get("executor"))),
                "duplicate live session/environment/executor mapping")
    require(row["thread"] is None or (row["thread"], row["environment"], row["generation"])
            == (result["thread"], result["environment"], result["generation"]), "conflicting dispatch mapping")
    require(not row.get("executor") or not result.get("executor") or row["executor"] == result["executor"],
            "conflicting executor mapping")
    row.update(thread=result["thread"], environment=result["environment"], generation=result["generation"])
    if result.get("executor"):
        row["executor"] = result["executor"]


def recovery_clear(record, row, observation, allow_prior=False):
    assignment = row["id"]
    if allow_prior and row.get("binding"):
        prior = row["binding"]["prior_assignment"]
        if observation.get("recovery", {}).get("assignment") == prior:
            assignment = prior
    recovery = observation.get("recovery", {})
    require((recovery.get("assignment"), recovery.get("environment"), recovery.get("generation"))
            == (assignment, row["environment"], row["generation"])
            and recovery.get("unknown_operations") == [] and recovery.get("build_slot_clear") is True
            and recovery.get("evidence"), "child recovery operations/build slot not reconciled")
    for lane in record.get("lanes", {}).values():
        if lane["assignment"] == assignment:
            require(not any(operation["outcome"] == "unknown" for operation in lane["operations"].values())
                    and (record.get("build_slot") or {}).get("lane") != lane["id"],
                    "matching local recovery operations/build slot not reconciled")


def managed_audit(record, row, repository):
    assignment = row["id"]
    contract = readiness_contract(record, assignment)
    digest = fingerprint(contract)
    identity = (row["thread"], row["environment"], row["generation"])
    operations = [o for o in record["cloud"]["operations"].values()
                  if o["assignment"] == assignment and o["effect"] == "applied"]
    origin = False
    for operation in operations:
        action = operation["request"]["action"]
        result = operation.get("reconciliation") or operation.get("result") or {}
        if action == "bind-existing":
            result = result.get("observation", {})
        if action in {"create", "bind-existing"}:
            origin |= tuple(result.get(k) for k in ("thread", "environment", "generation")) == identity
    require(origin, "managed completion lacks correlated create/binding provenance")
    prerequisites = fingerprint(dict(mapping=list(identity), preflight=row["preflight"],
        handoffs={d["id"]: record["cloud"]["handoffs"].get(d["id"]) for d in dependencies(record, assignment)}))
    for operation in operations:
        request = operation["request"]
        gate = request.get("readiness_gate")
        if (request["action"] != "follow_up" or request.get("parameters", {}).get("phase") != "implementation"
                or request.get("parameters", {}).get("role") == "review"
                or operation.get("acknowledgment", {}).get("status") != "completed"
                or not isinstance(gate, dict) or gate.get("status") != "ready"
                or request.get("readiness_contract") != digest or gate.get("contract") != digest
                or gate.get("prerequisites") != prerequisites):
            continue
        receipt = gate["receipt"]
        before = receipt["before"]
        oracle = next((r for r in row["acceptance"] if r["kind"] == before["kind"]
                       and r["command"] == before["command"]), None)
        if (receipt.get("scope") == contract and oracle and program_failed(before, oracle)
                and fresh(receipt, datetime.fromisoformat(request["created_utc"]))
                and any(e["receipt"] == receipt and e["status"] == "ready" for e in row.get("scouting", []))
                and inputs(repository, receipt["source_revision"], receipt["inputs"])
                    == inputs(repository, source_revision(row), receipt["inputs"])):
            return
    raise RecoveryError("managed completion requires acknowledged implementation and recorded readiness audit trail")


def accept(record, assignment, manifest, observation, repository, root, cleanup_review=False, import_receipt=None):
    row = record["cloud"]["assignments"][assignment]
    require(fresh(observation) and observation.get("revision") == record["revision"], "fresh final observation required")
    require((observation.get("thread"), observation.get("environment"), observation.get("generation"))
            == (row["thread"], row["environment"], row["generation"]) and row["thread"], "final runtime mapping mismatch")
    require(observation.get("runtime") in {"completed", "idle", "stopped"}, "runtime still running/unknown")
    require(not any(o["assignment"] == assignment and o["effect"] == "unknown"
                        and o["request"]["action"] != "read"
                    for o in record["cloud"]["operations"].values()), "unreconciled mutation prevents acceptance")
    require(not any(operation["assignment"] == assignment and operation["request"]["action"] in {"create", "follow_up"}
                    and operation["effect"] == "applied" and not operation.get("acknowledgment")
                    for operation in record["cloud"]["operations"].values()),
            "queued dispatch not acknowledged by child")
    require(not row.get("approval_required") and row["checkpoint"] and row["checkpoint"]["phase"] == "landed", "blocked/partial checkpoint cannot complete")
    require(all(d["id"] in record["cloud"]["handoffs"] for d in dependencies(record, assignment)), "dependency not handed off")
    require(row.get("preflight"), "verified preflight required before acceptance")
    recovery_clear(record, row, observation)
    for dependency in dependencies(record, assignment):
        git(repository, "merge-base", "--is-ancestor", record["cloud"]["handoffs"][dependency["id"]]["candidate"],
            sha(manifest["candidate"]))
    git(repository, "merge-base", "--is-ancestor", source_revision(row), sha(manifest["base"]))
    if import_receipt is None:
        managed_audit(record, row, repository)
        credit = "managed"
    else:
        require(import_receipt.get("schema") == 1 and import_receipt.get("assignment") == assignment
                and import_receipt.get("candidate") == manifest["candidate"] and fresh(import_receipt)
                and import_receipt.get("source") and import_receipt.get("reason"), "explicit reviewed completion import required")
        binding_evidence(root, import_receipt)
        credit = "imported"
    result = validate(repository, root, row, manifest, observation, cleanup_required=not cleanup_review)
    result["manifest_digest"] = fingerprint(manifest)
    result["credit"] = credit
    if cleanup_review:
        row["cleanup_review"] = dict(candidate=result["candidate"], manifest_digest=result["manifest_digest"],
                                     observed_utc=observation["observed_utc"], action="review_owned_cleanup", credit=credit)
        return
    previous = row["accepted"]
    require(previous is None or previous == result or credit == "imported" and "credit" not in previous
            and dict(previous, credit="imported") == result, "conflicting final acceptance")
    if import_receipt is not None:
        old_import = row.get("completion_import")
        require(old_import is None or old_import["receipt"] == import_receipt, "conflicting completion import review")
        if old_import is None:
            row["completion_import"] = dict(receipt=copy.deepcopy(import_receipt), previous_acceptance=previous)
    row["accepted"] = result


def batch_settled(record, batch):
    rows = record["cloud"]["batches"][batch]["manifest"]["assignments"]
    return all((record["cloud"]["assignments"][r["id"]]["accepted"] or {}).get("result") == "landed"
               and record["cloud"]["assignments"][r["id"]]["accepted"].get("user_rescue") is False
               and record["cloud"]["assignments"][r["id"]]["accepted"].get("cleanup") == "completed" for r in rows)


def batch_complete(record, batch):
    manifest = record["cloud"]["batches"][batch]["manifest"]
    return manifest.get("mode", "staged") == "staged" and batch_settled(record, batch) and all(
        record["cloud"]["assignments"][r["id"]]["accepted"].get("credit") == "managed"
        for r in manifest["assignments"])


def plan(record):
    cloud = record["cloud"]
    rows = []
    for name, row in cloud["assignments"].items():
        if row.get("mode") in {"observational-only", "preflight-only"}:
            continuations = [op for op in cloud["operations"].values()
                             if op["assignment"] == name and op["request"]["action"] == "follow_up"]
            action = "inspect_recorded_thread" if row["mode"] == "observational-only" else "prepare_preflight"
            if continuations:
                action = "read_reconcile" if continuations[0]["effect"] == "unknown" else "await_preflight_report"
            if row.get("approval_required"):
                action = "required_approval"
            rows.append(dict(assignment=name, mode=row["mode"], action=action,
                thread=row["thread"], environment=None, generation=None, runtime="unknown", accepted=None,
                observation=row["observation"], executor_readiness_verified=False,
                steps=[step_status(record, name, step) for step in cloud.get("steps", {}).get(name, {})]))
            continue
        pending = [key for key, op in cloud["operations"].items() if op["assignment"] == name and op["effect"] == "unknown" and op["request"]["action"] != "read"]
        waiting = [d["id"] for d in dependencies(record, name) if d["id"] not in cloud["handoffs"]]
        unacknowledged = [key for key, op in cloud["operations"].items()
                          if op["assignment"] == name and op["request"]["action"] in {"create", "follow_up"}
                          and op["effect"] == "applied" and not op.get("acknowledgment")]
        phase = (row["checkpoint"] or {}).get("phase", "preflight")
        if row["accepted"]:
            action = "verified_complete" if row["accepted"].get("credit") == "managed" else "verified_import" if row["accepted"].get("credit") == "imported" else "audit_completion_credit"
        elif row.get("approval_required") or phase == "required_approval":
            action = "required_approval"
        elif pending or unacknowledged:
            action = "read_reconcile"
        elif waiting or phase == "waiting_dependency":
            action = "waiting_dependency"
        elif phase == "blocked":
            action = "blocked"
        elif row.get("cleanup_review") and fresh(row["cleanup_review"]):
            action = "review_owned_cleanup"
        elif row["thread"] is None:
            action = "offer_create"
        elif not row["observation"] or not fresh(row["observation"]):
            action = "read_runtime"
        elif row["runtime"] == "running":
            action = "observe_running"
        elif row["runtime"] == "unknown":
            action = "read_runtime"
        elif not row.get("preflight") or not implementation_started(record, name) and readiness_state(record, name) != "ready":
            action = "scout_or_revalidate"
        else:
            action = "validate_acceptance_or_continue"
        rows.append(dict(assignment=name, action=action, dependencies=waiting, unknown_operations=pending, unacknowledged_operations=unacknowledged,
                         thread=row["thread"], environment=row["environment"], generation=row["generation"],
                         checkpoint=row["checkpoint"], runtime=row["runtime"], accepted=row["accepted"],
                         successor=row.get("successor"), reusable_slot=bool(row.get("binding") and row.get("accepted")
                             and row["accepted"].get("credit") == "managed" and row["accepted"].get("result") == "landed"
                             and row["accepted"].get("cleanup") == "completed"
                             and row["accepted"].get("user_rescue") is False and not row.get("successor")),
                         steps=[step_status(record, name, step) for step in cloud.get("steps", {}).get(name, {})],
                         readiness=readiness_state(record, name), scouting_attempts=len(row.get("scouting", [])),
                         scouting_seconds=sum(e["elapsed_seconds"] for e in row.get("scouting", []))))
    return dict(revision=record["revision"], assignments=rows,
                batches={b: dict(size=v["manifest"]["size"], mode=v["manifest"].get("mode", "staged"),
                                settled=batch_settled(record, b), complete=batch_complete(record, b)) for b, v in cloud["batches"].items()},
                executes_remote_actions=False)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--state", type=Path, required=True)
    parser.add_argument("--revision", type=int)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("status")
    for name in ("init", "observe-existing", "admit-preflight", "admit", "spec-context", "handoff", "checkpoint", "ownership", "preflight", "readiness", "bind-existing", "review-revision", "import-completion", "approval", "intent", "prepare-step", "dispatch-step", "save-step-result", "complete-step", "successor", "ingest", "cleanup-review", "accept"):
        command = commands.add_parser(name)
        command.add_argument("--input", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        store = Store(args.state)
        if args.command == "status":
            result = plan(store.load())
        else:
            require(args.revision is not None, "--revision required; reread current state")
            value = read_json(args.input)
            require(isinstance(value, dict), "JSON object input required")
            request = []
            def change(record):
                if args.command == "init": initialize(record, value)
                elif args.command == "observe-existing": observe_existing(record, value)
                elif args.command == "admit-preflight": admit_preflight(record, value)
                elif args.command == "admit": admit(record, value["manifest"], value["claims"])
                elif args.command == "spec-context": spec_context(record, value)
                elif args.command == "successor": successor(record, value)
                elif args.command == "prepare-step": request.append(prepare_step(record, value))
                elif args.command == "dispatch-step": request.append(dispatch_step(record, value["assignment"], value["step"]))
                elif args.command == "save-step-result": save_step_result(record, value["assignment"], value["step"], value["result"])
                elif args.command == "complete-step": complete_step(record, value["assignment"], value["step"])
                elif args.command == "handoff": handoff(record, value)
                elif args.command == "checkpoint": checkpoint(record, value["assignment"], value["checkpoint"])
                elif args.command == "ownership": ownership(record, value["assignment"], value["claims"])
                elif args.command == "preflight": preflight(record, value["assignment"], value["receipt"])
                elif args.command == "readiness": readiness(record, value["assignment"], value["receipt"], value["repository"], value["evidence_root"])
                elif args.command == "bind-existing": bind_existing(record, value)
                elif args.command == "review-revision": review_revision(record, value["assignment"], value["receipt"], value["repository"], value["evidence_root"])
                elif args.command == "approval": approval(record, value["assignment"], value["receipt"])
                elif args.command == "intent": request.append(intent(record, value["assignment"], value["action"], value["operation"], value.get("parameters", {})))
                elif args.command == "ingest": ingest(record, value)
                elif args.command in {"accept", "cleanup-review", "import-completion"}: accept(record, value["assignment"], value["manifest"], value["observation"], value["repository"], value["evidence_root"], cleanup_review=args.command == "cleanup-review",
                    import_receipt=value["import_receipt"] if args.command == "import-completion" else value.get("import_receipt") if args.command == "cleanup-review" else None)
            updated = store.update(args.revision, change)
            result = dict(revision=updated["revision"], request=request[0]) if request else plan(updated)
        print(json.dumps(result, indent=2))
        return 0
    except (RecoveryError, OSError, KeyError, TypeError, ValueError, AttributeError, subprocess.TimeoutExpired) as error:
        print(json.dumps(dict(error=str(error), action="preserve-and-reconcile")))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
