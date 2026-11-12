#!/bin/bash
# Classify HEAD for $BISECT_WITNESS.
# 0 = PIN ledger outcome
# 1 = exact PIN diagnostic (BISECT_MODE=pin) or any completed miss (BISECT_MODE=stops)
# 125 = build failure, ledger-only commit, or any other outcome in pin mode
# Builds only this commit, in CARGO_TARGET_DIR=/tmp/bisect-target-<sha>.
set -u
WITNESS="${BISECT_WITNESS:?}"
MODE="${BISECT_MODE:-pin}"
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
TOOL="$(cd "$(dirname "$0")" && pwd)"
SHA="$(git rev-parse HEAD)"
CACHE_DIR="/tmp/bisect-obs/${WITNESS//\//_}"
mkdir -p "$CACHE_DIR" /tmp/bisect-logs
OBS="$CACHE_DIR/$SHA.json"
FORCE="${BISECT_FORCE_RERUN:-0}"
KEEP="${BISECT_KEEP_TARGET:-0}"
TARGET="/tmp/bisect-target-$SHA"
MARKER="$TARGET/.bisect-built"
LOG="/tmp/bisect-logs/${SHA}.log"
LOCK="/tmp/bisect-one-build.lock"

finish() {
  local rc="$1"
  if [[ "$KEEP" != "1" ]]; then
    rm -rf "$TARGET"
  fi
  if [[ "$rc" -ne 0 && "$rc" -ne 1 && "$rc" -ne 125 ]]; then
    rc=125
  fi
  exit "$rc"
}

is_ledger_only_commit() {
  local c="$1"
  local -a files=()
  mapfile -t files < <(git diff-tree --no-commit-id --name-only -r "$c" 2>/dev/null || true)
  [[ ${#files[@]} -eq 0 ]] && return 1
  local f
  for f in "${files[@]}"; do
    [[ -z "$f" ]] && continue
    [[ "$f" == tests/omega/corpus_outcomes.txt ]] && continue
    [[ "$f" == *.md ]] && continue
    return 1
  done
  return 0
}

if [[ "$FORCE" != "1" && -f "$OBS" ]]; then
  BISECT_MODE="$MODE" python3 "$TOOL/bisect_test.py" --from-obs "$OBS"
  finish $?
fi

if is_ledger_only_commit "$SHA"; then
  printf '%s\n' '{"ledger_only":true}' >"$OBS"
  finish 125
fi

exec 9>"$LOCK"
flock 9

if [[ ! -f "$MARKER" ]]; then
  build_once() {
    rm -rf "$TARGET"
    mkdir -p "$TARGET"
    CARGO_PROFILE_DEV_OPT_LEVEL=0 \
      CARGO_TARGET_DIR="$TARGET" \
      CARGO_BUILD_JOBS=4 \
      MBX_SCHEDULER_CPUS=4 \
      nice -n 19 mbx test --no-run -p compiler --test corpus_runner < /dev/null >"$LOG" 2>&1
  }
  if ! build_once; then
    if ! build_once; then
      printf '%s\n' '{"error":"build"}' >"$OBS"
      finish 125
    fi
  fi
  touch "$MARKER"
fi

RUNNER=""
for c in "$TARGET"/debug/build/compiler/*/out/corpus_runner-* \
  "$TARGET"/debug/corpus_runner-* \
  "$TARGET"/debug/deps/corpus_runner-*; do
  if [[ -x "$c" && -f "$c" ]]; then
    RUNNER="$c"
    break
  fi
done
if [[ -z "$RUNNER" ]]; then
  printf '%s\n' '{"error":"no-runner"}' >"$OBS"
  finish 125
fi

REC="/tmp/bisect-rec-${SHA}.txt"
rm -f "$REC"
export OMEGA_REPO="$ROOT" OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_JOBS=1 OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS="$REC"
nice -n 19 "$RUNNER" < /dev/null >>"$LOG" 2>&1 || true
python3 - "$WITNESS" "$REC" "$OBS" "$SHA" <<'PY'
import json, sys
from pathlib import Path
witness, rec, obs_path, sha = sys.argv[1:]
text = Path(rec).read_text() if Path(rec).exists() else ""
status = None
diag = ""
for line in text.splitlines():
    if line.startswith(witness + " "):
        parts = line.split()
        status = parts[1] if len(parts) >= 2 else None
        diag = ""
        continue
    if line.startswith("\t") and status is not None and not diag:
        diag = line[1:].strip()
        continue
    if status is not None and line and not line.startswith("\t"):
        break
payload = {"status": status, "diag": diag, "sha": sha}
if status is None:
    payload["error"] = "no-record"
Path(obs_path).write_text(json.dumps(payload))
PY
BISECT_MODE="$MODE" python3 "$TOOL/bisect_test.py" --from-obs "$OBS"
finish $?
