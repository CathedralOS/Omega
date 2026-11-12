#!/bin/bash
set -euo pipefail
FILTER="${1:?}"
OUT="${2:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || { echo "missing corpus_runner" >&2; exit 2; }
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_JOBS=1 OMEGA_CORPUS_FIXTURE_FILTER="$FILTER" OMEGA_CORPUS_RECORDS="$OUT"
rm -f "$OUT"
nice -n 19 "$RUNNER" < /dev/null
