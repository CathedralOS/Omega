#!/bin/bash
set -euo pipefail
WITNESS="${BISECT_WITNESS:?}"
ROOT="${BISECT_REPO_ROOT:-$(git -C "$(dirname "$0")" rev-parse --show-toplevel)}"
MAIN_REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
STEP_LOG="${BISECT_STEP_LOG:-$MAIN_REPO/.agents/ledger-bisect-2026-11-11/bisect_steps.tsv}"
REASON_FILE="${BISECT_REASON_FILE:-/tmp/bisect_step_reason.txt}"
cd "$ROOT"
HEAD_SHA="$(git rev-parse HEAD)"
SUBJ="$(git log -1 --format='%s' 2>/dev/null | head -c 120)"

log_step() {
  local code="$1" kind="$2" detail="${3:-}"
  printf '%s\t%s\t%s\t%s\t%s\n' "$HEAD_SHA" "$code" "$kind" "${detail//$'\t'/ }" "$SUBJ" >>"$STEP_LOG"
}

# Ledger-only first-parent commits cannot change compiler behavior (git bisect skip).
if parent="$(git rev-parse -q --verify HEAD^1 2>/dev/null)"; then
  mapfile -t _changed < <(git diff --name-only "$parent" HEAD 2>/dev/null || true)
  if [[ ${#_changed[@]} -gt 0 ]]; then
    compiler=0
    for f in "${_changed[@]}"; do
      case "$f" in
        tests/omega/corpus_outcomes.txt) ;;
        *.md) ;;
        *) compiler=1; break ;;
      esac
    done
    if [[ "$compiler" -eq 0 ]]; then
      log_step 125 ledger-only "$(printf '%s,' "${_changed[@]}")"
      exit 125
    fi
  fi
fi

export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4 CARGO_INCREMENTAL=1
export BISECT_REASON_FILE="$REASON_FILE"
rm -f "$REASON_FILE"
# Incremental: compile only corpus_runner test harness (rebuilds when compiler sources change).
if ! CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$ROOT/target" nice -n 19 mbx build -p compiler --test corpus_runner < /dev/null >/tmp/bisect_build.log 2>&1; then
  log_step 125 build-fail "$(tail -3 /tmp/bisect_build.log | tr '\n' ' ')"
  exit 125
fi
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* 2>/dev/null | grep -v '\.d$' | head -1)
if [[ ! -x "$RUNNER" ]]; then
  log_step 125 no-runner "missing corpus_runner under $ROOT/target"
  exit 125
fi
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS=/tmp/bisect_one_record.txt
rm -f /tmp/bisect_one_record.txt
nice -n 19 "$RUNNER" < /dev/null >/tmp/bisect_runner.log 2>&1 || true
set +e
RC=$(python3 "$(dirname "$0")/bisect_test.py" "$WITNESS" /tmp/bisect_one_record.txt)
set -e
if [[ -f "$REASON_FILE" ]]; then
  detail=$(tr '\n' ' ' <"$REASON_FILE")
else
  detail=""
fi
case "$RC" in
  0) log_step 0 good "$detail" ;;
  1) log_step 1 pin-broken "$detail" ;;
  125) log_step 125 witness "$detail" ;;
  *) log_step "$RC" witness "$detail" ;;
esac
exit "$RC"
