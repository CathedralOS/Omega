#!/bin/bash
# git bisect run helper with fixed PIN ledger (see bisect_test.py).
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
TOOL_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(git -C "$TOOL_DIR" rev-parse --show-toplevel)"
cd "$ROOT"

mapfile -t files < <(git diff-tree --no-commit-id --name-only -r HEAD 2>/dev/null || true)
if ((${#files[@]} > 0)); then
  only_ledger=true
  for f in "${files[@]}"; do
    case "$f" in
      tests/omega/corpus_outcomes.txt|*.md) ;;
      *) only_ledger=false; break ;;
    esac
  done
  if $only_ledger; then
    exit 125
  fi
fi

export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
CARGO_TARGET_DIR="$ROOT/target" mbx test -p compiler corpus_runner --no-run < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "$TOOL_DIR/bisect_test.py" "$WITNESS" /tmp/bisect_one_record.txt
