#!/bin/bash
set -euo pipefail
WITNESS="${1:?}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
START_FROM="${3:-}"
export OMEGA_REPO="/agent/repos/omega"
export OMEGA_BISECT_TOOLS="/tmp/ledger-bisect-part-30-tools"
export OMEGA_PIN_ROOT="/tmp/ledger-bisect-part-30"
export OMEGA_PIN_LEDGER="$OMEGA_PIN_ROOT/pin_corpus_outcomes.txt"
export OMEGA_PIN_OBS="$OMEGA_PIN_ROOT/pin_obs_${WITNESS//\//__}.txt"
export PATH="/home/ubuntu/.local/share/mbx/bin:$PATH"
export BISECT_WITNESS="$WITNESS"
SCRIPT="$OMEGA_BISECT_TOOLS/bisect_witness.sh"
cd "$OMEGA_REPO"

restore_report() {
  git checkout -qf work/ledger-bisect-part-30 -- .agents/ledger-bisect-2026-11-11 2>/dev/null || true
}

test_commit() {
  git checkout -qf "$1"
  restore_report
  "$SCRIPT"
}

set +e
test_commit "$PIN"
rc=$?
set -e
[[ "$rc" -eq 1 ]] || { echo "ERROR: PIN witness exit $rc (want 1)" >&2; exit 2; }

CUR="$PIN"
for i in $(seq 1 100); do
  PARENT=$(git rev-parse "${CUR}^1" 2>/dev/null || true)
  [[ -z "$PARENT" ]] && break
  set +e
  test_commit "$PARENT"
  rc=$?
  set -e
  echo "try $i $(git log -1 --oneline "$PARENT") -> $rc" >&2
  if [[ "$rc" -eq 0 ]]; then
    git checkout -qf work/ledger-bisect-part-30
    echo "$PARENT"
    exit 0
  fi
  CUR="$PARENT"
done
echo "ERROR: no good within 100 first-parent steps" >&2
git checkout -qf work/ledger-bisect-part-30
exit 2
