#!/bin/bash
set -euo pipefail
WITNESS="${1:?}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
DIR="$(dirname "$0")"
if [[ -d /tmp/ledger-bisect-tools ]]; then
  DIR=/tmp/ledger-bisect-tools
fi
LOG="${3:-$ROOT/.agents/ledger-bisect-2026-11-11/work/find_good_${WITNESS//\//_}.log}"
cd "$ROOT"
export BISECT_WITNESS="$WITNESS"
"$DIR/prepare_bisect_pin.sh" "$WITNESS" "$PIN" "$ROOT/.agents/ledger-bisect-2026-11-11/work/pin_observed.txt"
mapfile -t COMMITS < <(git rev-list --first-parent "$PIN")
BRANCH="$(git symbolic-ref -q --short HEAD || true)"
: >"$LOG"
test_code() {
  local commit="$1"
  git checkout --quiet "$commit" || { echo 125; return; }
  mapfile -t CHANGED < <(git diff --name-only "${commit}^1" "$commit" 2>/dev/null || true)
  if ((${#CHANGED[@]} > 0)); then
    local ledger_only=1 f
    for f in "${CHANGED[@]}"; do
      case "$f" in tests/omega/corpus_outcomes.txt|*.md) ;; *) ledger_only=0; break ;; esac
    done
    if ((ledger_only)); then
      echo 125
      return
    fi
  fi
  "$DIR/bisect_witness.sh" >/dev/null 2>&1; echo $?
}
lo=0
code_lo=$(test_code "${COMMITS[$lo]}")
echo "pin index0 => ${code_lo}" | tee -a "$LOG"
if [[ "$code_lo" != "1" ]]; then
  [[ -n "$BRANCH" ]] && git checkout --quiet "$BRANCH" || git checkout --quiet "$PIN"
  echo "PIN_NOT_BAD code=${code_lo}" | tee -a "$LOG"
  exit 1
fi
hi=-1
for i in "${!COMMITS[@]}"; do
  if [[ "${COMMITS[$i]}" == 22f12808f94* ]]; then
    code=$(test_code "${COMMITS[$i]}")
    echo "anchor 22f12808 index=$i => $code" | tee -a "$LOG"
    if [[ "$code" == "0" ]]; then
      hi=$i
    fi
    break
  fi
done
if ((hi < 0)); then
  # Exponential probe fallback.
  step=1
  max=${#COMMITS[@]}
  while ((step < max)); do
    code=$(test_code "${COMMITS[$step]}")
    echo "probe index=$step => $code $(git log -1 --oneline "${COMMITS[$step]}")" | tee -a "$LOG"
    if [[ "$code" == "0" ]]; then
      hi=$step
      break
    fi
    step=$((step * 2))
  done
fi
if ((hi < 0)); then
  [[ -n "$BRANCH" ]] && git checkout --quiet "$BRANCH" || git checkout --quiet "$PIN"
  echo "NO_GOOD_ANCHOR" | tee -a "$LOG"
  exit 1
fi
while ((lo + 1 < hi)); do
  mid=$(( (lo + hi) / 2 ))
  code=$(test_code "${COMMITS[$mid]}")
  echo "mid index=$mid => $code $(git log -1 --oneline "${COMMITS[$mid]}")" | tee -a "$LOG"
  if [[ "$code" == "0" ]]; then
    hi=$mid
  elif [[ "$code" == "1" ]]; then
    lo=$mid
  else
    # skip 125: move toward older commits
    lo=$mid
  fi
done
GOOD="${COMMITS[$hi]}"
echo "GOOD=$GOOD $(git log -1 --oneline "$GOOD")" | tee -a "$LOG"
[[ -n "$BRANCH" ]] && git checkout --quiet "$BRANCH" || git checkout --quiet "$PIN"
echo "$GOOD"
