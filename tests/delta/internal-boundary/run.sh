#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Delta internal boundary: skipped (python3 absent)"
    exit 0
}

case "$(uname -s)-$(uname -m)" in
    Darwin-arm64|MINGW*-x86_64|MSYS*-x86_64) ;;
    *) echo "Delta internal boundary: unsupported host; needs macOS arm64 or Windows x64" >&2
       exit 2 ;;
esac

INTERNAL_TMP=$(mktemp -d)
trap 'rm -rf -- "$INTERNAL_TMP"' EXIT HUP INT TERM
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$GATE_DIR/controls.gamma.sources" "$INTERNAL_TMP/controls.gamma"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$INTERNAL_TMP/diagnostic.gamma" \
    --prefix "$INTERNAL_TMP/controls.gamma"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$INTERNAL_TMP/canonical.gamma" \
    --prefix "$OMEGA_PATH_DELTA_COMPILER_SOURCE"
materialize_gamma_evaluator "$INTERNAL_TMP/evaluator.exe" >/dev/null
python3 -B "$GATE_DIR/gate.py" "$INTERNAL_TMP"
