#!/bin/bash
# 0 = PIN ledger expectation, 1 = PIN status + PIN diagnostic class, 125 = skip.
# One fresh CARGO_TARGET_DIR per commit. Never uses $PWD/target.
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
PIN_JSON="${BISECT_WITNESS_PIN:?}"
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
SHA="$(git rev-parse HEAD)"
export BISECT_SHA="$SHA"

log125() {
  local why="$1"
  if [[ -n "${BISECT_STEP_LOG:-}" ]]; then
    printf '%s\t125\t\t%s\n' "$SHA" "$why" >>"$BISECT_STEP_LOG"
  fi
  exit 125
}

if git rev-parse --verify HEAD^1 >/dev/null 2>&1; then
  mapfile -t CHANGED < <(git diff --name-only HEAD^1 HEAD)
  if ((${#CHANGED[@]} == 0)); then
    log125 "empty-diff"
  fi
  other=""
  for f in "${CHANGED[@]}"; do
    case "$f" in
      tests/omega/corpus_outcomes.txt | *.md) ;;
      *) other="$f" ;;
    esac
  done
  if [[ -z "$other" ]]; then
    log125 "ledger-or-markdown-only"
  fi
fi

TDIR="/tmp/bisect-target-${SHA}"
export CARGO_TARGET_DIR="$TDIR"
export CARGO_PROFILE_DEV_OPT_LEVEL=1
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"

# Keep only this commit's target so disk stays one build wide.
shopt -s nullglob
for d in /tmp/bisect-target-*; do
  if [[ "$d" != "$TDIR" ]]; then
    rm -rf "$d"
  fi
done
shopt -u nullglob
mkdir -p "$TDIR"
chmod -R u+w "$TDIR" 2>/dev/null || true

find_runner() {
  find "$TDIR/debug/build/compiler" -type f -name 'corpus_runner-*' -perm -111 \
    ! -name '*.d' 2>/dev/null | head -1
}

RUNNER="$(find_runner || true)"
if [[ ! -x "${RUNNER:-}" ]]; then
  nice -n 19 mbx build -p omega < /dev/null >"/tmp/bisect-build-${SHA}.log" 2>&1 || log125 "build-omega-failed"
  nice -n 19 mbx test -p compiler --test corpus_runner --no-run < /dev/null >>"/tmp/bisect-build-${SHA}.log" 2>&1 \
    || log125 "build-runner-failed"
  RUNNER="$(find_runner || true)"
fi
[[ -x "${RUNNER:-}" ]] || log125 "runner-missing"

REC="/tmp/bisect-rec-${SHA}.txt"
rm -f "$REC"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS="$REC"
nice -n 19 "$RUNNER" < /dev/null >"/tmp/bisect-run-${SHA}.log" 2>&1 || true
python3 "$(dirname "$0")/bisect_test.py" "$REC"
