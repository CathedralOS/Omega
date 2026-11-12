#!/bin/bash
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="${OMEGA_REPO:-/agent/repos/omega}"
cd "$ROOT"

if git rev-parse --verify -q HEAD^1 >/dev/null 2>&1; then
  mapfile -t CHANGED < <(git diff --name-only HEAD^1 HEAD)
  if ((${#CHANGED[@]} > 0)); then
    ledger_only=1
    for f in "${CHANGED[@]}"; do
      if [[ "$f" != tests/omega/corpus_outcomes.txt ]] && [[ "$f" != *.md ]]; then
        ledger_only=0
        break
      fi
    done
    if ((ledger_only)); then
      exit 125
    fi
  fi
fi

export PATH="/home/ubuntu/.local/share/mbx/bin:$PATH"
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
BISECT_TARGET="${CARGO_TARGET_DIR:-/tmp/ledger-bisect-part-30-target}"
mkdir -p "$BISECT_TARGET"
HEAD_SHA=$(git rev-parse HEAD)
STAMP="$BISECT_TARGET/.bisect_head"
if [[ ! -f "$STAMP" ]] || [[ "$(cat "$STAMP")" != "$HEAD_SHA" ]]; then
  rm -rf "$BISECT_TARGET"
  mkdir -p "$BISECT_TARGET"
  CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$BISECT_TARGET" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
  CARGO_TARGET_DIR="$BISECT_TARGET" nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1 || exit 125
  echo "$HEAD_SHA" >"$STAMP"
fi
RUNNER=$(ls -t "$BISECT_TARGET"/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "${RUNNER:-}" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
TOOLS_DIR="${OMEGA_BISECT_TOOLS:-$(dirname "$0")}"
python3 "$TOOLS_DIR/bisect_test.py" /tmp/bisect_one_record.txt
