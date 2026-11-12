#!/bin/bash
# 0 = good (fixed PIN ledger expectation), 1 = bad (PIN break reproduced), 125 = skip
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="${BISECT_REPO_ROOT:-$(git rev-parse --show-toplevel)}"
TOOLS="$(dirname "$0")"
cd "$ROOT"
chmod -R u+w "$ROOT/target" 2>/dev/null || true

if git rev-parse -q HEAD^1 >/dev/null 2>&1; then
  mapfile -t changed < <(git diff --name-only HEAD^1 HEAD)
  if ((${#changed[@]} > 0)); then
    ledger_only=1
    for f in "${changed[@]}"; do
      [[ "$f" == tests/omega/corpus_outcomes.txt ]] && continue
      [[ "$f" == *.md ]] && continue
      ledger_only=0
      break
    done
    ((ledger_only)) && exit 125
  fi
fi

export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
SHA="$(git rev-parse --short HEAD)"
export CARGO_TARGET_DIR="$ROOT/target/bisect-$SHA"
export CARGO_PROFILE_DEV_OPT_LEVEL=1
nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$CARGO_TARGET_DIR"/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
export BISECT_WITNESS_SPEC
python3 "$TOOLS/bisect_test.py" "$WITNESS" /tmp/bisect_one_record.txt
