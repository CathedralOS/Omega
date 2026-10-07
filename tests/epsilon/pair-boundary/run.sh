#!/usr/bin/env sh
set -eu

# Explicit slow gate: the direct evaluator-level status-252 witness for the
# canonical Epsilon evaluator edge. Keep it separate from the routine
# evaluator-entry gate. A per-case watchdog defaults to 7,200 seconds:
#
#   sh tests/epsilon/pair-boundary/run.sh
#   sh tests/epsilon/pair-boundary/run.sh --case writes_exhaustion.epsilon
#   OMEGA_EPSILON_PAIR_SECONDS=10800 sh tests/epsilon/pair-boundary/run.sh

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
GATE_LABEL="Epsilon pair boundary"
GATE_ENVS="delta/compiler_env.sh epsilon/evaluator_env.sh"
. "$GATE_DIR/../../gate_env.sh"
. "$GATE_DIR/../../gate_host.sh"

PAIR_TMP=$(mktemp -d)
trap 'rm -rf -- "$PAIR_TMP"' EXIT HUP INT TERM
# Bound materializers refuse before writing when the canonical manifest,
# members, packed closure, entry source, or composed record differ from the
# audited edge records.
materialize_epsilon_evaluator "$PAIR_TMP/epsilon_compiler.delta"
materialize_delta_compiler "$PAIR_TMP/delta_compiler.gamma"
materialize_delta_support "$PAIR_TMP/support.bin"
materialize_gamma_evaluator "$PAIR_TMP/evaluator.exe" >/dev/null
require_epsilon_evaluator_entry_identity
cp "${OMEGA_PATH_EPSILON_EVALUATOR_ENTRY:-$OMEGA_REPO_ROOT/tests/epsilon/evaluator-entry/evaluator_entry.delta}" \
    "$PAIR_TMP/evaluator_entry.delta"

python3 -B "$GATE_DIR/gate.py" "$PAIR_TMP" "$@"
