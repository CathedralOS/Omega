#!/bin/bash
# One fresh-target witness measurement. Exit 0, 1, or 125.
# Ledger-only first-parent commits exit 125 before building when BISECT_ALLOW_LEDGER_SKIP=1.
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
FILTER="${BISECT_FILTER:-$WITNESS}"
PREDICATE="${BISECT_PREDICATE:-outcome}"
ROOT="${OMEGA_REPO:-/agent/repos/omega}"
TOOLS="/tmp/ledger_bisect_tools_2026_11_11"
ALLOW_SKIP="${BISECT_ALLOW_LEDGER_SKIP:-1}"
USE_CACHE="${BISECT_USE_CACHE:-0}"

mkdir -p "$TOOLS"
if [[ ! -f "$TOOLS/bisect_test.py" ]]; then
  git -C "$ROOT" archive "work/ledger-bisect-part-10:.agents/ledger-bisect-2026-11-11/tools" | tar -x -C "$TOOLS"
  chmod +x "$TOOLS"/*.sh "$TOOLS"/*.py 2>/dev/null || true
fi

cd "$ROOT"
SHA=$(git rev-parse HEAD)
TRACE=/tmp/bisect_trace.jsonl
CACHE_DIR="/tmp/bisect-cache/${SHA}/$(printf '%s' "$FILTER" | sha256sum | awk '{print $1}')"
RECORD=/tmp/bisect_one_record.txt

log_trace() {
  local code="$1" reason="$2"
  python3 - "$code" "$reason" <<'PY'
import json, os, sys
from pathlib import Path
code, reason = sys.argv[1], sys.argv[2]
detail = {}
p = Path("/tmp/bisect_last_detail.json")
if p.exists():
    try:
        detail = json.loads(p.read_text())
    except Exception:
        detail = {}
row = {
    "sha": os.environ.get("SHA", ""),
    "predicate": os.environ.get("PREDICATE", ""),
    "witness": os.environ.get("WITNESS", ""),
    "filter": os.environ.get("FILTER", ""),
    "exit": int(code) if str(code).lstrip("-").isdigit() else code,
    "reason": reason,
    "status": detail.get("status"),
    "diag": detail.get("diag"),
    "diag_equals_pin": detail.get("diag_equals_pin"),
    "diag_same_class": detail.get("diag_same_class"),
}
Path("/tmp/bisect_trace.jsonl").open("a").write(json.dumps(row) + "\n")
PY
}

export SHA WITNESS FILTER PREDICATE

if [[ "$ALLOW_SKIP" == "1" ]] && git rev-parse HEAD^1 >/dev/null 2>&1; then
  mapfile -t CHANGED < <(git diff --name-only HEAD^1 HEAD 2>/dev/null || true)
  if ((${#CHANGED[@]} > 0)); then
    only_docs=1
    for f in "${CHANGED[@]}"; do
      case "$f" in
        tests/omega/corpus_outcomes.txt|*.md) ;;
        *) only_docs=0; break ;;
      esac
    done
    if ((only_docs)); then
      echo '{"exit":125,"reason":"ledger-only"}' > /tmp/bisect_last_detail.json
      log_trace 125 ledger-only
      exit 125
    fi
  fi
fi

if [[ "$USE_CACHE" == "1" && -f "$CACHE_DIR/record.txt" ]]; then
  cp "$CACHE_DIR/record.txt" "$RECORD"
  set +e
  BISECT_PREDICATE="$PREDICATE" python3 "$TOOLS/bisect_test.py" "$WITNESS" "$RECORD"
  CODE=$?
  set -e
  log_trace "$CODE" cache
  exit "$CODE"
fi

exec 9>/tmp/bisect-build.lock
flock 9

TARGET="/tmp/bisect-target-${SHA}"
rm -rf "$TARGET"
mkdir -p "$TARGET"
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
export CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$TARGET"
unset NEXTEST || true

build_once() {
  nice -n 19 mbx test -p compiler corpus_runner --no-run < /dev/null >"$TARGET/build.log" 2>&1
}

if ! build_once; then
  if ! build_once; then
    tail -40 "$TARGET/build.log" > /tmp/bisect_build_fail_tail.txt || true
    rm -rf "$TARGET"
    echo '{"exit":125,"reason":"build-failed"}' > /tmp/bisect_last_detail.json
    log_trace 125 build-failed
    exit 125
  fi
fi

RUNNER=$(find "$TARGET/debug/build/compiler" -type f -name 'corpus_runner-*' -perm -111 -printf '%T@ %p\n' 2>/dev/null | sort -nr | awk 'NR==1 { print $2 }')
if [[ -z "${RUNNER:-}" || ! -x "$RUNNER" ]]; then
  rm -rf "$TARGET"
  echo '{"exit":125,"reason":"runner-missing"}' > /tmp/bisect_last_detail.json
  log_trace 125 runner-missing
  exit 125
fi

export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$FILTER" OMEGA_CORPUS_RECORDS="$RECORD"
rm -f "$RECORD"
nice -n 19 "$RUNNER" < /dev/null >/tmp/bisect_runner_stdout.txt 2>/tmp/bisect_runner_stderr.txt || true
rm -rf "$TARGET"

if [[ ! -s "$RECORD" ]]; then
  echo '{"exit":125,"reason":"empty-record"}' > /tmp/bisect_last_detail.json
  log_trace 125 empty-record
  exit 125
fi

mkdir -p "$CACHE_DIR"
cp "$RECORD" "$CACHE_DIR/record.txt"

set +e
BISECT_PREDICATE="$PREDICATE" python3 "$TOOLS/bisect_test.py" "$WITNESS" "$RECORD"
CODE=$?
set -e
log_trace "$CODE" ran
exit "$CODE"
