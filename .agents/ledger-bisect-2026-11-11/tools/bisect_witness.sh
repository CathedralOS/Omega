#!/bin/bash
# git bisect run helper: 0 = PIN-expected pass, 1 = PIN breakage, 125 = skip / other
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
TOOLDIR="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

if ! git rev-parse HEAD^ >/dev/null 2>&1; then
  exit 125
fi
changed="$(git diff --name-only HEAD^ HEAD 2>/dev/null || true)"
if [[ -n "$changed" ]]; then
  non_ledger=0
  while IFS= read -r f; do
    [[ -z "$f" ]] && continue
    if [[ "$f" == tests/omega/corpus_outcomes.txt ]]; then
      continue
    fi
    if [[ "$f" == *.md ]]; then
      continue
    fi
    non_ledger=1
    break
  done <<<"$changed"
  if [[ "$non_ledger" -eq 0 ]]; then
    exit 125
  fi
fi

export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx test -p compiler --no-run corpus_runner < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
PYTHONPATH="$TOOLDIR" python3 "$TOOLDIR/bisect_test.py" /tmp/bisect_one_record.txt
