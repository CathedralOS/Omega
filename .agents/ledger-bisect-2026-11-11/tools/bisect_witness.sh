#!/bin/bash
# git bisect: 0 = good (PIN-ledger expected), 1 = bad (PIN break), 125 = skip/build/other
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="${OMEGA_REPO_ROOT:?}"
TEST_PY="${BISECT_TEST_PY:?}"
cd "$ROOT"

# Ledger-only commits cannot change compiler behaviour.
if git rev-parse HEAD^1 >/dev/null 2>&1; then
  mapfile -t _changed < <(git diff --name-only HEAD^1 HEAD 2>/dev/null || true)
  if ((${#_changed[@]} > 0)); then
    _ledger_only=1
    for _f in "${_changed[@]}"; do
      case "$_f" in
        tests/omega/corpus_outcomes.txt) ;;
        *.md) ;;
        *) _ledger_only=0; break ;;
      esac
    done
    if ((_ledger_only)); then
      exit 125
    fi
  fi
fi

export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "$TEST_PY" "$WITNESS" /tmp/bisect_one_record.txt
