#!/usr/bin/env sh
set -eu

# Explicit slow gate: the direct evaluator-level status-252 witness for the
# canonical Epsilon evaluator edge. Keep it separate from the routine
# evaluator-entry gate. A per-case watchdog defaults to 7,200 seconds:
#
#   sh tests/epsilon/pair-boundary/run.sh
#   sh tests/epsilon/pair-boundary/run.sh --case writes_exhaustion.epsilon
#   OMEGA_EPSILON_PAIR_SECONDS=10800 sh tests/epsilon/pair-boundary/run.sh

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/delta/compiler_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Epsilon pair boundary: skipped (python3 absent)"
    exit 0
}
case "$(uname -s)-$(uname -m)" in
    Darwin-arm64|MINGW*-x86_64|MSYS*-x86_64) ;;
    *) echo "Epsilon pair boundary: unsupported host; needs macOS arm64 or Windows x64" >&2
       exit 2 ;;
esac

PAIR_TMP=$(mktemp -d)
trap 'rm -rf -- "$PAIR_TMP"' EXIT HUP INT TERM
# Bound materializers refuse before writing when the canonical manifest,
# members, packed closure, entry source, or composed record differ from the
# audited edge records.
materialize_epsilon_evaluator "$PAIR_TMP/epsilon_compiler.delta"
materialize_delta_compiler "$PAIR_TMP/delta_compiler.gamma"
materialize_delta_support "$PAIR_TMP/support.bin"
materialize_gamma_evaluator "$PAIR_TMP/evaluator.exe" >/dev/null
require_epsilon_evaluator_entry_identity
cp "${OMEGA_PATH_EPSILON_EVALUATOR_ENTRY:-$OMEGA_REPO_ROOT/tests/epsilon/evaluator-entry/evaluator_entry.delta}" \
    "$PAIR_TMP/evaluator_entry.delta"

python3 -B "$GATE_DIR/gate.py" "$PAIR_TMP" "$@"
