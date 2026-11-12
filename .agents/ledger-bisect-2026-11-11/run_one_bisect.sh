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
REPO="$OMEGA_REPO"
GOOD=$(git -C "$REPO" rev-parse "$GOOD")
PIN=$(git -C "$REPO" rev-parse "$PIN")
SCRIPT="$OMEGA_BISECT_TOOLS/bisect_witness.sh"
cd "$REPO"

restore_report() {
  git checkout -qf work/ledger-bisect-part-30 -- \
    .agents/ledger-bisect-2026-11-11/part-30.md \
    .agents/ledger-bisect-2026-11-11/part-30.tsv 2>/dev/null || true
}

sync_tools() {
  mkdir -p "$OMEGA_BISECT_TOOLS"
  cp "$REPO/.agents/ledger-bisect-2026-11-11/tools/"*.py "$OMEGA_BISECT_TOOLS/"
  cp "$REPO/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh" "$OMEGA_BISECT_TOOLS/"
  chmod +x "$OMEGA_BISECT_TOOLS/bisect_witness.sh"
}
sync_tools

verify() {
  local label="$1" c="$2" expect="$3"
  git checkout -qf "$c"
  restore_report
  sync_tools
  local ok=0 bad=0 skip=0
  for _ in 1 2; do
    set +e
    bash "$SCRIPT" >/dev/null 2>&1
    r=$?
    case $r in 0) ok=$((ok + 1));; 1) bad=$((bad + 1));; *) skip=$((skip + 1));; esac
  done
  echo "VERIFY $label $(git log -1 --oneline "$c") expect=$expect ok=$ok bad=$bad skip=$skip"
  if [[ "$expect" -eq 0 ]]; then [[ "$ok" -eq 2 && "$skip" -eq 0 ]] || return 2; fi
  if [[ "$expect" -eq 1 ]]; then [[ "$bad" -eq 2 && "$skip" -eq 0 ]] || return 2; fi
  return 2
}

: >"$LOG"
set +e
verify good "$GOOD" 0 | tee -a "$LOG"
gstat=$?
verify pin "$PIN" 1 | tee -a "$LOG"
pstat=$?
set -e
[[ "$gstat" -eq 0 ]] || exit 2
[[ "$pstat" -eq 0 ]] || exit 2

git checkout -qf "$PIN"
git bisect reset >/dev/null 2>&1 || true
git bisect start --first-parent "$PIN" "$GOOD" | tee -a "$LOG"
set +e
git bisect run bash "$SCRIPT" 2>&1 | tee -a "$LOG"
set -e
FIRST=$(grep -E '^[0-9a-f]{40} is the first bad commit' "$LOG" | tail -1 | sed 's/ is the first bad commit.*//')
git bisect reset "$PIN" >/dev/null
git checkout -qf work/ledger-bisect-part-30
[[ -n "$FIRST" && "$FIRST" != "$PIN" ]] || exit 2
mapfile -t CHANGED < <(git diff --name-only "${FIRST}^1" "$FIRST" 2>/dev/null || true)
if ((${#CHANGED[@]} > 0)); then
  ledger_only=1
  for f in "${CHANGED[@]}"; do
    if [[ "$f" != tests/omega/corpus_outcomes.txt ]] && [[ "$f" != *.md ]]; then
      ledger_only=0
      break
    fi
  done
  if ((ledger_only)); then
    echo "ERROR: first-bad is ledger-only $FIRST" | tee -a "$LOG"
    exit 2
  fi
fi
SUBJ=$(git log -1 --format=%s "$FIRST")
PR=$(sed -n 's/.*(#\([0-9][0-9]*\)).*/\1/p' <<<"$SUBJ" | head -1)
echo "BREAK=$FIRST" | tee -a "$LOG"
echo "SUBJ=$SUBJ" | tee -a "$LOG"
echo "PR=${PR:-unknown}" | tee -a "$LOG"
