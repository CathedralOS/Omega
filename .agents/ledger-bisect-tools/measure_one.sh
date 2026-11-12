#!/bin/bash
set -euo pipefail
WITNESS="${1:?witness}"
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx test --locked --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* | head -1)
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS="/tmp/measure_${WITNESS//\//_}.txt"
rm -f "$OMEGA_CORPUS_RECORDS"
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 /tmp/ledger-bisect-tools/bisect_test.py "$WITNESS" "$OMEGA_CORPUS_RECORDS"
rc=$?
cat "$OMEGA_CORPUS_RECORDS"
exit $rc
