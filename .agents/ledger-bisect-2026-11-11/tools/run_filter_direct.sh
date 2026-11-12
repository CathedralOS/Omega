#!/bin/bash
# Run corpus_runner on a comma-separated fixture filter; write corpus records to OUT.
set -euo pipefail
FILTER="${1:?filter}"
OUT="${2:?output path}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || { echo "no corpus_runner" >&2; exit 1; }
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_JOBS=1 OMEGA_CORPUS_FIXTURE_FILTER="$FILTER" OMEGA_CORPUS_RECORDS="$OUT"
nice -n 19 "$RUNNER" < /dev/null
wc -l "$OUT"
