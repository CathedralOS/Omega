#!/bin/bash
# Bisect five remaining statement-successor rows: good d26fa952 .. bad PIN (strict PIN rc=1).
set -euo pipefail
ROOT="/agent/repos/omega"
cd "$ROOT"
git checkout -q work/ledger-bisect-part-5
GOOD=d26fa952d579d52642970560c719d24fa2a89f8a
PIN=51e8c45c793007f211d0d9baee7de16767172444
DIR="$ROOT/.agents/ledger-bisect-2026-11-11"
chmod +x "$DIR/tools/"*.sh
cp -a "$DIR" /tmp/ledger-bisect-2026-11-11
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
  if [[ -f "$out" ]] && python3 -c "import json,sys; j=json.load(open('$out')); sys.exit(0 if j.get('breaking_commit') not in ('$PIN','$GOOD','79603f52ce125c0be7a8038fa57a4e7cd104d732') and j.get('bisect_exit')==0 else 1)" 2>/dev/null; then
    echo "=== skip (valid) $w ==="
    continue
  fi
  echo "=== bisect $w ===" | tee "$DIR/bisect_remain_${slug}.log"
  "$DIR/tools/run_bisect_worktree.sh" "$w" "$GOOD" "$PIN" "$out" 2>&1 | tee -a "$DIR/bisect_remain_${slug}.log"
done
echo "=== pin diagnostic class (witness other_state) d26fa952..PIN ===" | tee "$DIR/pin_class_other_state.log"
W=pass/facts/selected_native_code_layout_through_other_state_exit
"$DIR/tools/run_bisect_worktree.sh" "$W" "$GOOD" "$PIN" "$DIR/bisect_pin_class_other_state.json" 2>&1 | tee -a "$DIR/pin_class_other_state.log"
echo DONE >"$DIR/cluster_a_remaining.done"
