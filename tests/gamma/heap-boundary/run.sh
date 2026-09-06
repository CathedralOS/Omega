#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Gamma heap boundary: skipped (python3 absent)"
    exit 0
}
case "$(uname -s)-$(uname -m)" in
    Darwin-arm64|MINGW*-x86_64|MSYS*-x86_64) ;;
    *) echo "Gamma heap boundary: unsupported host; needs macOS arm64 or Windows x64" >&2
       exit 2 ;;
esac

HEAP_TMP=$(mktemp -d)
trap 'rm -rf -- "$HEAP_TMP"' EXIT HUP INT TERM
materialize_gamma_evaluator "$HEAP_TMP/evaluator.exe" >/dev/null
python3 -B "$GATE_DIR/gate.py" "$HEAP_TMP/evaluator.exe" \
    "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" "$OMEGA_PATH_GAMMA_EVALUATOR_TAPE" "$@"
