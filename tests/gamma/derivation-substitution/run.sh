#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
GATE_LABEL="Derivation substitution"
GATE_ENVS="alpha/seed_env.sh gamma/evaluator_env.sh proofs/sources_env.sh"
. "$GATE_DIR/../../gate_env.sh"
. "$GATE_DIR/../../gate_host.sh"


SUBSTITUTION_TMP=$(mktemp -d)
trap 'rm -rf -- "$SUBSTITUTION_TMP"' EXIT HUP INT TERM
# The bound materializer refuses before writing when the canonical manifest,
# members, or packed member closure differ from the audited proof record.
# The gate's bound diagnostic prefix and per-vector entries pack on top of
# those bound members.
materialize_derivation_checker "$SUBSTITUTION_TMP/implementation.gamma"
require_derivation_substitution_prefixes_identity
materialize_gamma_evaluator "$SUBSTITUTION_TMP/evaluator" >/dev/null
python3 -B "$GATE_DIR/gate.py" "$SUBSTITUTION_TMP"
