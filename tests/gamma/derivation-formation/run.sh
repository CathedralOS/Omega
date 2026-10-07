#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
GATE_LABEL="Derivation formation"
GATE_ENVS="alpha/seed_env.sh gamma/evaluator_env.sh proofs/sources_env.sh"
. "$GATE_DIR/../../gate_env.sh"
. "$GATE_DIR/../../gate_host.sh"


FORMATION_TMP=$(mktemp -d)
trap 'rm -rf -- "$FORMATION_TMP"' EXIT HUP INT TERM
# The bound member closure is checked against its audited record; the gate's
# bound diagnostic prefix entry packs on top of those bound members.
require_derivation_checker_identity
require_derivation_formation_entry_identity
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DERIVATION_CHECKER_SOURCES" \
    "$FORMATION_TMP/diagnostic.gamma" --prefix "$GATE_DIR/main.gamma"
materialize_gamma_evaluator "$FORMATION_TMP/evaluator" >/dev/null
python3 -B "$GATE_DIR/gate.py" "$FORMATION_TMP"
