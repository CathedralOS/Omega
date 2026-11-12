#!/bin/bash
# Part-17 hint: probe 60cb5e1f^1 (good) and 60cb5e1f (#1721) before full good-walk.
set -euo pipefail
WITNESS="pass/facts/opaque_independent_inherited_result_through_local_exit"
PIN="${1:-51e8c45c793007f211d0d9baee7de16767172444}"
BREAK="60cb5e1f79dd05465f77775990e46bcc2e789470"
GOOD_PARENT="${BREAK}^1"
REPO="/agent/repos/omega"
TOOLS="/tmp/ledger-bisect-part-30-tools"
ART="/opt/cursor/artifacts/ledger-bisect-part-30"
LOG="$ART/opaque_probe.log"
cd "$REPO"
export OMEGA_REPO="$REPO" OMEGA_BISECT_TOOLS="$TOOLS" PATH="/home/ubuntu/.local/share/mbx/bin:$PATH"
export OMEGA_PIN_ROOT=/tmp/ledger-bisect-part-30
export BISECT_WITNESS="$WITNESS"
mkdir -p "$ART" "$TOOLS"
git checkout -qf work/ledger-bisect-part-30 2>/dev/null || true
cp -a "$REPO/.agents/ledger-bisect-2026-11-11/tools/"* "$TOOLS/"
cp "$REPO/.agents/ledger-bisect-2026-11-11/"{run_one_bisect.sh,setup_pin_refs.sh} "$TOOLS/"
chmod +x "$TOOLS"/*.sh
bash "$TOOLS/setup_pin_refs.sh" "$PIN" "$WITNESS" >/dev/null
export OMEGA_PIN_LEDGER="$OMEGA_PIN_ROOT/pin_corpus_outcomes.txt"
export OMEGA_PIN_OBS="$OMEGA_PIN_ROOT/pin_obs_${WITNESS//\//__}.txt"

restore_report() {
  git checkout -qf work/ledger-bisect-part-30 -- \
    .agents/ledger-bisect-2026-11-11/part-30.md \
    .agents/ledger-bisect-2026-11-11/part-30.tsv 2>/dev/null || true
}

test_one() {
  local c="$1"
  git checkout -qf "$c"
  restore_report
  set +e
  "$TOOLS/bisect_witness.sh" >/dev/null 2>&1
  local r=$?
  set -e
  echo "PROBE $(git log -1 --oneline "$c") -> $r"
  return "$r"
}

: >"$LOG"
GOOD_PARENT=$(git rev-parse "$GOOD_PARENT")
PIN=$(git rev-parse "$PIN")

set +e
test_one "$GOOD_PARENT" | tee -a "$LOG"
prc=$?
test_one "$BREAK" | tee -a "$LOG"
brc=$?
set -e

if [[ "$prc" -eq 0 && "$brc" -eq 1 ]]; then
  echo "PROBE_OK good=$GOOD_PARENT break_hint=$BREAK" | tee -a "$LOG"
  bash "$TOOLS/run_one_bisect.sh" "$WITNESS" "$GOOD_PARENT" "$PIN" "$ART/bisect_opaque.log"
  python3 "$REPO/.agents/ledger-bisect-2026-11-11/record_cluster.py" opaque "$ART/bisect_opaque.log" bisected
  git checkout -qf work/ledger-bisect-part-30
  exit 0
fi

echo "PROBE_FAIL parent=$prc break=$brc (fall back to exp good-walk)" | tee -a "$LOG"
exit 1
