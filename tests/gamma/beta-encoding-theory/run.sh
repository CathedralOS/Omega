#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

case "${1:-}" in
    '') ENCODING_GATE=gate.py ;;
    --subject-shape) ENCODING_GATE=subject_shape.py ;;
    *) echo "usage: run.sh [--subject-shape]" >&2; exit 2 ;;
esac
[ "$#" -le 1 ] || { echo "usage: run.sh [--subject-shape]" >&2; exit 2; }

command -v python3 >/dev/null 2>&1 || {
    echo "Beta encoding theory: skipped (python3 absent)"
    exit 0
}
case "$(uname -s)-$(uname -m)" in
    Darwin-arm64|MINGW*-x86_64|MSYS*-x86_64) ;;
    *) echo "Beta encoding theory: unsupported host; needs macOS arm64 or Windows x64" >&2
       exit 2 ;;
esac

ENCODING_TMP=$(mktemp -d)
trap 'rm -rf -- "$ENCODING_TMP"' EXIT HUP INT TERM
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_GAMMA/beta_encoding/theory/theory.gamma.sources" \
    "$ENCODING_TMP/producer.gamma" --prefix "$GATE_DIR/main.gamma"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_GAMMA/derivation_checker/implementation/implementation.gamma.sources" \
    "$ENCODING_TMP/checker.gamma" \
    --prefix "$OMEGA_REPO_ROOT/tests/gamma/derivation-checking/main.gamma"
materialize_gamma_evaluator "$ENCODING_TMP/evaluator" >/dev/null
python3 -B "$GATE_DIR/$ENCODING_GATE" "$ENCODING_TMP"
