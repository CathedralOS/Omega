#!/bin/bash
# Run all PIN still-broken cluster bisects for part 10; commit after each.
set -euo pipefail
ROOT=/agent/repos/omega
PIN=51e8c45c793007f211d0d9baee7de16767172444
cd "$ROOT"
git checkout -f work/ledger-bisect-part-10
git reset --hard "$PIN"

CLUSTERS=(
  "pass/build/explicit_provider_storage_extent_exit:pin-c2"
  "pass/borrows/returned_record_case_field_store_exit:pin-c3"
  "pass/generics/runtime_const_data_symbolic_expression_exit:pin-c4"
  "pass/traits/applied_requirement_lexical_isolation_exit:pin-c5"
  "pass/build/default_component_abi_extents:pin-c1"
)

for item in "${CLUSTERS[@]}"; do
  WITNESS="${item%%:*}"
  CID="${item##*:}"
  echo "==== $CID $WITNESS ===="
  bash .agents/ledger-bisect-2026-11-11/tools/bisect_one_cluster.sh "$WITNESS" "$CID"
  python3 .agents/ledger-bisect-2026-11-11/update_part_report.py
  git add .agents/ledger-bisect-2026-11-11/
  git commit -m "repo: ledger bisect part 10 cluster ${CID}"
  git push -u origin work/ledger-bisect-part-10
done
