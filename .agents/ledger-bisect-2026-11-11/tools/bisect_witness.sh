#!/bin/bash
# git bisect run helper.
# 125 = skip: ledger-only commit, or this commit did not build / did not run.
# Outcome and class scores are decided by bisect_test.py (0 or 1 only when a record exists).
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
TD="/tmp/bisect-target-${rev}"
export CARGO_TARGET_DIR="$TD"
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
export CARGO_PROFILE_DEV_OPT_LEVEL=1
if [[ ! -f "$TD/.ready" ]]; then
  rm -rf "$TD"
  mkdir -p "$TD"
  if ! nice -n 19 mbx build -p omega < /dev/null >/tmp/bisect-build-"${rev}".log 2>&1; then
    rm -rf "$TD"
    exit 125
  fi
  if ! nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >>/tmp/bisect-build-"${rev}".log 2>&1; then
    rm -rf "$TD"
    exit 125
  fi
  touch "$TD/.ready"
fi
RUNNER=$(ls -t "$TD"/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | head -1 || true)
[[ -x "$RUNNER" ]] || RUNNER=$(ls -t "$TD"/debug/deps/corpus_runner-* 2>/dev/null | head -1 || true)
[[ -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
safe=$(printf '%s' "$WITNESS" | tr '/' '_')
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS"
export OMEGA_CORPUS_RECORDS="/tmp/bisect-record-${rev}-${safe}.txt"
rm -f "$OMEGA_CORPUS_RECORDS"
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "$(dirname "$0")/bisect_test.py" "$WITNESS" "$OMEGA_CORPUS_RECORDS"
