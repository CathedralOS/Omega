#!/bin/bash
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx test -p compiler --test corpus_runner --no-run < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "$(dirname "$0")/bisect_test_abs.py" "$WITNESS" /tmp/bisect_one_record.txt
