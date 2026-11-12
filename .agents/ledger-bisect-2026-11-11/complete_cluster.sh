#!/bin/bash
set -euo pipefail
TAG="${1:?tag}"
WITNESS="${2:?witness}"
BAD_ANCHOR="${3:-8e7394363d14aca7dae993adeb86fc348c0d2c7e}"
PIN="${4:-51e8c45c793007f211d0d9baee7de16767172444}"
REPO="/agent/repos/omega"
RUNTIME="/tmp/ledger-bisect-part-30"
TOOLS="/tmp/ledger-bisect-part-30-tools"
cd "$REPO"
git checkout -qf work/ledger-bisect-part-30
mkdir -p "$RUNTIME" "$TOOLS"
install_tools() {
  cp "$REPO/.agents/ledger-bisect-2026-11-11/tools/"*.py "$TOOLS/"
  cp "$REPO/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh" "$TOOLS/"
  cp "$REPO/.agents/ledger-bisect-2026-11-11/"{exp_walk_good.sh,run_one_bisect.sh,setup_pin_refs.sh} "$TOOLS/"
  chmod +x "$TOOLS"/*.sh
}
install_tools

bash "$TOOLS/setup_pin_refs.sh" "$PIN" "$WITNESS" >/dev/null

LOG="$RUNTIME/bisect_${TAG}.log"
EXP_LOG="$RUNTIME/exp_walk_${TAG}.log"

set +e
GOOD=$(bash "$TOOLS/exp_walk_good.sh" "$WITNESS" "$PIN" "$BAD_ANCHOR" "$EXP_LOG")
ew=$?
set -e

if [[ "$ew" -eq 3 ]]; then
  NAME="${WITNESS##*/}"
  SAMPLE=$(git ls-files "samples/**/*.omg" | rg "/${NAME}\.omg$" | head -1 || true)
  if [[ -z "$SAMPLE" ]]; then
    SAMPLE=$(git ls-files "tests/omega/**/*.omg" | rg "$NAME" | head -1 || true)
  fi
  ADD=""
  if [[ -n "$SAMPLE" ]]; then
    ADD=$(git log --diff-filter=A --first-parent -n 1 --format=%H -- "$SAMPLE" 2>/dev/null || true)
  fi
  if [[ -z "$ADD" ]]; then
    ADD=$(git log --diff-filter=A --first-parent -n 1 --format=%H -S "$WITNESS" -- tests/omega/corpus_outcomes.txt 2>/dev/null || true)
  fi
  [[ -n "$ADD" ]] || { echo "cannot find add commit for $WITNESS"; exit 2; }
  export OMEGA_REPO="$REPO" OMEGA_BISECT_TOOLS="$TOOLS" PATH="/home/ubuntu/.local/share/mbx/bin:$PATH"
  export OMEGA_PIN_ROOT="$RUNTIME" OMEGA_PIN_LEDGER="$RUNTIME/pin_corpus_outcomes.txt"
  export OMEGA_PIN_OBS="$RUNTIME/pin_obs_${WITNESS//\//__}.txt"
  export BISECT_WITNESS="$WITNESS"
  git checkout -qf "$ADD"
  git checkout -qf work/ledger-bisect-part-30 -- \
    .agents/ledger-bisect-2026-11-11/part-30.md \
    .agents/ledger-bisect-2026-11-11/part-30.tsv
  set +e
  bash "$TOOLS/bisect_witness.sh" >/dev/null 2>&1
  arc=$?
  set -e
  SUBJ=$(git log -1 --format=%s "$ADD")
  PR=$(sed -n 's/.*(#\([0-9][0-9]*\)).*/\1/p' <<<"$SUBJ" | head -1)
  {
    echo "VERDICT=never-good"
    echo "ADD=$ADD"
    echo "SUBJ=$SUBJ"
    echo "PR=${PR:-unknown}"
    echo "ADD_TEST_RC=$arc"
  } | tee "$LOG"
  git checkout -qf work/ledger-bisect-part-30
  python3 "$REPO/.agents/ledger-bisect-2026-11-11/record_cluster.py" "$TAG" "$LOG" never-good
  exit 0
fi

[[ "$ew" -eq 0 ]] || exit "$ew"
echo "GOOD=$GOOD" | tee -a "$EXP_LOG"
bash "$TOOLS/run_one_bisect.sh" "$WITNESS" "$GOOD" "$PIN" "$LOG"
python3 "$REPO/.agents/ledger-bisect-2026-11-11/record_cluster.py" "$TAG" "$LOG" bisected
git checkout -qf work/ledger-bisect-part-30
