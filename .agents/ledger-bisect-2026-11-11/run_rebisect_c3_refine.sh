#!/bin/bash
set -euo pipefail
ROOT=/agent/repos/omega
PIN=51e8c45c793007f211d0d9baee7de16767172444
WITNESS=pass/generics/generic_result_parameter_receipt
LOG="$ROOT/.agents/ledger-bisect-2026-11-11/part-31-rebisect-c3.log"
export OMEGA_REPO="$ROOT"
export CARGO_TARGET_DIR=/tmp/ledger-bisect-rebisect-c3
cd "$ROOT"
mapfile -t REVS < <(git rev-list --first-parent "$PIN")
lo=331
hi=662

test_sha() {
  local sha=$1
  git checkout -q -f "$sha"
  CARGO_PROFILE_DEV_OPT_LEVEL=1 nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || return 125
  CARGO_PROFILE_DEV_OPT_LEVEL=1 nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1 || return 125
  local RUNNER
  RUNNER=$(ls -t "$CARGO_TARGET_DIR/debug/build/compiler"/*/out/corpus_runner-* 2>/dev/null | head -1)
  [[ -x "$RUNNER" ]] || return 125
  export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
  export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/refine_c3.txt
  rm -f /tmp/refine_c3.txt
  nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
  python3 /tmp/ledger-bisect-pin/tools/bisect_test.py "$WITNESS" /tmp/refine_c3.txt
}

echo "--- refine lo=$lo hi=$hi ---" | tee -a "$LOG"
while (( lo + 1 < hi )); do
  mid=$(( (lo + hi) / 2 ))
  sha="${REVS[mid]}"
  set +e
  test_sha "$sha"
  rc=$?
  set -e
  echo "  refine idx=$mid sha=$sha exit=$rc" | tee -a "$LOG"
  if [[ "$rc" == 125 ]]; then
  hi=$(( mid - 1 ))
    continue
  fi
  if [[ "$rc" == 0 ]]; then
    lo="$mid"
  else
    hi="$mid"
  fi
done
new_bad="${REVS[hi]}"
parent=$(git rev-parse "${new_bad}^1")
echo "REFINED_BAD_IDX=$hi" | tee -a "$LOG"
echo "REFINED_FIRST_BAD=$new_bad" | tee -a "$LOG"
echo "REFINED_PARENT=$parent" | tee -a "$LOG"
git log -1 --format='%H %s' "$new_bad" | tee -a "$LOG"
git checkout -q -f work/ledger-bisect-part-31
