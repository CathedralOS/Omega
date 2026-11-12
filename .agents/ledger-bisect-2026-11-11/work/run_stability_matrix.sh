#!/bin/bash
set -euo pipefail
ROOT=/agent/repos/omega
TOOLS=/tmp/ledger-bisect-tools
mkdir -p "$TOOLS"
cp -f "$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_test.py" "$TOOLS/"
PIN=51e8c45c793007f211d0d9baee7de16767172444
PREPARE="$ROOT/.agents/ledger-bisect-2026-11-11/tools/prepare_bisect_pin.sh"
cp "$ROOT/tests/omega/corpus_outcomes.txt" /tmp/ledger_bisect_corpus_outcomes.txt
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
export BISECT_PIN_LEDGER=/tmp/ledger_bisect_corpus_outcomes.txt

build_at() {
  local wt="$1"
  export REPO_ROOT="$wt" OMEGA_REPO="$wt"
  export CARGO_TARGET_DIR="$wt/target/bisect-stability"
  mkdir -p "$CARGO_TARGET_DIR"
  chmod -R u+w "$CARGO_TARGET_DIR" 2>/dev/null || true
  cd "$wt"
  CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$CARGO_TARGET_DIR" nice -n 19 mbx build -p omega </dev/null >/dev/null
  CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$CARGO_TARGET_DIR" nice -n 19 mbx test --no-run -p compiler --test corpus_runner </dev/null >/dev/null
  ls -t "$CARGO_TARGET_DIR"/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | grep -v '\.d$' | head -1
}

run_fixture() {
  local wt="$1" runner="$2" fixture="$3"
  export REPO_ROOT="$wt" OMEGA_REPO="$wt"
  export OMEGA_CORPUS_ROOT="$wt" OMEGA_LIBRARY_ROOT="$wt/source/library"
  export OMEGA_CORPUS_FIXTURE_FILTER="$fixture" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
  rm -f /tmp/bisect_one_record.txt
  nice -n 19 "$runner" </dev/null >/dev/null 2>&1 || true
  python3 "$TOOLS/bisect_test.py" "$fixture" /tmp/bisect_one_record.txt
}

ensure_wt() {
  local commit="$1"
  local dir="/tmp/ledger-bisect-wt-${commit:0:12}"
  if [[ ! -d "$dir/.git" ]]; then
    git -C "$ROOT" worktree add --detach "$dir" "$commit" >/dev/null
  fi
  echo "$dir"
}

OUT="$ROOT/.agents/ledger-bisect-2026-11-11/work/stability_matrix.txt"
: >"$OUT"

matrix() {
  local label="$1" commit="$2"
  shift 2
  local wt runner
  wt=$(ensure_wt "$commit")
  runner=$(build_at "$wt") || { echo "$label $commit BUILD_FAILED" | tee -a "$OUT"; return; }
  for fixture in "$@"; do
    bash "$PREPARE" "$fixture" "$PIN" >/dev/null
    for i in 1 2 3; do
      run_fixture "$wt" "$runner" "$fixture"
      code=$?
      echo "$label commit=${commit:0:12} fixture=$fixture run$i exit=$code" | tee -a "$OUT"
    done
  done
}

C1_BAD=2476d1be2203009f1b32b8164bb6224c562c3bd5
C1_PARENT=c15929193d3993f264c54338440f4e1405dff0f6
C1=(
  pass/borrows/runtime_returned_nonzero_nested_window_exit
  pass/facts/matched_field_windows_through_other_state_exit
  pass/slices/runtime_returned_nested_array_window_home_exit
)
C2_BAD=22f215eb3e7d9814cb3e29f42856e56bb57e60ed
C2_PARENT=d25889014d5e1a5f3770dacd77612125f046e372
C2=pass/ownership/runtime_match_nested_observation_cleanup_exit

matrix cluster1-parent "$C1_PARENT" "${C1[@]}"
matrix cluster1-bad "$C1_BAD" "${C1[@]}"
matrix cluster2-parent "$C2_PARENT" "$C2"
matrix cluster2-bad "$C2_BAD" "$C2"

echo "done -> $OUT"
