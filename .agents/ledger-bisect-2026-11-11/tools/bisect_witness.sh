#!/bin/bash
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
if [[ -d /tmp/ledger-bisect-tools ]]; then
  SCRIPT_DIR=/tmp/ledger-bisect-tools
fi
ROOT="${REPO_ROOT:-/agent/repos/omega}"
if [[ -d "$ROOT/.git" ]]; then
  cd "$ROOT"
else
  ROOT="$(git rev-parse --show-toplevel)"
  cd "$ROOT"
fi
export OMEGA_REPO="$ROOT"
export BISECT_PIN_LEDGER="${BISECT_PIN_LEDGER:-/tmp/ledger_bisect_corpus_outcomes.txt}"
export BISECT_PIN_WITNESS_JSON="${BISECT_PIN_WITNESS_JSON:-/tmp/ledger_bisect_witness_pin.json}"

# Skip ledger-only first-parent commits (no compiler behavior change).
if git rev-parse -q --verify HEAD^1 >/dev/null 2>&1; then
  mapfile -t CHANGED < <(git diff --name-only HEAD^1 HEAD)
  if ((${#CHANGED[@]} > 0)); then
    compiler_touch=0
    for f in "${CHANGED[@]}"; do
      case "$f" in
        tests/omega/corpus_outcomes.txt) ;;
        *.md) ;;
        *) compiler_touch=1; break ;;
      esac
    done
    if ((compiler_touch == 0)); then
      exit 125
    fi
  fi
fi

export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
BISECT_TARGET="${CARGO_TARGET_DIR:-$ROOT/target/bisect-pool}"
mkdir -p "$BISECT_TARGET"
chmod -R u+w "$BISECT_TARGET" "$ROOT/target" 2>/dev/null || true
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$BISECT_TARGET" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$BISECT_TARGET" nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$BISECT_TARGET"/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | grep -v '\.d$' | head -1)
[[ -n "$RUNNER" && -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "$SCRIPT_DIR/bisect_test.py" "$WITNESS" /tmp/bisect_one_record.txt
