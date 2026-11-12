#!/bin/bash
# 0 = good (PIN ledger expectation), 1 = bad (PIN break), 125 = skip / build fail
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
export OMEGA_REPO="$ROOT"

is_ledger_only_commit() {
  local c="${1:-HEAD}"
  local -a files=()
  mapfile -t files < <(git diff-tree --no-commit-id --name-only -r "$c" 2>/dev/null || true)
  [[ ${#files[@]} -eq 0 ]] && return 1
  local f
  for f in "${files[@]}"; do
    [[ -z "$f" ]] && continue
    [[ "$f" == tests/omega/corpus_outcomes.txt ]] && continue
    [[ "$f" == *.md ]] && continue
    return 1
  done
  return 0
}

if is_ledger_only_commit "$(git rev-parse HEAD)"; then
  exit 125
fi

export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "$(dirname "$0")/bisect_test.py" "$BISECT_WITNESS" /tmp/bisect_one_record.txt
