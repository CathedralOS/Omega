#!/bin/bash
set -euo pipefail
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
DIR="$(dirname "$0")"
LOG="$DIR/part-31-ledger-checked.log"
: >"$LOG"
cd "$ROOT"
W=6ace467e4f78bb55d5d3e6727539e4a47814ec0a
echo "ledger_writer $W (#1807)" | tee -a "$LOG"
for f in \
  pass/borrows/runtime_selected_reference_mutation_complement_exit \
  pass/borrows/runtime_uniform_projected_reference_selection_exit \
  pass/write_only/returned_destination_transport_exit \
  pass/generics/generic_result_parameter_receipt; do
  nice -n 19 bash "$DIR/run_triple_scoped.sh" "$W" "$f" ledger-checked-1807 | tee -a "$LOG"
done
# c3 stamp host ada93c78b02f+wt (idx 1228, #1226)
A=ada93c78b02f2f5c031fc8513f9ff13c9641f29a
echo "c3_stamp_host $A (#1226)" | tee -a "$LOG"
nice -n 19 bash "$DIR/run_triple_scoped.sh" "$A" pass/generics/generic_result_parameter_receipt stamp-1226 | tee -a "$LOG"
git checkout -q -f work/ledger-bisect-part-31
echo DONE | tee -a "$LOG"
