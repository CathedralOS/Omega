#!/bin/bash
# Measure one witness at a fixed commit with PIN-fixed ledger (verification).
set -euo pipefail
COMMIT="${1:?commit}"
WITNESS="${2:?witness}"
ROOT="$(git rev-parse --show-toplevel)"
TOOLS="${VERIFY_TOOLS:?VERIFY_TOOLS must point at /tmp copy}"
PIN="${PIN_COMMIT:-51e8c45c793007f211d0d9baee7de16767172444}"
OBS="${PIN_OBSERVED:-/tmp/ledger_bisect_pin_observed.txt}"
LEDGER="${PIN_LEDGER:-/tmp/ledger_bisect_corpus_outcomes.txt}"
SHORT="$(echo "$COMMIT" | cut -c1-12)"
export CARGO_TARGET_DIR="$ROOT/target/verify-$SHORT"
cd "$ROOT"
git checkout -f "$COMMIT" >/dev/null
PIN_LEDGER="$LEDGER" bash "$TOOLS/prepare_bisect_witness.sh" "$WITNESS" "$PIN" "$OBS" >/dev/null
if [[ "${VERIFY_NO_BUILD:-}" != 1 ]]; then
  export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4 VERIFY_STRICT_DIAG=1
  export CARGO_PROFILE_DEV_OPT_LEVEL=1
  nice -n 19 mbx build -p omega < /dev/null >/dev/null || exit 125
  nice -n 19 mbx test --locked --no-run -p compiler --test corpus_runner < /dev/null >/dev/null || exit 125
fi
RUNNER=$(ls -t "$CARGO_TARGET_DIR/debug/build/compiler/"*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/verify_one_record.txt
rm -f /tmp/verify_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "$TOOLS/bisect_test.py" "$WITNESS" /tmp/verify_one_record.txt
