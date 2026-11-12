#!/bin/bash
# Run first-parent bisect for one witness; print BREAK= and SUBJ= on success.
set -euo pipefail
WITNESS="${1:?witness}"
GOOD="${2:?good commit}"
PIN="${3:-51e8c45c793007f211d0d9baee7de16767172444}"
export OMEGA_REPO="/agent/repos/omega"
ROOT="$OMEGA_REPO"
cd "$ROOT"
export PATH="/home/ubuntu/.local/share/mbx/bin:$PATH"
export OMEGA_BISECT_TOOLS="${OMEGA_BISECT_TOOLS:-/tmp/ledger-bisect-part-30-tools}"
SCRIPT="$OMEGA_BISECT_TOOLS/bisect_witness.sh"
LOG="${4:-$ROOT/.agents/ledger-bisect-2026-11-11/bisect_${WITNESS//\//_}.log}"

GOOD=$(git rev-parse "$GOOD")
PIN=$(git rev-parse "$PIN")
export BISECT_WITNESS="$WITNESS"

git checkout -q work/ledger-bisect-part-30
git bisect reset >/dev/null 2>&1 || true

verify() {
  local label="$1" c="$2" n="$3"
  git checkout -q "$c"
  local ok=0 bad=0 skip=0 i
  for i in $(seq 1 "$n"); do
    set +e
    "$SCRIPT" >/dev/null 2>&1
    r=$?
    set -e
    case $r in
      0) ok=$((ok + 1));;
      1) bad=$((bad + 1));;
      *) skip=$((skip + 1));;
    esac
  done
  echo "VERIFY $label $(git log -1 --oneline "$c") ok=$ok bad=$bad skip=$skip" | tee -a "$LOG"
}

: >"$LOG"
verify good "$GOOD" 2
verify pin "$PIN" 2
verify mid 741270df60f7 2

git checkout -q "$PIN"
git bisect start --first-parent "$PIN" "$GOOD" | tee -a "$LOG"
set +e
git bisect run "$SCRIPT" 2>&1 | tee -a "$LOG"
set -e
FIRST=$(grep -E '^[0-9a-f]{40} is the first bad commit' "$LOG" | tail -1 | sed 's/ is the first bad commit.*//')
git bisect reset "$PIN" >/dev/null
git checkout -q work/ledger-bisect-part-30

if [[ -z "$FIRST" ]]; then
  echo "ERROR: bisect did not find first bad" | tee -a "$LOG"
  exit 2
fi
if [[ "$FIRST" == "$PIN" ]]; then
  echo "ERROR: first bad equals PIN; check good commit" | tee -a "$LOG"
  exit 2
fi
SUBJ=$(git log -1 --format=%s "$FIRST")
echo "BREAK=$FIRST" | tee -a "$LOG"
echo "SUBJ=$SUBJ" | tee -a "$LOG"
PR=$(sed -n 's/.*(#\([0-9]*\)).*/\1/p' <<<"$SUBJ" | head -1)
echo "PR=${PR:-unknown}" | tee -a "$LOG"
