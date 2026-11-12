#!/bin/bash
# Newest first-bad for c3 between ledger-good (pass at idx) and PIN (fail).
set -euo pipefail
ROOT=/agent/repos/omega
PIN=51e8c45c793007f211d0d9baee7de16767172444
GOOD=6ace467e4f78bb55d5d3e6727539e4a47814ec0a
WITNESS=pass/generics/generic_result_parameter_receipt
LOG="$ROOT/.agents/ledger-bisect-2026-11-11/part-31-rebisect-c3.log"
cd "$ROOT"
git checkout -q -f work/ledger-bisect-part-31
export OMEGA_REPO="$ROOT"
python3 /tmp/ledger-bisect-pin/tools/setup_pin_specs.py >/dev/null 2>&1 || python3 "$ROOT/.agents/ledger-bisect-2026-11-11/setup_pin_specs.py" >/dev/null
export CARGO_TARGET_DIR=/tmp/ledger-bisect-rebisect-c3
mkdir -p "$CARGO_TARGET_DIR"
mapfile -t REVS < <(git rev-list --first-parent "$PIN")
good_i=$(python3 -c "import sys; revs=sys.stdin.read().split(); print(revs.index('$GOOD'))" <<<"${REVS[*]}")
echo "good_idx=$good_i good=$GOOD pin=$PIN" | tee -a "$LOG"

bisect_test() {
  git checkout -q -f "$1"
  export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
  CARGO_PROFILE_DEV_OPT_LEVEL=1 nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || return 125
  CARGO_PROFILE_DEV_OPT_LEVEL=1 nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1 || return 125
  local RUNNER
  RUNNER=$(ls -t "$CARGO_TARGET_DIR/debug/build/compiler"/*/out/corpus_runner-* 2>/dev/null | head -1)
  [[ -x "$RUNNER" ]] || return 125
  export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
  export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/rebisect_c3_record.txt
  rm -f /tmp/rebisect_c3_record.txt
  nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
  python3 /tmp/ledger-bisect-pin/tools/bisect_test.py "$WITNESS" /tmp/rebisect_c3_record.txt
}

# Monotonic: pass at good_i .. fail at 0 — find lowest idx with fail (newest first-bad).
lo=0
hi="$good_i"
while (( lo + 1 < hi )); do
  mid=$(( (lo + hi) / 2 ))
  sha="${REVS[mid]}"
  set +e
  bisect_test "$sha"
  rc=$?
  set -e
  echo "  probe idx=$mid sha=$sha exit=$rc" | tee -a "$LOG"
  if [[ "$rc" == 0 ]]; then
    lo="$mid"
  else
    hi="$mid"
  fi
done
new_bad="${REVS[hi]}"
parent=$(git rev-parse "${new_bad}^1")
echo "NEW_FIRST_BAD_IDX=$hi" | tee -a "$LOG"
echo "NEW_FIRST_BAD=$new_bad" | tee -a "$LOG"
echo "NEW_PARENT=$parent" | tee -a "$LOG"
git log -1 --format='%H %s' "$new_bad" | tee -a "$LOG"
git checkout -q -f work/ledger-bisect-part-31
echo REBISECT_DONE | tee -a "$LOG"
