#!/usr/bin/env python3
"""Bisect one PIN cluster with two witnesses. No linear scan.

Witness stop: 0 = ledger-expected outcome, 1 = any other outcome that ran.
Witness class: 1 = PIN diagnostic class, 0 = any other outcome that ran.
125 is build/run failure or a ledger-only commit, never a different rejection.

Find-good is an exponential first-parent walk from the PIN. Bisect is
first-parent between that 0 and the nearest sampled 1. Each commit builds
into a fresh CARGO_TARGET_DIR and that build is reused for every fixture
measured there.
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

MAIN = Path(__file__).resolve().parents[2]
DIR = Path(__file__).resolve().parent
WT = MAIN / ".agents" / "ledger-bisect-wt"
SCRIPT = DIR / "tools" / "bisect_witness.sh"
PREPARE = DIR / "tools" / "prepare_bisect_expect.py"
TEST = DIR / "tools" / "bisect_test.py"
PIN = "51e8c45c793007f211d0d9baee7de16767172444"
FORBIDDEN = {
    PIN,
    "f3612364ffbb9489d7be1409397df6867fcbebc6",
}
LEDGER_COPY = Path("/tmp/bisect_pin_corpus_outcomes.txt")
PIN_OBS = DIR / "pin_observed.txt"
CACHE = DIR / "measure-cache"
VERSION = 2


def git(args: list[str], cwd: Path, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(["git", *args], cwd=cwd, text=True, capture_output=True, check=check)


def checkout(sha: str) -> None:
    git(["bisect", "reset"], WT, check=False)
    git(["checkout", "-f", sha], WT)
    got = git(["rev-parse", "HEAD"], WT).stdout.strip()
    if got != sha:
        raise RuntimeError(f"checkout {sha} landed on {got}")


def changed_files(sha: str) -> list[str]:
    parent = git(["rev-parse", "-q", "--verify", f"{sha}^1"], WT, check=False)
    if parent.returncode != 0:
        return ["<root>"]
    diff = git(["diff", "--name-only", parent.stdout.strip(), sha], WT, check=False)
    return [line for line in diff.stdout.splitlines() if line.strip()]


def ledger_only(sha: str) -> bool:
    files = changed_files(sha)
    if not files or files == ["<root>"]:
        return False
    return all(f == "tests/omega/corpus_outcomes.txt" or f.endswith(".md") for f in files)


def pr_number(sha: str) -> str | None:
    msg = git(["log", "-1", "--format=%B", sha], WT).stdout
    match = re.search(r"\(#(\d+)\)", msg) or re.search(r"#(\d+)\b", msg)
    return match.group(1) if match else None


def diff_hunk(sha: str) -> str:
    files = [
        f
        for f in changed_files(sha)
        if f != "tests/omega/corpus_outcomes.txt" and not f.endswith(".md") and f != "<root>"
    ]
    if not files:
        return "mechanism not established"
    show = git(["show", "--format=", "-U3", sha, "--", files[0]], WT, check=False)
    text = show.stdout
    if "@@" not in text:
        return "mechanism not established"
    return "\n".join(text.splitlines()[:40])


def fixture_add_sha(witness: str) -> str:
    # Fixtures are directories with main.omg, or a single .omg file.
    for rel in (f"tests/omega/{witness}/main.omg", f"tests/omega/{witness}.omg"):
        found = git(
            ["log", "-1", "--diff-filter=A", "--format=%H", "--first-parent", "--", rel],
            WT,
            check=False,
        )
        sha = found.stdout.strip()
        if len(sha) == 40:
            return sha
        found = git(["log", "-1", "--diff-filter=A", "--format=%H", "--", rel], WT, check=False)
        sha = found.stdout.strip()
        if len(sha) == 40:
            return sha
    return ""


def first_parent(sha: str) -> list[str]:
    out = git(["rev-list", "--first-parent", sha], WT)
    return out.stdout.splitlines()


def prepare_all(fixtures: list[str]) -> dict[str, Path]:
    env = os.environ.copy()
    env["BISECT_PIN_LEDGER"] = str(LEDGER_COPY)
    env["BISECT_PIN_OBSERVED"] = str(PIN_OBS)
    paths: dict[str, Path] = {}
    for fixture in fixtures:
        proc = subprocess.run(
            ["python3", str(PREPARE), fixture],
            text=True,
            capture_output=True,
            check=True,
            env=env,
        )
        paths[fixture] = Path(proc.stdout.strip())
    return paths


def classify_record(record: Path, fixture: str, mode: str, expect: Path) -> int:
    env = os.environ.copy()
    env["BISECT_WITNESS_MODE"] = mode
    env["BISECT_EXPECT_FILE"] = str(expect)
    env.pop("BISECT_REASON_FILE", None)
    proc = subprocess.run(
        ["python3", str(TEST), fixture, str(record)],
        text=True,
        capture_output=True,
        env=env,
    )
    if proc.returncode not in (0, 1, 125):
        return 125
    return proc.returncode


def run_witness(
    witness: str,
    mode: str,
    fixtures: list[str],
    expect: Path,
    use_cache: bool,
    step_log: Path,
) -> int:
    env = os.environ.copy()
    env.update(
        {
            "BISECT_WITNESS": witness,
            "BISECT_WITNESS_MODE": mode,
            "BISECT_EXPECT_FILE": str(expect),
            "BISECT_REPO_ROOT": str(WT),
            "BISECT_MAIN_REPO": str(MAIN),
            "BISECT_PIN_LEDGER": str(LEDGER_COPY),
            "BISECT_FIXTURE_FILTER": ",".join(fixtures),
            "BISECT_USE_CACHE": "1" if use_cache else "0",
            "BISECT_STEP_LOG": str(step_log),
            "BISECT_CACHE_DIR": str(CACHE),
            "BISECT_REASON_FILE": "/tmp/bisect_step_reason.txt",
            "CARGO_BUILD_JOBS": "4",
        }
    )
    proc = subprocess.run(["bash", str(SCRIPT)], cwd=WT, env=env)
    if proc.returncode not in (0, 1, 125):
        return 125
    return proc.returncode


def saved_record(sha: str) -> Path | None:
    path = CACHE / sha / "record.txt"
    return path if path.exists() else None


def measure(
    sha: str,
    witness: str,
    mode: str,
    fixtures: list[str],
    expects: dict[str, Path],
    step_log: Path,
    use_cache: bool,
) -> tuple[int, dict[str, dict[str, int]]]:
    """Return witness exit and per-fixture {stop, class} from one build."""
    checkout(sha)
    rc = run_witness(witness, mode, fixtures, expects[witness], use_cache, step_log)
    rec = saved_record(sha)
    per: dict[str, dict[str, int]] = {}
    if rec is None:
        per = {fixture: {"stop": 125, "class": 125} for fixture in fixtures}
        return rc, per
    for fixture in fixtures:
        per[fixture] = {
            "stop": classify_record(rec, fixture, "stop", expects[fixture]),
            "class": classify_record(rec, fixture, "class", expects[fixture]),
        }
    # The shell's exit is the requested mode for the witness. Prefer the
    # record classification so both modes come from the same run.
    witness_rc = per[witness][mode]
    print(f"measure {sha[:12]} mode={mode} rc={witness_rc}", flush=True)
    return witness_rc, per


def nearest_one(tested: dict[int, int], good_off: int) -> int | None:
    nearest: int | None = None
    for off, rc in tested.items():
        if off < good_off and rc == 1 and (nearest is None or off > nearest):
            nearest = off
    return nearest


def walk_mode(
    mode: str,
    commits: list[str],
    fix_sha: str,
    witness: str,
    fixtures: list[str],
    expects: dict[str, Path],
    step_log: Path,
) -> dict:
    tested: dict[int, int] = {}
    samples: list[dict] = []

    def eval_off(off: int) -> int:
        if off in tested:
            return tested[off]
        sha = commits[off]
        rc, _per = measure(sha, witness, mode, fixtures, expects, step_log, True)
        tested[off] = rc
        samples.append({"offset": off, "sha": sha, "rc": rc})
        return rc

    pin_rc = eval_off(0)
    fidx = commits.index(fix_sha) if fix_sha and fix_sha in commits else None
    if pin_rc == 125:
        return {"kind": "unresolved", "reason": "PIN did not run", "samples": samples, "pin_rc": pin_rc}
    if pin_rc == 0:
        return {"kind": "pin-zero", "samples": samples, "pin_rc": pin_rc}

    offset = 1
    good_off: int | None = None
    while offset < len(commits):
        rc = eval_off(offset)
        if rc == 0:
            good_off = offset
            break
        if fidx is not None and offset >= fidx:
            rc_f = eval_off(fidx)
            if rc_f == 0:
                good_off = fidx
                break
            if rc_f == 125:
                return {
                    "kind": "unresolved",
                    "reason": "fixture-add commit did not run",
                    "fixture_added": fix_sha,
                    "samples": samples,
                }
            return {
                "kind": "never-good",
                "fixture_added": fix_sha,
                "samples": samples,
            }
        offset *= 2
    else:
        if fidx is None:
            return {
                "kind": "unresolved",
                "reason": "no ledger-matching commit and fixture-add SHA was not found",
                "samples": samples,
            }
        rc_f = eval_off(fidx)
        if rc_f == 0:
            good_off = fidx
        elif rc_f == 125:
            return {
                "kind": "unresolved",
                "reason": "fixture-add commit did not run",
                "fixture_added": fix_sha,
                "samples": samples,
            }
        else:
            return {"kind": "never-good", "fixture_added": fix_sha, "samples": samples}

    assert good_off is not None
    near = nearest_one(tested, good_off)
    if near is None and tested.get(0) == 1:
        near = 0
    if near is None:
        return {
            "kind": "unresolved",
            "reason": "exponential walk found a 0 but no sampled 1 toward the PIN",
            "good_commit": commits[good_off],
            "samples": samples,
        }
    return {
        "kind": "bounds",
        "good_commit": commits[good_off],
        "good_offset": good_off,
        "bad_bound": commits[near],
        "bad_offset": near,
        "samples": samples,
    }


def git_bisect(bad: str, good: str, witness: str, mode: str, fixtures: list[str], expect: Path, step_log: Path) -> str:
    git(["bisect", "reset"], WT, check=False)
    start = git(["bisect", "start", "--first-parent", bad, good], WT, check=False)
    print(start.stderr or start.stdout, flush=True)
    if start.returncode != 0:
        raise RuntimeError(f"git bisect start failed: {start.stderr}")
    env = os.environ.copy()
    env.update(
        {
            "BISECT_WITNESS": witness,
            "BISECT_WITNESS_MODE": mode,
            "BISECT_EXPECT_FILE": str(expect),
            "BISECT_REPO_ROOT": str(WT),
            "BISECT_MAIN_REPO": str(MAIN),
            "BISECT_PIN_LEDGER": str(LEDGER_COPY),
            "BISECT_FIXTURE_FILTER": ",".join(fixtures),
            "BISECT_USE_CACHE": "1",
            "BISECT_STEP_LOG": str(step_log),
            "BISECT_CACHE_DIR": str(CACHE),
            "BISECT_REASON_FILE": "/tmp/bisect_step_reason.txt",
            "CARGO_BUILD_JOBS": "4",
        }
    )
    proc = subprocess.run(["git", "bisect", "run", "bash", str(SCRIPT)], cwd=WT, env=env)
    print(f"git bisect run exit {proc.returncode}", flush=True)
    head = git(["rev-parse", "HEAD"], WT, check=False)
    sha = head.stdout.strip() if head.returncode == 0 else ""
    log = git(["bisect", "log"], WT, check=False).stdout
    git(["bisect", "reset"], WT, check=False)
    (DIR / "bisect_last.log").write_text(log)
    if "first bad commit" not in log and proc.returncode != 0:
        raise RuntimeError(f"bisect did not name a first bad commit (exit {proc.returncode})")
    if len(sha) != 40:
        raise RuntimeError("bisect HEAD is not a full SHA")
    return sha


def confirm(
    sha: str,
    witness: str,
    fixtures: list[str],
    expects: dict[str, Path],
    step_log: Path,
) -> list[dict[str, dict[str, int]]]:
    runs: list[dict[str, dict[str, int]]] = []
    checkout(sha)
    for _ in range(3):
        _rc, per = measure(sha, witness, "stop", fixtures, expects, step_log, False)
        runs.append(per)
    return runs


def triplet(runs: list[dict[str, dict[str, int]]], fixture: str, mode: str) -> list[int]:
    return [run[fixture][mode] for run in runs]


def flips(parent_runs: list[dict[str, dict[str, int]]], bad_runs: list[dict[str, dict[str, int]]], fixture: str, mode: str) -> bool:
    return triplet(parent_runs, fixture, mode) == [0, 0, 0] and triplet(bad_runs, fixture, mode) == [1, 1, 1]


def edge_from_bounds(
    bounds: dict,
    mode: str,
    witness: str,
    fixtures: list[str],
    expects: dict[str, Path],
    step_log: Path,
) -> dict:
    good = bounds["good_commit"]
    bad_bound = bounds["bad_bound"]
    parent_of_bound = git(["rev-parse", f"{bad_bound}^1"], WT).stdout.strip()
    if parent_of_bound == good:
        first = bad_bound
        print(f"adjacent bounds; first candidate {first}", flush=True)
    else:
        first = git_bisect(bad_bound, good, witness, mode, fixtures, expects[witness], step_log)
        print(f"bisect first {first}", flush=True)
    edge: dict = {
        "role": mode,
        "good_commit": good,
        "bad_bound": bad_bound,
        "breaking_commit": first,
        "pr": pr_number(first),
    }
    if first in FORBIDDEN or ledger_only(first):
        edge["accepted"] = False
        edge["reject_reason"] = "forbidden or ledger-only commit"
        edge["diff_hunk"] = diff_hunk(first) if not ledger_only(first) else "mechanism not established"
        return edge
    parent = git(["rev-parse", f"{first}^1"], WT).stdout.strip()
    edge["parent_sha"] = parent
    bad_runs = confirm(first, witness, fixtures, expects, step_log)
    parent_runs = confirm(parent, witness, fixtures, expects, step_log)
    edge["breaking_exit_triplet"] = triplet(bad_runs, witness, mode)
    edge["parent_exit_triplet"] = triplet(parent_runs, witness, mode)
    edge["class_triplet_at_breaking"] = triplet(bad_runs, witness, "class")
    edge["class_triplet_at_parent"] = triplet(parent_runs, witness, "class")
    edge["stop_triplet_at_breaking"] = triplet(bad_runs, witness, "stop")
    edge["stop_triplet_at_parent"] = triplet(parent_runs, witness, "stop")
    edge["members_at_breaking"] = {
        fixture: {
            "stop": triplet(bad_runs, fixture, "stop"),
            "class": triplet(bad_runs, fixture, "class"),
        }
        for fixture in fixtures
    }
    edge["members_at_parent"] = {
        fixture: {
            "stop": triplet(parent_runs, fixture, "stop"),
            "class": triplet(parent_runs, fixture, "class"),
        }
        for fixture in fixtures
    }
    edge["diff_hunk"] = diff_hunk(first)
    accepted = (
        edge["breaking_exit_triplet"] == [1, 1, 1]
        and edge["parent_exit_triplet"] == [0, 0, 0]
        and (edge["diff_hunk"] == "mechanism not established" or "@@" in edge["diff_hunk"])
    )
    edge["accepted"] = accepted
    if not accepted:
        edge["reject_reason"] = "3x confirmation was not 1,1,1 at first-bad and 0,0,0 at its first parent"
    return edge


def run_mode(
    mode: str,
    commits: list[str],
    fix_sha: str,
    witness: str,
    fixtures: list[str],
    expects: dict[str, Path],
    step_log: Path,
) -> dict:
    print(f"=== walk {mode} witness={witness}", flush=True)
    bounds = walk_mode(mode, commits, fix_sha, witness, fixtures, expects, step_log)
    if bounds["kind"] != "bounds":
        return bounds
    print(
        f"bounds {mode} good={bounds['good_commit']} bad={bounds['bad_bound']}",
        flush=True,
    )
    edge = edge_from_bounds(bounds, mode, witness, fixtures, expects, step_log)
    edge["samples"] = bounds.get("samples")
    return {"kind": "edge", "edge": edge}


def assign_members(edges: list[dict], members: list[str]) -> tuple[list[str], list[str]]:
    assigned: list[str] = []
    for edge in edges:
        if not edge.get("accepted"):
            continue
        role = edge["role"]
        modes = ["stop", "class"] if role == "stop+class" else [role]
        flippers = []
        for fixture in members:
            if any(
                flips_stored(edge, fixture, mode)
                for mode in modes
            ):
                flippers.append(fixture)
                if fixture not in assigned:
                    assigned.append(fixture)
        edge["flippers"] = flippers
    non = [fixture for fixture in members if fixture not in assigned]
    return assigned, non


def flips_stored(edge: dict, fixture: str, mode: str) -> bool:
    bad = (edge.get("members_at_breaking") or {}).get(fixture) or {}
    parent = (edge.get("members_at_parent") or {}).get(fixture) or {}
    return bad.get(mode) == [1, 1, 1] and parent.get(mode) == [0, 0, 0]


def run_cluster(cluster: dict) -> dict:
    witness = cluster["witness"]
    members = list(cluster["members"])
    if witness not in members:
        members.insert(0, witness)
    step_log = DIR / f"bisect_steps_cluster_{cluster['cluster_id']}.tsv"
    step_log.write_text("")
    print(f"=== cluster {cluster['cluster_id']} rows={len(members)} witness={witness}", flush=True)
    checkout(PIN)
    if not LEDGER_COPY.exists():
        LEDGER_COPY.write_text((WT / "tests/omega/corpus_outcomes.txt").read_text())
    expects = prepare_all(members)
    commits = first_parent(PIN)
    if not commits or commits[0] != PIN:
        raise RuntimeError("first-parent list does not start at PIN")
    fix_sha = fixture_add_sha(witness)
    print(f"fixture-add {fix_sha or 'NOT-FOUND'}", flush=True)

    stop = run_mode("stop", commits, fix_sha, witness, members, expects, step_log)
    edges: list[dict] = []
    payload: dict = {
        "witness": witness,
        "members": members,
        "cluster_id": cluster["cluster_id"],
        "find_good_method": "exponential",
        "witness_version": VERSION,
        "fixture_added": fix_sha,
        "finished": True,
        "kind": cluster.get("kind"),
        "diag_norm": cluster.get("diag_norm"),
    }
    if stop["kind"] == "edge":
        edge = stop["edge"]
        same_class = (
            edge.get("accepted")
            and edge.get("class_triplet_at_breaking") == [1, 1, 1]
            and edge.get("class_triplet_at_parent") == [0, 0, 0]
        )
        if same_class:
            edge["role"] = "stop+class"
        edges.append(edge)
        if edge.get("accepted") and not same_class:
            print("class edge differs from stop edge; walking class witness", flush=True)
            klass = run_mode("class", commits, fix_sha, witness, members, expects, step_log)
            payload["class_walk"] = {k: v for k, v in klass.items() if k != "edge"}
            if klass["kind"] == "edge":
                edges.append(klass["edge"])
            elif klass["kind"] == "never-good":
                payload["class_never_good"] = klass["fixture_added"]
        elif not edge.get("accepted"):
            klass = run_mode("class", commits, fix_sha, witness, members, expects, step_log)
            payload["class_walk"] = {k: v for k, v in klass.items() if k != "edge"}
            if klass["kind"] == "edge":
                edges.append(klass["edge"])
    elif stop["kind"] == "never-good":
        payload["mechanism"] = f"never-good since {stop['fixture_added']}"
        payload["stop_walk"] = "never-good"
        klass = run_mode("class", commits, fix_sha, witness, members, expects, step_log)
        payload["class_walk"] = {k: v for k, v in klass.items() if k != "edge"}
        if klass["kind"] == "edge":
            edges.append(klass["edge"])
        elif klass["kind"] == "never-good":
            payload["class_never_good"] = klass["fixture_added"]
    else:
        payload["stop_walk"] = stop
        klass = run_mode("class", commits, fix_sha, witness, members, expects, step_log)
        payload["class_walk"] = {k: v for k, v in klass.items() if k != "edge"}
        if klass["kind"] == "edge":
            edges.append(klass["edge"])
        elif klass["kind"] == "never-good":
            payload["mechanism"] = f"never-good since {klass['fixture_added']}"

    payload["edges"] = edges
    accepted = [edge for edge in edges if edge.get("accepted")]
    _assigned, non = assign_members(accepted, members)
    payload["non_flippers"] = non
    if accepted:
        payload["status"] = "bisected"
        payload["owns"] = _assigned
        first = accepted[0]
        payload["breaking_commit"] = first["breaking_commit"]
        payload["pr"] = first.get("pr")
        payload["breaking_exit_triplet"] = first.get("breaking_exit_triplet")
        payload["parent_exit_triplet"] = first.get("parent_exit_triplet")
        payload["parent_sha"] = first.get("parent_sha")
        payload["diff_hunk"] = first.get("diff_hunk")
        payload["good_commit"] = first.get("good_commit")
        if str(payload.get("mechanism") or "").startswith("never-good since"):
            payload["stop_mechanism"] = payload["mechanism"]
            payload["mechanism"] = "class edge confirmed after stop walk found no ledger match"
    elif str(payload.get("mechanism") or "").startswith("never-good since"):
        payload["status"] = "never-good"
        payload["owns"] = [witness]
        payload["non_flippers"] = [fixture for fixture in members if fixture != witness]
    else:
        payload["status"] = "unresolved"
        payload["mechanism"] = payload.get("mechanism") or "no confirmed edge"
        payload["owns"] = members
        payload["non_flippers"] = []
    out = DIR / "bisects" / f"cluster_{cluster['cluster_id']}.json"
    out.parent.mkdir(exist_ok=True)
    out.write_text(json.dumps(payload, indent=2) + "\n")
    print(f"wrote {out} status={payload['status']} non_flippers={len(non)}", flush=True)
    return payload


def remeasure() -> None:
    """Re-measure #2478 and #2498 with witness (a). Prior exit 1 is not evidence."""
    out = DIR / "remeasure_stop_witness.json"
    if out.exists():
        prev = json.loads(out.read_text())
        if prev.get("witness_version") == VERSION and len(prev.get("commits") or []) == 2:
            print("remeasure already recorded", flush=True)
            return
    witness = "pass/relevance/runtime_static_proof_namespace_exit"
    shas = [
        "c85ac9612799e047bdbbc03e42229ef45a1f526f",
        "5d618db9a45a6dba9dd3e6c019bc69530f7d7992",
    ]
    checkout(PIN)
    LEDGER_COPY.write_text((WT / "tests/omega/corpus_outcomes.txt").read_text())
    expects = prepare_all([witness])
    step_log = DIR / "bisect_steps_remeasure.tsv"
    rows = []
    for sha in shas:
        rc, per = measure(sha, witness, "stop", [witness], expects, step_log, False)
        rows.append({"sha": sha, "stop_rc": rc, "modes": per[witness], "pr_subject": git(["log", "-1", "--format=%s", sha], WT).stdout.strip()})
        print(f"remeasure {sha} stop={rc} class={per[witness]['class']}", flush=True)
    out.write_text(json.dumps({"witness_version": VERSION, "witness": witness, "commits": rows}, indent=2) + "\n")


def broken_fixtures() -> list[str]:
    """Slice rows whose saved PIN observation is not the ledger outcome."""
    if not LEDGER_COPY.exists() or not PIN_OBS.exists():
        return []
    prior = json.loads((DIR / "prior_slice4.json").read_text())
    ledger = LEDGER_COPY.read_text()
    obs = PIN_OBS.read_text()
    sys.path.insert(0, str(DIR / "tools"))
    from bisect_test import classify_obs, observe
    from prepare_bisect_expect import expected_fragments, ledger_expectation, pin_bad

    broken: list[str] = []
    for fixture in prior["fixtures"]:
        try:
            good, tier, ledger_status, ledger_diag = ledger_expectation(fixture, ledger)
        except KeyError:
            continue
        expect = {
            "witness": fixture,
            "tier": tier,
            "good_status": good,
            "ledger_status": ledger_status,
            "ledger_diag_norm": ledger_diag,
            "expected_fragments": expected_fragments(fixture),
            **pin_bad(fixture, obs),
        }
        if classify_obs(observe(obs, fixture), expect, "stop") == 1:
            broken.append(fixture)
    return broken


def load_queue() -> list[dict]:
    state_path = DIR / "run_state.json"
    if state_path.exists():
        return json.loads(state_path.read_text())["queue"]
    clusters = json.loads((DIR / "pin_clusters.json").read_text())
    clusters.sort(key=lambda item: (-item["rows"], item["cluster_id"]))
    covered = {member for item in clusters for member in item["members"]}
    for fixture in broken_fixtures():
        if fixture in covered:
            continue
        slug = re.sub(r"[^a-zA-Z0-9_.-]+", "_", fixture)
        clusters.append(
            {
                "cluster_id": f"extra_{slug}",
                "rows": 1,
                "kind": "slice row outside the PIN clusters",
                "diag_norm": "",
                "witness": fixture,
                "members": [fixture],
            }
        )
    return clusters


def save_queue(queue: list[dict]) -> None:
    (DIR / "run_state.json").write_text(json.dumps({"queue": queue}, indent=2) + "\n")


def finished_ok(cluster: dict) -> bool:
    path = DIR / "bisects" / f"cluster_{cluster['cluster_id']}.json"
    if not path.exists():
        return False
    data = json.loads(path.read_text())
    return bool(data.get("finished")) and data.get("witness_version") == VERSION


def enqueue_non_flippers(payload: dict, queue: list[dict]) -> None:
    if payload.get("status") not in ("bisected", "never-good"):
        return
    existing = {item["witness"] for item in queue}
    done_dir = DIR / "bisects"
    for fixture in payload.get("non_flippers") or []:
        if fixture in existing:
            continue
        slug = re.sub(r"[^a-zA-Z0-9_.-]+", "_", fixture)
        if (done_dir / f"cluster_split_{slug}.json").exists():
            prev = json.loads((done_dir / f"cluster_split_{slug}.json").read_text())
            if prev.get("finished") and prev.get("witness_version") == VERSION:
                continue
        queue.append(
            {
                "cluster_id": f"split_{slug}",
                "rows": 1,
                "kind": payload.get("kind") or "",
                "diag_norm": payload.get("diag_norm") or "",
                "witness": fixture,
                "members": [fixture],
                "split_from": payload.get("cluster_id"),
            }
        )


def commit_progress(cluster_id: str) -> None:
    branch = git(["rev-parse", "--abbrev-ref", "HEAD"], MAIN).stdout.strip()
    if branch != "work/ledger-bisect-part-4":
        raise RuntimeError(f"refusing to commit from {branch}")
    paths = [
        ".agents/ledger-bisect-2026-11-11/part-4.md",
        ".agents/ledger-bisect-2026-11-11/part-4.tsv",
        ".agents/ledger-bisect-2026-11-11/run_state.json",
        ".agents/ledger-bisect-2026-11-11/remeasure_stop_witness.json",
        f".agents/ledger-bisect-2026-11-11/bisects/cluster_{cluster_id}.json",
        f".agents/ledger-bisect-2026-11-11/bisect_steps_cluster_{cluster_id}.tsv",
    ]
    rels = [p.strip() for p in paths]
    existing = [p for p in rels if (MAIN / p).exists()]
    git(["add", *existing], MAIN)
    diff = git(["diff", "--cached", "--quiet"], MAIN, check=False)
    if diff.returncode == 0:
        print("no changes to commit", flush=True)
        return
    msg = f"docs: ledger bisect part 4 cluster {cluster_id} stop and class witnesses"
    if len(msg) > 85:
        msg = f"docs: ledger bisect part 4 cluster {cluster_id}"
    body = (
        "Record the exponential first-parent bisect for this cluster using the\n"
        "stop-matching witness and, when it differs, the PIN-class witness.\n"
        "\n"
        "Agent: Cursor | Model: Grok 4.7 | Effort: high | Tokens: n/a in (n/a cached), n/a out | Steps: n/a | Time: n/a | Steers: 1\n"
    )
    subprocess.run(["git", "commit", "-m", msg, "-m", body], cwd=MAIN, check=True)
    subprocess.run(["git", "push", "origin", "work/ledger-bisect-part-4"], cwd=MAIN, check=True)
    print(f"pushed cluster {cluster_id}", flush=True)


def write_report() -> None:
    subprocess.run(["python3", str(DIR / "write_part4_report.py")], cwd=MAIN, check=True)


def run_all() -> None:
    checkout(PIN)
    LEDGER_COPY.write_text((WT / "tests/omega/corpus_outcomes.txt").read_text())
    remeasure()
    queue = load_queue()
    while queue:
        cluster = queue.pop(0)
        save_queue(queue)
        if finished_ok(cluster):
            print(f"skip finished cluster {cluster['cluster_id']}", flush=True)
            continue
        payload = run_cluster(cluster)
        enqueue_non_flippers(payload, queue)
        save_queue(queue)
        write_report()
        commit_progress(str(cluster["cluster_id"]))
    write_report()
    print("all clusters finished", flush=True)


def self_test() -> None:
    assert nearest_one({0: 1, 1: 1, 2: 125, 4: 1, 8: 0}, 8) == 4
    assert nearest_one({0: 1, 8: 0}, 8) == 0
    assert nearest_one({8: 0, 4: 125, 2: 125}, 8) is None
    if not LEDGER_COPY.exists():
        checkout(PIN)
        LEDGER_COPY.write_text((WT / "tests/omega/corpus_outcomes.txt").read_text())
    expects = prepare_all(
        [
            "pass/relevance/runtime_static_proof_namespace_exit",
            "fail/facts/unsigned_64bit_bitwise_xor_bound_through_call_result_rejected",
        ]
    )
    obs = PIN_OBS.read_text()
    witness = "pass/relevance/runtime_static_proof_namespace_exit"
    rec = Path("/tmp/bisect-selftest-record.txt")
    rec.write_text(obs)
    stop = classify_record(rec, witness, "stop", expects[witness])
    klass = classify_record(rec, witness, "class", expects[witness])
    if stop != 1 or klass != 1:
        raise SystemExit(f"PIN witness classify stop={stop} class={klass}, expected 1/1")
    good = Path("/tmp/bisect-selftest-good.txt")
    good.write_text(witness + " checked 10ms\n")
    if classify_record(good, witness, "stop", expects[witness]) != 0:
        raise SystemExit("checked pass row was not ledger-match")
    if classify_record(good, witness, "class", expects[witness]) != 0:
        raise SystemExit("checked pass row matched PIN class")
    other = Path("/tmp/bisect-selftest-other.txt")
    other.write_text(witness + " rejected 10ms\n\tsome other diagnostic\n")
    if classify_record(other, witness, "stop", expects[witness]) != 1:
        raise SystemExit("different rejection was not stop=1")
    if classify_record(other, witness, "class", expects[witness]) != 0:
        raise SystemExit("different rejection was not class=0")
    fail = "fail/facts/unsigned_64bit_bitwise_xor_bound_through_call_result_rejected"
    if classify_record(rec, fail, "stop", expects[fail]) != 1:
        raise SystemExit("fail wrong-reason PIN row was not stop=1")
    if classify_record(rec, fail, "class", expects[fail]) != 1:
        raise SystemExit("fail wrong-reason PIN row was not class=1")
    for wrong in (
        "fail/generics/const_data_argument_out_of_range",
        "fail/generics/grouped_decimal_const_fraction_rejected",
        "fail/generics/grouped_decimal_const_zero_division_rejected",
        "fail/modules/packaged_structured_const_wrong_carrier",
    ):
        expects[wrong] = prepare_all([wrong])[wrong]
        if classify_record(rec, wrong, "stop", expects[wrong]) != 1:
            raise SystemExit(f"{wrong} matched ledger despite expected.txt")
    print("self-test ok", flush=True)


if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else "--all"
    if cmd == "--self-test":
        self_test()
    elif cmd == "--all":
        run_all()
    else:
        raise SystemExit(f"unknown {cmd}")
