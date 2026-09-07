#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Beta lexical theory: skipped (python3 absent)"
    exit 0
}
case "$(uname -s)-$(uname -m)" in
    Darwin-arm64|MINGW*-x86_64|MSYS*-x86_64) ;;
    *) echo "Beta lexical theory: unsupported host; needs macOS arm64 or Windows x64" >&2
       exit 2 ;;
esac

LEXICAL_TMP=$(mktemp -d)
trap 'rm -rf -- "$LEXICAL_TMP"' EXIT HUP INT TERM
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_GAMMA/beta_encoding/lexical_theory/theory.gamma.sources" \
    "$LEXICAL_TMP/producer.gamma" --prefix "$GATE_DIR/main.gamma"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_GAMMA/derivation_checker/implementation/implementation.gamma.sources" \
    "$LEXICAL_TMP/checker.gamma" \
    --prefix "$OMEGA_REPO_ROOT/tests/gamma/derivation-checking/main.gamma"
materialize_gamma_evaluator "$LEXICAL_TMP/evaluator" >/dev/null
python3 -B "$GATE_DIR/gate.py" "$LEXICAL_TMP"
