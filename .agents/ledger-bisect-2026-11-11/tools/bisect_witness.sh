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
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
if [[ ! -x "${RUNNER:-}" ]]; then
  nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1 || exit 125
  RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
fi
[[ -x "${RUNNER:-}" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
TOOLS_DIR="${OMEGA_BISECT_TOOLS:-$(dirname "$0")}"
python3 "$TOOLS_DIR/bisect_test.py" /tmp/bisect_one_record.txt
