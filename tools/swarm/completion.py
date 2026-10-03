"""Final acceptance is bound to landed bytes, independently of runtime turns.

The adapter supplies evidence locators, host identities and fresh registry/process
reads. Git verifies local objects and ancestry; hashes verify reviewed evidence.
This is an acceptance contract, not a test runner or a publication bypass.
"""

from pathlib import Path
import hashlib
import re
import subprocess

from recovery_state import RecoveryError


def require(condition, reason):
    if not condition:
        raise RecoveryError(reason)


def sha(value):
    require(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{40}", value), "full Git SHA required")
    return value


def git(repository, *arguments):
    result = subprocess.run(["git", "-C", str(repository), *arguments], capture_output=True,
                            text=True, timeout=30)
    require(result.returncode == 0, "Git verification failed: " + result.stderr.strip())
    return result.stdout.strip()


def inputs(repository, revision, paths):
    require(isinstance(paths, list) and bool(paths), "explicit complete evidence input paths required")
    result = {}
    for path in paths:
        require(isinstance(path, str) and path and not path.startswith(("/", ":"))
                and ".." not in Path(path).parts and "\\" not in path, "unsafe evidence input path")
        # A tree object covers every descendant, including deletions/additions.
        result[path] = git(repository, "rev-parse", sha(revision) + ":" + path)
    return result


def artifact(root, value):
    path = Path(value["path"])
    require(not path.is_absolute() and ".." not in path.parts, "evidence must be inside evidence root")
    root = Path(root).resolve()
    resolved = (root / path).resolve()
    require(root in resolved.parents and resolved.is_file(), "missing/outside evidence artifact")
    require(hashlib.sha256(resolved.read_bytes()).hexdigest() == value.get("sha256"),
            "evidence artifact digest mismatch")


def program_failed(before, requirement):
    """A failed label alone is not a failed authored execution."""
    if (type(before.get("exit")) is not int
            or before.get("classification", "valid-should-work") != "valid-should-work"
            or before.get("expected_result", requirement["expected_result"]) != requirement["expected_result"]):
        return False
    if before.get("failure_kind") == "compile":
        return before["exit"] != 0
    actual = before.get("program_result")
    expected = requirement["expected_result"]
    return (before.get("failure_kind") == "run" and isinstance(actual, dict)
            and set(expected) <= set(actual) and type(actual.get("exit")) is int
            and ("stdout_sha256" not in expected or isinstance(actual.get("stdout_sha256"), str)
                 and re.fullmatch(r"[0-9a-f]{64}", actual["stdout_sha256"]) is not None)
            and (before["exit"] != requirement["exit"] or any(actual[k] != expected[k] for k in expected)))


def validate(repository, root, assignment, manifest, observation, cleanup_required=True):
    require(manifest.get("schema") == 1 and manifest.get("assignment") == assignment["id"],
            "acceptance assignment/schema mismatch")
    candidate = sha(manifest.get("candidate"))
    base = sha(manifest.get("base"))
    git(repository, "merge-base", "--is-ancestor", sha(assignment["base"]), base)
    git(repository, "merge-base", "--is-ancestor", base, candidate)
    require(manifest.get("outcome") == assignment["outcome"], "acceptance outcome mismatch")
    require(manifest.get("user_rescue") is False, "user rescue prevents batch promotion")
    require(observation.get("landed_head") == candidate and observation.get("main") == manifest.get("remote_main")
            and observation.get("remote_evidence"), "fresh remote ancestry receipt required")
    # Parent must fetch the actual push destination before this read. No network
    # endpoint or trust in an uncorrelated client error is inferred here.
    require(git(repository, "rev-parse", "refs/remotes/origin/main") == manifest["remote_main"],
            "remote receipt differs from fetched origin/main")
    git(repository, "merge-base", "--is-ancestor", candidate, sha(manifest["remote_main"]))
    require(observation.get("claims") == [] and observation.get("landing_tickets") == []
            and observation.get("registry_evidence"), "claims/tickets not reconciled")
    require(observation.get("processes_stopped") is True, "processes/children not confirmed stopped")
    workspace = observation.get("workspace", {})
    require(workspace.get("head") == candidate and workspace.get("clean") is True
            and workspace.get("primary") is False and workspace.get("owned") is True
            and workspace.get("evidence"), "exact owned clean non-primary workspace inventory required")
    if cleanup_required:
        require(observation.get("cleanup") == "completed" and observation.get("cleanup_evidence"),
                "owned-resource cleanup not verified")
    receipt = manifest["final_receipt"]
    require(receipt.get("schema") == "omega-landing-receipt-v1"
            and receipt.get("candidate") == candidate and receipt.get("base") == base
            and receipt.get("exit_code") == 0, "exact final-candidate landing receipt required")
    artifact(root, manifest["receipt_artifact"])
    import json
    require(json.loads((Path(root) / manifest["receipt_artifact"]["path"]).read_text()) == receipt,
            "landing receipt artifact differs from acceptance")
    import sys
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
    import landing
    affected = landing.affected_packages(repository, base, candidate)
    require(set(receipt.get("packages", [])) >= affected, "final receipt misses affected packages")
    history = {row["id"]: row for row in manifest.get("historical", [])}
    require(len(history) == len(manifest.get("historical", [])), "duplicate historical evidence ID")
    final = manifest["final_evidence"]
    require(isinstance(final, list), "final evidence must be an explicit list")
    expected = assignment["acceptance"]
    require({row["id"] for row in final} == {row["id"] for row in expected}
            and len(final) == len(expected), "missing/duplicate final acceptance evidence")
    for requirement in expected:
        row = next(value for value in final if value["id"] == requirement["id"])
        require(row.get("kind") == requirement["kind"] and row.get("command") == requirement["command"],
                "acceptance command/kind differs from dispatch")
        require(row.get("revision") == candidate, "final evidence covers wrong SHA")
        source = row
        if "reuse" in row:
            reuse = row["reuse"]
            require(reuse.get("reason") and reuse.get("historical_id") in history, "unjustified historical reuse")
            source = history[reuse["historical_id"]]
            require(source.get("kind") == row["kind"] and source.get("command") == row["command"],
                    "reused evidence command mismatch")
            require(source.get("environment") == manifest.get("environment") and source.get("environment"),
                    "reused evidence host/toolchain inputs changed")
            require(source.get("inputs") == requirement["inputs"], "reuse input closure differs from dispatch")
            require(inputs(repository, sha(source.get("revision")), source["inputs"])
                    == inputs(repository, candidate, source["inputs"]), "historical inputs changed")
        require(source.get("inputs") == requirement["inputs"], "evidence input closure differs from dispatch")
        artifact(root, source["artifact"])
        require(source.get("exit") == requirement["exit"] and type(source.get("exit")) is int,
                "evidence command did not meet expected exit")
        require(source.get("verdict") == requirement["verdict"], "program/test verdict not accepted")
        if requirement["kind"] in {"native", "interpreter"}:
            require(requirement.get("expected_result") and source.get("program_result") == requirement["expected_result"],
                    "program differs from authored exit/output oracle")
        if requirement["kind"] in {"tests", "negative"}:
            require(type(source.get("count")) is int and source["count"] > 0, "empty scoped test evidence")
        require(source.get("environment") == manifest.get("environment") and source.get("environment"),
                "evidence environment missing/different")
    before = manifest["before"]
    require(before.get("revision") == assignment["base"] and before.get("outcome") == assignment["outcome"]
            and before.get("verdict") == "failed" and type(before.get("exit")) is int
            and before.get("command"), "recorded before-program failure required")
    require(before.get("kind") in {"native", "interpreter"}
            and before.get("failure_kind") in {"compile", "run"}
            and before.get("environment") == manifest.get("environment")
            and any(requirement["kind"] == before["kind"] and requirement["command"] == before["command"]
                    for requirement in expected), "before failure must be a program observation on the recorded host")
    artifact(root, before["artifact"])
    oracle = next(r for r in expected if r["kind"] == before["kind"] and r["command"] == before["command"])
    require(program_failed(before, oracle), "before evidence does not establish a meaningful program failure")
    require({r["kind"] for r in expected} >= {"native", "interpreter", "tests", "negative"},
            "program, scoped tests and negatives required")
    return dict(candidate=candidate, remote_main=manifest["remote_main"], result="landed",
                user_rescue=False, cleanup="completed")
