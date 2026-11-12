#!/bin/bash
# Run witness N times at a commit; print status lines and ledger-test rc (0=good 1=bad 125=skip).
set -euo pipefail
COMMIT="${1:?commit}"
WITNESS="${2:?witness}"
N="${3:-1}"
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"
git checkout -q "$COMMIT"
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" mbx build -p omega < /dev/null >/dev/null 2>&1
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" mbx test -p compiler --test corpus_runner --no-run < /dev/null >/dev/null 2>&1
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* | head -1)
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
for i in $(seq 1 "$N"); do
  export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS="/tmp/verify_${i}.txt"
  rm -f "/tmp/verify_${i}.txt"
  nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
  python3 "$(dirname "$0")/bisect_test.py" "$WITNESS" "/tmp/verify_${i}.txt" 2>/dev/null || true
  rc=$?
  status=$(head -1 "/tmp/verify_${i}.txt" 2>/dev/null || echo "missing")
  echo "run=$i rc=$rc $status"
done
