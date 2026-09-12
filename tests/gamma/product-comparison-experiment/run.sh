#!/usr/bin/env sh
# GAMMA-PRODUCT-COMPARISON experiment: one seven-field continuation payload
# built and destructured as ordinary nested pairs (candidate A) and as a
# source-emulated named product with identity and arity header words
# (candidate B). fixtures.tsv pins exit code and published byte per fixture.
#
# Default: the selected Beta evaluator is materialized and runs each fixture.
# `--reference`: the UNTRUSTED Python Alpha reference interpreter runs the
# selected evaluator tape instead and additionally reports Alpha instruction
# counts. That mode is a diagnostic for hosts without an Alpha seed; it is not
# selected-evaluator acceptance and says so in its output.
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

if [ "$MODE" = selected ]; then
    . "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"
    materialize_gamma_evaluator "$TMP/evaluator" >/dev/null
    EVALUATOR="$TMP/evaluator"
else
    EVALUATOR=
fi

MODE="$MODE" EVALUATOR="$EVALUATOR" GATE_DIR="$GATE_DIR" \
    REFERENCE="$OMEGA_REPO_ROOT/tests/alpha/reference/alpha_ref.py" \
    TAPE="$OMEGA_PATH_GAMMA_EVALUATOR_TAPE" python3 "$GATE_DIR/gate.py"
