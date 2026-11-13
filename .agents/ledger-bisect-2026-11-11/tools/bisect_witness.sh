#!/bin/bash
# One bisect step. Exit 0 or 1 when the fixture ran. Exit 125 only when the
# commit did not build or the corpus run produced no record.
set -uo pipefail
MODE="${BISECT_WITNESS_MODE:?stop or class}"
WITNESS="${BISECT_WITNESS:?}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MAIN_REPO="${BISECT_MAIN_REPO:-$(cd "$SCRIPT_DIR/../../.." && pwd)}"
ROOT="${BISECT_REPO_ROOT:?}"
STEP_LOG="${BISECT_STEP_LOG:-$MAIN_REPO/.agents/ledger-bisect-2026-11-11/bisect_steps.tsv}"
CACHE="${BISECT_CACHE_DIR:-$MAIN_REPO/.agents/ledger-bisect-2026-11-11/measure-cache}"
USE_CACHE="${BISECT_USE_CACHE:-1}"
REASON_FILE="${BISECT_REASON_FILE:-/tmp/bisect_step_reason.txt}"
mkdir -p "$(dirname "$STEP_LOG")" "$CACHE"
cd "$ROOT"
SHA="$(git rev-parse HEAD)"
SUBJ="$(git log -1 --format='%s' 2>/dev/null | head -c 140)"
REC_SAVED="$CACHE/$SHA/record.txt"
mkdir -p "$CACHE/$SHA"

log_step() {
  local code="$1" kind="$2" detail="${3:-}"
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$SHA" "$code" "$MODE" "$kind" "${detail//$'\t'/ }" "$SUBJ" >>"$STEP_LOG"
}

ledger_only() {
  local parent
  parent="$(git rev-parse -q --verify HEAD^1 2>/dev/null || true)"
  [[ -z "$parent" ]] && return 1
  local files
  mapfile -t files < <(git diff --name-only "$parent" HEAD 2>/dev/null || true)
  [[ ${#files[@]} -eq 0 ]] && return 1
  local f
  for f in "${files[@]}"; do
    case "$f" in
      tests/omega/corpus_outcomes.txt|*.md) ;;
      *) return 1 ;;
    esac
  done
  return 0
}

classify_saved() {
  local rec="$1"
  rm -f "$REASON_FILE"
  local rc=0
  python3 "$SCRIPT_DIR/bisect_test.py" "$WITNESS" "$rec" || rc=$?
  return "$rc"
}

# Ledger-only commits do not change the compiler. Reuse the parent run when
# one exists so they stay in the search without a second build. They are
# rejected later if bisect names them as an edge.
if [[ "$USE_CACHE" == 1 ]] && ledger_only; then
  parent="$(git rev-parse -q --verify HEAD^1 2>/dev/null || true)"
  if [[ -n "$parent" && -f "$CACHE/$parent/record.txt" ]]; then
    cp "$CACHE/$parent/record.txt" "$REC_SAVED"
    if awk -v w="$WITNESS" '$1==w {found=1} END{exit !found}' "$REC_SAVED"; then
      rc=0
      classify_saved "$REC_SAVED" || rc=$?
      log_step "$rc" ledger-parent "reused $parent"
      echo "measure $SHA mode=$MODE rc=$rc ledger-parent" >&2
      exit "$rc"
    fi
  fi
fi

if [[ "$USE_CACHE" == 1 && -f "$REC_SAVED" ]]; then
  if awk -v w="$WITNESS" '$1==w {found=1} END{exit !found}' "$REC_SAVED"; then
    rc=0
    classify_saved "$REC_SAVED" || rc=$?
    detail=""
    [[ -f "$REASON_FILE" ]] && detail="$(tr '\n' ' ' <"$REASON_FILE")"
    log_step "$rc" cache-record "$detail"
    echo "measure $SHA mode=$MODE rc=$rc cache-record" >&2
    exit "$rc"
  fi
fi

exec 9>/tmp/bisect-one-build.lock
flock 9

TARGET="/tmp/bisect-target-$SHA"
MARKER="$TARGET/.bisect-build-ok"
if [[ ! -f "$MARKER" ]]; then
  rm -rf "$TARGET"
  mkdir -p "$TARGET"
  echo "build $SHA -> $TARGET" >&2
  if ! CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_INCREMENTAL=1 \
      CARGO_TARGET_DIR="$TARGET" CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}" \
      nice -n 19 mbx build -p compiler --test corpus_runner < /dev/null >/tmp/bisect_build.log 2>&1; then
    tail -c 400 /tmp/bisect_build.log >&2 || true
    log_step 125 build-fail "$(tail -c 180 /tmp/bisect_build.log | tr '\n' ' ')"
    rm -rf "$TARGET"
    echo "measure $SHA mode=$MODE rc=125 build-fail" >&2
    exit 125
  fi
  touch "$MARKER"
  # Keep only this commit's target so fresh dirs do not fill the disk.
  for d in /tmp/bisect-target-*; do
    [[ "$d" == "$TARGET" ]] && continue
    rm -rf "$d"
  done
fi

RUNNER="$(find "$TARGET/debug/build/compiler" -type f -name 'corpus_runner-*' ! -name '*.d' -printf '%T@ %p\n' 2>/dev/null | sort -nr | awk 'NR==1 {print $2}')"
if [[ -z "${RUNNER:-}" || ! -x "$RUNNER" ]]; then
  log_step 125 no-runner "missing corpus_runner under $TARGET"
  echo "measure $SHA mode=$MODE rc=125 no-runner" >&2
  exit 125
fi

FILTER="${BISECT_FIXTURE_FILTER:-$WITNESS}"
RUN_REC="${BISECT_RECORD_PATH:-/tmp/bisect-rec-$SHA.txt}"
export OMEGA_CORPUS_JOBS=1 OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$FILTER" OMEGA_CORPUS_RECORDS="$RUN_REC"
rm -f "$RUN_REC"
nice -n 19 "$RUNNER" < /dev/null >/tmp/bisect_runner.log 2>&1 || true
if [[ ! -s "$RUN_REC" ]]; then
  log_step 125 no-output "$(tail -c 180 /tmp/bisect_runner.log | tr '\n' ' ')"
  echo "measure $SHA mode=$MODE rc=125 no-output" >&2
  exit 125
fi
cp "$RUN_REC" "$REC_SAVED"
rc=0
classify_saved "$REC_SAVED" || rc=$?
detail=""
[[ -f "$REASON_FILE" ]] && detail="$(tr '\n' ' ' <"$REASON_FILE")"
log_step "$rc" ran "$detail"
echo "measure $SHA mode=$MODE rc=$rc ran" >&2
exit "$rc"
