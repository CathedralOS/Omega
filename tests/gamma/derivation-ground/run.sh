#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
GATE_LABEL="Derivation ground"
. "$GATE_DIR/../gate_env.sh"


GROUND_TMP=$(mktemp -d)
trap 'rm -rf -- "$GROUND_TMP"' EXIT HUP INT TERM
# The bound member closure is checked against its audited record; the gate's
# bound diagnostic prefix entry packs on top of those bound members.
require_derivation_checker_identity
require_derivation_ground_entry_identity
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DERIVATION_CHECKER_SOURCES" \
    "$GROUND_TMP/diagnostic.gamma" --prefix "$GATE_DIR/main.gamma"
materialize_gamma_evaluator "$GROUND_TMP/evaluator" >/dev/null
python3 -B "$GATE_DIR/gate.py" "$GROUND_TMP"
