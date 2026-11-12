#!/bin/bash
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

# Skip ledger-only first-parent commits (cannot change compiler behaviour).
if git rev-parse --verify HEAD^1 >/dev/null 2>&1; then
  mapfile -t CHANGED < <(git diff --name-only HEAD^1 HEAD)
  if ((${#CHANGED[@]} > 0)); then
    only_ledger=1
    for f in "${CHANGED[@]}"; do
      if [[ "$f" != tests/omega/corpus_outcomes.txt && ! "$f" =~ \.md$ ]]; then
        only_ledger=0
        break
      fi
    done
    if ((only_ledger)); then
      exit 125
    fi
  fi
fi

export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
chmod -R u+w "$ROOT/target" 2>/dev/null || true
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx build -p compiler --bins --tests < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 /tmp/bisect_test.py "$WITNESS"
