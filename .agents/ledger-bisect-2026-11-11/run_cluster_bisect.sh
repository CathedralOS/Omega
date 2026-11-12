#!/bin/bash
set -euo pipefail
PIN=51e8c45c793007f211d0d9baee7de16767172444
SCRIPT_REPO=/agent/repos/omega
WT=/tmp/omega-bisect-wt
DIR="$SCRIPT_REPO/.agents/ledger-bisect-2026-11-11"
LEDGER=/tmp/ledger_bisect_part23_corpus_outcomes.txt
CLUSTER_ID="$1"
WITNESS="$2"
LOG="$DIR/bisect_${CLUSTER_ID}.log"
OBS="$DIR/pin_observed.txt"
if [[ ! -f "$WT/.git" ]]; then
  git -C "$SCRIPT_REPO" worktree add -f "$WT" "$PIN" >/dev/null 2>&1 || true
fi
if [[ ! -f "$WT/.git" ]]; then
  echo "missing bisect worktree at $WT" >&2
  exit 1
fi
export LEDGER_BISECT_REPO="$WT"
export LEDGER_BISECT_TOOL_ROOT="$DIR"
cd "$WT"
SPEC=$(python3 "$DIR/build_spec_from_pin.py" "$CLUSTER_ID" "$WITNESS" "$OBS" "$LEDGER")
export BISECT_WITNESS_SPEC="$SPEC"
echo "cluster=$CLUSTER_ID witness=$WITNESS spec=$SPEC" | tee "$LOG"
git checkout -q "$PIN"
for i in 1 2; do
  rc=0; "$DIR/bisect_witness.sh" || rc=$?
  echo "PIN sanity $i: exit $rc" | tee -a "$LOG"
  [[ "$rc" -eq 1 ]] || { echo "PIN not bad"; exit 1; }
done
GOOD=$(bash "$DIR/find_good_commit.sh" "$PIN" "$SPEC" | tee -a "$LOG")
echo "good=$GOOD" | tee -a "$LOG"
for i in 1 2; do
  git checkout -q "$GOOD"
  rc=0; "$DIR/bisect_witness.sh" || rc=$?
  echo "good sanity $i: exit $rc" | tee -a "$LOG"
  [[ "$rc" -eq 0 ]] || { echo "good not good"; exit 1; }
done
git checkout -q "$PIN"
export BISECT_WITNESS_SPEC="$SPEC"
git bisect start --first-parent "$PIN" "$GOOD" 2>&1 | tee -a "$LOG"
git bisect run "$DIR/bisect_witness.sh" 2>&1 | tee -a "$LOG"
FIRST=$(grep -m1 ' is the first bad commit' "$LOG" | awk '{print $1}' || true)
git bisect reset "$PIN" 2>&1 | tee -a "$LOG"
git checkout -q "$PIN"
SUBJ=$(git log -1 --format=%s "$FIRST" 2>/dev/null || true)
PR=$(sed -n 's/.*(#\([0-9][0-9]*\)).*/\1/p' <<<"$SUBJ" | head -1)
MECH=$(git show --stat --format= "$FIRST" 2>/dev/null | grep -E '^\s+.*\|' | grep -v corpus_outcomes | head -2 | tr '\n' '; ' || echo unknown)
{
  echo "first_bad=$FIRST"
  echo "pr=$PR"
  echo "subject=$SUBJ"
  echo "mechanism=$MECH"
} | tee -a "$LOG"
cd "$SCRIPT_REPO"
git checkout -q work/ledger-bisect-part-23
