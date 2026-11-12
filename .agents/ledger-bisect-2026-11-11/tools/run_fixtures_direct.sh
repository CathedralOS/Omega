#!/bin/bash
# Run corpus_runner with a comma-separated filter list file (one fixture per line).
set -euo pipefail
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
LIST="${1:?fixture list file}"
OUT="${2:?output records file}"
cd "$ROOT"
FILTER="$(tr '\n' ',' <"$LIST" | sed 's/,$//')"
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || { echo "no corpus_runner; build omega first" >&2; exit 1; }
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_JOBS=1 OMEGA_CORPUS_FIXTURE_FILTER="$FILTER" OMEGA_CORPUS_RECORDS="$OUT"
rm -f "$OUT"
nice -n 19 "$RUNNER" < /dev/null
wc -l "$OUT"
