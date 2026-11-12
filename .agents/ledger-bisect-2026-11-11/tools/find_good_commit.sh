#!/bin/bash
# Newest first-parent ancestor of PIN where witness matches fixed-PIN good (binary search).
set -euo pipefail
PIN="${BISECT_PIN:?}"
SCRIPT="${BISECT_WITNESS_SCRIPT:?}"
ROOT="${BISECT_REPO_ROOT:?}"
cd "$ROOT"
start_ref="$(git rev-parse HEAD)"
trap 'git checkout -fq "$start_ref" 2>/dev/null || git checkout -fq "$PIN"' EXIT

mapfile -t commits < <(git rev-list --first-parent "$PIN")
if ((${#commits[@]} < 2)); then
  echo "find_good_commit: not enough history" >&2
  exit 1
fi

run_test() {
  local commit=$1
  git checkout -fq "$commit"
  set +e
  "$SCRIPT"
  local ec=$?
  set -e
  echo "$ec"
}

echo "find_good: verifying PIN bad" >&2
ec_pin="$(run_test "${commits[0]}")"
if [[ "$ec_pin" != "1" ]]; then
  echo "find_good: PIN witness must exit 1, got $ec_pin" >&2
  exit 1
fi

lo=1
hi=$((${#commits[@]} - 1))
answer=""

while ((lo <= hi)); do
  mid=$(((lo + hi) / 2))
  echo "find_good probe mid=$mid/$((${#commits[@]} - 1)) $(git log -1 --oneline "${commits[mid]}")" >&2
  ec="$(run_test "${commits[mid]}")"
  if [[ "$ec" == "0" ]]; then
    answer="${commits[mid]}"
    hi=$((mid - 1))
  elif [[ "$ec" == "1" ]]; then
    lo=$((mid + 1))
  else
    # skip build/ambiguous: treat as inconclusive toward older
    lo=$((mid + 1))
  fi
done

if [[ -z "$answer" ]]; then
  echo "find_good_commit: no good commit in first-parent chain" >&2
  exit 1
fi
echo "find_good: ${answer}" >&2
echo "$answer"
