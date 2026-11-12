#!/bin/bash
# Run corpus_runner for a comma-separated OMEGA_CORPUS_FIXTURE_FILTER list.
set -euo pipefail
ROOT="/agent/repos/omega"
FILTER="${1:?filter comma list}"
OUT="${2:?output records path}"
cd "$ROOT"
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* | head -1)
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_JOBS=1 OMEGA_CORPUS_FIXTURE_FILTER="$FILTER" OMEGA_CORPUS_RECORDS="$OUT"
nice -n 19 "$RUNNER" < /dev/null
wc -l "$OUT"
