#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/delta/compiler_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/evaluator_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/omega/compiler_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Epsilon D composition: skipped (python3 absent)"
    exit 0
}

case "$(uname -s)-$(uname -m)" in
    Darwin-arm64|MINGW*-x86_64|MSYS*-x86_64) ;;
    *) echo "Epsilon D composition: unsupported host; needs macOS arm64 or Windows x64" >&2
       exit 2 ;;
esac

COMPOSITION_TMP=$(mktemp -d)
trap 'rm -rf -- "$COMPOSITION_TMP"' EXIT HUP INT TERM
# Bound materializers refuse before writing when the canonical manifest,
# members, packed closure, entry source, or packed D closure differ from the
# audited edge records.
materialize_epsilon_evaluator "$COMPOSITION_TMP/epsilon_compiler.delta"
require_epsilon_evaluator_entry_identity
materialize_delta_compiler "$COMPOSITION_TMP/delta_compiler.gamma"
materialize_delta_support "$COMPOSITION_TMP/support.bin"
materialize_omega_compiler "$COMPOSITION_TMP/omega_compiler.epsilon"
materialize_gamma_evaluator "$COMPOSITION_TMP/evaluator.exe" >/dev/null
python3 "$GATE_DIR/gate.py" "$COMPOSITION_TMP" "$@"
