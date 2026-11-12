#!/bin/bash
set -euo pipefail
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
git fetch origin work/ledger-bisect-part-4
git checkout work/ledger-bisect-part-4
git pull --ff-only origin work/ledger-bisect-part-4 2>/dev/null || git reset --hard origin/work/ledger-bisect-part-4
export BISECT_REPO_ROOT="$ROOT/.agents/ledger-bisect-wt"
if [[ ! -f "$BISECT_REPO_ROOT/.git" ]]; then
  git worktree add -f "$BISECT_REPO_ROOT" 51e8c45c793007f211d0d9baee7de16767172444
fi
PIN=51e8c45c793007f211d0d9baee7de16767172444
cd "$BISECT_REPO_ROOT"
git fetch origin "$PIN" 2>/dev/null || git fetch origin
git checkout -f "$PIN"
cp "$BISECT_REPO_ROOT/tests/omega/corpus_outcomes.txt" /tmp/bisect_pin_corpus_outcomes.txt
export BISECT_PIN_LEDGER=/tmp/bisect_pin_corpus_outcomes.txt
python3 <<'PY'
import json
import os
import subprocess
from pathlib import Path

PIN = "51e8c45c793007f211d0d9baee7de16767172444"
root = Path("/agent/repos/omega")
clusters = json.loads((root / ".agents/ledger-bisect-2026-11-11/pin_clusters.json").read_text())


def ledger_only(sha: str) -> bool:
    if len(sha) != 40:
        return False
    st = subprocess.run(
        ["git", "-C", str(root / ".agents/ledger-bisect-wt"), "diff-tree", "--no-commit-id", "--name-only", "-r", sha],
        capture_output=True,
        text=True,
    )
    files = [f for f in st.stdout.splitlines() if f.strip()]
    if not files:
        return False
    for f in files:
        if f != "tests/omega/corpus_outcomes.txt" and not f.endswith(".md"):
            return False
    return True


def cluster_needs_run(prev: dict | None) -> bool:
    if not prev:
        return True
    if prev.get("find_good_method") != "exponential":
        return True
    mech = prev.get("mechanism") or ""
    if mech.startswith("never-good since"):
        fa = prev.get("fixture_added") or ""
        return len(fa) != 40
    if prev.get("status") != "bisected":
        return True
    brk = prev.get("breaking_commit") or ""
    if len(brk) != 40 or brk == PIN or ledger_only(brk):
        return True
    if not prev.get("breaking_exit_triplet"):
        return True
    return False


for c in sorted(clusters, key=lambda x: (-x["rows"], x["cluster_id"])):
    cid = c["cluster_id"]
    w = c["witness"]
    out = root / f".agents/ledger-bisect-2026-11-11/bisects/cluster_{cid}.json"
    prev = json.loads(out.read_text()) if out.exists() else None
    if prev and not cluster_needs_run(prev):
        print(f"=== cluster {cid} skip (complete)", flush=True)
        continue
    tag = "redo" if prev else "cluster"
    print(f"=== {tag} {cid} rows={c['rows']} witness={w}", flush=True)
    step_log = root / f".agents/ledger-bisect-2026-11-11/bisect_steps_cluster_{cid}.tsv"
    env = {
        **dict(**os.environ),
        "BISECT_REPO_ROOT": str(root / ".agents/ledger-bisect-wt"),
        "BISECT_PIN_LEDGER": "/tmp/bisect_pin_corpus_outcomes.txt",
        "BISECT_STEP_LOG": str(step_log),
    }
    subprocess.run(
        ["nice", "-n", "19", str(root / ".agents/ledger-bisect-2026-11-11/bisect_one.sh"), w, str(out)],
        cwd=root,
        env=env,
    )
    subprocess.run(["python3", str(root / ".agents/ledger-bisect-2026-11-11/write_part4_report.py")], cwd=root)
    paths = [
        ".agents/ledger-bisect-2026-11-11/part-4.md",
        ".agents/ledger-bisect-2026-11-11/part-4.tsv",
        f".agents/ledger-bisect-2026-11-11/bisects/cluster_{cid}.json",
        f".agents/ledger-bisect-2026-11-11/bisect_steps_cluster_{cid}.tsv",
    ]
    subprocess.run(["git", "add", *[p for p in paths if (root / p).exists()]], cwd=root)
    st = subprocess.run(["git", "diff", "--cached", "--quiet"], cwd=root)
    if st.returncode != 0:
        msg = f"docs: ledger bisect part 4 cluster {cid} witness {w.split('/')[-1]}"
        subprocess.run(["git", "commit", "-m", msg], cwd=root)
        subprocess.run(["git", "push", "-u", "origin", "work/ledger-bisect-part-4"], cwd=root)
PY
