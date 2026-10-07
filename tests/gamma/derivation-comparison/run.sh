#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
GATE_LABEL="Derivation comparison"
. "$GATE_DIR/../gate_env.sh"


COMPARISON_TMP=$(mktemp -d)
trap 'rm -rf -- "$COMPARISON_TMP"' EXIT HUP INT TERM
# The bound materializer refuses before writing when the canonical manifest,
# members, or packed member closure differ from the audited proof record.
# The gate's bound diagnostic prefix and per-vector entries pack on top of
# those bound members.
materialize_derivation_checker "$COMPARISON_TMP/implementation.gamma"
require_derivation_comparison_prefixes_identity
materialize_gamma_evaluator "$COMPARISON_TMP/evaluator" >/dev/null
python3 -B "$GATE_DIR/gate.py" "$COMPARISON_TMP"
