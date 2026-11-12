#!/bin/bash
# Nearest first-parent ancestor of PIN where witness meets fixed-PIN ledger expectation.
set -euo pipefail
PIN="${1:?PIN}"
WITNESS="${2:?witness}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
export BISECT_WITNESS="$WITNESS"
export BISECT_WITNESS_PIN="${BISECT_WITNESS_PIN:?}"
SCRIPT="$ROOT/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
SLUG="${WITNESS//\//_}"
LOG="$ROOT/.agents/ledger-bisect-2026-11-11/find-good-${SLUG}.log"
: >"$LOG"

test_at() {
  local c="$1"
  git -c advice.detachedHead=false checkout -f "$c" >/dev/null 2>&1
  local code=0
  bash "$SCRIPT" >>"$LOG" 2>&1 || code=$?
  echo "$code"
}

rev_at_depth() {
  local depth="$1"
  git rev-parse "${PIN}~${depth}" 2>/dev/null || true
}

git checkout -f work/ledger-bisect-part-27 2>/dev/null || git checkout -f "$PIN"
pin_code=$(test_at "$PIN")
echo "pin=$pin_code" >>"$LOG"
if [[ "$pin_code" == "0" ]]; then
  echo "$PIN"
  exit 0
fi
[[ "$pin_code" == "1" ]] || { echo "PIN exit $pin_code" >&2; exit 1; }

depth=1
last_bad_depth=0
good_depth=""
while [[ $depth -le 16384 ]]; do
  c=$(rev_at_depth "$depth") || break
  code=$(test_at "$c")
  echo "depth=$depth code=$code commit=${c:0:12}" >>"$LOG"
  if [[ "$code" == "0" ]]; then
    good_depth="$depth"
  break
  fi
  if [[ "$code" == "1" ]]; then
    last_bad_depth="$depth"
  fi
  depth=$((depth * 2))
done
if [[ -n "$good_depth" && "$last_bad_depth" -eq 0 ]]; then
  last_bad_depth=$((good_depth / 2))
fi

if [[ -z "$good_depth" ]]; then
  echo "no good commit within 16384 first-parent steps of PIN" >&2
  git checkout -f work/ledger-bisect-part-27 2>/dev/null || git checkout -f "$PIN"
  exit 1
fi

lo=$((good_depth / 2))
hi="$good_depth"
if [[ "$lo" -lt 1 ]]; then
  lo=1
fi
best_depth="$good_depth"
while [[ $lo -lt $hi ]]; do
  mid=$(( (lo + hi) / 2 ))
  c=$(rev_at_depth "$mid") || break
  code=$(test_at "$mid")
  echo "binary depth=$mid code=$code" >>"$LOG"
  if [[ "$code" == "0" ]]; then
    best_depth="$mid"
    hi="$mid"
  elif [[ "$code" == "1" ]]; then
    lo=$((mid + 1))
  else
    lo=$((mid + 1))
  fi
done

GOOD=$(rev_at_depth "$best_depth")
git checkout -f work/ledger-bisect-part-27 2>/dev/null || git checkout -f "$PIN"
echo "$GOOD"
