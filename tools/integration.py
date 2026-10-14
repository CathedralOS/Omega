#!/usr/bin/env python3
"""Portable ready fixes and frozen selections for ordinary qualification/landing.

No worker registry, build scheduler, runtime transport or publication protocol.
Imported observations remain original supporting evidence, never local runs.
"""
import argparse
import copy
import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
import uuid

import qualification as q
from recovery_state import atomic_json, exclusive, read_json, RecoveryError

require = q.require
SCHEMA = "omega-integration-submission-1"
LIMIT = 2 * 1024 ** 3


def git(repository, *arguments):
    return q.git(repository, *map(str, arguments))


def patch_id(repository, commit):
    delta = git(repository, "show", "--pretty=format:", commit)
    result = subprocess.run(["git", "patch-id", "--stable"], input=delta, text=True, capture_output=True, timeout=30)
    require(result.returncode == 0 and result.stdout.strip(), "empty/unreadable logical fix")
    return result.stdout.split()[0]


def submit(repository, state, base, review_path, output, identifier, outcome, requires):
    repository = Path(repository).resolve()
    head = q.clean_head(repository)
    require(re.fullmatch(r"[A-Za-z0-9_.-]{1,80}", identifier) and outcome, "bounded submission ID and outcome required")
    require(not Path(output).exists(), "submission exists; preserve it")
    record = q.load(state)
    snapshot_digest = q.file_digest(state)
    readiness = q.validate(repository, state, head, base)
    require(readiness["sha256"] == snapshot_digest and record == q.load(state), "qualification changed during export snapshot")
    require(any(c["kind"] in {"native", "terminal", "tests"} for c in record["config"]["checks"])
            and any(c["kind"] == "negative" for c in record["config"]["checks"]), "focused positive and negative evidence required")
    review = read_json(review_path)
    require(review.get("approved") is True and review.get("candidate") == head and review.get("reviewer"),
            "actual independent exact-fix review required; readiness is not combined-tree approval")
    require(review.get("config_digest") == record["config_digest"], "worker review must bind the focused qualification configuration")
    commits = git(repository, "rev-list", "--reverse", base + ".." + head).splitlines()
    require(commits, "submission contains no logical fix")
    previous = base
    for commit in commits:
        require(git(repository, "show", "-s", "--format=%P", commit).split() == [previous], "logical commits must be linear")
        previous = commit
    paths = {str(Path(state).resolve()), str(Path(review_path).resolve())}
    for run in record["runs"]:
        paths.update(run["artifacts"])
        paths.update(run.get("prepared_inputs", {}).get("files", {}))
        paths.update(run[key]["path"] for key in ("stdout", "stderr", "receipt"))
    for value in record.get("source_reuse", []):
        paths.add(value["path"])
        paths.add(read_json(value["path"])["review"]["path"])
    payload, originals = {}, {}
    for index, value in enumerate(sorted(paths)):
        path = Path(value)
        require(path.is_file(), "submission evidence unavailable: " + value)
        before = q.artifact_stamps([value])
        name = "payload/" + str(index) + ".bin"
        payload[name] = path.read_bytes()
        require(before == q.artifact_stamps([value]), "submission evidence changed during export")
        originals[value] = name
    with tempfile.TemporaryDirectory(prefix="omega-submission-") as temporary:
        bundle = Path(temporary) / "source.bundle"
        evidence_revisions = sorted({r["revision"] for r in record["runs"]} |
                                    {c["producer"]["revision"] for c in record["config"]["checks"] if c.get("producer")})
        git(repository, "bundle", "create", bundle, "HEAD", *evidence_revisions, "^" + base)
        bundle_heads = git(repository, "bundle", "list-heads", bundle).splitlines()
        payload["source.bundle"] = bundle.read_bytes()
    require(sum(map(len, payload.values())) <= LIMIT, "submission exceeds 2 GiB; narrow retained evidence")
    packet = dict(schema=SCHEMA, id=identifier, outcome=outcome, base=base, candidate=head,
                  tree=git(repository, "rev-parse", head + "^{tree}"), commits=commits,
                  patch_ids=[patch_id(repository, c) for c in commits], requires=requires,
                  evidence_revisions=evidence_revisions, bundle_heads=bundle_heads,
                  config=record["config"], handoff=q.work_handoff(repository, record["config"], base, "continuation"),
                  readiness=readiness, review=review, review_path=str(Path(review_path).resolve()), originals=originals,
                  files={name: q.hashlib.sha256(data).hexdigest() for name, data in payload.items()},
                  historical_evidence_only=True)
    packet["digest"] = q.digest(packet)
    require(q.clean_head(repository) == head and q.file_digest(state) == readiness["sha256"], "source/qualification moved during export")
    output = Path(output).resolve(); output.parent.mkdir(parents=True, exist_ok=True)
    with exclusive(str(output) + ".lock"):
        require(not output.exists(), "submission already exported")
        temporary = output.with_name(output.name + ".partial")
        try:
            with tarfile.open(temporary, "w:gz") as archive:
                for name, data in {"manifest.json": (json.dumps(packet, sort_keys=True) + "\n").encode(), **payload}.items():
                    info = tarfile.TarInfo(name); info.size = len(data); info.mode = 0o600
                    archive.addfile(info, io.BytesIO(data))
            os.replace(temporary, output)
        finally:
            temporary.unlink(missing_ok=True)
    return dict(archive=str(output), sha256=q.file_digest(output), digest=packet["digest"], candidate=head)


def inspect_archive(archive):
    data, total = {}, 0
    with tarfile.open(archive, "r:gz") as stream:
        for entry in stream:
            require(entry.isfile() and entry.name not in data and
                    (entry.name in {"manifest.json", "source.bundle"} or re.fullmatch(r"payload/[0-9]+\.bin", entry.name)),
                    "unexpected/duplicate/nonregular archive member")
            total += entry.size
            require(total <= LIMIT and len(data) < 2000, "submission resource limit exceeded")
            data[entry.name] = stream.extractfile(entry).read()
    require("manifest.json" in data, "manifest missing")
    packet = json.loads(data.pop("manifest.json"))
    digest = packet.pop("digest")
    require(packet.get("schema") == SCHEMA and q.digest(packet) == digest, "submission manifest mismatch")
    packet["digest"] = digest
    require(all(re.fullmatch(r"[0-9a-f]{40}", value) for value in
                [packet["base"], packet["candidate"], packet["tree"], *packet["commits"], *packet["evidence_revisions"]]), "invalid source identities")
    require(re.fullmatch(r"[0-9a-f]{64}", digest), "invalid submission identity")
    require(set(packet["files"]) == set(data) and all(q.hashlib.sha256(value).hexdigest() == packet["files"][name]
            for name, value in data.items()), "submission payload mismatch")
    require(packet["review"].get("approved") is True and packet["review"].get("candidate") == packet["candidate"], "stale worker review")
    return packet, data


def receive(repository, archive, destination):
    packet, data = inspect_archive(archive)
    directory = Path(destination).resolve() / packet["digest"]
    require(not directory.exists(), "submission already received; inspect retained copy")
    directory.mkdir(parents=True)
    for name, content in data.items():
        path = directory / name; path.parent.mkdir(exist_ok=True); path.write_bytes(content)
    atomic_json(directory / "manifest.json", packet)
    git(repository, "bundle", "verify", directory / "source.bundle")
    heads = git(repository, "bundle", "list-heads", directory / "source.bundle").splitlines()
    require(heads == packet["bundle_heads"] and packet["candidate"] + " HEAD" in heads,
            "bundle advertises a different candidate")
    require(all(line.split()[0] in set(packet["evidence_revisions"]) | {packet["candidate"]}
                for line in heads), "unexpected bundle source identity")
    git(repository, "fetch", "--no-tags", "--no-recurse-submodules", directory / "source.bundle",
        "HEAD:refs/integration/submissions/" + packet["digest"],
        *(line.split()[1] + ":refs/integration/evidence/" + packet["digest"] + "/" + line.split()[0]
          for line in heads if line.split()[1] != "HEAD"))
    require(git(repository, "rev-parse", packet["candidate"] + "^{tree}") == packet["tree"], "submitted tree mismatch")
    require(git(repository, "rev-list", "--reverse", packet["base"] + ".." + packet["candidate"]).splitlines() == packet["commits"],
            "submitted commit range mismatch")
    verify_support(repository, packet, data)
    payload_paths = [str(directory / name) for name in data]
    atomic_json(directory / "received.json", dict(digest=packet["digest"], manifest_sha256=q.file_digest(directory / "manifest.json"),
                candidate=packet["candidate"], payload_paths=payload_paths, payload_stamps=q.artifact_stamps(payload_paths)))
    return dict(manifest=str(directory / "manifest.json"), digest=packet["digest"], candidate=packet["candidate"],
                instruction="Inspect source/evidence; reconcile logical commits manually. Import ran no worker commands.")


def verify_support(repository, packet, data):
    """Cross-check original observations without rewriting their absolute identities."""
    require(packet["patch_ids"] == [patch_id(repository, c) for c in packet["commits"]], "original logical patch identity mismatch")
    def content(path):
        return data[packet["originals"][path]]
    original = content(packet["readiness"]["path"])
    require(q.hashlib.sha256(original).hexdigest() == packet["readiness"]["sha256"], "readiness state payload mismatch")
    record = json.loads(original)
    require(record["schema"] == q.SCHEMA and record["config"] == packet["config"]
            and record["config_digest"] == q.digest(packet["config"]), "packet omitted/changed qualification definitions")
    require(json.loads(content(packet["review_path"])) == packet["review"]
            and packet["review"]["config_digest"] == record["config_digest"], "packet review/configuration mismatch")
    require(record["assessments"][-1]["candidate"] == packet["candidate"] and not record["running"], "worker readiness candidate mismatch")
    contracts = q.contract_mapping(repository, record["config"], packet["base"], packet["candidate"])
    require(packet["handoff"].get("canonical_context", []) == contracts, "worker focused context mismatch")
    for check in record["config"]["checks"]:
        run = next((r for r in reversed(record["runs"]) if r["check"] == check["id"]), None)
        require(run and run["definition"] == q.digest(check) and run["exit"] == check["expected_exit"]
                and not run["invalidations"], "worker check is not accepted")
        require(json.loads(content(run["receipt"]["path"])) == {k: v for k, v in run.items() if k != "receipt"}
                and q.hashlib.sha256(content(run["receipt"]["path"])).hexdigest() == run["receipt"]["sha256"], "original observation receipt mismatch")
        require(run["source_inputs"] == q.source_inputs(repository, run["revision"], q.input_paths(check)), "original source identity mismatch")
        for path, expected in run["artifacts"].items():
            require(q.hashlib.sha256(content(path)).hexdigest() == expected, "original artifact identity mismatch")
        for path, expected in run.get("prepared_inputs", {}).get("files", {}).items():
            require(q.hashlib.sha256(content(path)).hexdigest() == expected, "original prepared input identity mismatch")
        for key in ("stdout", "stderr"):
            require(q.hashlib.sha256(content(run[key]["path"])).hexdigest() == run[key]["sha256"], "original observation stream mismatch")


def build(repository, plan, receipt):
    """One actual optimized-dev invocation; existing executor build admission applies."""
    require(not Path(receipt).exists(), "producer receipt already exists; retain immutable producer")
    command = plan["command"]
    require(isinstance(command, list) and len(command) >= 2 and all(isinstance(c, str) for c in command), "build argv required")
    require(Path(command[0]).name in {"mbx", "cargo", "mbx.exe", "cargo.exe"} and command[1] == "build"
            and not any(c == "--release" or c.startswith(("--profile", "--target-dir", "--config", "--manifest-path")) for c in command),
            "use optimized-dev MBX build (Cargo fallback), not profile/storage/config overrides")
    require(plan["environment"].get("CARGO_PROFILE_DEV_OPT_LEVEL") == "1"
            and Path(plan["environment"].get("CARGO_TARGET_DIR", "")).is_absolute(), "stable isolated optimized-dev target required")
    require(set(plan["environment"]) <= set(q.PROFILE_KEYS), "only explicit build profile variables supported")
    require(isinstance(plan.get("timeout_seconds", 1800), (int, float))
            and 0 < plan.get("timeout_seconds", 1800) <= 86400, "finite bounded build timeout required")
    candidate = q.clean_head(repository); sources = q.source_inputs(repository, candidate, plan["inputs"])
    require(sources and all(sources.values()), "complete existing producer source inputs required")
    before = q.source_stamps(repository, candidate, plan["inputs"])
    environment = dict(os.environ, **plan["environment"])
    path = Path(receipt).resolve(); path.parent.mkdir(parents=True, exist_ok=True)
    launcher = shutil.which(command[0]) or command[0]
    launcher_before = dict(path=launcher, sha256=q.file_digest(launcher))
    launcher_stamps = q.artifact_stamps([str(Path(launcher).absolute())])
    toolchain = subprocess.run(["rustc", "--version", "--verbose"], cwd=repository, env=environment,
                               stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=30)
    require(toolchain.returncode == 0, "producer toolchain identity unavailable")
    actual_environment = q.environment(plan.get("context", {}))
    actual_environment["profiles"] = {key: environment.get(key) for key in q.PROFILE_KEYS}
    started = time.monotonic()
    with exclusive(str(path) + ".lock"):
        require(not path.exists(), "producer receipt already exists")
        return _capture_build(repository, plan, path, command, candidate, sources, before, environment,
                              started, launcher, launcher_before, launcher_stamps, toolchain, actual_environment)


def _capture_build(repository, plan, path, command, candidate, sources, before, environment,
                   started, launcher, launcher_before, launcher_stamps, toolchain, actual_environment):
    with path.with_suffix(".stdout").open("wb") as out, path.with_suffix(".stderr").open("wb") as err:
        result = subprocess.run(command, cwd=repository, env=environment, stdin=subprocess.DEVNULL,
                                stdout=out, stderr=err, timeout=plan.get("timeout_seconds", 1800))
        value = dict(schema="omega-integration-producer-1", candidate=candidate, configuration=plan["configuration"],
                     inputs=sources, command=command, environment=plan["environment"], exit=result.returncode,
                     wall_seconds=time.monotonic() - started, artifacts={},
                     stdout=q.file_digest(path.with_suffix(".stdout")), stderr=q.file_digest(path.with_suffix(".stderr")))
        require(q.file_digest(launcher) == launcher_before["sha256"]
                and q.artifact_stamps([str(Path(launcher).absolute())]) == launcher_stamps, "build launcher changed during capture")
        value["launcher"] = launcher_before
        value["toolchain"] = toolchain.stdout
        value["actual_environment"] = actual_environment
        require(result.returncode == 0, "producer build failed; logs retained, no successful receipt")
        require(q.clean_head(repository) == candidate and before == q.source_stamps(repository, candidate, plan["inputs"]), "source changed during build")
        for name in plan["outputs"]:
            require(Path(name).is_absolute() and Path(name).is_file(), "producer output missing")
            require(Path(plan["environment"]["CARGO_TARGET_DIR"]).resolve() in Path(name).resolve().parents,
                    "producer outputs must belong to isolated target directory")
            value["artifacts"][name] = q.file_digest(name)
        require(value["artifacts"], "complete producer output closure required")
        atomic_json(path, value)
    return value


def seal(repository, state, base, config, selection, review_path, expected=None, preview=False):
    candidate = q.clean_head(repository)
    q.definition(config)
    context = q.contract_mapping(repository, config, base, candidate)
    members, covered, identifiers, patches, obligations = [], set(), set(), set(), {}
    for entry in selection:
        path = Path(entry["manifest"]).resolve(); packet = read_json(path)
        body = {key: value for key, value in packet.items() if key != "digest"}
        require(q.digest(body) == packet["digest"], "received manifest changed")
        received_path = path.with_name("received.json"); received = read_json(received_path)
        require(received["digest"] == packet["digest"] and received["candidate"] == packet["candidate"]
                and received["manifest_sha256"] == q.file_digest(path), "submission has no matching successful receive receipt")
        require(received["payload_stamps"] == q.artifact_stamps(received["payload_paths"]), "received evidence changed")
        verify_support(repository, packet, {name: (path.parent / name).read_bytes() for name in packet["files"]})
        require(packet["id"] not in identifiers, "duplicate outcome submission")
        identifiers.add(packet["id"])
        require(not patches.intersection(packet["patch_ids"]), "duplicate logical patch; select it once")
        patches.update(packet["patch_ids"])
        integrated = entry["integrated_commits"]
        require(len(integrated) == len(packet["commits"]), "record every integrated logical commit")
        for commit in integrated:
            require(git(repository, "merge-base", "--is-ancestor", commit, candidate) == "", "integrated commit is not in candidate")
        require([patch_id(repository, commit) for commit in integrated] == packet["patch_ids"] or entry.get("reconciliation"),
                "changed logical delta needs explicit reconciliation and combined-tree review")
        for contract in packet["handoff"].get("canonical_context", []):
            relevant = [row for row in context if row["contract"] == contract["contract"]]
            require(relevant and set(contract["implementation"]) <= {name for row in relevant for name in row["implementation"]},
                    "union omitted worker semantic contract coverage")
            if any(row["blob"] != contract["blob"] for row in relevant):
                require(contract["contract"] in config.get("refreshed_contracts", []), "selected spec changed; read/refresh and review context")
        covered.update((packet["id"], check["id"]) for check in packet["config"]["checks"] if check["kind"] not in {"build", "review"})
        obligations.update({(packet["id"], check["id"]): check for check in packet["config"]["checks"] if check["kind"] not in {"build", "review"}})
        members.append(dict(id=packet["id"], digest=packet["digest"], source_candidate=packet["candidate"],
                            integrated_commits=integrated, reconciliation=entry.get("reconciliation"),
                            manifest_path=str(path), manifest_sha256=q.file_digest(path), requires=packet["requires"],
                            receive_path=str(received_path), receive_sha256=q.file_digest(received_path)))
    require(members, "select any nonempty ready subset; do not wait for all workers")
    require(all(set(member["requires"]) <= identifiers for member in members), "required submission not selected")
    actual = {tuple(origin) for check in config["checks"] for origin in check.get("integration_checks", [])}
    require(covered <= actual, "union omitted focused worker checks")
    for check in config["checks"]:
        for origin in check.get("integration_checks", []):
            required = obligations.get(tuple(origin))
            require(required and check["kind"] == required["kind"], "union cannot replace behavioral controls with build/review labels")
            require(all(any(path == declared or path.startswith(declared.rstrip("/") + "/")
                            for declared in q.input_paths(check)) for path in q.input_paths(required)),
                    "union omitted worker source/oracle/dependency closure")
    require(any(check["kind"] == "review" and check["final"] for check in config["checks"]), "independent final combined-tree review required")
    groups, receipt_hashes, producers, configurations = {}, {}, {}, set()
    for name, path in config.get("producer_groups", {}).items():
        receipt = read_json(path)
        require(receipt["schema"] == "omega-integration-producer-1" and receipt["exit"] == 0,
                "stale/mismatched shared producer receipt")
        require(receipt["configuration"] not in configurations, "select one producer per necessary configuration")
        configurations.add(receipt["configuration"])
        require(receipt["inputs"] == q.source_inputs(repository, candidate, list(receipt["inputs"])), "producer source mismatch")
        require(all(q.file_digest(p) == value for p, value in receipt["artifacts"].items()), "producer artifact mismatch")
        groups[name] = receipt["artifacts"]; receipt_hashes[str(Path(path).resolve())] = q.file_digest(path)
        producers[name] = dict(revision=receipt["candidate"], inputs=sorted(receipt["inputs"]))
    for check in config["checks"]:
        if check.get("producer"):
            name = check.get("producer_group")
            require(name in groups and set(groups[name]) <= set(check["artifacts"]), "check does not consume its shared producer closure")
            require(check["producer"] == producers[name], "check uses a different producer source closure")
    paths = sorted({p for c in config["checks"] for p in c.get("prepared_inputs", [])})
    prepared = q.prepared_inputs(paths)
    binding = dict(candidate=candidate, base=base, config_digest=q.digest(config), members=members,
                   producer_artifacts=groups, producer_receipts=receipt_hashes, prepared=prepared)
    if preview:
        return dict(binding=binding, approved=False, reviewer=None,
                    handoff=q.work_handoff(repository, config, base, "review"), members=members,
                    instruction="Review complete integrated diff, union coverage, producer and prepared inputs; later exact final review remains required.")
    review = read_json(review_path)
    require(review.get("approved") is True and review.get("reviewer") and review.get("binding") == binding,
            "independent prospective union/context/preparation review must bind exact selection and configuration")
    value = dict(schema="omega-integration-seal-1", candidate=candidate, base=base, config_digest=q.digest(config),
                 members=members, prepared_paths=paths,
                 prepared=prepared, producer_artifacts=groups, producer_receipts=receipt_hashes,
                 review=dict(path=str(Path(review_path).resolve()), sha256=q.file_digest(review_path)))
    if expected is None:
        record = q.start(repository, state, base, config)
    else:
        record = q.load(state)
        require(record["config_digest"] == q.digest(config), "amend qualification definitions explicitly before resealing")
    destination = Path(state).with_suffix(".seals") / (uuid.uuid4().hex + ".json")
    atomic_json(destination, value)
    def change(record):
        require(not record["running"], "capture active; no submissions/main integration during capture")
        if record.get("integration"):
            record.setdefault("integration_history", []).append(record["integration"])
        record["integration"] = dict(path=str(destination.resolve()), sha256=q.file_digest(destination))
    return q.update(state, record["revision"] if expected is None else expected, change)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("submit", "inspect", "receive", "build", "preview", "freeze", "reseal", "status"))
    for flag in ("repository", "state", "review", "output", "archive", "destination", "config", "selection", "plan", "receipt"):
        parser.add_argument("--" + flag, type=Path)
    parser.add_argument("--base"); parser.add_argument("--id"); parser.add_argument("--outcome")
    parser.add_argument("--requires", action="append", default=[]); parser.add_argument("--revision", type=int)
    args = parser.parse_args(argv); repository = args.repository or Path.cwd()
    try:
        if args.command == "submit":
            result = submit(repository, args.state, args.base, args.review, args.output, args.id, args.outcome, args.requires)
        elif args.command == "inspect":
            packet, _ = inspect_archive(args.archive)
            result = {key: packet[key] for key in ("id", "outcome", "base", "candidate", "digest", "requires", "historical_evidence_only")}
        elif args.command == "receive":
            result = receive(repository, args.archive, args.destination)
        elif args.command == "build":
            result = build(repository, read_json(args.plan), args.receipt)
        elif args.command in {"preview", "freeze", "reseal"}:
            require(args.command != "reseal" or args.revision is not None, "reseal requires explicit current revision")
            result = seal(repository, args.state, args.base, read_json(args.config), read_json(args.selection), args.review,
                          args.revision if args.command == "reseal" else None, args.command == "preview")
            if args.command != "preview":
                result = dict(revision=result["revision"], candidate=q.clean_head(repository), integration=result["integration"])
        else:
            result = q.load(args.state)
            result = dict(revision=result["revision"], running=result["running"], integration=result.get("integration"),
                          checks=len(result["config"]["checks"]), runs=len(result["runs"]),
                          repeated_work=sum(bool(row["repeated_work"]) for row in result["runs"]))
        print(json.dumps(result, indent=2)); return 0
    except (RecoveryError, OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError, tarfile.TarError) as error:
        print(json.dumps(dict(error=str(error), action="preserve-and-reconcile"))); return 1


if __name__ == "__main__":
    sys.exit(main())
