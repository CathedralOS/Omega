#!/bin/bash
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
git checkout -q work/ledger-bisect-part-20
CLUSTERS=(
  "pass/borrows/incoming_reference_slot_replacement_exit:cluster_ucf"
  "pass/borrows/runtime_selected_reference_record_helper_exit:cluster_borrow"
  "pass/generics/open_computed_quantity_result:cluster_index"
  "pass/borrows/runtime_partial_reference_partition_exit:cluster_overlap"
)
for pair in "${CLUSTERS[@]}"; do
  W=${pair%%:*}
  T=${pair##*:}
  bash "$ROOT/.agents/ledger-bisect-2026-11-11/bisect_cluster.sh" "$W" "$T"
  python3 "$ROOT/.agents/ledger-bisect-2026-11-11/update_part20.py" --cluster "$T"
  git add ".agents/ledger-bisect-2026-11-11/part-20.md" ".agents/ledger-bisect-2026-11-11/part-20.tsv"
  git commit -m "repo: ledger bisect part 20 cluster ${T}"
  git push -u origin work/ledger-bisect-part-20
done
