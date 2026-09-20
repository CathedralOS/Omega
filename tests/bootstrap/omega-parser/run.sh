#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/delta/compiler_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/evaluator_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/omega/compiler_env.sh"

IDENTITY_ONLY=0
if [ "$#" -gt 1 ] || { [ "$#" -eq 1 ] && [ "$1" != --identity ]; }; then
    echo "usage: $0 [--identity]" >&2
    exit 2
fi
if [ "$#" -eq 1 ]; then
    IDENTITY_ONLY=1
fi

command -v python3 >/dev/null 2>&1 || {
    echo "Interpreted D parser: skipped (python3 absent)"
    exit 0
}

# --identity validates every bound identity and assembles both byte streams
# without executing the seed, so any Python-3 host (including Linux, or a
# Windows host before the multi-hour run) can check the gate's whole
# non-executing surface. The default run still requires a seed host.
if [ "$IDENTITY_ONLY" = 0 ]; then
    require_seed_execution_host "Interpreted D parser"
fi

PARSER_TMP=$(mktemp -d)
trap 'rm -rf -- "$PARSER_TMP"' EXIT HUP INT TERM
# Bound materializers refuse before writing when the canonical entry,
# manifest, members, packed closure, or composed record differ from the
# audited edge records.
materialize_delta_compiler "$PARSER_TMP/delta_compiler.gamma"
materialize_delta_support "$PARSER_TMP/support.bin"
materialize_epsilon_evaluator "$PARSER_TMP/epsilon_compiler.delta"
materialize_omega_compiler "$PARSER_TMP/omega_compiler.epsilon"
require_omega_parser_entry_identity
materialize_gamma_evaluator "$PARSER_TMP/evaluator.exe" >/dev/null
if [ "$IDENTITY_ONLY" = 1 ]; then
    python3 "$GATE_DIR/gate.py" --identity "$PARSER_TMP" "$OMEGA_PATH_EPSILON_EXECUTION_DRIVER"
else
    python3 "$GATE_DIR/gate.py" "$PARSER_TMP" "$OMEGA_PATH_EPSILON_EXECUTION_DRIVER"
fi
