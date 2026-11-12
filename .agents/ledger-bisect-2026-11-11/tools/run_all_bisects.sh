#!/bin/bash
# Run bisect for each pending PIN cluster in the worktree, updating state after each.
set -euo pipefail
PIN=51e8c45c793007f211d0d9baee7de16767172444
ROOT=/agent/repos/omega
WT=/tmp/bisect-wt
TOOL=$ROOT/.agents/ledger-bisect-2026-11-11/tools
STATE=$ROOT/.agents/ledger-bisect-2026-11-11/state.json
cd "$WT"
witnesses=(
  pass/borrows/runtime_reassigned_view_capture_exit
  pass/facts/open_const_implementation_private_record_package_exit
  pass/borrows/runtime_shared_reference_array_cohort_restoration_exit
  pass/facts/open_const_implementation_statement_const_result_exit
  pass/memory/runtime_linear_native_allocation_call_result_exit
)
for w in "${witnesses[@]}"; do
  if python3 -c "import json; s=json.load(open('$STATE')); import sys; sys.exit(0 if any(c.get('witness')=='$w' and c.get('bisect',{}).get('status')=='pending' for c in s['pin_clusters']) else 1)"; then
    echo "=== bisect $w ==="
    nice -n 19 "$TOOL/run_bisect_cluster.sh" "$w" "$PIN" | tee "/tmp/bisect-$(echo "$w" | tr / _).log" | python3 "$TOOL/apply_bisect_result.py"
    python3 "$TOOL/write_part0.py" "$STATE"
    cd "$ROOT" && git add .agents/ledger-bisect-2026-11-11/ && git commit -m "repo: ledger bisect part 0 cluster $(echo "$w" | tr / _)" && git push -u origin work/ledger-bisect-part-0
    cd "$WT"
  fi
done
