#!/bin/bash
set -euo pipefail
ROOT=/agent/repos/omega
PIN=51e8c45c793007f211d0d9baee7de16767172444
cd "$ROOT"
git checkout -f -q work/ledger-bisect-part-21
clusters=(
"c0:pass/facts/non_owning_reference_erasure_through_other_machine_exit"
"c1:pass/borrows/dynamic_window_return_custody_exit"
"c2:pass/facts/recoverable_replacement_window_through_target_body_exit"
"c3:pass/facts/non_owning_reference_erasure_through_target_body_exit"
"c4:pass/terminal_psi/debug_identity_complete_module"
"c5:pass/control_flow/runtime_ranked_state_cycle_borrowed_record_exit"
)
for item in "${clusters[@]}"; do
  id="${item%%:*}"
  w="${item#*:}"
  git checkout -f -q work/ledger-bisect-part-21
  rm -f ".agents/ledger-bisect-2026-11-11/bisect-${id}.log"
  nice -n 19 .agents/ledger-bisect-2026-11-11/run_bisect_cluster.sh "$w" "$id" 2>&1 | tee ".agents/ledger-bisect-2026-11-11/bisect-${id}.log"
  git checkout -f -q work/ledger-bisect-part-21
  cp "/tmp/find-good-${id}.log" ".agents/ledger-bisect-2026-11-11/find-good-${id}.log" 2>/dev/null || true
  python3 .agents/ledger-bisect-2026-11-11/generate_part21.py
  git checkout -f -q work/ledger-bisect-part-21
  git add .agents/ledger-bisect-2026-11-11/
  git commit -m "repo: ledger bisect part 21 cluster ${id}" || true
  git push -u origin work/ledger-bisect-part-21 || true
done
git checkout -f -q work/ledger-bisect-part-21
