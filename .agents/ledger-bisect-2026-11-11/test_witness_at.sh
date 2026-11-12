#!/bin/bash
set -euo pipefail
COMMIT="${1:?}"
WITNESS="${2:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
git checkout -q "$COMMIT"
export BISECT_WITNESS="$WITNESS"
DIR="$(dirname "$0")"
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" mbx build -p omega < /dev/null >/dev/null 2>&1
mbx test -p compiler corpus_runner --no-run < /dev/null >/dev/null 2>&1
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/test_one.txt
rm -f /tmp/test_one.txt
"$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "$DIR/bisect_test.py" "$WITNESS" /tmp/test_one.txt
