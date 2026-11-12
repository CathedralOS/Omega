#!/bin/bash
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="${BISECT_ROOT:-$(git -C "$(dirname "$0")" rev-parse --show-toplevel)}"
TOOL="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

# Skip ledger-only first-parent commits (cannot change compiler behaviour).
if parent=$(git rev-parse --verify HEAD^@ 2>/dev/null); then
  mapfile -t touched < <(git diff --name-only "$parent" HEAD)
  if ((${#touched[@]} > 0)); then
    touches_compiler=0
    for f in "${touched[@]}"; do
      [[ "$f" == "tests/omega/corpus_outcomes.txt" ]] && continue
      [[ "$f" == *.md ]] && continue
      touches_compiler=1
      break
    done
    (( touches_compiler )) || exit 125
  fi
fi

export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
export LEDGER_PIN_FILE="${LEDGER_PIN_FILE:-/tmp/ledger-bisect-pin/corpus_outcomes.txt}"
COMMIT=$(git rev-parse HEAD)
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_INCREMENTAL=0
CARGO_TARGET_DIR="$ROOT/target/bisect/${COMMIT}"
export CARGO_TARGET_DIR
mkdir -p "$CARGO_TARGET_DIR"
nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$CARGO_TARGET_DIR"/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "$TOOL/bisect_test.py" "$WITNESS" /tmp/bisect_one_record.txt
