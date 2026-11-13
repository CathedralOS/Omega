#!/usr/bin/env python3
"""Exponential first-parent walk from PIN, then bisect to the nearest 1.

Does not use fixture-add as a starting good. A wiped CARGO_TARGET_DIR is
created inside bisect_witness.sh for every uncached commit.
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

REPO = Path("/agent/repos/omega")
PIN = "51e8c45c793007f211d0d9baee7de16767172444"
BRANCH = "work/ledger-bisect-part-10"
FORBIDDEN_EDGES = {
    "51e8c45c793007f211d0d9baee7de16767172444",
    "f3612364ffbb9489d7be1409397df6867fcbebc6",
}
TOOLS = Path("/tmp/ledger_bisect_tools_2026_11_11")


def git(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(["git", "-C", str(REPO), *args], capture_output=True, text=True, check=check)


def rev_parse(ref: str) -> str:
    return git("rev-parse", f"{ref}^{{commit}}").stdout.strip()


def subject(sha: str) -> str:
    return git("log", "-1", "--format=%s", sha).stdout.strip()


def pr_of(sha: str) -> str:
    m = re.search(r"\(#(\d+)\)\s*$", subject(sha))
    return m.group(1) if m else ""


def first_parent_chain() -> list[str]:
    return [ln for ln in git("rev-list", "--first-parent", PIN).stdout.splitlines() if ln]


def fixture_add(witness: str) -> str:
    path = f"tests/omega/{witness}"
    sha = git("log", "--first-parent", "--reverse", "-1", "--format=%H", PIN, "--", path).stdout.strip()
    if not sha:
        raise SystemExit(f"no fixture-add commit for {witness}")
    return sha


def checkout(sha: str) -> None:
    subprocess.run(
        ["git", "-C", str(REPO), "-c", "advice.detachedHead=false", "checkout", "-f", sha],
        check=True,
    )


def restore_branch() -> None:
    subprocess.run(["git", "-C", str(REPO), "bisect", "reset"], check=False, capture_output=True)
    subprocess.run(["git", "-C", str(REPO), "checkout", "-f", BRANCH], check=False)


def run_witness(witness: str, predicate: str, *, use_cache: bool, allow_skip: bool, filt: str | None = None) -> dict:
    env = os.environ.copy()
    env.update(
        {
            "BISECT_WITNESS": witness,
            "BISECT_FILTER": filt or witness,
            "BISECT_PREDICATE": predicate,
            "BISECT_USE_CACHE": "1" if use_cache else "0",
            "BISECT_ALLOW_LEDGER_SKIP": "1" if allow_skip else "0",
            "OMEGA_REPO": str(REPO),
        }
    )
    proc = subprocess.run(["bash", str(TOOLS / "bisect_witness.sh")], cwd=str(REPO), env=env)
    detail_path = Path("/tmp/bisect_last_detail.json")
    detail = json.loads(detail_path.read_text()) if detail_path.exists() else {}
    detail["exit"] = proc.returncode
    return detail


def walk(witness: str, predicate: str, revs: list[str], floor_idx: int) -> tuple[list[dict], dict | None, dict | None]:
    """Return (steps, good_step, nearest_bad_step)."""
    indices = [0]
    i = 1
    while i < floor_idx:
        indices.append(i)
        i *= 2
    if floor_idx not in indices:
        indices.append(floor_idx)

    steps: list[dict] = []
    good = None
    tested: set[int] = set()
    for idx in indices:
        if idx in tested or idx >= len(revs):
            continue
        tested.add(idx)
        sha = revs[idx]
        checkout(sha)
        detail = run_witness(witness, predicate, use_cache=True, allow_skip=True)
        step = {
            "index": idx,
            "sha": sha,
            "pr": pr_of(sha),
            "subject": subject(sha),
            "exit": detail.get("exit"),
            "reason": detail.get("reason"),
            "status": detail.get("status"),
            "diag": detail.get("diag"),
            "diag_equals_pin": detail.get("diag_equals_pin"),
            "diag_same_class": detail.get("diag_same_class"),
        }
        steps.append(step)
        print(f"WALK {predicate} idx={idx} {sha} pr={step['pr']} exit={step['exit']} status={step['status']}", flush=True)
        if step["exit"] == 0:
            good = step
            break
    nearest = None
    if good is not None:
        earlier = [s for s in steps if s["index"] < good["index"] and s["exit"] == 1]
        if earlier:
            nearest = max(earlier, key=lambda s: s["index"])
    return steps, good, nearest


def bisect(witness: str, predicate: str, bad: str, good: str) -> tuple[str, int, str]:
    restore_branch()
    subprocess.run(["git", "-C", str(REPO), "bisect", "reset"], check=False, capture_output=True)
    checkout(PIN)
    start = subprocess.run(
        ["git", "-C", str(REPO), "bisect", "start", "--first-parent", bad, good],
        capture_output=True,
        text=True,
    )
    if start.returncode != 0:
        return "", start.returncode, start.stderr
    env = os.environ.copy()
    env.update(
        {
            "BISECT_WITNESS": witness,
            "BISECT_FILTER": witness,
            "BISECT_PREDICATE": predicate,
            "BISECT_USE_CACHE": "1",
            "BISECT_ALLOW_LEDGER_SKIP": "1",
            "OMEGA_REPO": str(REPO),
        }
    )
    log_path = Path(f"/tmp/bisect_{predicate}.log")
    with log_path.open("w") as log:
        proc = subprocess.run(
            ["git", "-C", str(REPO), "bisect", "run", "bash", str(TOOLS / "bisect_witness.sh")],
            cwd=str(REPO),
            env=env,
            stdout=log,
            stderr=subprocess.STDOUT,
        )
    text = log_path.read_text()
    first = ""
    if any(line.endswith("is the first bad commit") or " is the first bad commit" in line for line in text.splitlines()):
        first = git("rev-parse", "HEAD", check=False).stdout.strip()
    subprocess.run(["git", "-C", str(REPO), "bisect", "log"], check=False, stdout=Path(f"/tmp/bisect_{predicate}_gitlog.txt").open("w"))
    restore_branch()
    return first, proc.returncode, text


def confirm(witness: str, predicate: str, sha: str, filt: str | None = None) -> list[dict]:
    outs = []
    for n in range(3):
        checkout(sha)
        detail = run_witness(witness, predicate, use_cache=False, allow_skip=False, filt=filt)
        outs.append(
            {
                "run": n + 1,
                "exit": detail.get("exit"),
                "status": detail.get("status"),
                "diag": detail.get("diag"),
                "diag_equals_pin": detail.get("diag_equals_pin"),
                "diag_same_class": detail.get("diag_same_class"),
                "members": detail.get("members"),
                "reason": detail.get("reason"),
            }
        )
        print(f"CONFIRM {predicate} {sha} run={n+1} exit={detail.get('exit')} status={detail.get('status')}", flush=True)
    return outs


def main() -> int:
    witness, cluster_id = sys.argv[1], sys.argv[2]
    members = json.loads(sys.argv[3]) if len(sys.argv) > 3 else [witness]
    src = REPO / ".agents/ledger-bisect-2026-11-11/tools"
    if TOOLS.exists():
        subprocess.run(["rm", "-rf", str(TOOLS)], check=True)
    subprocess.run(["cp", "-a", str(src), str(TOOLS)], check=True)
    os.chmod(TOOLS / "bisect_witness.sh", 0o755)

    restore_branch()
    subprocess.run(
        ["git", "-C", str(REPO), "show", f"{PIN}:tests/omega/corpus_outcomes.txt"],
        check=True,
        stdout=Path("/tmp/pin_corpus_outcomes.txt").open("w"),
    )
    snap = subprocess.run(
        [
            "python3",
            str(TOOLS / "pin_snapshot.py"),
            witness,
            str(REPO / ".agents/ledger-bisect-2026-11-11/part10_pin_measure.txt"),
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    print(snap.stdout, flush=True)
    Path("/tmp/bisect_trace.jsonl").write_text("")

    revs = first_parent_chain()
    add = fixture_add(witness)
    if add not in revs:
        raise SystemExit(f"fixture-add {add} not on first-parent chain")
    floor_idx = revs.index(add)
    print(f"fixture-add {add} index {floor_idx}", flush=True)

    steps, good, nearest = walk(witness, "outcome", revs, floor_idx)
    result: dict = {
        "cluster_id": cluster_id,
        "witness": witness,
        "members": members,
        "fixture_add": add,
        "outcome_walk": steps,
        "outcome_good": good["sha"] if good else None,
        "outcome_nearest_bad": nearest["sha"] if nearest else None,
        "status": "pending",
    }
    if good is None:
        result["status"] = "never-good"
        result["verdict"] = f"never-good since {add}"
        _write(cluster_id, result)
        restore_branch()
        return 0
    if nearest is None:
        result["status"] = "no-nearest-bad"
        _write(cluster_id, result)
        restore_branch()
        return 0
    result["status"] = "walk-done"
    _write(cluster_id, result)

    first, code, blog = bisect(witness, "outcome", nearest["sha"], good["sha"])
    result["outcome_bisect_exit"] = code
    result["outcome_bisect_log_tail"] = blog[-4000:]
    result["breaking_commit"] = first
    if not first or first in FORBIDDEN_EDGES:
        frontier = _skip_frontier(blog)
        result["skip_frontier"] = frontier
        resolved = _resolve_frontier(witness, "outcome", frontier, revs)
        result["frontier_resolution"] = resolved
        first = resolved.get("breaking_commit") or ""
        if not first or first in FORBIDDEN_EDGES:
            result["status"] = "unresolved-frontier" if not first else "rejected-edge"
            result["breaking_commit"] = first or None
            _write(cluster_id, result)
            restore_branch()
            return 0
    parent = rev_parse(f"{first}^1")
    result["breaking_commit"] = first
    result["breaking_parent"] = parent
    result["pr"] = pr_of(first)
    result["subject"] = subject(first)
    if parent in FORBIDDEN_EDGES:
        result["parent_is_forbidden_sha"] = parent

    filt = ",".join(members) if len(members) > 1 else witness
    result["confirm_bad"] = confirm(witness, "outcome", first, filt)
    result["confirm_parent"] = confirm(witness, "outcome", parent, filt)
    bad_exits = [c["exit"] for c in result["confirm_bad"]]
    parent_exits = [c["exit"] for c in result["confirm_parent"]]
    result["confirm_bad_exits"] = bad_exits
    result["confirm_parent_exits"] = parent_exits
    result["outcome_edge_confirmed"] = bad_exits == [1, 1, 1] and parent_exits == [0, 0, 0]
    diag = result["confirm_bad"][0].get("diag")
    pin_diag = json.loads(Path("/tmp/pin_witness_observation.json").read_text())["pin_bad_diag"]
    result["diag_at_bad"] = diag
    result["diag_at_pin"] = pin_diag
    result["diag_equals_pin"] = diag == pin_diag

    if len(members) > 1:
        result["member_flip"] = _member_flip(result["confirm_bad"], result["confirm_parent"], members)

    if result.get("outcome_edge_confirmed") and diag != pin_diag:
        csteps, cgood, cnear = walk(witness, "class", revs, floor_idx)
        result["class_walk"] = csteps
        result["class_good"] = cgood["sha"] if cgood else None
        result["class_nearest_bad"] = cnear["sha"] if cnear else None
        if cgood and cnear:
            cfirst, ccode, clog = bisect(witness, "class", cnear["sha"], cgood["sha"])
            result["class_bisect_exit"] = ccode
            result["class_bisect_log_tail"] = clog[-4000:]
            if not cfirst or cfirst in FORBIDDEN_EDGES:
                resolved = _resolve_frontier(witness, "class", _skip_frontier(clog), revs)
                result["class_frontier_resolution"] = resolved
                cfirst = resolved.get("breaking_commit") or ""
            result["class_breaking_commit"] = cfirst or None
            if cfirst and cfirst not in FORBIDDEN_EDGES:
                cparent = rev_parse(f"{cfirst}^1")
                result["class_breaking_parent"] = cparent
                result["class_pr"] = pr_of(cfirst)
                result["class_subject"] = subject(cfirst)
                result["class_confirm_bad"] = confirm(witness, "class", cfirst)
                result["class_confirm_parent"] = confirm(witness, "class", cparent)
                result["class_confirm_bad_exits"] = [c["exit"] for c in result["class_confirm_bad"]]
                result["class_confirm_parent_exits"] = [c["exit"] for c in result["class_confirm_parent"]]
                result["class_edge_confirmed"] = (
                    result["class_confirm_bad_exits"] == [1, 1, 1]
                    and result["class_confirm_parent_exits"] == [0, 0, 0]
                )
        elif cgood is None:
            result["class_verdict"] = f"PIN class absent back through {add}"

    skips = []
    trace = Path("/tmp/bisect_trace.jsonl")
    if trace.exists():
        for line in trace.read_text().splitlines():
            try:
                row = json.loads(line)
            except Exception:
                continue
            if row.get("witness") == witness and row.get("exit") == 125:
                skips.append({"sha": row.get("sha"), "reason": row.get("reason"), "predicate": row.get("predicate")})
    result["skips_125"] = skips
    if result.get("outcome_edge_confirmed") is not True:
        result["status"] = "confirm-failed"
    elif len(members) > 1 and any(not row["flipped"] for row in result.get("member_flip", {}).values()):
        result["status"] = "measured-split"
        result["unflipped_members"] = [m for m, row in result["member_flip"].items() if not row["flipped"]]
    else:
        result["status"] = "measured"
    _write(cluster_id, result)
    restore_branch()
    print(json.dumps({k: result[k] for k in result if not k.endswith("log_tail")}, indent=2)[:8000])
    return 0


def _skip_frontier(log: str) -> list[str]:
    found: list[str] = []
    grab = False
    for line in log.splitlines():
        if "could be any of" in line:
            grab = True
            continue
        if not grab:
            continue
        token = line.strip().split()[0] if line.strip() else ""
        if re.fullmatch(r"[0-9a-f]{7,40}", token):
            found.append(rev_parse(token))
        elif found:
            break
    return found


def _ledger_only(sha: str) -> bool:
    parent = git("rev-parse", "--verify", f"{sha}^1", check=False)
    if parent.returncode != 0:
        return False
    names = git("diff", "--name-only", f"{sha}^1", sha, check=False).stdout.splitlines()
    if not names:
        return False
    return all(n == "tests/omega/corpus_outcomes.txt" or n.endswith(".md") for n in names)


def _nearest_buildable(sha: str, toward_older: bool, revs: list[str]) -> str | None:
    if toward_older:
        cur = sha
        for _ in range(50):
            if not _ledger_only(cur):
                return cur
            nxt = git("rev-parse", "--verify", f"{cur}^1", check=False).stdout.strip()
            if not nxt:
                return None
            cur = nxt
        return None
    if sha not in revs:
        sha = rev_parse(sha)
    if sha not in revs:
        return None
    for j in range(revs.index(sha), -1, -1):
        if not _ledger_only(revs[j]):
            return revs[j]
    return None


def _resolve_frontier(witness: str, predicate: str, shas: list[str], revs: list[str]) -> dict:
    notes = []
    buildable: list[str] = []
    for sha in shas:
        full = rev_parse(sha)
        older = _nearest_buildable(full, True, revs)
        newer = _nearest_buildable(full, False, revs)
        notes.append(
            {
                "frontier": full,
                "ledger_only": _ledger_only(full),
                "pr": pr_of(full),
                "subject": subject(full),
                "older_buildable": older,
                "newer_buildable": newer,
            }
        )
        for cand in (older, newer):
            if cand and cand not in buildable and cand not in FORBIDDEN_EDGES:
                buildable.append(cand)
    tested = []
    breaking = ""
    for cand in buildable:
        checkout(cand)
        detail = run_witness(witness, predicate, use_cache=False, allow_skip=False)
        row = {
            "sha": cand,
            "exit": detail.get("exit"),
            "status": detail.get("status"),
            "diag": detail.get("diag"),
            "pr": pr_of(cand),
            "subject": subject(cand),
        }
        if row["exit"] == 1:
            parent = rev_parse(f"{cand}^1")
            checkout(parent)
            parent_detail = run_witness(witness, predicate, use_cache=False, allow_skip=False)
            row["parent"] = parent
            row["parent_exit"] = parent_detail.get("exit")
            row["parent_pr"] = pr_of(parent)
            if parent_detail.get("exit") == 0 and cand not in FORBIDDEN_EDGES and not breaking:
                breaking = cand
        tested.append(row)
        print(f"FRONTIER {predicate} {cand} exit={row['exit']}", flush=True)
    return {"notes": notes, "tested": tested, "breaking_commit": breaking}


def _member_flip(bad_runs: list[dict], parent_runs: list[dict], members: list[str]) -> dict:
    out = {}
    for member in members:
        bad_statuses = []
        parent_statuses = []
        for run in bad_runs:
            mem = (run.get("members") or {}).get(member) or {}
            bad_statuses.append(mem.get("status"))
        for run in parent_runs:
            mem = (run.get("members") or {}).get(member) or {}
            parent_statuses.append(mem.get("status"))
        flipped = all(s == "rejected" for s in bad_statuses) and all(s == "checked" for s in parent_statuses)
        out[member] = {"at_bad": bad_statuses, "at_parent": parent_statuses, "flipped": flipped}
    return out


def _write(cluster_id: str, result: dict) -> None:
    restore_branch()
    dest = REPO / ".agents/ledger-bisect-2026-11-11/results" / f"{cluster_id}.json"
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(json.dumps(result, indent=2) + "\n")
    print(f"wrote {dest}", flush=True)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception:
        restore_branch()
        raise
