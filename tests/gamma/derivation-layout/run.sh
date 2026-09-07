#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Derivation layout: skipped (python3 absent)"
    exit 0
}
case "$(uname -s)-$(uname -m)" in
    Darwin-arm64|MINGW*-x86_64|MSYS*-x86_64) ;;
    *) echo "Derivation layout: unsupported host; needs macOS arm64 or Windows x64" >&2
       exit 2 ;;
esac

LAYOUT_TMP=$(mktemp -d)
trap 'rm -rf -- "$LAYOUT_TMP"' EXIT HUP INT TERM
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_GAMMA/derivation_checker/implementation/implementation.gamma.sources" \
    "$LAYOUT_TMP/diagnostic.gamma" --prefix "$GATE_DIR/main.gamma"
materialize_gamma_evaluator "$LAYOUT_TMP/evaluator" >/dev/null
python3 -B "$GATE_DIR/gate.py" "$LAYOUT_TMP"
