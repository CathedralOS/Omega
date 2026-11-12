#!/bin/bash
set -euo pipefail
cd /agent/repos/omega
export MBX_SCHEDULER_CPUS=8 CARGO_BUILD_JOBS=8 OMEGA_CORPUS_JOBS=1
git checkout work/ledger-bisect-part-26
WITNESSES=(
  "pass/termination/nested_helper_record_measure_result_equation_exit"
  "pass/calls/runtime_constructed_borrowed_payload_arguments_exit"
  "pass/borrows/copied_intermediate_descriptor_snapshot_exit"
  "pass/facts/saved_observation_snapshot_indexed_handback_call_result_exit"
  "pass/borrows/runtime_indexed_bounded_primitive_loan_write_exit"
)
i=0
for w in "${WITNESSES[@]}"; do
  echo "=== cluster $i witness $w ==="
  .agents/ledger-bisect-2026-11-11/tools/run_bisect_cluster.sh "$w" ".agents/ledger-bisect-2026-11-11/work/bisect_c${i}.json" \
    | tee ".agents/ledger-bisect-2026-11-11/work/bisect_c${i}.run.log"
  python3 .agents/ledger-bisect-2026-11-11/tools/write_part26.py
  git add .agents/ledger-bisect-2026-11-11/part-26.md .agents/ledger-bisect-2026-11-11/part-26.tsv \
    .agents/ledger-bisect-2026-11-11/work/bisect_c${i}.json \
    .agents/ledger-bisect-2026-11-11/work/bisect_c${i}.log 2>/dev/null || \
  git add .agents/ledger-bisect-2026-11-11/part-26.md .agents/ledger-bisect-2026-11-11/part-26.tsv \
    .agents/ledger-bisect-2026-11-11/work/bisect_c${i}.json
  git add .agents/ledger-bisect-2026-11-11/work/bisect_c${i}.log 2>/dev/null || true
  git add .agents/ledger-bisect-2026-11-11/tools/ .agents/ledger-bisect-2026-11-11/work/part26_meta.json .agents/ledger-bisect-2026-11-11/work/pin_*.json .agents/ledger-bisect-2026-11-11/work/pin_measure.txt 2>/dev/null || true
  git commit -m "repo: ledger bisect part 26 cluster ${i}" || true
  git push -u origin work/ledger-bisect-part-26
  i=$((i+1))
done
