#!/bin/bash
# Pin-fixed observation bisect via binary search (avoids git bisect skip stalls).
set -euo pipefail
WITNESS="${1:?witness}"
CLUSTER_TAG="${2:?tag}"
PIN=51e8c45c793007f211d0d9baee7de16767172444
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
SCRIPT_DIR="$ROOT/.agents/ledger-bisect-2026-11-11"
TOOLKIT=/tmp/bisect_toolkit
LOG="/tmp/bisect_${CLUSTER_TAG}.log"
: >"$LOG"

git checkout -q work/ledger-bisect-part-20
mkdir -p "$TOOLKIT"
cp "$SCRIPT_DIR/prepare_witness.py" "$SCRIPT_DIR/prepare_witness.sh" \
  "$SCRIPT_DIR/bisect_test.py" "$SCRIPT_DIR/bisect_witness.sh" "$TOOLKIT/"
chmod +x "$TOOLKIT"/*.sh

export BISECT_WITNESS="$WITNESS"
export BISECT_PREPARE_PY="$TOOLKIT/prepare_witness.py"
cp "$TOOLKIT/prepare_witness.sh" /tmp/bisect_prepare_witness.sh
bash /tmp/bisect_prepare_witness.sh 2>&1 | tee -a "$LOG"
cp "$TOOLKIT/bisect_test.py" /tmp/bisect_tools_test.py
cp "$TOOLKIT/bisect_witness.sh" /tmp/bisect_tools_witness.sh
chmod +x /tmp/bisect_tools_witness.sh

test_ref() {
  git checkout -f -q "$1"
  cp /tmp/bisect_tools_test.py /tmp/bisect_test.py
  cp /tmp/bisect_tools_witness.sh /tmp/bisect_witness.sh
  chmod +x /tmp/bisect_witness.sh
  /tmp/bisect_witness.sh
  echo $?
}

mapfile -t COMMITS < <(git rev-list --first-parent "$PIN")
last_idx=$((${#COMMITS[@]} - 1))

pin1_rc=$(test_ref "${COMMITS[1]}")
echo "PIN^1 ${COMMITS[1]:0:12} rc=$pin1_rc" | tee -a "$LOG"
[[ "$pin1_rc" -eq 1 ]] || { echo "PIN^1 not bad" | tee -a "$LOG"; exit 1; }

if [[ -n "${KNOWN_GOOD:-}" ]]; then
  GOOD="$KNOWN_GOOD"
  echo "KNOWN_GOOD=$GOOD" | tee -a "$LOG"
else
  anchor=0
  for probe in 34000 30000 25000 20000 15000 12000 10000 8000 6000 4365 3000 2000 1500 1000; do
    [[ "$probe" -gt "$last_idx" ]] && continue
    rc=$(test_ref "${COMMITS[$probe]}")
    echo "anchor_probe idx=$probe rc=$rc" | tee -a "$LOG"
    if [[ "$rc" -eq 0 ]]; then
      anchor=$probe
      break
    fi
  done
  if [[ "$anchor" -gt 0 ]]; then
    lo=1
    hi=$anchor
    while [[ "$lo" -lt "$hi" ]]; do
      mid=$(( (lo + hi) / 2 ))
      rc=$(test_ref "${COMMITS[$mid]}")
      echo "find_good idx=$mid rc=$rc" | tee -a "$LOG"
      if [[ "$rc" -eq 0 ]]; then
        hi=$mid
      else
        lo=$((mid + 1))
      fi
    done
    GOOD="${COMMITS[$hi]}"
    good_idx=$hi
    echo "GOOD=$GOOD idx=$good_idx" | tee -a "$LOG"
  else
    echo "no_rc0_anchor; searching last idx where witness is not PIN-break (rc!=1)" | tee -a "$LOG"
    lo=0
    hi=$last_idx
    best=-1
    while [[ "$lo" -le "$hi" ]]; do
      mid=$(( (lo + hi) / 2 ))
      rc=$(test_ref "${COMMITS[$mid]}")
      echo "find_not_pin_break idx=$mid rc=$rc" | tee -a "$LOG"
      if [[ "$rc" -ne 1 ]]; then
        best=$mid
        lo=$((mid + 1))
      else
        hi=$((mid - 1))
      fi
    done
    if [[ "$best" -lt 0 ]]; then
      echo "ALL_PIN_BREAK_ON_CHAIN" | tee -a "$LOG"
      FIRST="${COMMITS[1]}"
      SUBJ=$(git log -1 --format=%s "$FIRST")
      PR=$(echo "$SUBJ" | grep -oE '#[0-9]+' | head -1 | tr -d '#')
      git checkout -q work/ledger-bisect-part-20
      echo "FIRST_BAD=$FIRST" | tee -a "$LOG"
      echo "SUBJECT=$SUBJ" | tee -a "$LOG"
      echo "PR=${PR:-none}" | tee -a "$LOG"
      echo "STATUS=propagated_all_history_pin_break" | tee -a "$LOG"
      exit 0
    fi
    if [[ "$best" -eq 0 ]]; then
      FIRST="${COMMITS[1]}"
    else
      FIRST="${COMMITS[$((best - 1))]}"
    fi
    SUBJ=$(git log -1 --format=%s "$FIRST")
    PR=$(echo "$SUBJ" | grep -oE '#[0-9]+' | head -1 | tr -d '#')
    git checkout -q work/ledger-bisect-part-20
    echo "FIRST_BAD=$FIRST" | tee -a "$LOG"
    echo "SUBJECT=$SUBJ" | tee -a "$LOG"
    echo "PR=${PR:-none}" | tee -a "$LOG"
    echo "GOOD=${COMMITS[$best]}" | tee -a "$LOG"
    echo "STATUS=no_checked_ok_used_not_pin_break_transition" | tee -a "$LOG"
    exit 0
  fi
fi

if [[ -n "${KNOWN_GOOD:-}" ]]; then
  good_idx=0
  for i in "${!COMMITS[@]}"; do
    [[ "${COMMITS[$i]}" == "$GOOD" ]] && good_idx=$i
  done
  [[ "$good_idx" -gt 0 ]] || { echo "KNOWN_GOOD not on first-parent chain" | tee -a "$LOG"; exit 1; }
fi

pin_rc=$(test_ref "$PIN")
[[ "$pin_rc" -eq 1 ]] || { echo "PIN not bad rc=$pin_rc" | tee -a "$LOG"; exit 1; }
good_rc=$(test_ref "$GOOD")
[[ "$good_rc" -eq 0 ]] || { echo "GOOD not good rc=$good_rc" | tee -a "$LOG"; exit 1; }

lo=1
hi=$good_idx
while [[ "$lo" -lt "$hi" ]]; do
  mid=$(( (lo + hi) / 2 ))
  rc=$(test_ref "${COMMITS[$mid]}")
  echo "find_bad idx=$mid rc=$rc" | tee -a "$LOG"
  if [[ "$rc" -eq 1 ]]; then
    hi=$mid
  else
    lo=$((mid + 1))
  fi
done
FIRST="${COMMITS[$hi]}"
first_idx=$hi
first_rc=$(test_ref "$FIRST")
is_noncompiler_commit() {
  local c="$1"
  mapfile -t changed < <(git diff-tree --no-commit-id --name-only -r "$c")
  local only_noncompiler=1
  for f in "${changed[@]}"; do
    case "$f" in
      tests/omega/corpus_outcomes.txt|tests/*|*.md|.agents/*|tools/*|wiki/*|AGENTS.md) ;;
      omega-rust/*|source/*|samples/*) only_noncompiler=0; break ;;
      *) only_noncompiler=0; break ;;
    esac
  done
  [[ "$only_noncompiler" -eq 1 ]]
}
while [[ "$first_idx" -gt 1 ]] && is_noncompiler_commit "$FIRST"; do
  echo "skip_noncompiler_first_bad idx=$first_idx $FIRST" | tee -a "$LOG"
  first_idx=$((first_idx + 1))
  FIRST="${COMMITS[$first_idx]}"
  first_rc=$(test_ref "$FIRST")
done
if [[ "$first_rc" -ne 1 ]]; then
  echo "refine_bad linear from PIN^1 (first_rc=$first_rc)" | tee -a "$LOG"
  FIRST=""
  scan=$good_idx
  [[ "$scan" -gt 250 ]] && scan=250
  for ((idx = 1; idx <= scan; idx++)); do
    rc_new=$(test_ref "${COMMITS[$idx]}")
    rc_old=$(test_ref "${COMMITS[$((idx + 1))]}")
    echo "scan_bad idx=$idx new=$rc_new old=$rc_old" | tee -a "$LOG"
    if [[ "$rc_new" -eq 1 && "$rc_old" -eq 0 ]]; then
      FIRST="${COMMITS[$idx]}"
      break
    fi
  done
  [[ -n "$FIRST" ]] || { echo "no first-bad found" | tee -a "$LOG"; exit 1; }
fi
SUBJ=$(git log -1 --format=%s "$FIRST")
PR=$(echo "$SUBJ" | grep -oE '#[0-9]+' | head -1 | tr -d '#')
only_ledger=$(git diff-tree --no-commit-id --name-only -r "$FIRST" | grep -vE '^tests/omega/corpus_outcomes\.txt$|\.md$' | head -1 || true)
if [[ -z "$only_ledger" ]]; then
  echo "FIRST_BAD_IS_LEDGER_ONLY=$FIRST" | tee -a "$LOG"
fi
git checkout -q work/ledger-bisect-part-20
echo "FIRST_BAD=$FIRST" | tee -a "$LOG"
echo "SUBJECT=$SUBJ" | tee -a "$LOG"
echo "PR=${PR:-none}" | tee -a "$LOG"
