#!/bin/bash
# git bisect run helper: 0 = PIN-expected outcome, 1 = PIN-broken, 125 = skip
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
if git rev-parse --verify HEAD^1 >/dev/null 2>&1; then
  mapfile -t changed < <(git diff --name-only HEAD^1 HEAD 2>/dev/null || true)
  if [[ ${#changed[@]} -gt 0 ]]; then
    ledger_only=true
    for p in "${changed[@]}"; do
      [[ "$p" == "tests/omega/corpus_outcomes.txt" ]] && continue
      [[ "$p" == *.md ]] && continue
      ledger_only=false
      break
    done
    $ledger_only && exit 125
  fi
fi
git submodule update --init --recursive --depth 1 </dev/null >/dev/null 2>&1 || true
chmod -R u+w "$ROOT/target" 2>/dev/null || true
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(find "$ROOT/target" -name 'corpus_runner-*' -type f ! -name '*.d' 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
head_sha=$(git rev-parse HEAD)
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS="/tmp/bisect_one_record_${head_sha}.txt"
rm -f "$OMEGA_CORPUS_RECORDS"
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "$(dirname "$0")/bisect_test.py" "$WITNESS" "$OMEGA_CORPUS_RECORDS"
