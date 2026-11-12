#!/bin/bash
# Direct corpus_runner for batches that exceed xtask 1800s cap.
set -euo pipefail
BATCH_ID="${1:?batch id}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
FILTER="$(tr '\n' ',' <"$ROOT/wiki/drafts/audits/corpus/batches/batch_${BATCH_ID}.filter" | sed 's/,$//')"
OUT="$ROOT/wiki/drafts/audits/corpus/batch_${BATCH_ID}.txt"
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* | head -1)
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_JOBS=1 OMEGA_CORPUS_FIXTURE_FILTER="$FILTER" OMEGA_CORPUS_RECORDS="$OUT"
nice -n 19 "$RUNNER" < /dev/null
wc -l "$OUT"
