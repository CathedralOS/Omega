#!/bin/bash
set -euo pipefail
cd /agent/repos/omega
export BISECT_RETURN_BRANCH=work/ledger-bisect-part-29
TOOL=.agents/ledger-bisect-tools
OUT=.agents/ledger-bisect-2026-11-11
run_one() {
  local id=$1 witness=$2
  echo "=== bisect $id $witness"
  bash "$TOOL/bisect_cluster.sh" "$id" "$witness" "$OUT/$id.json"
  python3 "$TOOL/write_part29.py"
  git add "$OUT/part-29.md" "$OUT/part-29.tsv" "$OUT/$id.json" "$TOOL"
  git commit -m "docs: ledger bisect part 29 cluster $id"
  git push -u origin work/ledger-bisect-part-29
}
run_one c0 pass/ownership/runtime_match_conditional_observation_cleanup_exit
run_one c1 run/borrows/returned_descriptor_entry_snapshot
run_one c2 run/slices/returned_record_fixed_array_window
run_one c3 pass/proofs/reviewed_mathematical_data_contract_exit
echo ALL_PART29_DONE
