#!/bin/bash
# git bisect run helper: 0 = good (expected outcome), 1 = PIN-bad, 125 = skip
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="${BISECT_REPO_ROOT:-$(git rev-parse --show-toplevel)}"
export BISECT_REPO_ROOT="$ROOT"
cd "$ROOT"

# Skip ledger-only commits on first-parent history.
if git rev-parse HEAD^1 >/dev/null 2>&1; then
  mapfile -t _changed < <(git diff --name-only HEAD^1 HEAD)
  if ((${#_changed[@]} > 0)); then
    _skip=true
    for _f in "${_changed[@]}"; do
      case "$_f" in
        tests/omega/corpus_outcomes.txt) ;;
        *.md) ;;
        *) _skip=false ;;
      esac
    done
    if $_skip; then
      exit 125
    fi
  fi
fi

export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx test -p compiler --no-run --test corpus_runner < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
python3 "${BISECT_TEST_PY:-$SCRIPT_DIR/bisect_test.py}" "$WITNESS" /tmp/bisect_one_record.txt
