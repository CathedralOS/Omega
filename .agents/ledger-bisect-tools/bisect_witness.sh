#!/bin/bash
# git bisect run helper: 0=good(PIN expected), 1=bad(PIN broken), 125=skip
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"

# Skip ledger-only commits on first-parent history.
if git rev-parse HEAD^1 >/dev/null 2>&1; then
  mapfile -t touched < <(git diff --name-only HEAD^1 HEAD 2>/dev/null || true)
  if [[ ${#touched[@]} -gt 0 ]]; then
    compiler_touch=0
    for f in "${touched[@]}"; do
      case "$f" in
        tests/omega/corpus_outcomes.txt) ;;
        *.md) ;;
        *) compiler_touch=1; break ;;
      esac
    done
    [[ "$compiler_touch" -eq 0 ]] && exit 125
  fi
fi

export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx test --locked --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "$(dirname "$0")/bisect_test.py" "$WITNESS" /tmp/bisect_one_record.txt
