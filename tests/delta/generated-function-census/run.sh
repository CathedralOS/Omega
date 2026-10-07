#!/usr/bin/env sh
set -eu

if [ "$#" -gt 1 ] || { [ "$#" -eq 1 ] && [ "$1" != --normalization ]; }; then
    echo "usage: $0 [--normalization]" >&2
    exit 2
fi

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
GATE_LABEL="Delta generated function census"
GATE_ENVS="delta/compiler_env.sh"
. "$GATE_DIR/../../gate_env.sh"
. "$GATE_DIR/../../gate_host.sh"

CENSUS_TMP=$(mktemp -d)
trap 'rm -rf -- "$CENSUS_TMP"' EXIT HUP INT TERM
materialize_delta_compiler "$CENSUS_TMP/compiler.gamma"
materialize_gamma_evaluator "$CENSUS_TMP/evaluator" >/dev/null
materialize_delta_support "$CENSUS_TMP/support.bin"
python3 "$GATE_DIR/gate.py" "$CENSUS_TMP" "$@"
