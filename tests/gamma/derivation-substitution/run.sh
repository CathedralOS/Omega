#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/proofs/sources_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Derivation substitution: skipped (python3 absent)"
    exit 0
}
case "$(uname -s)-$(uname -m)" in
    Darwin-arm64|MINGW*-x86_64|MSYS*-x86_64) ;;
    *) echo "Derivation substitution: unsupported host; needs macOS arm64 or Windows x64" >&2
       exit 2 ;;
esac

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
