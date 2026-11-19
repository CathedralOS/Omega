#!/usr/bin/env sh
set -eu

# Synthetic Epsilon refinement. D customer observations and mutations share
# one execution route in tests/epsilon/d-composition/run.sh.
if [ "$#" -ne 0 ]; then
    echo "usage: sh tests/epsilon/refinement/run.sh; select D customers with d-composition/run.sh" >&2
    exit 2
fi

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
GATE_LABEL="Epsilon refinement"
GATE_ENVS="delta/compiler_env.sh epsilon/evaluator_env.sh"
. "$GATE_DIR/../../gate_env.sh"
. "$GATE_DIR/../../gate_host.sh"

REFINE_TMP=$(mktemp -d)
trap 'rm -rf -- "$REFINE_TMP"' EXIT HUP INT TERM
# Bound materializers refuse before writing when the canonical manifest,
# members, packed closure, entry source, or composed record differ from the
# audited edge records.
materialize_epsilon_evaluator "$REFINE_TMP/epsilon_compiler.delta"
materialize_delta_compiler "$REFINE_TMP/delta_compiler.gamma"
materialize_delta_support "$REFINE_TMP/support.bin"
materialize_gamma_evaluator "$REFINE_TMP/evaluator.exe" >/dev/null
require_epsilon_evaluator_entry_identity
cp "${OMEGA_PATH_EPSILON_EVALUATOR_ENTRY:-$OMEGA_REPO_ROOT/tests/epsilon/evaluator-entry/evaluator_entry.delta}" \
    "$REFINE_TMP/evaluator_entry.delta"

python3 -B "$GATE_DIR/gate.py" "$REFINE_TMP" "$@"
