#!/bin/bash
set -euo pipefail
WITNESS="${BISECT_WITNESS:?pass/borrows/runtime_mutable_byte_loan_read_exit}"
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
chmod -R u+w "$ROOT/target" 2>/dev/null || true

# Skip ledger-only first-parent commits.
if git rev-parse --verify HEAD^1 >/dev/null 2>&1; then
  mapfile -t touched < <(git diff --name-only HEAD^1 HEAD)
  if ((${#touched[@]} > 0)); then
    only_ledger=1
    for f in "${touched[@]}"; do
      case "$f" in
        tests/omega/corpus_outcomes.txt|tests/omega/corpus_native_*.txt) ;;
        *.md) ;;
        *) only_ledger=0; break ;;
      esac
    done
    if ((only_ledger)); then
      exit 125
    fi
  fi
fi

export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
export OMEGA_CORPUS_FIXTURE_SECS="${OMEGA_CORPUS_FIXTURE_SECS:-120}"
export PIN_FROZEN_LEDGER=/tmp/pin_corpus_outcomes.txt
export PIN_WITNESS_OBS=/tmp/pin_witness_observation.txt
export CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target"
nice -n 19 mbx test --locked --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | grep -v '\.d$' | head -1)
if [[ ! -x "$RUNNER" ]]; then
  RUNNER="$ROOT/target/debug/corpus_runner"
fi
if [[ ! -x "$RUNNER" ]]; then
  RUNNER=$(ls -t "$ROOT"/target/debug/deps/corpus_runner-* 2>/dev/null | head -1)
fi
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "${LEDGER_BISECT_PIN_FIXED:-/tmp/ledger_bisect_pin_fixed.py}"
