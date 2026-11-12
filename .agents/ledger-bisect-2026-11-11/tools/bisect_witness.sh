#!/bin/bash
# git bisect run helper: 0=good (expected PIN outcome), 1=bad (PIN break), 125=skip
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="${LEDGER_BISECT_REPO:?set LEDGER_BISECT_REPO to omega checkout}"
cd "$ROOT"

# Skip ledger-only first-parent commits (cannot change compiler behavior).
changed=$(git diff --name-only HEAD^1 HEAD 2>/dev/null || true)
if [[ -n "$changed" ]]; then
  non_doc=0
  while IFS= read -r f; do
    [[ -z "$f" ]] && continue
    if [[ "$f" == tests/omega/corpus_outcomes.txt ]]; then
      continue
    fi
    if [[ "$f" == *.md ]]; then
      continue
    fi
    non_doc=1
    break
  done <<<"$changed"
  if [[ "$non_doc" -eq 0 ]]; then
    exit 125
  fi
fi

export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
COMMIT_SHA="$(git rev-parse HEAD)"
export CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target/bisect-${COMMIT_SHA}"
nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
nice -n 19 mbx test -p compiler corpus_runner --no-run < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$ROOT/target/bisect-${COMMIT_SHA}"/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
TOOLS_DIR="${LEDGER_BISECT_TOOLS:-$(dirname "$0")}"
python3 "$TOOLS_DIR/bisect_test.py"
