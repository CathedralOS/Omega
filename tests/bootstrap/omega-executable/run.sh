#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Omega executable: skipped (python3 absent)"
    exit 0
}

case "$(uname -s)-$(uname -m)" in
    Darwin-arm64|MINGW*-x86_64|MSYS*-x86_64) ;;
    *) echo "Omega executable: unsupported host; needs macOS arm64 or Windows x64" >&2
       exit 2 ;;
esac

OUTPUT_DIR=${OMEGA_EXECUTABLE_BUILD_DIR:-"$OMEGA_REPO_ROOT/build/omega-executable"}
mkdir -p "$OUTPUT_DIR"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$OUTPUT_DIR/delta_compiler.gamma" \
    --prefix "$OMEGA_PATH_DELTA_COMPILER_SOURCE"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_EPSILON_COMPILER_SOURCES" "$OUTPUT_DIR/epsilon_compiler.delta"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_OMEGA_COMPILER_SOURCES" "$OUTPUT_DIR/omega_compiler.epsilon"
materialize_gamma_evaluator "$OUTPUT_DIR/evaluator.exe" >/dev/null
python3 "$GATE_DIR/gate.py" "$OUTPUT_DIR" "$OMEGA_PATH_EPSILON_EXECUTION_DRIVER" "$@"