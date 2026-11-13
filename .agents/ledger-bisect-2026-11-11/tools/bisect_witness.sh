#!/bin/bash
# One bisect step. Exit 125 only when this commit did not build or the witness did not run.
# BISECT_MODE=stop|class selects the classifier. One build at a time.
set -uo pipefail
WITNESS="${BISECT_WITNESS:?}"
MODE="${BISECT_MODE:?}"
ROOT="${BISECT_ROOT:-$(git rev-parse --show-toplevel)}"
TOOL="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"
SHA=$(git rev-parse HEAD)
LOG="${BISECT_RC_LOG:-/tmp/bisect-rc.log}"
LOCK=/tmp/bisect-build.lock
TARGET="/tmp/bisect-target-${SHA}"
REC="/tmp/bisect-record-${SHA}.txt"

exec 9>"$LOCK"
flock 9

rm -rf "$TARGET"
mkdir -p "$TARGET"
export CARGO_TARGET_DIR="$TARGET"
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_OPT_LEVEL=1
export MBX_SCHEDULER_CPUS=3
export CARGO_BUILD_JOBS=3
export OMEGA_CORPUS_JOBS=1
export OMEGA_CORPUS_ROOT="$ROOT"
export OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS"
export OMEGA_CORPUS_RECORDS="$REC"

finish() {
  local rc=$1
  echo "${SHA} ${MODE} ${rc}" >>"$LOG"
  rm -rf "$TARGET" "$REC"
  exit "$rc"
}

if ! nice -n 19 mbx build -p omega < /dev/null >"$TARGET/build.log" 2>&1; then
  cp "$TARGET/build.log" "/tmp/bisect-fail-${SHA}-build.log" 2>/dev/null || true
  finish 125
fi
if ! nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >"$TARGET/runner-build.log" 2>&1; then
  cp "$TARGET/runner-build.log" "/tmp/bisect-fail-${SHA}-runner.log" 2>/dev/null || true
  finish 125
fi
RUNNER=$(find "$TARGET/debug/build/compiler" -type f -name 'corpus_runner-*' ! -name '*.d' -executable | head -1)
if [[ -z "$RUNNER" || ! -x "$RUNNER" ]]; then
  finish 125
fi
rm -f "$REC"
# A rejected fixture still writes a record; only a missing record means the witness did not run.
nice -n 19 "$RUNNER" < /dev/null >"$TARGET/run.log" 2>&1 || true
if [[ ! -s "$REC" ]] || ! grep -q "^${WITNESS} " "$REC"; then
  cp "$TARGET/run.log" "/tmp/bisect-fail-${SHA}-run.log" 2>/dev/null || true
  finish 125
fi
if [[ "$MODE" == "both" ]]; then
  BISECT_MODE=stop python3 "$TOOL/bisect_test.py" "$WITNESS" "$REC" >/dev/null
  stop=$?
  BISECT_MODE=class python3 "$TOOL/bisect_test.py" "$WITNESS" "$REC" >/dev/null
  class=$?
  echo "${SHA} both stop=${stop} class=${class}" >>"$LOG"
  rm -rf "$TARGET" "$REC"
  exit 0
fi
BISECT_MODE="$MODE" python3 "$TOOL/bisect_test.py" "$WITNESS" "$REC" >/dev/null
finish $?
