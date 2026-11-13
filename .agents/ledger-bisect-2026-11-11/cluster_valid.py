#!/usr/bin/env python3
"""Count finished part-4 clusters. A wrong closed edge does not count."""
from __future__ import annotations

import json
import subprocess
from pathlib import Path

PIN = "51e8c45c793007f211d0d9baee7de16767172444"
FORBIDDEN = {PIN, "f3612364ffbb9489d7be1409397df6867fcbebc6"}
DIR = Path(__file__).resolve().parent
WT = DIR.parents[1] / ".agents" / "ledger-bisect-wt"


def ledger_only(sha: str) -> bool:
    if len(sha) != 40:
        return False
    proc = subprocess.run(
        ["git", "-C", str(WT), "diff-tree", "--no-commit-id", "--name-only", "-r", "-m", sha],
        capture_output=True,
        text=True,
    )
    files = [line for line in proc.stdout.splitlines() if line.strip()]
    if not files:
        return False
    return all(item == "tests/omega/corpus_outcomes.txt" or item.endswith(".md") for item in files)


def edge_ok(edge: dict) -> bool:
    if not edge.get("accepted"):
        return False
    if edge.get("breaking_exit_triplet") != [1, 1, 1]:
        return False
    if edge.get("parent_exit_triplet") != [0, 0, 0]:
        return False
    brk = edge.get("breaking_commit") or ""
    if len(brk) != 40 or brk in FORBIDDEN or ledger_only(brk):
        return False
    hunk = edge.get("diff_hunk") or ""
    if hunk != "mechanism not established" and "@@" not in hunk:
        return False
    return True


def main() -> int:
    clusters = json.loads((DIR / "pin_clusters.json").read_text())
    ok = 0
    for cluster in clusters:
        path = DIR / "bisects" / f"cluster_{cluster['cluster_id']}.json"
        if not path.exists():
            print(cluster["cluster_id"], "missing")
            continue
        data = json.loads(path.read_text())
        if data.get("witness_version") != 2 or not data.get("finished"):
            print(cluster["cluster_id"], "old", data.get("status"))
            continue
        status = data.get("status")
        if status == "never-good" and str(data.get("mechanism", "")).startswith("never-good since "):
            sha = data["mechanism"].split()[-1]
            good = len(sha) == 40
        elif status == "bisected":
            good = any(edge_ok(edge) for edge in data.get("edges") or [])
        elif status == "unresolved":
            good = True
        else:
            good = False
        if good:
            ok += 1
            print(cluster["cluster_id"], status, (data.get("breaking_commit") or data.get("mechanism") or "")[:48])
        else:
            print(cluster["cluster_id"], "invalid", status)
    print(f"valid={ok}/12")
    state = DIR / "run_state.json"
    if state.exists():
        queue = json.loads(state.read_text()).get("queue") or []
        print(f"queue={len(queue)}")
    return 0 if ok == 12 else 1


if __name__ == "__main__":
    raise SystemExit(main())
