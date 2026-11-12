#!/bin/bash
# PIN-fixed bisect helper: 0 good, 1 bad (PIN repro), 125 skip/build/other
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="$(git -C /agent/repos/omega rev-parse --show-toplevel)"
cd "$ROOT"
TOOL_DIR="$(cd "$(dirname "$0")" && pwd)"

# Skip ledger-only first-parent commits.
if git rev-parse HEAD^1 >/dev/null 2>&1; then
  mapfile -t changed < <(git diff --name-only HEAD^1 HEAD)
  if ((${#changed[@]} > 0)); then
  only_ledger=1
  for f in "${changed[@]}"; do
    [[ "$f" == tests/omega/corpus_outcomes.txt ]] && continue
    [[ "$f" == *.md ]] && continue
    only_ledger=0
    break
  done
  if ((only_ledger)); then
    exit 125
  fi
  fi
fi

export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
unset CARGO_TARGET_DIR
BISECT_TARGET="/tmp/omega-target-bisect-$(git rev-parse HEAD)"
mkdir -p "$BISECT_TARGET"
chmod u+w "$BISECT_TARGET" 2>/dev/null || true
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$BISECT_TARGET" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$BISECT_TARGET" nice -n 19 mbx test -p compiler --test corpus_runner --no-run < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$BISECT_TARGET"/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "$TOOL_DIR/bisect_pin_test.py" "$WITNESS" /tmp/bisect_one_record.txt
