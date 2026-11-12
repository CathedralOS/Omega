#!/bin/bash
# Newest first-parent ancestor of PIN where witness matches fixed-PIN expectation.
set -euo pipefail
PIN="${BISECT_PIN:?}"
SCRIPT="${BISECT_WITNESS_SCRIPT:?}"
ROOT="${BISECT_REPO_ROOT:?}"
MAX_STEPS="${BISECT_FIND_GOOD_MAX:-1500}"
cd "$ROOT"
start_ref="$(git rev-parse HEAD)"
trap 'git checkout -fq "$start_ref" 2>/dev/null || git checkout -fq "$PIN"' EXIT

mapfile -t commits < <(git rev-list --first-parent "$PIN")
if ((${#commits[@]} < 2)); then
  echo "find_good_commit: not enough history" >&2
  exit 1
fi

run_test() {
  git checkout -fq "$1"
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
((hi > MAX_STEPS)) && hi=$MAX_STEPS
best=""

while ((lo <= hi)); do
  mid=$(((lo + hi) / 2))
  echo "find_good probe ${mid}/${hi} $(git log -1 --oneline "${commits[mid]}")" >&2
  ec="$(run_test "${commits[mid]}")"
  if [[ "$ec" == "0" ]]; then
    best="${commits[mid]}"
    hi=$((mid - 1))
  else
    lo=$((mid + 1))
  fi
done

if [[ -z "$best" ]]; then
  echo "find_good_commit: no good commit within ${MAX_STEPS} first-parent steps of PIN" >&2
  exit 1
fi
echo "find_good: ${best}" >&2
echo "$best"
