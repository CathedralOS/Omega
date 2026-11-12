#!/bin/bash
# Run corpus_runner with OMEGA_CORPUS_RECORDS for a comma-separated fixture filter.
set -euo pipefail
FILTER="${1:?filter}"
OUT="${2:?output path}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
RUNNER=$(find "$ROOT/target" -name 'corpus_runner-*' -type f ! -name '*.d' 2>/dev/null | head -1)
if [[ ! -x "$RUNNER" ]]; then
  CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx build -p omega < /dev/null
  CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null
  RUNNER=$(find "$ROOT/target" -name 'corpus_runner-*' -type f ! -name '*.d' 2>/dev/null | head -1)
fi
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_JOBS=1 OMEGA_CORPUS_FIXTURE_FILTER="$FILTER" OMEGA_CORPUS_RECORDS="$OUT"
nice -n 19 "$RUNNER" < /dev/null
wc -l "$OUT"
