#!/bin/bash
# Binary search on the first-parent chain for the newest commit where the witness is good.
set -euo pipefail
PIN="${PIN_COMMIT:?51e8c45c793007f211d0d9baee7de16767172444}"
WITNESS="${BISECT_WITNESS:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
export PIN_COMMIT="$PIN"
bash "$(dirname "$0")/setup_pin_bisect.sh" "$WITNESS"

mapfile -t COMMITS < <(git rev-list --first-parent "$PIN")
n=${#COMMITS[@]}

test_commit() {
  git checkout -q "$2"
  set +e
  bash "$(dirname "$0")/bisect_witness.sh"
  local code=$?
  set -e
  printf '%s' "$code"
}

pin_code="$(test_commit x "${COMMITS[0]}")"
[[ "$pin_code" == "1" ]] || {
  echo "find_good_commit: expected PIN bad (1), got $pin_code" >&2
  exit 1
}

lo=0
hi=$((n - 1))
# Exponential backoff from PIN until good or inconclusive limit.
probe=$lo
step=1
while ((probe < n)); do
  code="$(test_commit x "${COMMITS[$probe]}")"
  if [[ "$code" == "0" ]]; then
  hi=$probe
  break
  fi
  lo=$probe
  step=$((step * 2))
  probe=$((lo + step))
done
if [[ "$code" != "0" ]]; then
  echo "find_good_commit: no good within $probe steps" >&2
  exit 1
fi

while ((lo + 1 < hi)); do
  mid=$(( (lo + hi) / 2 ))
  code="$(test_commit x "${COMMITS[$mid]}")"
  if [[ "$code" == "0" ]]; then
    hi=$mid
  else
    lo=$mid
  fi
done

echo "${COMMITS[$hi]}"
