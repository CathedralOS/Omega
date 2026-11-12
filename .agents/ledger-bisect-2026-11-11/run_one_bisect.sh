#!/bin/bash
set -euo pipefail
WITNESS="${1:?witness}"
CLUSTER_TAG="${2:?tag}"
PIN=51e8c45c793007f211d0d9baee7de16767172444
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
SCRIPT_DIR="$ROOT/.agents/ledger-bisect-2026-11-11"
LOG="/tmp/bisect_${CLUSTER_TAG}.log"
: >"$LOG"

git checkout -q work/ledger-bisect-part-20
export BISECT_WITNESS="$WITNESS"
bash "$SCRIPT_DIR/prepare_witness.sh" 2>&1 | tee -a "$LOG"
cp "$SCRIPT_DIR/bisect_test.py" /tmp/bisect_tools_test.py
cp "$SCRIPT_DIR/bisect_witness.sh" /tmp/bisect_tools_witness.sh
chmod +x /tmp/bisect_tools_witness.sh

sync_scripts() {
  cp /tmp/bisect_tools_test.py /tmp/bisect_test.py
  cp /tmp/bisect_tools_witness.sh /tmp/bisect_witness.sh
  chmod +x /tmp/bisect_witness.sh
}
sync_scripts

test_ref() {
  local ref=$1
  git checkout -q "$ref"
  sync_scripts
  /tmp/bisect_witness.sh || return $?
}

verify() {
  local ref=$1 label=$2 expect=$3
  local i code codes=()
  for i in 1 2 3; do
    set +e
    test_ref "$ref"
    code=$?
    set -e
    codes+=("$code")
  done
  printf '%s %s expect=%s got: %s\n' "$label" "$ref" "$expect" "${codes[*]}" | tee -a "$LOG"
  local ok=0
  for code in "${codes[@]}"; do [[ "$code" == "$expect" ]] && ok=$((ok + 1)); done
  [[ "$ok" -ge 2 ]] || { echo "SANITY FAIL $label" | tee -a "$LOG"; return 1; }
  if [[ "$(printf '%s\n' "${codes[@]}" | sort -u | wc -l)" -gt 1 ]]; then
    echo "FLAKY $label: ${codes[*]}" | tee -a "$LOG"
  fi
}

mapfile -t COMMITS < <(git rev-list --first-parent "$PIN")
last_idx=$((${#COMMITS[@]} - 1))

idx_for() {
  local ref=$1 i
  for i in "${!COMMITS[@]}"; do
    [[ "${COMMITS[$i]}" == "$ref" ]] && { echo "$i"; return 0; }
  done
  return 1
}

INTRO=$(git log -S "$WITNESS" --format=%H --first-parent -n 1 -- tests/omega/corpus_outcomes.txt)
intro_idx=$(idx_for "$INTRO")
[[ -n "$intro_idx" ]] || intro_idx=$last_idx
echo "intro=$INTRO idx=$intro_idx" | tee -a "$LOG"

set +e
test_ref "$INTRO"
intro_rc=$?
set -e
[[ "$intro_rc" -eq 0 ]] || { echo "intro must be good (rc=$intro_rc)" | tee -a "$LOG"; exit 1; }

set +e
test_ref "${COMMITS[1]}"
lo_rc=$?
set -e
[[ "$lo_rc" -eq 1 ]] || { echo "PIN^1 must be bad (rc=$lo_rc)" | tee -a "$LOG"; exit 1; }

search_lo=1
search_hi=$intro_idx
while [[ "$search_lo" -lt "$search_hi" ]]; do
  mid=$(( (search_lo + search_hi) / 2 ))
  set +e
  test_ref "${COMMITS[$mid]}"
  rc=$?
  set -e
  echo "find_good idx=$mid ${COMMITS[$mid]:0:12} rc=$rc" | tee -a "$LOG"
  if [[ "$rc" -eq 0 ]]; then
    search_hi=$mid
  else
    search_lo=$((mid + 1))
  fi
done
GOOD="${COMMITS[$search_hi]}"
echo "GOOD=$GOOD" | tee -a "$LOG"

verify "$PIN" pin 1
verify "$GOOD" good 0

git bisect reset 2>/dev/null || true
git bisect start --first-parent "$PIN" "$GOOD" 2>&1 | tee -a "$LOG"
git bisect run /tmp/bisect_witness.sh 2>&1 | tee -a "$LOG"
FIRST=$(git rev-parse HEAD)
SUBJ=$(git log -1 --format=%s "$FIRST")
PR=$(echo "$SUBJ" | grep -oE '#[0-9]+' | head -1 | tr -d '#')
git bisect reset "$PIN" 2>/dev/null || git bisect reset
git checkout -q work/ledger-bisect-part-20
echo "FIRST_BAD=$FIRST" | tee -a "$LOG"
echo "SUBJECT=$SUBJ" | tee -a "$LOG"
echo "PR=${PR:-none}" | tee -a "$LOG"
