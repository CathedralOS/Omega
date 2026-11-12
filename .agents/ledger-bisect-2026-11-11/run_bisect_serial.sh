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

root = Path("/agent/repos/omega")
clusters = json.loads((root / ".agents/ledger-bisect-2026-11-11/pin_clusters.json").read_text())
start_id = int(os.environ.get("BISECT_START_CLUSTER", "0"))
for c in sorted(clusters, key=lambda x: (-x["rows"], x["cluster_id"])):
    if c["cluster_id"] < start_id:
        continue
    cid = c["cluster_id"]
    w = c["witness"]
    out = root / f".agents/ledger-bisect-2026-11-11/bisects/cluster_{cid}.json"
    print(f"=== cluster {cid} rows={c['rows']} witness={w}", flush=True)
    env = {
        **dict(**os.environ),
        "BISECT_REPO_ROOT": str(root / ".agents/ledger-bisect-wt"),
        "BISECT_PIN_LEDGER": "/tmp/bisect_pin_corpus_outcomes.txt",
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
    ]
    subprocess.run(["git", "add", *paths], cwd=root)
    st = subprocess.run(["git", "diff", "--cached", "--quiet"], cwd=root)
    if st.returncode != 0:
        msg = f"docs: ledger bisect part 4 cluster {cid} witness {w.split('/')[-1]}"
        subprocess.run(["git", "commit", "-m", msg], cwd=root)
        subprocess.run(["git", "push", "-u", "origin", "work/ledger-bisect-part-4"], cwd=root)
# second pass: clusters that did not get a compiler first-bad
for c in sorted(clusters, key=lambda x: x["cluster_id"]):
    cid = c["cluster_id"]
    out = root / f".agents/ledger-bisect-2026-11-11/bisects/cluster_{cid}.json"
    if not out.exists():
        continue
    prev = json.loads(out.read_text())
    if prev.get("status") == "bisected" and prev.get("breaking_commit") and not (
        (prev.get("mechanism") or "").startswith("git bisect landed on ledger-only")
    ):
        continue
    w = c["witness"]
    print(f"=== redo cluster {cid} witness={w}", flush=True)
    env = {
        **dict(**os.environ),
        "BISECT_REPO_ROOT": str(root / ".agents/ledger-bisect-wt"),
        "BISECT_PIN_LEDGER": "/tmp/bisect_pin_corpus_outcomes.txt",
    }
    subprocess.run(
        ["nice", "-n", "19", str(root / ".agents/ledger-bisect-2026-11-11/bisect_one.sh"), w, str(out)],
        cwd=root,
        env=env,
    )
    subprocess.run(["python3", str(root / ".agents/ledger-bisect-2026-11-11/write_part4_report.py")], cwd=root)
    subprocess.run(["git", "add", ".agents/ledger-bisect-2026-11-11/part-4.md", ".agents/ledger-bisect-2026-11-11/part-4.tsv", f".agents/ledger-bisect-2026-11-11/bisects/cluster_{cid}.json"], cwd=root)
    st = subprocess.run(["git", "diff", "--cached", "--quiet"], cwd=root)
    if st.returncode != 0:
        subprocess.run(["git", "commit", "-m", f"docs: ledger bisect part 4 redo cluster {cid} witness {w.split('/')[-1]}"], cwd=root)
        subprocess.run(["git", "push", "-u", "origin", "work/ledger-bisect-part-4"], cwd=root)
PY
