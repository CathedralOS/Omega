#!/bin/bash
set -euo pipefail
ROOT=/agent/repos/omega
cd "$ROOT"
git checkout -f -q work/ledger-bisect-part-21
clusters=(
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
  nice -n 19 .agents/ledger-bisect-2026-11-11/run_bisect_cluster.sh "$w" "$id" 2>&1 | tee "/tmp/bisect-${id}.log"
  git checkout -f -q work/ledger-bisect-part-21
  cp "/tmp/bisect-${id}.log" ".agents/ledger-bisect-2026-11-11/bisect-${id}.log"
  cp "/tmp/find-good-${id}.log" ".agents/ledger-bisect-2026-11-11/find-good-${id}.log" 2>/dev/null || true
  cp "/tmp/ledger-bisect-saved/find-good-${id}.log" ".agents/ledger-bisect-2026-11-11/find-good-${id}.log" 2>/dev/null || true
  python3 .agents/ledger-bisect-2026-11-11/generate_part21.py
  git add .agents/ledger-bisect-2026-11-11/
  git commit -m "repo: ledger bisect part 21 cluster ${id}" || true
  git push -u origin work/ledger-bisect-part-21 || true
done
git checkout -f -q work/ledger-bisect-part-21
