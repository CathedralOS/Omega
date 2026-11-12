#!/bin/bash
set -euo pipefail
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
git checkout work/ledger-bisect-part-2
MEASURE="$ROOT/.agents/ledger-bisect-2026-11-11/slice_2_pin_measure.txt"
OUT_DIR="$ROOT/.agents/ledger-bisect-2026-11-11/bisect_results"
WITNESSES=(
  run/ownership/sequence_owned_local_cleanup
  pass/facts/static_callback_reach_through_field
  pass/borrows/returned_record_case_field_store_exit
  pass/borrows/runtime_reassigned_view_capture_exit
  pass/optimizer/pre_allocation_joint_frontier_exit
)
for _w in "${WITNESSES[@]}"; do
  python3 "$ROOT/.agents/ledger-bisect-2026-11-11/tools/pin_snapshot.py" "$_w" "$MEASURE"
done
for w in "${WITNESSES[@]}"; do
  echo "=== bisect $w ==="
  set +e
  bash "$ROOT/.agents/ledger-bisect-2026-11-11/bisect_one.sh" "$w"
  rc=$?
  set -e
  if [[ "$rc" -ne 0 ]]; then
    SAFE="${w//\//_}"
    python3 - "$OUT_DIR/${SAFE}.json" "$w" "$rc" <<'PY'
import json, sys
out, witness, rc = sys.argv[1:4]
json.dump(
    {
        "witness": witness,
        "status": "no_good_on_first_parent",
        "bisect_rc": int(rc),
        "method": "pin-fixed-ledger",
    },
    open(out, "w"),
    indent=2,
)
PY
    echo "WARN: bisect failed for $w (rc=$rc)" >&2
  fi
  python3 "$ROOT/.agents/ledger-bisect-2026-11-11/write_part_2.py"
  git add .agents/ledger-bisect-2026-11-11/
  git commit -m "repo: ledger bisect part 2 witness ${w##*/}"
  git push -u origin work/ledger-bisect-part-2
done
