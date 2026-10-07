#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
GATE_LABEL="Epsilon checking invariants"
GATE_ENVS="delta/compiler_env.sh epsilon/evaluator_env.sh"
. "$GATE_DIR/../../gate_env.sh"
. "$GATE_DIR/../../gate_host.sh"

CHECKING_TMP=$(mktemp -d)
trap 'rm -rf -- "$CHECKING_TMP"' EXIT HUP INT TERM
# Bound materializers refuse before writing when the canonical manifest,
# members, or packed closure differ from the audited edge records. The
# gate-local controls closure is a diagnostic artifact packed directly.
materialize_epsilon_evaluator "$CHECKING_TMP/epsilon_compiler.delta"
materialize_delta_compiler "$CHECKING_TMP/delta_compiler.gamma"
materialize_delta_support "$CHECKING_TMP/support.bin"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$GATE_DIR/checking_invariants.delta.sources" "$CHECKING_TMP/controls.delta"
materialize_gamma_evaluator "$CHECKING_TMP/evaluator" >/dev/null
python3 "$GATE_DIR/gate.py" "$CHECKING_TMP"
