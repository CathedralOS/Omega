#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
GATE_LABEL="Omega request"
. "$GATE_DIR/../gate_preamble.sh"

OUTPUT_DIR=${OMEGA_REQUEST_BUILD_DIR:-"$OMEGA_REPO_ROOT/build/omega-request"}
mkdir -p "$OUTPUT_DIR"
# Bound materializers refuse before writing when the canonical entry,
# manifest, members, packed closure, or composed record differ from the
# audited edge records.
materialize_delta_compiler "$OUTPUT_DIR/delta_compiler.gamma"
materialize_delta_support "$OUTPUT_DIR/support.bin"
materialize_epsilon_evaluator "$OUTPUT_DIR/epsilon_compiler.delta"
materialize_omega_compiler "$OUTPUT_DIR/omega_compiler.epsilon"
require_omega_request_entry_identity
require_omega_request_fixture_identity
materialize_gamma_evaluator "$OUTPUT_DIR/evaluator.exe" >/dev/null
# gate.py runs under a Windows Python on the MINGW/MSYS route, and a Windows
# Python cannot resolve MSYS virtual paths ("/tmp/...", "/c/..."). Translate
# the interpreter-facing paths to Windows form when the shell ships cygpath;
# POSIX hosts have no cygpath and keep the native paths unchanged.
GATE_PY=$GATE_DIR/gate.py
OUTPUT_DIR_ARG=$OUTPUT_DIR
DRIVER_ARG=$OMEGA_PATH_EPSILON_EXECUTION_DRIVER
if command -v cygpath >/dev/null 2>&1; then
    GATE_PY=$(cygpath -w "$GATE_PY")
    OUTPUT_DIR_ARG=$(cygpath -w "$OUTPUT_DIR_ARG")
    DRIVER_ARG=$(cygpath -w "$DRIVER_ARG")
fi
if [ "$IDENTITY_ONLY" = 1 ]; then
    python3 "$GATE_PY" --identity "$OUTPUT_DIR_ARG" "$DRIVER_ARG"
else
    python3 "$GATE_PY" "$OUTPUT_DIR_ARG" "$DRIVER_ARG"
fi
