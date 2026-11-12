#!/bin/bash
set -euo pipefail
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
DIR="$(dirname "$0")"
LOG="$DIR/part-31-good-confirm.log"
cd "$ROOT"
python3 "$DIR/setup_pin_specs.py" >/dev/null
: >"$LOG"
build_once() {
  export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
  CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1
  CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1
}
witness_three() {
  local commit=$1 witness=$2 label=$3
  git checkout -q "$commit"
  echo "=== $label commit=$commit witness=$witness ===" | tee -a "$LOG"
  build_once
  RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
  export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
  export OMEGA_CORPUS_FIXTURE_FILTER="$witness" OMEGA_CORPUS_RECORDS=/tmp/good_confirm_record.txt
  for i in 1 2 3; do
    rm -f /tmp/good_confirm_record.txt
    nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
    set +e
    python3 /tmp/ledger-bisect-pin/tools/bisect_test.py "$witness" /tmp/good_confirm_record.txt
    rc=$?
    set -e
    echo "  run$i exit=$rc" | tee -a "$LOG"
  done
}

witness_three c64728d90e83036051d52fb69961b786423230c6 pass/borrows/runtime_selected_reference_mutation_complement_exit c0-good
witness_three 773b8422694b5a8e64b43284bcba1ceb6de2b1e9 pass/facts/computed_record_window_through_other_machine_exit c1-good
witness_three 442a95195e64d20d6d373b6bbddcd11c7f6da38a pass/facts/match_computed_root_through_other_state_exit c2-good
witness_three 42ef40205f19c7797b352647f55b82519407191c pass/generics/generic_result_parameter_receipt c3-good
witness_three 741270df60f7eab218e879eb6771d2bc75e64c00 pass/borrows/runtime_selected_reference_mutation_complement_exit sanity-741270
git checkout -q work/ledger-bisect-part-31
