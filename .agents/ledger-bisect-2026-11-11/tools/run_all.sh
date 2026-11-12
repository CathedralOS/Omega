#!/bin/bash
# Run all part-27 cluster bisects sequentially.
set -euo pipefail
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
git checkout -f work/ledger-bisect-part-27
TOOLS="$ROOT/.agents/ledger-bisect-2026-11-11/tools"
while IFS=$'\t' read -r cid witness; do
  [[ -z "$cid" || "$cid" == \#* ]] && continue
  bash "$TOOLS/bisect_cluster.sh" "$cid" "$witness"
  python3 "$TOOLS/update_part27.py" "$cid"
  git add .agents/ledger-bisect-2026-11-11/
  git commit -m "repo: ledger bisect part 27 cluster ${cid}"
  git push origin work/ledger-bisect-part-27
done <<'EOF'
c01	pass/targets/private_target_supplies_remain_disclosed
c02	pass/targets/retained_target_family_bodies
c03	pass/borrows/runtime_stored_reference_case_residual_retirement_exit
c04	run/borrows/terminal_case_completed_observation
c05	pass/traits/prepared_callable_nested_borrow_paths_exit
c06	pass/facts/saved_observation_snapshot_indexed_handback_other_machine_exit
c07	pass/facts/case_payload_exclusive_loan_through_other_machine_exit
EOF
