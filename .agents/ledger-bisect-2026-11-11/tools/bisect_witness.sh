#!/bin/bash
# git bisect run helper: 0 good, 1 bad (PIN regression), 125 skip
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="${BISECT_REPO_ROOT:?}"
cd "$ROOT"
if parent=$(git rev-parse -q HEAD^1 2>/dev/null); then
  mapfile -t touched < <(git diff --name-only "$parent" HEAD 2>/dev/null || true)
  if [[ ${#touched[@]} -gt 0 ]]; then
    compiler=0
    for f in "${touched[@]}"; do
      [[ "$f" == tests/omega/corpus_outcomes.txt ]] && continue
      case "$f" in *.md) continue ;; esac
      compiler=1
      break
    done
    [[ "$compiler" -eq 0 ]] && exit 125
  fi
fi
rev=$(git rev-parse HEAD)
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
export CARGO_PROFILE_DEV_OPT_LEVEL=1
export CARGO_TARGET_DIR="${BISECT_TARGET_DIR:-$ROOT/target/verify-${rev}}"
mkdir -p "$CARGO_TARGET_DIR"
nice -n 19 mbx build -p omega < /dev/null >/dev/null 2>&1 || exit 125
nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >/dev/null 2>&1 || exit 125
RUNNER=$(ls -t "$CARGO_TARGET_DIR"/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || RUNNER=$(ls -t "$CARGO_TARGET_DIR"/debug/deps/corpus_runner-* 2>/dev/null | head -1)
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "$(dirname "$0")/bisect_test.py" "$WITNESS" /tmp/bisect_one_record.txt
