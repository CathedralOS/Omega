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
    echo "Interpreted D parser: skipped (python3 absent)"
    exit 0
}

case "$(uname -s)-$(uname -m)" in
    Darwin-arm64|MINGW*-x86_64|MSYS*-x86_64) ;;
    *) echo "Interpreted D parser: unsupported host; needs macOS arm64 or Windows x64" >&2
       exit 2 ;;
esac

PARSER_TMP=$(mktemp -d)
trap 'rm -rf -- "$PARSER_TMP"' EXIT HUP INT TERM
# Bound materializers refuse before writing when the canonical entry,
# manifest, members, packed closure, or composed record differ from the
# audited edge records.
materialize_delta_compiler "$PARSER_TMP/delta_compiler.gamma"
materialize_delta_support "$PARSER_TMP/support.bin"
materialize_epsilon_evaluator "$PARSER_TMP/epsilon_compiler.delta"
materialize_omega_compiler "$PARSER_TMP/omega_compiler.epsilon"
require_omega_parser_entry_identity
materialize_gamma_evaluator "$PARSER_TMP/evaluator.exe" >/dev/null
python3 "$GATE_DIR/gate.py" "$PARSER_TMP" "$OMEGA_PATH_EPSILON_EXECUTION_DRIVER"
