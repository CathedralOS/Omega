#!/bin/bash
set -euo pipefail
ROOT=/agent/repos/omega
cd "$ROOT"
PIN=51e8c45c793007f211d0d9baee7de16767172444
SCRIPT="$ROOT/.agents/ledger-bisect-tools/bisect_witness.sh"
OUTDIR="$ROOT/.agents/ledger-bisect-2026-11-11"
run_one() {
  local id=$1 witness=$2 good=$3
  export BISECT_WITNESS=$witness
  git bisect reset 2>/dev/null || true
  if ! git bisect start --first-parent "$PIN" "$good" 2>/dev/null; then
    echo "bisect start failed id=$id" | tee "$OUTDIR/bisect-$id.log"
    return
  fi
  set +e
  git bisect run "$SCRIPT" 2>&1 | tee "$OUTDIR/bisect-$id.log"
  local rc=$?
  set -e
  local brk
  brk=$(git rev-parse HEAD)
  local pr subj
  subj=$(git log -1 --format='%s' "$brk" 2>/dev/null || true)
  pr=$(sed -n 's/.*#\([0-9][0-9]*\).*/\1/p' <<<"$subj" | head -1)
  git bisect reset 2>/dev/null || true
  echo "id=$id break=$brk pr=${pr:-none} rc=$rc subj=$subj" | tee -a "$OUTDIR/bisect-summary.txt"
}
: > "$OUTDIR/bisect-summary.txt"
run_one c0 pass/ownership/runtime_match_conditional_observation_cleanup_exit 763971f1b33
run_one c1 run/borrows/returned_descriptor_entry_snapshot c64728d90e8
run_one c2 run/slices/returned_record_fixed_array_window 3e187851339
run_one c3 pass/proofs/reviewed_mathematical_data_contract_exit 11bdd2fe41f
