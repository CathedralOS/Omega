#!/bin/bash
# git bisect run helper: 0 = good (PIN ledger expected), 1 = bad (PIN regression), 125 = skip
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
TOOLS="${BISECT_TOOLS_ROOT:-/tmp/ledger_bisect_tools}"
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

if [[ "${BISECT_SKIP_LEDGER_ONLY:-}" == 1 ]] && parent=$(git rev-parse --verify HEAD^1 2>/dev/null); then
  mapfile -t files < <(git diff --name-only "$parent" HEAD 2>/dev/null || true)
  if ((${#files[@]} > 0)); then
    compiler_touch=0
    for f in "${files[@]}"; do
      case "$f" in
        tests/omega/corpus_outcomes.txt) ;;
        *.md) ;;
        *) compiler_touch=1; break ;;
      esac
    done
    ((compiler_touch)) || exit 125
  fi
fi

FULL="$(git rev-parse HEAD)"
RECORD="/tmp/bisect_one_${FULL}.txt"
if ! compgen -G "$ROOT/tests/omega/${WITNESS}/*.omg" >/dev/null \
  && ! compgen -G "$ROOT/tests/omega/${WITNESS}.omg" >/dev/null; then
  exit 125
fi
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
BISECT_TARGET="${BISECT_TARGET_DIR:-$ROOT/target/bisect-ledgers}"
chmod -R u+w "$BISECT_TARGET" 2>/dev/null || true
mkdir -p "$BISECT_TARGET"
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$BISECT_TARGET" \
  nice -n 19 mbx build -p compiler --tests < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$BISECT_TARGET"/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS="$RECORD"
rm -f "$RECORD"
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "$TOOLS/bisect_test.py" "$WITNESS" "$RECORD"
