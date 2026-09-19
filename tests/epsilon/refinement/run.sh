#!/usr/bin/env sh
set -eu

# Direct RunEpsilon refinement: the canonical evaluator edge checked against
# the test-owned independent model under source, stdin, profile, and
# observation mutations.
#
#   sh tests/epsilon/refinement/run.sh

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/delta/compiler_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Epsilon refinement: skipped (python3 absent)"
    exit 0
}
case "$(uname -s)-$(uname -m)" in
    Darwin-arm64|MINGW*-x86_64|MSYS*-x86_64) ;;
    *) echo "Epsilon refinement: unsupported host; needs macOS arm64 or Windows x64" >&2
       exit 2 ;;
esac

REFINE_TMP=$(mktemp -d)
trap 'rm -rf -- "$REFINE_TMP"' EXIT HUP INT TERM
# Bound materializers refuse before writing when the canonical manifest,
# members, packed closure, entry source, or composed record differ from the
# audited edge records.
materialize_epsilon_evaluator "$REFINE_TMP/epsilon_compiler.delta"
materialize_delta_compiler "$REFINE_TMP/delta_compiler.gamma"
materialize_delta_support "$REFINE_TMP/support.bin"
materialize_gamma_evaluator "$REFINE_TMP/evaluator.exe" >/dev/null
require_epsilon_evaluator_entry_identity
cp "${OMEGA_PATH_EPSILON_EVALUATOR_ENTRY:-$OMEGA_REPO_ROOT/tests/epsilon/evaluator-entry/evaluator_entry.delta}" \
    "$REFINE_TMP/evaluator_entry.delta"

python3 -B "$GATE_DIR/gate.py" "$REFINE_TMP"
