#!/usr/bin/env python3
"""Pinned qualification and measured, dependency-aware requalification.

A passing command belongs to the bytes it actually consumed, not the latest
main SHA. Capture before/after identities while running on one clean revision;
after integration compare each reviewed input closure and retain the original
observation when unchanged. This never replaces landing's exact-SHA build check,
independent final review, package authority or remote publication freshness.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import stat
import subprocess
import sys
import time
import uuid

sys.path.insert(0, str(Path(__file__).resolve().parent / "swarm"))
from recovery_state import RecoveryError, atomic_json, exclusive, read_json

SCHEMA = "omega-qualification-v1"
PROFILE_KEYS = ("CARGO_TARGET_DIR", "CARGO_BUILD_JOBS", "RUSTFLAGS",
                "CARGO_PROFILE_DEV_OPT_LEVEL", "CARGO_PROFILE_DEV_DEBUG",
                "CARGO_PROFILE_TEST_OPT_LEVEL", "CARGO_PROFILE_TEST_DEBUG")


def require(condition, reason):
    if not condition:
        raise RecoveryError(reason)


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, allow_nan=False).encode()).hexdigest()


def file_digest(path):
    value = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def is_capture_link(information):
    # Path.is_junction only exists on Python 3.12+. lstat's Windows reparse
    # tag works on the supported older hosts too; unknown reparses cannot
    # silently inherit ordinary-directory capture authority.
    tag = getattr(information, "st_reparse_tag", 0)
    mount = getattr(stat, "IO_REPARSE_TAG_MOUNT_POINT", 0xA0000003)
    symbolic = getattr(stat, "IO_REPARSE_TAG_SYMLINK", 0xA000000C)
    require(not tag or tag in {mount, symbolic}, "unsupported artifact reparse point")
    return stat.S_ISLNK(information.st_mode) or tag in {mount, symbolic}


def capture_path_chain(value):
    path = Path(value).absolute()
    places = {path}
    current = Path(path.anchor)
    pending = list(path.parts[len(current.parts):])
    links = 0
    while pending:
        component = pending.pop(0)
        if component == "..":
            current = current.parent
            continue
        if component == ".":
            continue
        selected = current / component
        information = selected.lstat()
        if is_capture_link(information):
            places.add(selected)
            links += 1
            require(links <= 40, "cyclic/excessive artifact link chain")
            target = selected.readlink()
            if target.is_absolute():
                current = Path(target.anchor)
                pending = list(target.parts[len(current.parts):]) + pending
            else:
                pending = list(target.parts) + pending
        else:
            current = selected
    places.add(current)
    return places


def artifact_stamps(paths):
    # A concurrent writer may restore identical bytes before command exit.
    # Inode identity and kernel change timestamps still expose that mutation;
    # byte equality alone cannot establish a stable capture interval. These
    # stamps constrain one run, not reuse of independently equal later bytes.
    result = {}
    places = set()
    for value in paths:
        # Record the declared link and every traversed link, not only its
        # resolved referent. A restored link can otherwise hide another input.
        places.update(capture_path_chain(value))
    for path in sorted(places):
        stat = path.lstat()
        result[str(path)] = [stat.st_dev, stat.st_ino, stat.st_size, stat.st_mtime_ns, stat.st_ctime_ns]
    return result


def prepared_inputs(paths):
    """Exact prepared files/directory membership; outputs must live elsewhere."""
    files, directories = {}, {}
    for value in paths:
        root = Path(value)
        require(root.is_absolute() and root.exists(), "prepared input unavailable: " + value)
        places = [root] + (list(root.rglob("*")) if root.is_dir() else [])
        require(len(places) <= 20000, "prepared input closure exceeds 20000 entries")
        for path in places:
            require(not (path.is_symlink() and path.is_dir()), "prepared directory link must be resolved explicitly")
            if path.is_dir():
                directories[str(path)] = sorted(item.name for item in path.iterdir())
            else:
                require(path.is_file(), "prepared input must be a regular file or directory")
                files[str(path)] = file_digest(path)
    return dict(files=files, directories=directories)


def integration_seal(record, candidate):
    """A frozen selection adds no registry or alternative publication authority."""
    seal = record.get("integration")
    if not seal:
        return
    path = Path(seal["path"])
    require(path.is_file() and file_digest(path) == seal["sha256"], "integration seal changed")
    value = read_json(path)
    require(value["candidate"] == candidate and value["config_digest"] == record["config_digest"],
            "integration candidate/config moved; explicitly reseal after preparation and review")
    require(value["prepared"] == prepared_inputs(value["prepared_paths"]),
            "prepared package inputs changed; preserve evidence and prepare/reseal before capture")
    require(file_digest(value["review"]["path"]) == value["review"]["sha256"], "prospective integration review changed")
    for member in value["members"]:
        require(file_digest(member["manifest_path"]) == member["manifest_sha256"], "selected submission changed")
        require(file_digest(member["receive_path"]) == member["receive_sha256"], "submission receive receipt changed")
        received = read_json(member["receive_path"])
        require(received["payload_stamps"] == artifact_stamps(received["payload_paths"]), "received evidence mutated")
    for path, expected in value.get("producer_receipts", {}).items():
        require(file_digest(path) == expected, "shared producer receipt changed")
    return value


def git(repository, *arguments, allow_failure=False):
    result = subprocess.run(["git", "-C", str(repository), *arguments],
                            capture_output=True, text=True, timeout=30)
    require(allow_failure or result.returncode == 0, result.stderr.strip() or "Git read failed")
    return result.stdout.strip()


def revision(repository, value):
    require(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{40}", value), "full Git SHA required")
    require(git(repository, "rev-parse", value + "^{commit}") == value, "not an exact commit")
    return value


def clean_head(repository):
    require(not git(repository, "status", "--porcelain"), "qualification requires a clean checkpoint")
    return git(repository, "rev-parse", "HEAD")


def retain_source(repository, path, commit):
    # Rebase/amend can make the observed checkpoint unreachable from main.
    # A local evidence ref keeps its objects available without a worker branch
    # or remote mutation. Never replace a different object under that identity.
    namespace = hashlib.sha256(str(Path(path).resolve()).encode()).hexdigest()[:24]
    reference = "refs/qualification/" + namespace + "/" + commit
    result = subprocess.run(["git", "-C", str(repository), "show-ref", "--verify", "--hash", reference],
                            capture_output=True, text=True, timeout=30)
    require(result.returncode != 0 or result.stdout.strip() == commit, "qualification source ref mismatch")
    git(repository, "update-ref", reference, commit, commit if result.returncode == 0 else "0" * 40)
    return reference


def source_inputs(repository, commit, paths):
    revision(repository, commit)
    result = {}
    for path in paths:
        query = subprocess.run(["git", "-C", str(repository), "rev-parse", commit + ":" + path],
                               capture_output=True, text=True, timeout=30)
        result[path] = query.stdout.strip() if query.returncode == 0 else None
    return result


def input_paths(check):
    return sorted(set(check["inputs"] + check.get("producer", {}).get("inputs", [])))


def source_stamps(repository, commit, paths):
    names = git(repository, "ls-tree", "-r", "-z", "--name-only", commit, "--", *paths).split("\0")
    root = Path(repository).resolve()
    places = {root / path for path in paths}
    for name in names:
        if not name:
            continue
        leaf = root / name
        places.add(leaf)
        # Declared source trees also bind directory membership during the run.
        # This catches add/read/remove even if Git is clean again at exit.
        places.update(parent for parent in leaf.parents if parent != root and root in parent.parents)
    return artifact_stamps([str(path) for path in places])


def artifacts(repository, check):
    # Hash the actual launcher too: an unchanged command spelling cannot hide
    # a changed executable on PATH. Linked producer libraries must be listed in
    # the reviewed closure, not inferred from a log's asserted compiler name.
    executable = shutil.which(check["command"][0])
    if not executable:
        candidate = Path(repository) / check["command"][0]
        executable = str(candidate) if candidate.is_file() else None
    require(executable, "command executable unavailable")
    paths = [str(Path(executable).absolute())] + check["artifacts"]
    result = {}
    for value in paths:
        path = Path(value)
        require(path.is_absolute() and path.is_file(), "artifact must name an existing absolute file: " + value)
        result[str(path.absolute())] = file_digest(path)
    return result


def environment(context):
    return dict(context=context, system=platform.system(), machine=platform.machine(),
                python=platform.python_version(), profiles={key: os.environ.get(key) for key in PROFILE_KEYS})


def validate_contracts(contracts):
    require(isinstance(contracts, list), "semantic contracts must be a compact explicit mapping")
    for row in contracts:
        require(isinstance(row, dict) and isinstance(row.get("contract"), str)
                and row["contract"].startswith("wiki/spec/") and row.get("implementation")
                and row.get("reason"), "semantic repair needs implementation paths and a canonical contract")
        require(row.get("classification") in {"missing_implementation", "owner_decision"},
                "distinguish missing implementation from a language change")
        require(isinstance(row.get("passages"), list) and row["passages"]
                and all(isinstance(text, str) and 0 < len(text) <= 2000 for text in row["passages"])
                and sum(len(text) for text in row["passages"]) <= 6000,
                "focused canonical passages required; a link is not implementation context")
        if row["classification"] == "owner_decision":
            require(row.get("authorization"), "new semantics require the explicit owner's decision")


def definition(config):
    require(isinstance(config, dict) and config.get("context"), "explicit host/target/toolchain context required")
    checks = config.get("checks")
    require(isinstance(checks, list) and checks, "nonempty reviewed check list required")
    contracts = config.get("semantic_contracts", [])
    validate_contracts(contracts)
    if contracts:
        require(any(check.get("kind") == "review" and check.get("final") is True for check in checks),
                "semantic contract mapping requires independent final-candidate review")
    names = []
    for check in checks:
        require(isinstance(check, dict), "invalid check definition")
        name = check.get("id")
        require(isinstance(name, str) and re.fullmatch(r"[a-zA-Z0-9_-]+", name), "invalid check ID")
        names.append(name)
        require(check.get("kind") in {"build", "tests", "native", "terminal", "negative", "review"},
                "explicit check kind required")
        require(isinstance(check.get("command"), list) and check["command"]
                and all(isinstance(value, str) and value for value in check["command"]), "argv command required")
        require(isinstance(check.get("inputs"), list) and check["inputs"], "complete reviewed input closure required")
        check["inputs"] = sorted(set(check["inputs"] + [row["contract"] for row in contracts]))
        producer = check.get("producer", {})
        if check["kind"] in {"native", "terminal"} or producer:
            require(isinstance(producer, dict) and producer.get("inputs") and producer.get("revision")
                    and check.get("artifacts"), "compiler-backed check needs producer revision/inputs and artifacts")
        for path in input_paths(check):
            require(isinstance(path, str) and path and not path.startswith(("/", ":"))
                    and ".." not in Path(path).parts and "\\" not in path,
                    "unsafe Git input path")
        require(isinstance(check.get("artifacts"), list), "explicit artifact list required (empty when none)")
        require(isinstance(check.get("prepared_inputs", []), list)
                and all(isinstance(value, str) and Path(value).is_absolute()
                        for value in check.get("prepared_inputs", [])), "prepared input paths must be absolute")
        require(check.get("closure_reason"), "reviewed dependency closure justification required")
        require(type(check.get("final")) is bool, "explicit final-candidate check policy required")
        require(type(check.get("expected_exit")) is int, "expected command exit required")
        require(type(check.get("timeout_seconds")) is int and 1 <= check["timeout_seconds"] <= 36000,
                "bounded command timeout required")
    require(len(set(names)) == len(names), "duplicate check IDs")
    return config


def contract_mapping(repository, config, base, candidate):
    validate_contracts(config.get("semantic_contracts", []))
    result = []
    for row in config.get("semantic_contracts", []):
        path = row["contract"]
        authority = source_inputs(repository, base, [path])[path]
        require(authority and authority == source_inputs(repository, candidate, [path])[path],
                "canonical contract must exist in the integration base; candidate spec edits are not retroactive authority")
        text = git(repository, "show", base + ":" + path)
        require(all(passage in text for passage in row["passages"]),
                "canonical passage missing/stale; identify the semantic gap before implementation")
        result.append(dict(row, revision=base, blob=authority))
    return result


def handoff(repository, config, base, role, previous=None, evidence=None):
    """Focused implementation/review packets and fingerprinted continuation."""
    require(role in {"implementation", "review", "continuation"}, "explicit handoff role required")
    candidate = git(repository, "rev-parse", "HEAD")
    context = contract_mapping(repository, config, base, candidate)
    # Revision-only advances do not change the retrieved contract or mapping.
    fingerprint = digest([{key: value for key, value in row.items() if key != "revision"}
                          for row in context])
    if previous is not None:
        require(previous.get("schema") == "qualification-handoff-1"
                and previous.get("context_fingerprint") == digest([
                    {key: value for key, value in row.items() if key != "revision"}
                    for row in previous.get("canonical_context", [])]),
                "missing/corrupt prior handoff context; regenerate the focused packet")
    refresh = role != "continuation" or previous is None or previous["context_fingerprint"] != fingerprint
    return dict(schema="qualification-handoff-1", role=role, candidate=candidate, source_revision=base,
                changed_paths=git(repository, "diff", "--name-only", base, candidate).splitlines(),
                canonical_context=context, context_fingerprint=fingerprint,
                reinject_context=context if refresh else [], context_refresh=refresh,
                checks=config.get("checks", []), assessments=(evidence or {}).get("assessments", []),
                review_instruction="Judge behavior against these passages; uncovered semantics need an explicit owner decision. "
                "Inspect the candidate diff and check evidence; passage identity alone is not semantic compliance.")


def work_classification(config):
    classification = config.get("work_classification", {"kind": "semantic"})
    require(isinstance(classification, dict) and classification.get("kind") in {"semantic", "non_semantic"},
            "work classification must be semantic or explicit non_semantic")
    if classification["kind"] == "non_semantic":
        require(isinstance(classification.get("reason"), str) and classification["reason"].strip()
                and not config.get("semantic_contracts"),
                "non_semantic work needs a concrete reason and cannot also declare semantic contracts")
    else:
        require(config.get("semantic_contracts"),
                "compiler work requires focused semantic_contracts; classify genuine non-semantic work explicitly")
        validate_contracts(config["semantic_contracts"])
    return classification


def work_handoff(repository, config, base, role, previous=None, retained=False):
    """Default semantic dispatch; uncertain prior delivery reinjects passages."""
    classification = work_classification(config)
    require(role in {"implementation", "review", "continuation"}, "explicit dispatch role required")
    if classification["kind"] == "non_semantic":
        packet = dict(schema="qualification-work-context-1", role=role, work_classification=classification)
        if repository is not None:
            packet.update(candidate=git(repository, "rev-parse", "HEAD"), source_revision=base)
        return packet
    packet = handoff(repository, config, base, role, previous if retained else None)
    if not retained:
        packet["context_refresh"] = True
        packet["reinject_context"] = packet["canonical_context"]
    packet["prior_context_retained"] = bool(retained and previous is not None)
    return packet


def work_context_prompt(packet):
    role = packet["role"]
    if role == "review":
        instructions = ("Independent read-only final review. Read AGENTS.md. Inspect the exact candidate diff, "
                        "canonical passages and captured check evidence. Report exact-candidate approval or concrete blockers. "
                        "Do not edit files, claim implementation ownership or publish. The implementation owner retains landing.")
    else:
        instructions = ("Retain implementation ownership through validation, independent final review and normal verified landing. "
                        "Map behavior to the canonical passages; uncovered semantics require an explicit owner decision.")
    if packet.get("work_classification", {}).get("kind") == "non_semantic":
        return instructions + "\nExplicit non-semantic work context:\n" + json.dumps(packet, indent=2) + "\n"
    visible = {key: value for key, value in packet.items() if key != "canonical_context"}
    return instructions + "\nValidated focused work context:\n" + json.dumps(visible, indent=2) + (
        "\nPassage identity does not prove semantic compliance.\n" if packet["context_refresh"] else
        "\nFocused canonical continuation context verified unchanged; reuse the retained passages. "
        "Semantic compliance still requires independent judgment.\n")


def load(path):
    record = read_json(path)
    require(record.get("schema") == SCHEMA and type(record.get("revision")) is int,
            "unsupported qualification record")
    definition(record["config"])
    require(record["config_digest"] == digest(record["config"]), "qualification definition mismatch")
    return record


def update(path, expected, change):
    with exclusive(str(path) + ".lock"):
        record = load(path)
        require(record["revision"] == expected, "stale qualification revision; reread, do not overwrite")
        change(record)
        record["revision"] += 1
        atomic_json(path, record)
        return record


def start(repository, path, base, config):
    repository = Path(repository).resolve()
    head = clean_head(repository)
    revision(repository, base)
    subprocess.run(["git", "-C", str(repository), "merge-base", "--is-ancestor", base, head], check=True)
    definition(config)
    canonical_context = contract_mapping(repository, config, base, head)
    active = repository / "build" / "qualification-active.json"
    with exclusive(str(active) + ".lock"), exclusive(str(path) + ".lock"):
        require(not Path(path).exists(), "qualification already exists; retain its history")
        require(not active.exists(), "an active qualification already owns this worktree")
        record = dict(schema=SCHEMA, revision=0, repository=str(repository), base=base,
                      qualification_revision=head, config=config, config_digest=digest(config),
                      canonical_context=canonical_context, runs=[], assessments=[], running=None)
        record["source_ref"] = retain_source(repository, path, head)
        atomic_json(path, record)
        # Register the active assessment in this worktree so ordinary verify
        # cannot accidentally forget it after a rebase makes a new SHA.
        atomic_json(active, dict(state=str(Path(path).resolve())))
        return record


def active_path(repository):
    path = Path(repository) / "build" / "qualification-active.json"
    if not path.exists():
        return None
    registration = read_json(path)
    require(isinstance(registration, dict) and isinstance(registration.get("state"), str)
            and Path(registration["state"]).is_absolute() and Path(registration["state"]).is_file(),
            "missing/malformed active qualification registration")
    return registration["state"]


def amend(path, expected, config, reason):
    definition(config)
    require(reason, "amendment needs a reviewed reason")
    def change(record):
        require(not record["running"], "cannot amend while qualification command runs")
        base = record["assessments"][-1]["base"] if record["assessments"] else record["base"]
        record["canonical_context"] = contract_mapping(record["repository"], config, base, clean_head(record["repository"]))
        record.setdefault("definitions", []).append(dict(config=record["config"],
            digest=record["config_digest"], reason=reason, revision=record["revision"]))
        record["config"] = config
        record["config_digest"] = digest(config)
    return update(path, expected, change)


def reasons(repository, record, check, run, candidate):
    if not run:
        return ["not_run"]
    result = []
    receipt = run.get("receipt", {})
    receipt_path = Path(receipt.get("path", ""))
    if (not receipt_path.is_file() or file_digest(receipt_path) != receipt.get("sha256")
            or read_json(receipt_path) != {key: value for key, value in run.items() if key != "receipt"}):
        result.append("observation_record_mismatch")
    if run.get("definition") != digest(check):
        result.append("check_definition_changed")
    if run.get("exit") != check["expected_exit"] or run.get("invalidations"):
        result.append("prior_run_not_accepted")
    if run.get("source_inputs") != source_inputs(repository, run["revision"], input_paths(check)):
        result.append("recorded_source_identity_mismatch")
    current = source_inputs(repository, candidate, input_paths(check))
    for name in input_paths(check):
        if run.get("source_inputs", {}).get(name) != current[name]:
            result.append("source_changed:" + name)
    if run.get("artifacts") != artifacts(repository, check):
        result.append("compiler_or_artifact_changed")
    if run.get("environment") != environment(record["config"]["context"]):
        result.append("environment_changed")
    if run.get("producer") != check.get("producer"):
        result.append("producer_identity_mismatch")
    if run.get("prepared_inputs", prepared_inputs([])) != prepared_inputs(check.get("prepared_inputs", [])):
        result.append("prepared_inputs_changed")
    for key in ("stdout", "stderr"):
        value = run.get(key, {})
        path = Path(value.get("path", ""))
        if not path.is_file() or file_digest(path) != value.get("sha256"):
            result.append("observation_changed:" + key)
    if check["final"] and run["revision"] != candidate:
        result.append("final_candidate_required")
    return result


def source_changes(repository, observed, candidate):
    """Complete blob delta, including paths outside a check's input closure."""
    result = subprocess.run(["git", "-C", str(repository), "diff", "--no-renames", "--raw", "--no-abbrev", "-z", observed, candidate],
                            capture_output=True, text=True, timeout=30)
    require(result.returncode == 0, result.stderr.strip() or "Git blob delta failed")
    fields = result.stdout.split("\0")
    require(fields[-1] == "" and (len(fields) - 1) % 2 == 0, "invalid Git blob delta")
    changes = {}
    for index in range(0, len(fields) - 1, 2):
        header, name = fields[index].split(), fields[index + 1]
        require(len(header) == 5 and header[0].startswith(":") and name not in changes,
                "invalid or duplicate Git blob delta entry")
        before, after = header[2:4]
        require(all(re.fullmatch(r"[0-9a-f]{40}", value) for value in (before, after)), "incomplete Git blob identity")
        changes[name] = dict(before=None if before == "0" * 40 else before,
                             after=None if after == "0" * 40 else after)
    return changes


def reuse_binding(repository, record, check, run, candidate, base):
    return dict(check=check["id"], run=run["id"], receipt=run["receipt"],
                observed_revision=run["revision"], candidate=candidate, base=base,
                definition=digest(check), config_digest=record["config_digest"],
                changes=source_changes(repository, run["revision"], candidate))


def attest_source_reuse(repository, path, expected, name, base, review_path):
    """Record independent semantic scope review; never manufacture a new run.

    Whole-file identities conservatively invalidate unexercised branches too.
    The reviewer must bind the complete old/new blob delta to the exact accepted
    observation. This only covers source-change reasons in planning. Capture's
    producer/source equality and every non-source invalidation remain intact.
    """
    candidate = clean_head(repository)
    revision(repository, base)
    require(git(repository, "merge-base", base, candidate) == base, "reuse base is not an ancestor")
    review_path = Path(review_path).resolve()
    before = artifact_stamps([review_path])
    review = read_json(review_path)
    review_digest = file_digest(review_path)
    require(before == artifact_stamps([review_path]), "reuse review changed during capture")
    def change(record):
        require(record["repository"] == str(Path(repository).resolve()), "qualification worktree mismatch")
        require(not record["running"], "cannot attest while qualification command runs")
        selected = [row for row in record["config"]["checks"] if row["id"] == name]
        require(len(selected) == 1, "unknown source reuse check")
        check = selected[0]
        require(not check["final"], "final-candidate checks cannot receive source reuse attestations")
        runs = [row for row in record["runs"] if row["check"] == name]
        require(runs, "source reuse needs a prior observation")
        run = runs[-1]
        invalidations = reasons(repository, record, check, run, candidate)
        require(invalidations and all(value.startswith("source_changed:") for value in invalidations),
                "source reuse requires an accepted observation with only source changes")
        binding = reuse_binding(repository, record, check, run, candidate, base)
        require(review.get("binding") == binding and review.get("accepted") is True
                and isinstance(review.get("reviewer"), str) and review["reviewer"].strip()
                and isinstance(review.get("rationale"), str) and review["rationale"].strip(),
                "independent reuse review must bind the exact observation, candidate and complete blob delta")
        require(clean_head(repository) == candidate, "candidate changed during reuse capture")
        require(before == artifact_stamps([review_path]), "reuse review changed during registration")
        value = dict(schema="qualification-source-reuse-1", binding=binding,
                     covered_reasons=invalidations,
                     review=dict(path=str(review_path), sha256=review_digest))
        destination = Path(path).with_suffix(".records") / (uuid.uuid4().hex + ".reuse.json")
        require(not destination.exists(), "reuse receipt already exists")
        atomic_json(destination, value)
        record.setdefault("source_reuse", []).append(dict(path=str(destination), sha256=file_digest(destination)))
    return update(path, expected, change)


def reviewed_source_reuse(repository, record, check, run, candidate, base):
    if check["final"]:
        return None
    for receipt in reversed(record.get("source_reuse", [])):
        try:
            if file_digest(receipt["path"]) != receipt["sha256"]:
                continue
            value = read_json(receipt["path"])
            if value.get("schema") != "qualification-source-reuse-1":
                continue
            binding = value["binding"]
            if not isinstance(binding, dict):
                continue
            if (binding.get("check"), binding.get("run"), binding.get("candidate"), binding.get("base")) != (
                    check["id"], run["id"], candidate, base):
                continue
            if value["binding"] != reuse_binding(repository, record, check, run, candidate, base):
                continue
            review = value["review"]
            if file_digest(review["path"]) != review["sha256"]:
                continue
            # Registration validated this review; retain that binding on every
            # assessment and again at exact-candidate verification/publication.
            return dict(receipt=receipt, covered_reasons=value["covered_reasons"], review=review)
        except (OSError, ValueError, KeyError, RecoveryError):
            continue
    return None


def plan(repository, record, candidate, base=None):
    rows = []
    for check in record["config"]["checks"]:
        run = next((row for row in reversed(record["runs"]) if row["check"] == check["id"]), None)
        invalidations = reasons(repository, record, check, run, candidate)
        attestation = reviewed_source_reuse(repository, record, check, run, candidate, base) if run else None
        if attestation:
            invalidations = [value for value in invalidations
                             if not (value.startswith("source_changed:")
                                     and value in attestation["covered_reasons"])]
        rows.append(dict(check=check["id"], action="rerun" if invalidations else "reuse",
                         reasons=invalidations or ["reviewed_source_scope_unchanged" if attestation else "reviewed_inputs_unchanged"],
                         evidence=run["id"] if run else None,
                         observed_revision=run["revision"] if run else None))
        if attestation:
            rows[-1]["source_reuse"] = attestation
    return rows


def run_check(repository, path, expected, name):
    started = time.monotonic()
    head = clean_head(repository)
    record = load(path)
    require(record["repository"] == str(Path(repository).resolve()), "qualification worktree mismatch")
    allowed = record["assessments"][-1]["candidate"] if record["assessments"] else record["qualification_revision"]
    require(head == allowed, "HEAD moved; assess the integrated candidate before requalification")
    frozen = integration_seal(record, head)
    check = next((row for row in record["config"]["checks"] if row["id"] == name), None)
    require(check, "unknown check")
    producer = check.get("producer")
    if producer:
        require(source_inputs(repository, producer["revision"], producer["inputs"])
                == source_inputs(repository, head, producer["inputs"]),
                "producer source/dependencies do not match the qualification revision")
    identities = dict(source_inputs=source_inputs(repository, head, input_paths(check)), producer=producer,
                      artifacts=artifacts(repository, check), environment=environment(record["config"]["context"]))
    if frozen and check.get("producer_group"):
        expected_artifacts = frozen["producer_artifacts"][check["producer_group"]]
        require(all(identities["artifacts"].get(path) == expected
                    for path, expected in expected_artifacts.items()), "shared producer artifacts changed after freeze")
    if check.get("prepared_inputs"):
        identities["prepared_inputs"] = prepared_inputs(check["prepared_inputs"])
        prepared_paths = list(identities["prepared_inputs"]["files"]) + list(identities["prepared_inputs"]["directories"])
        identities["prepared_stamps"] = artifact_stamps(prepared_paths)
    identities["source_ref"] = retain_source(repository, path, head)
    require(all(identities["source_inputs"].values()), "declared source dependency absent; review the closure")
    identities["artifact_stamps"] = artifact_stamps(identities["artifacts"])
    identities["source_stamps"] = source_stamps(repository, head, input_paths(check))
    require(identities["artifacts"] == artifacts(repository, check)
            and identities["artifact_stamps"] == artifact_stamps(identities["artifacts"]),
            "artifacts changed while establishing capture inputs")
    prior = next((row for row in reversed(record["runs"]) if row["check"] == name), None)
    repeated = bool(prior and not reasons(repository, record, check, prior, head))
    operation = uuid.uuid4().hex
    def reserve(value):
        require(not value["running"], "qualification command already running; reconcile it first")
        value["running"] = dict(id=operation, check=name, candidate=head)
    reserved = update(path, expected, reserve)
    directory = Path(path).parent / (Path(path).stem + ".records")
    directory.mkdir(parents=True, exist_ok=True)
    stdout, stderr = directory / (operation + ".stdout"), directory / (operation + ".stderr")
    invalidations = []
    with stdout.open("wb") as output, stderr.open("wb") as errors:
        try:
            completed = subprocess.run(check["command"], cwd=repository, stdin=subprocess.DEVNULL,
                                       stdout=output, stderr=errors, timeout=check["timeout_seconds"])
            exit_code = completed.returncode
        except (OSError, subprocess.TimeoutExpired) as error:
            errors.write(str(error).encode())
            exit_code = None
            invalidations.append("command_unfinished")
    try:
        if clean_head(repository) != head:
            invalidations.append("revision_changed_during_run")
        if identities["source_inputs"] != source_inputs(repository, head, input_paths(check)):
            invalidations.append("source_identity_changed_during_run")
        if identities["source_stamps"] != source_stamps(repository, head, input_paths(check)):
            invalidations.append("source_mutated_during_run")
        if identities["artifacts"] != artifacts(repository, check):
            invalidations.append("artifact_changed_during_run")
        if identities["artifact_stamps"] != artifact_stamps(identities["artifacts"]):
            invalidations.append("artifact_mutated_during_run")
        if identities["environment"] != environment(record["config"]["context"]):
            invalidations.append("environment_changed_during_run")
        if check.get("prepared_inputs"):
            if identities["prepared_inputs"] != prepared_inputs(check["prepared_inputs"]):
                invalidations.append("prepared_inputs_changed_during_run")
            if identities["prepared_stamps"] != artifact_stamps(prepared_paths):
                invalidations.append("prepared_inputs_mutated_during_run")
    except (RecoveryError, OSError) as error:
        invalidations.append("inputs_unavailable_during_run:" + str(error))
    observation = dict(id=operation, check=name, revision=head, definition=digest(check),
                       **identities, exit=exit_code, invalidations=invalidations,
                       duration_seconds=time.monotonic() - started, repeated_work=repeated,
                       utc=datetime.now(timezone.utc).isoformat(),
                       command=check["command"],
                       stdout=dict(path=str(stdout.resolve()), sha256=file_digest(stdout)),
                       stderr=dict(path=str(stderr.resolve()), sha256=file_digest(stderr)))
    observation_path = directory / (operation + ".json")
    atomic_json(observation_path, observation)
    observation["receipt"] = dict(path=str(observation_path.resolve()), sha256=file_digest(observation_path))
    def finish(value):
        require(value["running"] and value["running"]["id"] == operation, "command reservation changed")
        value["runs"].append(observation)
        value["running"] = None
    result = update(path, reserved["revision"], finish)
    return result, exit_code == check["expected_exit"] and not invalidations


def assess(repository, path, expected, base):
    started = time.monotonic()
    candidate = clean_head(repository)
    revision(repository, base)
    def change(record):
        require(record["repository"] == str(Path(repository).resolve()), "qualification worktree mismatch")
        require(not record["running"], "cannot integrate while qualification command runs")
        for before, after in ((record["base"], base), (base, candidate)):
            result = subprocess.run(["git", "-C", str(repository), "merge-base", "--is-ancestor", before, after])
            require(result.returncode == 0, "integration base is not a compatible ancestor")
        canonical_context = contract_mapping(repository, record["config"], base, candidate)
        rows = plan(repository, record, candidate, base)
        record["assessments"].append(dict(candidate=candidate, base=base, checks=rows,
            canonical_context=canonical_context,
            incoming_paths=git(repository, "diff", "--name-only", record["base"], base).splitlines(),
            duration_seconds=time.monotonic() - started,
            reused=len([row for row in rows if row["action"] == "reuse"]),
            invalidated=len([row for row in rows if row["action"] == "rerun"])))
    return update(path, expected, change)


def validate(repository, path, candidate, base):
    with exclusive(str(path) + ".lock"):
        return _validate(repository, path, candidate, base)


def _validate(repository, path, candidate, base):
    record = load(path)
    require(record["repository"] == str(Path(repository).resolve()), "qualification worktree mismatch")
    require(not record["running"] and record["assessments"], "qualification assessment unfinished")
    assessment = record["assessments"][-1]
    require(assessment["candidate"] == candidate and assessment["base"] == base,
            "qualification assessment covers a stale candidate/base")
    require(clean_head(repository) == candidate, "qualification candidate is not clean HEAD")
    integration_seal(record, candidate)
    rows = plan(repository, record, candidate, base)
    require(all(row["action"] == "reuse" for row in rows),
            "qualification requires affected/final checks: " + json.dumps(rows))
    return dict(path=str(Path(path).resolve()), sha256=file_digest(path),
                candidate=candidate, base=base, checks=rows)


def main(arguments=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("context", "handoff", "start", "amend", "run", "assess", "run-pending", "status", "attest-source-reuse", "source-reuse-context"))
    parser.add_argument("--repository", default=".")
    parser.add_argument("--state")
    parser.add_argument("--revision", type=int)
    parser.add_argument("--base")
    parser.add_argument("--config")
    parser.add_argument("--check")
    parser.add_argument("--reason")
    parser.add_argument("--role", choices=("implementation", "review", "continuation"))
    parser.add_argument("--previous", help="prior handoff JSON for focused continuation fingerprint comparison")
    parser.add_argument("--review", help="independent source-reuse review JSON with the exact observation/blob binding")
    options = parser.parse_args(arguments)
    try:
        repository = Path(options.repository).resolve()
        if options.command in {"context", "handoff"}:
            record = None
            if options.state:
                record = load(Path(options.state).resolve())
                base = options.base or (record["assessments"][-1]["base"] if record["assessments"] else record["base"])
                config = record["config"]
            else:
                require(options.base and options.config, "context needs --state or --base/--config")
                base, config = options.base, definition(read_json(options.config))
            if options.command == "handoff":
                require(options.role, "handoff needs --role")
                packet = work_handoff(repository, config, base, options.role,
                                      read_json(options.previous) if options.previous else None)
                if record and packet.get("canonical_context") is not None:
                    packet["assessments"] = record.get("assessments", [])
                print(json.dumps(packet, indent=2))
                return 0
            context = contract_mapping(repository, config, base, git(repository, "rev-parse", "HEAD"))
            print(json.dumps(dict(source_revision=base, canonical_context=context), indent=2))
            return 0
        require(options.state, "qualification state path required")
        path = Path(options.state).resolve()
        if options.command == "start":
            require(options.base and options.config, "start needs --base and --config")
            result = start(repository, path, options.base, read_json(options.config))
        elif options.command == "status":
            result = load(path)
        elif options.command == "source-reuse-context":
            require(options.base and options.check, "reuse context needs --base and --check")
            record = load(path)
            checks = [row for row in record["config"]["checks"] if row["id"] == options.check]
            runs = [row for row in record["runs"] if row["check"] == options.check]
            require(len(checks) == 1 and runs, "reuse context needs a known observed check")
            result = dict(binding=reuse_binding(repository, record, checks[0], runs[-1], clean_head(repository),
                                                revision(repository, options.base)),
                          accepted=False, reviewer=None, rationale=None)
        else:
            require(options.revision is not None, "mutation requires --revision from the last record")
            if options.command == "amend":
                require(options.config, "amend needs --config")
                result = amend(path, options.revision, read_json(options.config), options.reason)
                print(json.dumps(result, indent=2))
                return 0
            if options.command == "run":
                require(options.check, "run needs --check")
                result, accepted = run_check(repository, path, options.revision, options.check)
                print(json.dumps(result, indent=2))
                return 0 if accepted else 1
            if options.command == "attest-source-reuse":
                require(options.base and options.check and options.review, "source reuse needs --base, --check and --review")
                result = attest_source_reuse(repository, path, options.revision, options.check, options.base, options.review)
                print(json.dumps(result, indent=2))
                return 0
            require(options.base, "assess needs the integrated --base")
            result = assess(repository, path, options.revision, options.base)
            if options.command == "run-pending":
                for row in result["assessments"][-1]["checks"]:
                    if row["action"] == "rerun":
                        result, accepted = run_check(repository, path, result["revision"], row["check"])
                        if not accepted:
                            print(json.dumps(result, indent=2))
                            return 1
                result = assess(repository, path, result["revision"], options.base)
        print(json.dumps(result, indent=2))
        return 0
    except (RecoveryError, OSError, subprocess.SubprocessError, KeyError, ValueError) as error:
        print("qualification: " + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
