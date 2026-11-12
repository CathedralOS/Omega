#!/bin/bash
set -euo pipefail
PIN=51e8c45c793007f211d0d9baee7de16767172444
ART=/opt/cursor/artifacts/ledger-bisect-part-30
REPO=/agent/repos/omega
cd "$REPO"
git checkout -qf work/ledger-bisect-part-30
mkdir -p "$ART" /tmp/ledger-bisect-part-30-tools
cp -a "$REPO/.agents/ledger-bisect-2026-11-11/tools/"* /tmp/ledger-bisect-part-30-tools/
chmod +x /tmp/ledger-bisect-part-30-tools/*.sh "$REPO/.agents/ledger-bisect-2026-11-11"/*.sh

WITNESSES=(
  "pass/facts/recoverable_replacement_window_through_const_name_exit:recoverable"
  "pass/facts/opaque_independent_inherited_result_through_local_exit:opaque"
  "pass/traits/runtime_qualified_owned_requirement_calls_exit:traits"
  "pass/effects/gui_semantic_services:gui"
)

bash "$REPO/.agents/ledger-bisect-2026-11-11/setup_pin_refs.sh" "$PIN" \
  pass/facts/recoverable_replacement_window_through_const_name_exit \
  pass/facts/opaque_independent_inherited_result_through_local_exit \
  pass/traits/runtime_qualified_owned_requirement_calls_exit \
  pass/effects/gui_semantic_services

for pair in "${WITNESSES[@]}"; do
  WITNESS="${pair%%:*}"
  TAG="${pair##*:}"
  LOG="$ART/bisect_${TAG}.log"
  echo "=== $TAG ===" | tee "$ART/run_all.log"
  GOOD=$(bash "$REPO/.agents/ledger-bisect-2026-11-11/walk_good.sh" "$WITNESS" "$PIN" 2>"$ART/walk_${TAG}.err")
  echo "GOOD=$GOOD" | tee -a "$ART/run_all.log"
  bash "$REPO/.agents/ledger-bisect-2026-11-11/run_one_bisect.sh" "$WITNESS" "$GOOD" "$PIN" "$LOG" | tee -a "$ART/run_all.log"
  git checkout -qf work/ledger-bisect-part-30
  python3 "$REPO/.agents/ledger-bisect-2026-11-11/record_cluster.py" "$TAG" "$LOG"
  git -C "$REPO" add .agents/ledger-bisect-2026-11-11/part-30.md .agents/ledger-bisect-2026-11-11/part-30.tsv
  git -C "$REPO" commit -m "repo: ledger bisect part 30 cluster ${TAG}"
  git -C "$REPO" push origin work/ledger-bisect-part-30
done
echo DONE
