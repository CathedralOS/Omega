#!/bin/bash
# Newest first-parent ancestor of PIN where witness meets PIN expected good (rc=0).
set -euo pipefail
WITNESS="${1:?witness}"
PIN="${2:-51e8c45c793007f211d0d9baee7de16767172444}"
MAX="${3:-1200}"
STEP="${4:-48}"
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
TOOLS="$(dirname "$0")"
cd "$ROOT"
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4

test_commit() {
  local c=$1
  git checkout -q "$c"
  TD="${CARGO_TARGET_DIR:-$ROOT/target-pin}"
  CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$TD" mbx build -p omega < /dev/null >/dev/null 2>&1 || return 2
  local runner
  runner=$(bash "$TOOLS/get_corpus_runner.sh")
  [[ -x "$runner" ]] || return 2
  export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
  export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/walk_one.txt
  rm -f /tmp/walk_one.txt
  nice -n 19 "$runner" < /dev/null >/dev/null 2>&1 || true
  python3 "$TOOLS/bisect_test_pin.py" "$WITNESS" /tmp/walk_one.txt
}

mapfile -t commits < <(git rev-list --first-parent "$PIN" | head -n "$MAX")
n=${#commits[@]}
found=-1
for ((idx=STEP; idx<n; idx+=STEP)); do
  rc=$(test_commit "${commits[$idx]}" || echo 2)
  if [[ "$rc" -eq 0 ]]; then
    found=$idx
    break
  fi
done
if [[ "$found" -lt 0 ]]; then
  rc=$(test_commit "${commits[$((n-1))]}" || echo 2)
  if [[ "$rc" -eq 0 ]]; then
    found=$((n-1))
  fi
fi
if [[ "$found" -lt 0 ]]; then
  echo NO_GOOD >&2
  git checkout -q "$PIN"
  exit 1
fi
# Refine upward to newest good between found-STEP and found
start=$(( found - STEP ))
(( start < 1 )) && start=1
for ((idx=found; idx>=start; idx--)); do
  rc=$(test_commit "${commits[$idx]}" || echo 2)
  if [[ "$rc" -eq 0 ]]; then
    found=$idx
    break
  fi
done
for ((idx=start; idx<=found; idx++)); do
  rc=$(test_commit "${commits[$idx]}" || echo 2)
  if [[ "$rc" -eq 0 ]]; then
    echo "${commits[$idx]}"
    git checkout -q "$PIN"
    exit 0
  fi
done
echo NO_GOOD >&2
git checkout -q "$PIN"
exit 1
