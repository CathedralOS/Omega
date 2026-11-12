#!/bin/bash
set -euo pipefail
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
MAP="$ROOT/.agents/ledger-bisect-2026-11-11/pin_clusters.json"
OUTDIR="$ROOT/.agents/ledger-bisect-2026-11-11/bisects"
mkdir -p "$OUTDIR"
python3 - <<'PY'
import json, subprocess, os
clusters = json.load(open(".agents/ledger-bisect-2026-11-11/pin_clusters.json"))
for c in clusters:
    wid = c["witness"]
    cid = c["cluster_id"]
    out = f".agents/ledger-bisect-2026-11-11/bisects/cluster_{cid}.json"
    print(f"=== bisect cluster {cid} witness {wid}", flush=True)
    subprocess.run(["nice", "-n", "19", ".agents/ledger-bisect-2026-11-11/bisect_one.sh", wid, out], check=False)
PY
