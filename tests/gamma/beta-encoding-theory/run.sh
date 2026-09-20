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
    *) echo "usage: run.sh [--subject-shape|--counter-cost|--full-subject]" >&2; exit 2 ;;
esac
[ "$#" -le 1 ] || { echo "usage: run.sh [--subject-shape|--counter-cost|--full-subject]" >&2; exit 2; }

command -v python3 >/dev/null 2>&1 || {
    echo "Beta encoding theory: skipped (python3 absent)"
    exit 0
}
if [ "$ENCODING_GATE" = "full_subject.py" ]; then
    # The stepper production runs host-side: no evaluator seed is
    # materialized, so this mode is not bound to the native hosts.  The
    # theory and subject identities are the same pins the native gates use.
    require_beta_encoding_theory_identity
    require_gamma_evaluator_identity
    exec python3 -B "$GATE_DIR/$ENCODING_GATE"
fi
case "$(uname -s)-$(uname -m)" in
    Darwin-arm64|MINGW*-x86_64|MSYS*-x86_64) ;;
    *) echo "Beta encoding theory: unsupported host; needs macOS arm64 or Windows x64" >&2
       exit 2 ;;
esac

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
