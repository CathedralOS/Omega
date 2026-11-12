#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
GATE_ENVS="alpha/seed_env.sh beta/artifact_env.sh delta/compiler_env.sh epsilon/evaluator_env.sh omega/compiler_env.sh"
. "$GATE_DIR/../../gate_env.sh"

# --identity is a lone flag: it materializes every bound artifact and checks
# every wrapper-side identity without executing the seed, so any Python-3
# host (including a Windows host before the multi-hour run) can check the
# gate's non-executing surface. Every other argument belongs to gate.py and
# passes through unchanged; the default run still requires a seed host.
IDENTITY_ONLY=0
if [ "$#" -eq 1 ] && [ "$1" = --identity ]; then
    IDENTITY_ONLY=1
fi

command -v python3 >/dev/null 2>&1 || {
    echo "Omega executable: skipped (python3 absent)"
    exit 0
}

if [ "$IDENTITY_ONLY" = 0 ]; then
    require_seed_execution_host "Omega executable"
fi

OUTPUT_DIR=${OMEGA_EXECUTABLE_BUILD_DIR:-"$OMEGA_REPO_ROOT/build/omega-executable"}
mkdir -p "$OUTPUT_DIR"
# gate.py runs under a Windows Python on the MINGW/MSYS route, and a Windows
# Python cannot resolve MSYS virtual paths ("/c/...", "/tmp/..."). Translate
# the interpreter-facing paths to Windows form when the shell ships cygpath;
# POSIX hosts have no cygpath and keep the native paths unchanged. The same
# route drives the materialize_* helpers' python3 calls below, so a PATH shim
# translates every path-shaped argument of every python3 invocation — the shim
# covers the sourced helpers' script, manifest, destination, and prefix paths
# without the shared env files knowing the host form.
GATE_PY=$GATE_DIR/gate.py
OUTPUT_DIR_ARG=$OUTPUT_DIR
DRIVER_ARG=$OMEGA_PATH_EPSILON_EXECUTION_DRIVER
if command -v cygpath >/dev/null 2>&1; then
    GATE_PY=$(cygpath -w "$GATE_PY")
    OUTPUT_DIR_ARG=$(cygpath -w "$OUTPUT_DIR_ARG")
    DRIVER_ARG=$(cygpath -w "$DRIVER_ARG")
    EXECUTABLE_SHIM_TMP=$(mktemp -d)
    trap 'rm -rf -- "$EXECUTABLE_SHIM_TMP"' EXIT HUP INT TERM
    install_mingw_python3_shim "$EXECUTABLE_SHIM_TMP/python3-shim"
fi
# Bound materializers refuse before writing when the canonical entry,
# manifest, members, packed closure, or composed record differ from the
# audited edge records.
materialize_delta_compiler "$OUTPUT_DIR/delta_compiler.gamma"
materialize_delta_support "$OUTPUT_DIR/support.bin"
materialize_epsilon_evaluator "$OUTPUT_DIR/epsilon_compiler.delta"
materialize_omega_compiler "$OUTPUT_DIR/omega_compiler.epsilon"
require_omega_executable_entries_identity
require_beta_compiler_identity
if [ "$IDENTITY_ONLY" = 1 ]; then
    materialize_gamma_evaluator "$OUTPUT_DIR/evaluator.exe" >/dev/null
    # The executing path delegates its remaining pins to gate.py; this leg
    # covers the execution-adapter record through its shared env-file
    # identity instead.
    require_epsilon_execution_driver_identity
    echo "Omega executable: identity legs green; execution legs need a seed host"
    exit 0
fi
# Start from the audited Alpha container and admitted Beta tape. Successor
# execution uses the constructed bytes, with every native status checked by
# set -e before comparison or publication as the next executable.
materialize_beta_compiler "$OUTPUT_DIR/beta-seed.exe" >/dev/null
"$OUTPUT_DIR/beta-seed.exe" < "$OMEGA_PATH_BETA_COMPILER_SOURCE" > "$OUTPUT_DIR/beta.tape"
cmp "$OUTPUT_DIR/beta.tape" "$OMEGA_PATH_BETA_COMPILER_TAPE"
stamp_seed "$OUTPUT_DIR/beta.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$OUTPUT_DIR/beta.exe" >/dev/null
require_gamma_evaluator_identity
"$OUTPUT_DIR/beta.exe" < "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" > "$OUTPUT_DIR/gamma.tape"
cmp "$OUTPUT_DIR/gamma.tape" "$OMEGA_PATH_GAMMA_EVALUATOR_TAPE"
stamp_seed "$OUTPUT_DIR/gamma.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$OUTPUT_DIR/evaluator.exe" >/dev/null
echo "Beta self-reconstruction and Gamma source construction: byte-exact"
if [ -n "${EXECUTABLE_SHIM_TMP:-}" ]; then
    # Windows needs the path shim until Python finishes. Keep EXIT cleanup and
    # exit within this parsed branch instead of resuming the script afterward.
    python3 "$GATE_PY" "$OUTPUT_DIR_ARG" "$DRIVER_ARG" "$@"
    exit $?
fi
exec python3 "$GATE_PY" "$OUTPUT_DIR_ARG" "$DRIVER_ARG" "$@"
