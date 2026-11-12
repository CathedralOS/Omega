#!/bin/bash
set -euo pipefail
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
PIN="${PIN_SHA:-51e8c45c793007f211d0d9baee7de16767172444}"
RECORDS="${1:-/tmp/part14_records.txt}"
cp "$ROOT/tests/omega/corpus_outcomes.txt" /tmp/pin_corpus_outcomes.txt
python3 - "$RECORDS" "$ROOT" <<'PY'
import json, re, sys
from pathlib import Path
sys.path.insert(0, str(Path(sys.argv[2]) / ".agents/ledger-bisect-2026-11-11/tools"))
from classify_corpus import parse_records, norm_diag

observed = parse_records(Path(sys.argv[1]).read_text())  # records path
witnesses = Path("/tmp/part14_broken.txt").read_text().splitlines()
# cluster witnesses from broken list — use largest per cluster file if exists
cluster_path = Path("/tmp/part14_cluster_witnesses.txt")
if cluster_path.exists():
    witness_list = [l.strip() for l in cluster_path.read_text().splitlines() if l.strip()]
else:
    witness_list = witnesses
out = {}
for w in witness_list:
    if w not in observed:
        continue
    o = observed[w]
    d = (o["diagnostics"] or [""])[0]
    out[w] = {
        "broken_status": o["status"],
        "broken_diag": d,
        "diag_norm": norm_diag(d),
    }
Path("/tmp/pin_witness_obs.json").write_text(json.dumps(out, indent=2))
print(f"witnesses={len(out)}")
PY
