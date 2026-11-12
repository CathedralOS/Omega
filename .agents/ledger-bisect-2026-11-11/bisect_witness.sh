#!/bin/bash
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="${BISECT_ROOT:-/agent/repos/omega}"
cd "$ROOT"

if git rev-parse --verify HEAD^1 >/dev/null 2>&1; then
  changed=$(git diff --name-only HEAD^1 HEAD || true)
  if [[ -n "$changed" ]]; then
    only_ledger=1
    while IFS= read -r f; do
      [[ -z "$f" ]] && continue
      case "$f" in
        tests/omega/corpus_outcomes.txt) ;;
        *.md) ;;
        *) only_ledger=0; break ;;
      esac
    done <<<"$changed"
    if [[ "$only_ledger" -eq 1 ]]; then
      exit 125
    fi
  fi
fi

export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
export PIN_LEDGER="${PIN_LEDGER:-/tmp/pin_corpus_outcomes.txt}"
export PIN_BISECT_EXPECT="${PIN_BISECT_EXPECT:-/tmp/pin_bisect_expect.json}"
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
mbx test --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "$ROOT/.agents/ledger-bisect-2026-11-11/bisect_test.py" "$WITNESS" /tmp/bisect_one_record.txt
