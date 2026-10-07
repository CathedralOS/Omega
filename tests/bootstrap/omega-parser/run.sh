#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
GATE_LABEL="Interpreted D parser"
. "$GATE_DIR/../gate_preamble.sh"

PARSER_TMP=$(mktemp -d)
trap 'rm -rf -- "$PARSER_TMP"' EXIT HUP INT TERM
# gate.py runs under a Windows Python on the MINGW/MSYS route, and a Windows
# Python cannot resolve MSYS virtual paths ("/tmp/...", "/c/..."). Translate
# the interpreter-facing paths to Windows form when the shell ships cygpath;
# POSIX hosts have no cygpath and keep the native paths unchanged. The same
# route drives the materialize_* helpers' python3 calls below, so a PATH shim
# translates every argument of every python3 invocation — the shim covers the
# sourced helpers' script, manifest, destination, and prefix paths without the
# shared env files knowing the host form.
GATE_PY=$GATE_DIR/gate.py
PARSER_TMP_ARG=$PARSER_TMP
DRIVER_ARG=$OMEGA_PATH_EPSILON_EXECUTION_DRIVER
if command -v cygpath >/dev/null 2>&1; then
    GATE_PY=$(cygpath -w "$GATE_PY")
    PARSER_TMP_ARG=$(cygpath -w "$PARSER_TMP_ARG")
    DRIVER_ARG=$(cygpath -w "$DRIVER_ARG")
    install_mingw_python3_shim "$PARSER_TMP/python3-shim"
fi
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
    python3 "$GATE_PY" --identity "$PARSER_TMP_ARG" "$DRIVER_ARG"
else
    python3 "$GATE_PY" "$PARSER_TMP_ARG" "$DRIVER_ARG"
fi
