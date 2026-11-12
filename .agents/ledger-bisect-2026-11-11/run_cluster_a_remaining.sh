#!/bin/bash
set -euo pipefail
ROOT="/agent/repos/omega"
cd "$ROOT"
git checkout -q work/ledger-bisect-part-5
GOOD=d26fa952d579d52642970560c719d24fa2a89f8a
PIN=51e8c45c793007f211d0d9baee7de16767172444
DIR="$ROOT/.agents/ledger-bisect-2026-11-11"
LOGDIR="/tmp/cluster-a-bisect-logs"
mkdir -p "$LOGDIR"
rm -rf /tmp/ledger-bisect-2026-11-11
cp -a "$DIR" /tmp/ledger-bisect-2026-11-11
chmod +x /tmp/ledger-bisect-2026-11-11/tools/*.sh
MANUAL="$DIR/tools/bisect_pin_manual.py"
WITNESSES=(
  pass/calls/native_code_layout_discarded_exit
  pass/facts/selected_native_code_layout_through_match_join_exit
  pass/facts/source_indexed_family_alpha_positive
  pass/facts/source_indexed_family_recursive_index_positive
  pass/proofs/erased_mathematical_initializers
)
for w in "${WITNESSES[@]}"; do
  slug=$(echo "$w" | tr '/' '_' )
  out="$DIR/bisect_remain_${slug}.json"
  log="$LOGDIR/bisect_remain_${slug}.log"
  if [[ -f "$out" ]] && python3 -c "import json,sys; j=json.load(open('$out')); sys.exit(0 if j.get('status')=='bisected' and j.get('breaking_commit') not in (None,'$PIN') and not str(j.get('breaking_commit','')).startswith('d6d75e') else 1)" 2>/dev/null; then
    echo "=== skip valid $w ===" | tee -a "$LOGDIR/summary.log"
    continue
  fi
  echo "=== manual bisect $w ===" | tee "$log"
  python3 "$MANUAL" "$w" "$GOOD" "$PIN" "$out" "$DIR/tools/bisect_step_pin_diag.sh" 2>&1 | tee -a "$log"
done
W=pass/facts/selected_native_code_layout_through_other_state_exit
out="$DIR/bisect_pin_class_other_state.json"
log="$LOGDIR/pin_class_other_state.log"
echo "=== pin diagnostic class $W ===" | tee "$log"
python3 "$MANUAL" "$W" "$GOOD" "$PIN" "$out" "$DIR/tools/bisect_step_pin_diag.sh" 2>&1 | tee -a "$log"
echo DONE >"$LOGDIR/cluster_a_remaining.done"
