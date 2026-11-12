#!/bin/bash
set -euo pipefail
ROOT="$(git -C "$(dirname "$0")/../../.." rev-parse --show-toplevel)"
cd "$ROOT"
LIST="${1:?fixture list file}"
OUT="${2:?output records}"
FILTER="$(tr '\n' ',' <"$LIST" | sed 's/,$//')"
export CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target"
nice -n 19 mbx build -p omega < /dev/null
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || { echo "no corpus_runner"; exit 1; }
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_JOBS=1 OMEGA_CORPUS_FIXTURE_FILTER="$FILTER" OMEGA_CORPUS_RECORDS="$OUT"
nice -n 19 "$RUNNER" < /dev/null
wc -l "$OUT"
