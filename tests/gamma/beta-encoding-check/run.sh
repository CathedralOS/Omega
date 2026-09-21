#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/proofs/sources_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Beta encoding certificate check: skipped (python3 absent)"
    exit 0
}

REFERENCE_VM=
if [ "${1:-}" = "--reference-vm" ]; then
    REFERENCE_VM=1
    shift
fi
[ "$#" -eq 0 ] || { echo "usage: run.sh [--reference-vm]" >&2; exit 2; }

CHECK_TMP=$(mktemp -d)
trap 'rm -rf -- "$CHECK_TMP"' EXIT HUP INT TERM
# The same audited packings the theory and checking gates use: the emitter's
# bound closure produces the theory section inside the evaluator, and the
# checker's bound closure carries the explicit diagnostic entry.
require_beta_encoding_theory_identity
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_BETA_ENCODING_SOURCES" \
    "$CHECK_TMP/producer.gamma" --prefix "$GATE_DIR/../beta-encoding-theory/main.gamma"
require_derivation_checker_identity
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DERIVATION_CHECKER_SOURCES" \
    "$CHECK_TMP/checker.gamma" \
    --prefix "$OMEGA_REPO_ROOT/tests/gamma/derivation-checking/main.gamma"

if [ -n "$REFERENCE_VM" ]; then
    # Diagnostic leg: drive the selected evaluator tape through the gate-local
    # host-instrumentation VM (alpha_vm.c) instead of a stamped audited seed.
    # This runs on any host with a C compiler and python3; its observation is
    # a reference reading for the profile record, never artifact admission —
    # the seed-mediated run remains the admission route.
    command -v "${CC:-cc}" >/dev/null 2>&1 || {
        echo "Beta encoding certificate check: skipped (C compiler absent)" >&2
        exit 0
    }
    require_gamma_evaluator_identity
    "${CC:-cc}" -O2 -o "$CHECK_TMP/alpha_vm" "$GATE_DIR/alpha_vm.c"
    cat > "$CHECK_TMP/evaluator" <<EOF
#!/bin/sh
exec "$CHECK_TMP/alpha_vm" "$OMEGA_PATH_GAMMA_EVALUATOR_TAPE"
EOF
    chmod +x "$CHECK_TMP/evaluator"
    echo "Beta encoding certificate check: reference VM (diagnostic, not admission)" >&2
else
    require_seed_execution_host "Beta encoding certificate check"
    materialize_gamma_evaluator "$CHECK_TMP/evaluator" >/dev/null
fi
python3 -B "$GATE_DIR/check.py" "$CHECK_TMP"
