#!/bin/bash
set -euo pipefail
WITNESS="${1:?}"
GOOD="${2:?}"
PIN="${3:-51e8c45c793007f211d0d9baee7de16767172444}"
LOG="${4:?log path}"
export OMEGA_REPO="/agent/repos/omega"
export OMEGA_BISECT_TOOLS="/tmp/ledger-bisect-part-30-tools"
export OMEGA_PIN_ROOT="/tmp/ledger-bisect-part-30"
export OMEGA_PIN_LEDGER="$OMEGA_PIN_ROOT/pin_corpus_outcomes.txt"
export OMEGA_PIN_OBS="$OMEGA_PIN_ROOT/pin_obs_${WITNESS//\//__}.txt"
export PATH="/home/ubuntu/.local/share/mbx/bin:$PATH"
export BISECT_WITNESS="$WITNESS"
GOOD=$(git -C "$OMEGA_REPO" rev-parse "$GOOD")
PIN=$(git -C "$OMEGA_REPO" rev-parse "$PIN")
SCRIPT="$OMEGA_BISECT_TOOLS/bisect_witness.sh"
cd "$OMEGA_REPO"

verify() {
  local label="$1" c="$2"
  git checkout -qf "$c"
  local ok=0 bad=0 skip=0
  for _ in 1 2; do
    set +e
    "$SCRIPT" >/dev/null 2>&1
    r=$?
    set -e
    case $r in 0) ok=$((ok + 1));; 1) bad=$((bad + 1));; *) skip=$((skip + 1));; esac
  done
  echo "VERIFY $label $(git log -1 --oneline "$c") ok=$ok bad=$bad skip=$skip"
}

: >"$LOG"
verify good "$GOOD" | tee -a "$LOG"
verify pin "$PIN" | tee -a "$LOG"
grep 'VERIFY good' "$LOG" | tail -1 | grep -q 'ok=2' || exit 2
grep 'VERIFY pin' "$LOG" | tail -1 | grep -q 'bad=2' || exit 2

git checkout -qf "$PIN"
git bisect reset >/dev/null 2>&1 || true
git bisect start --first-parent "$PIN" "$GOOD" | tee -a "$LOG"
set +e
git bisect run "$SCRIPT" 2>&1 | tee -a "$LOG"
set -e
FIRST=$(grep -E '^[0-9a-f]{40} is the first bad commit' "$LOG" | tail -1 | sed 's/ is the first bad commit.*//')
git bisect reset "$PIN" >/dev/null
git checkout -qf work/ledger-bisect-part-30
[[ -n "$FIRST" && "$FIRST" != "$PIN" ]] || exit 2
SUBJ=$(git log -1 --format=%s "$FIRST")
PR=$(sed -n 's/.*(#\([0-9][0-9]*\)).*/\1/p' <<<"$SUBJ" | head -1)
echo "BREAK=$FIRST" | tee -a "$LOG"
echo "SUBJ=$SUBJ" | tee -a "$LOG"
echo "PR=${PR:-unknown}" | tee -a "$LOG"
