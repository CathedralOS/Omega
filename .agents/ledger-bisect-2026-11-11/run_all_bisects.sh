#!/bin/bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
export CARGO_TARGET_DIR="$ROOT/target-pin"
export BISECT_GOOD_RECOVERABLE=d25889014d5e1a5f3770dacd77612125f046e372
TOOLS="$ROOT/.agents/ledger-bisect-2026-11-11/tools"
PIN=51e8c45c793007f211d0d9baee7de16767172444

run_one() {
  local cid=$1 witness=$2 good=${3:-}
  echo "=== cluster $cid witness $witness"
  if [[ -n "$good" ]]; then export BISECT_GOOD="$good"; else unset BISECT_GOOD; fi
  bash "$TOOLS/run_bisect_pin.sh" "$witness" "$ROOT/.agents/ledger-bisect-2026-11-11/bisect_${cid}.json"
  python3 - <<PY
import json
from pathlib import Path
meta=json.loads(Path("$ROOT/.agents/ledger-bisect-2026-11-11/part16_meta.json").read_text())
b=json.loads(Path("$ROOT/.agents/ledger-bisect-2026-11-11/bisect_${cid}.json").read_text())
meta.setdefault("bisects",{})[ "${cid}".replace("bisect_","") if False else "$cid" ] = b
# map pin-N cluster id
for c in meta["pin_clusters"]:
    if c["witness"]=="$witness":
        meta["bisects"][c["cluster_id"]]=b
Path("$ROOT/.agents/ledger-bisect-2026-11-11/part16_meta.json").write_text(json.dumps(meta,indent=2))
PY
  bash "$ROOT/.agents/ledger-bisect-2026-11-11/commit_cluster.sh" "$cid"
}

git checkout -q "$PIN"
run_one pin-0 pass/facts/recoverable_replacement_window_through_target_body_exit "$BISECT_GOOD_RECOVERABLE"
run_one pin-1 pass/borrows/runtime_selected_reference_record_helper_exit ""
run_one pin-2 pass/providers/checked_operator_declaration_without_application ""
