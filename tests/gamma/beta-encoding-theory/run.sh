#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/proofs/sources_env.sh"

case "${1:-}" in
    '') ENCODING_GATE=gate.py ;;
    --subject-shape) ENCODING_GATE=subject_shape.py ;;
    --counter-cost) ENCODING_GATE=counter_cost.py ;;
    --full-subject) ENCODING_GATE=full_subject.py ;;
    --mutations) ENCODING_GATE=mutations.py ;;
    --mutations-self-test) ENCODING_GATE=mutations.py; MUTATION_MODE=--self-test ;;
    *) echo "usage: run.sh [--subject-shape|--counter-cost|--full-subject|--mutations|--mutations-self-test]" >&2; exit 2 ;;
esac
[ "$#" -le 1 ] || { echo "usage: run.sh [--subject-shape|--counter-cost|--full-subject|--mutations|--mutations-self-test]" >&2; exit 2; }

command -v python3 >/dev/null 2>&1 || {
    echo "Beta encoding theory: skipped (python3 absent)"
    exit 0
}
if [ "$ENCODING_GATE" = "full_subject.py" ] || [ "${MUTATION_MODE:-}" = "--self-test" ]; then
    # The stepper production and mutation construction run host-side: no
    # evaluator seed is materialized, so these modes are not bound to the
    # native hosts.  The theory and subject identities are the same pins
    # the native gates use.  --mutations-self-test builds every mutated
    # request and verifies each patch lands at its claimed field without
    # asserting any checker verdict.
    require_beta_encoding_theory_identity
    require_gamma_evaluator_identity
    exec python3 -B "$GATE_DIR/$ENCODING_GATE" ${MUTATION_MODE:-}
fi
require_seed_execution_host "Beta encoding theory"

ENCODING_TMP=$(mktemp -d)
trap 'rm -rf -- "$ENCODING_TMP"' EXIT HUP INT TERM
# The bound member closures are checked against their audited records; the
# gate's own diagnostic prefix entries pack on top of those bound members.
require_beta_encoding_theory_identity
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_BETA_ENCODING_SOURCES" \
    "$ENCODING_TMP/producer.gamma" --prefix "$GATE_DIR/main.gamma"
require_derivation_checker_identity
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DERIVATION_CHECKER_SOURCES" \
    "$ENCODING_TMP/checker.gamma" \
    --prefix "$OMEGA_REPO_ROOT/tests/gamma/derivation-checking/main.gamma"
materialize_gamma_evaluator "$ENCODING_TMP/evaluator" >/dev/null
python3 -B "$GATE_DIR/$ENCODING_GATE" "$ENCODING_TMP"
