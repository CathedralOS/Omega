#!/bin/bash
# Acceptance verification: triple witness (one build/commit), members, good walks, #348 fixture audit.
set -euo pipefail
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
DIR="$(dirname "$0")"
VERIFY="$DIR/part-31-verify.log"
MEMBERS_LOG="$DIR/part-31-members.log"
GOOD_LOG="$DIR/part-31-good-walk.log"
FIXTURE_LOG="$DIR/part-31-fixture-audit.log"
BRANCH=work/ledger-bisect-part-31
cd "$ROOT"
: >"$VERIFY"
: >"$MEMBERS_LOG"
: >"$GOOD_LOG"
: >"$FIXTURE_LOG"
python3 "$DIR/setup_pin_specs.py" >/dev/null

build_at_commit() {
  export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
  CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || return 125
  CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1 || return 125
  return 0
}

witness_exit() {
  local witness=$1
  local RUNNER
  RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
  [[ -x "$RUNNER" ]] || return 125
  export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
  export OMEGA_CORPUS_FIXTURE_FILTER="$witness" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
  rm -f /tmp/bisect_one_record.txt
  nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
  python3 /tmp/ledger-bisect-pin/tools/bisect_test.py "$witness" /tmp/bisect_one_record.txt
}

triple_witness() {
  local label=$1 commit=$2 witness=$3
  git checkout -q "$commit"
  echo "=== $label commit=$commit witness=$witness ===" | tee -a "$VERIFY"
  local brc=0
  build_at_commit || brc=$?
  if [[ $brc -eq 125 ]]; then
    echo "  build exit=125" | tee -a "$VERIFY"
    for i in 1 2 3; do echo "  run$i exit=125" | tee -a "$VERIFY"; done
    return
  fi
  for i in 1 2 3; do
    set +e
  witness_exit "$witness"
    rc=$?
    set -e
    echo "  run$i exit=$rc" | tee -a "$VERIFY"
  done
}

run_members_at() {
  local label=$1 commit=$2
  shift 2
  local members=("$@")
  git checkout -q "$commit"
  echo "=== members $label commit=$commit ===" | tee -a "$MEMBERS_LOG"
  if ! build_at_commit; then
    echo "  build exit=125" | tee -a "$MEMBERS_LOG"
    for m in "${members[@]}"; do echo "$m build_skip" | tee -a "$MEMBERS_LOG"; done
    return
  fi
  local RUNNER FILTER
  RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
  FILTER=$(IFS=,; echo "${members[*]}")
  export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
  export OMEGA_CORPUS_FIXTURE_FILTER="$FILTER" OMEGA_CORPUS_RECORDS=/tmp/members_record.txt
  rm -f /tmp/members_record.txt
  nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
  for m in "${members[@]}"; do
    line=$(grep -m1 "^$m " /tmp/members_record.txt || echo "$m MISSING")
    echo "  $line" | tee -a "$MEMBERS_LOG"
    set +e
    python3 /tmp/ledger-bisect-pin/tools/bisect_test.py "$m" /tmp/members_record.txt
    mrc=$?
    set -e
    echo "  $m bisect_test_exit=$mrc" | tee -a "$MEMBERS_LOG"
  done
}

# c0 assignment
C0_W=pass/borrows/runtime_selected_reference_mutation_complement_exit
C0_BAD=b7e951cdab51dda4e199f3fd33c53a0408fe72db
C0_PARENT=c0b334362da0ce997052dc277a639af7471555e9
C0_MEMBERS=(
  pass/borrows/runtime_selected_reference_mutation_complement_exit
  pass/borrows/runtime_uniform_projected_reference_selection_exit
  pass/write_only/returned_destination_transport_exit
)

triple_witness c0-first-bad "$C0_BAD" "$C0_W"
triple_witness c0-parent "$C0_PARENT" "$C0_W"
run_members_at c0-first-bad "$C0_BAD" "${C0_MEMBERS[@]}"
run_members_at c0-parent "$C0_PARENT" "${C0_MEMBERS[@]}"

# c1 window
C1_W=pass/facts/computed_record_window_through_other_machine_exit
C1_BAD=2174c0d10bb146aea6a86f55262bc536feb86ea3
C1_PARENT=bebbe455e7517a069862d87aead2f1ca911588c3
C1_MEMBERS=(
  pass/facts/computed_record_window_through_other_machine_exit
  pass/facts/computed_record_window_through_other_state_exit
)

triple_witness c1-first-bad "$C1_BAD" "$C1_W"
triple_witness c1-parent "$C1_PARENT" "$C1_W"
run_members_at c1-first-bad "$C1_BAD" "${C1_MEMBERS[@]}"
run_members_at c1-parent "$C1_PARENT" "${C1_MEMBERS[@]}"

# c2 match root
C2_W=pass/facts/match_computed_root_through_other_state_exit
C2_BAD=d30fccc7aae65a2358024cf39788d1007e2661b4
C2_PARENT=6f0eea318690404f6d4e3d962434415493382377
C2_MEMBERS=(pass/facts/match_computed_root_through_other_state_exit)

triple_witness c2-first-bad "$C2_BAD" "$C2_W"
triple_witness c2-parent "$C2_PARENT" "$C2_W"
run_members_at c2-first-bad "$C2_BAD" "${C2_MEMBERS[@]}"
run_members_at c2-parent "$C2_PARENT" "${C2_MEMBERS[@]}"

# c3 generic
C3_W=pass/generics/generic_result_parameter_receipt
C3_BAD=5ab4e12334fba74babfc3ae8d9992572fba307e6
C3_PARENT=eab58bf427c48063e3fcf597e4550ca53ac7cb8c
C3_MEMBERS=(pass/generics/generic_result_parameter_receipt)

triple_witness c3-first-bad "$C3_BAD" "$C3_W"
triple_witness c3-parent "$C3_PARENT" "$C3_W"
run_members_at c3-first-bad "$C3_BAD" "${C3_MEMBERS[@]}"
run_members_at c3-parent "$C3_PARENT" "${C3_MEMBERS[@]}"

# #348 parent fixture existence
echo "=== #348 parent fixture audit commit=$C3_PARENT ===" | tee -a "$FIXTURE_LOG"
git checkout -q "$C3_PARENT"
for path in \
  tests/omega/pass/generics/generic_result_parameter_receipt/main.omg \
  tests/omega/pass/generics/generic_result_parameter_receipt/build.omg \
  source/library \
  tests/omega/corpus_outcomes.txt; do
  if git cat-file -e "$C3_PARENT:$path" 2>/dev/null; then
    echo "  present $path" | tee -a "$FIXTURE_LOG"
  else
    echo "  MISSING $path" | tee -a "$FIXTURE_LOG"
  fi
done
line=$(grep -m1 '^pass/generics/generic_result_parameter_receipt ' /tmp/ledger-bisect-pin/corpus_outcomes.txt || true)
echo "  frozen_pin_ledger_line=$line" | tee -a "$FIXTURE_LOG"

# good walks (exponential first-parent from PIN)
for pair in \
  "c0 $C0_W" \
  "c1 $C1_W" \
  "c2 $C2_W" \
  "c3 $C3_W"; do
  set -- $pair
  echo "=== good-walk $1 witness=$2 ===" | tee -a "$GOOD_LOG"
  python3 "$DIR/verify_good_walk.py" "$2" 2>&1 | tee -a "$GOOD_LOG" || true
done

git checkout -q "$BRANCH"
echo "DONE acceptance verify" | tee -a "$VERIFY"
