#!/bin/bash
# Bisect clusters largest-first; one cluster per invocation when BATCH=1 env set.
set -euo pipefail
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
AUDIT="$ROOT/wiki/drafts/audits"
CLUSTERS="$AUDIT/clusters.json"
BSECT="$AUDIT/bisects"
mkdir -p "$BSECT"
MAX="${BISECT_MAX:-9999}"
START="${BISECT_START:-0}"
python3 - <<'PY' "$CLUSTERS" "$BSECT" "$MAX" "$START"
import json, subprocess, sys
from pathlib import Path
clusters = json.loads(Path(sys.argv[1]).read_text())["clusters"]
out_dir = Path(sys.argv[2])
max_n, start = int(sys.argv[3]), int(sys.argv[4])
done = 0
for i, c in enumerate(clusters):
    if i < start:
        continue
    if done >= max_n:
        break
    key = f"{i:04d}_{c['count']}"
    out = out_dir / f"{key}.json"
    if out.exists():
        continue
    witness = c["witness"]
    print(f"bisect cluster {i} rows={c['count']} witness={witness}")
    subprocess.check_call([
        "wiki/drafts/audits/tools/run_bisect_cluster.sh", witness, str(out.with_suffix(""))
    ])
    done += 1
    subprocess.check_call(["git", "add", str(out), str(out.with_suffix(".log"))])
    subprocess.check_call(["git", "commit", "-m", f"docs: bisect cluster {i} ({c['count']} rows) witness {witness}"])
    subprocess.check_call(["git", "push", "origin", "cursor/ledger-regressions-report-d9c6"])
PY
