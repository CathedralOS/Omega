#!/bin/bash
# Build and classify one witness at current HEAD; prints bisect exit code (0 good, 1 bad, 125 skip).
set -euo pipefail
WITNESS="${1:?witness}"
ROOT="${OMEGA_REPO:-/agent/repos/omega}"
cd "$ROOT"
TOOLS="${BISECT_TOOLS:-$ROOT/.agents/ledger-bisect-2026-11-11/tools}"
export OMEGA_REPO="$ROOT"
REV=$(git rev-parse HEAD^{commit})
if ! git show "$REV:omega-rust/omega/compilation/compiler/Cargo.toml" 2>/dev/null | grep -q 'name = "corpus_runner"'; then
  exit 125
fi
SHORT=$(git rev-parse --short=12 HEAD)
TARGET="$ROOT/target/bisect-$SHORT"
rm -rf "$TARGET"
export OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$TARGET" nice -n 19 mbx test -p compiler --test corpus_runner --no-run < /dev/null >/dev/null 2>&1 || exit 125
shopt -s nullglob
candidates=("$TARGET"/debug/build/compiler/*/out/corpus_runner-*)
shopt -u nullglob
RUNNER=""
if ((${#candidates[@]} > 0)); then
  RUNNER=$(ls -t "${candidates[@]}" | head -1)
fi
[[ -n "$RUNNER" && -x "$RUNNER" ]] || exit 125
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
REC="/tmp/bisect_one_record_${SHORT}.txt"
export OMEGA_CORPUS_FIXTURE_FILTER="$WITNESS" OMEGA_CORPUS_RECORDS="$REC"
rm -f "$REC"
nice -n 19 "$RUNNER" < /dev/null >/dev/null 2>&1 || true
python3 "$TOOLS/bisect_test.py" "$WITNESS" "$REC"
