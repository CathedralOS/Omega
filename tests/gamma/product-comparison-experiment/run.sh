#!/usr/bin/env sh
# GAMMA-PRODUCT-COMPARISON experiment: one seven-field continuation payload
# built and destructured as ordinary nested pairs (candidate A), as a
# source-emulated named product with identity and arity header words
# (candidate B), as the customer's kind-checked frame boundary (candidate C),
# and as a nominal product compiled by the selected Delta compiler, the
# customer's own static-typing implementation (candidate D). fixtures.tsv pins
# exit code and published byte per Gamma fixture; delta_fixtures.tsv pins the
# canonical compile status, receipt or DCOUT bytes, and receipt execution.
#
# Default: the selected Beta evaluator is materialized and runs each fixture.
# `--reference`: the UNTRUSTED Python Alpha reference interpreter runs the
# selected evaluator tape instead and additionally reports Alpha instruction
# counts. That mode is a diagnostic for hosts without an Alpha seed; it is not
# selected-evaluator acceptance and says so in its output. The candidate-D
# leg runs only in the default mode: the Delta compiler is far too large for
# the instruction-counting reference interpreter.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Gamma product comparison: skipped (python3 absent)"
    exit 0
}

MODE=selected
if [ "$#" -eq 1 ] && [ "$1" = --reference ]; then
    MODE=reference
elif [ "$#" -ne 0 ]; then
    echo "usage: $0 [--reference]" >&2
    exit 2
fi

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM

DELTA_COMPILER=
DELTA_SUPPORT=
if [ "$MODE" = selected ]; then
    . "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"
    materialize_gamma_evaluator "$TMP/evaluator" >/dev/null
    EVALUATOR="$TMP/evaluator"
    if [ -f "$GATE_DIR/delta_fixtures.tsv" ]; then
        . "$OMEGA_REPO_ROOT/tools/bootstrap/delta/compiler_env.sh"
        materialize_delta_compiler "$TMP/delta_compiler.gamma"
        materialize_delta_support "$TMP/delta_support.bin"
        DELTA_COMPILER="$TMP/delta_compiler.gamma"
        DELTA_SUPPORT="$TMP/delta_support.bin"
    fi
else
    EVALUATOR=
fi

MODE="$MODE" EVALUATOR="$EVALUATOR" GATE_DIR="$GATE_DIR" \
    DELTA_COMPILER="$DELTA_COMPILER" DELTA_SUPPORT="$DELTA_SUPPORT" \
    REFERENCE="$OMEGA_REPO_ROOT/tests/alpha/reference/alpha_ref.py" \
    TAPE="$OMEGA_PATH_GAMMA_EVALUATOR_TAPE" python3 "$GATE_DIR/gate.py"
