#!/usr/bin/env sh
# Diagnostic observation only; outputs stay outside the source closures.
set -eu
if [ "$#" -lt 1 ] || [ "$#" -gt 2 ] || { [ "$#" -eq 2 ] && [ "$2" != --prepare-only ]; }; then
    echo "usage: sh tests/bootstrap/omega-parser/measure.sh ABSOLUTE_RESULT_DIRECTORY [--prepare-only]" >&2
    exit 2
fi
GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
GATE_LABEL="D layer-0 measurement"
GATE_ENVS="alpha/seed_env.sh delta/compiler_env.sh epsilon/evaluator_env.sh omega/compiler_env.sh"
. "$GATE_DIR/../../gate_env.sh"
. "$GATE_DIR/../../gate_host.sh"
case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) ;;
    *) echo "$GATE_LABEL: use Linux x86-64 (Windows: WSL; macOS: Linux guest)" >&2; exit 2 ;;
esac
MEASUREMENT_DIR=$1
if [ -e "$MEASUREMENT_DIR" ]; then
    echo "$GATE_LABEL: use a fresh result directory" >&2
    exit 2
fi
mkdir -p "$MEASUREMENT_DIR"
materialize_delta_compiler "$MEASUREMENT_DIR/delta_compiler.gamma"
materialize_delta_support "$MEASUREMENT_DIR/support.bin"
materialize_epsilon_evaluator "$MEASUREMENT_DIR/epsilon_compiler.delta"
require_epsilon_evaluator_entry_identity
cp "$OMEGA_PATH_EPSILON_EVALUATOR_ENTRY" "$MEASUREMENT_DIR/evaluator_entry.delta"
cp "$OMEGA_PATH_EPSILON_REFINEMENT_MODEL" "$MEASUREMENT_DIR/epsilon_model.py"
materialize_omega_compiler "$MEASUREMENT_DIR/omega_compiler.epsilon"
materialize_gamma_evaluator "$MEASUREMENT_DIR/evaluator.exe"
python3 "$GATE_DIR/measure.py" prepare "$MEASUREMENT_DIR"
if [ "${2:-}" = --prepare-only ]; then exit 0; fi
python3 "$GATE_DIR/measure.py" receipt "$MEASUREMENT_DIR"
python3 "$GATE_DIR/measure.py" selfcheck "$MEASUREMENT_DIR"
python3 "$GATE_DIR/measure.py" cases "$MEASUREMENT_DIR"
