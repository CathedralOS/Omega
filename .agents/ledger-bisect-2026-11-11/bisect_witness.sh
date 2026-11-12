#!/bin/bash
# git bisect run helper using fixed PIN witness spec (not checkout ledger).
set -euo pipefail
SPEC="${BISECT_WITNESS_SPEC:?}"
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
if [[ -L "$ROOT/target" ]] && [[ ! -e "$ROOT/target" ]]; then
  rm -f "$ROOT/target"
fi
mkdir -p "$ROOT/target"
# Skip ledger-only first-parent commits.
mapfile -t CHANGED < <(git diff --name-only HEAD^1 HEAD 2>/dev/null || true)
if ((${#CHANGED[@]} > 0)); then
  NON_LEDGER=0
  for f in "${CHANGED[@]}"; do
  case "$f" in
      tests/omega/corpus_outcomes.txt|*.md) ;;
      *) NON_LEDGER=1; break ;;
    esac
  done
  if ((NON_LEDGER == 0)); then
    exit 125
  fi
fi
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx test -p compiler corpus_runner --no-run < /dev/null >/dev/null 2>&1 || exit 125
WITNESS="$(python3 -c "import json; print(json.load(open('$SPEC'))['witness'])")"
LEDGER_PIN="${LEDGER_BISECT_PIN_LEDGER:-/tmp/ledger_bisect_part23_corpus_outcomes.txt}"
unset OMEGA_CORPUS_NATIVE OMEGA_CORPUS_NATIVE_TARGET OMEGA_CORPUS_INTERPRET
if [[ -f "$LEDGER_PIN" ]]; then
  LINE=$(grep -m1 "^${WITNESS} " "$LEDGER_PIN" || true)
  if [[ "$LINE" == *"check-target:macos_arm64"* ]]; then
    export OMEGA_CORPUS_NATIVE=1 OMEGA_CORPUS_NATIVE_TARGET=macos_arm64
  elif [[ "$LINE" == *"check-target:linux_x86_64"* ]]; then
    export OMEGA_CORPUS_NATIVE=1 OMEGA_CORPUS_NATIVE_TARGET=linux_x86_64
  fi
fi
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "$(dirname "$0")/bisect_test.py" /tmp/bisect_one_record.txt
