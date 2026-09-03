#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/artifact_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Direct Beta feasibility experiment: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
GAMMA1_LOWERER="$OMEGA_REPO_ROOT/tests/gamma/gamma1-augmentation-experiment/lowerer.gamma"
SEED_SOURCE="$OMEGA_REPO_ROOT/tests/delta/streaming-compiler-experiment/compiler.gamma"
HAND_EVALUATOR="$OMEGA_PATH_CONCATENATIVE_GAMMA_EVALUATOR_SOURCE"

materialize_beta_compiler "$TMP/beta" >/dev/null
materialize_gamma_compiler "$TMP/gamma" >/dev/null
compile_gamma_source_to_tape "$TMP/gamma" "$TMP/beta" \
    "$GAMMA1_LOWERER" "$TMP/gamma1.tape"
stamp_seed "$TMP/gamma1.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/gamma1" >/dev/null
"$TMP/gamma1" < "$SEED_SOURCE" > "$TMP/seed.gamma"
"$TMP/gamma" < "$TMP/seed.gamma" > "$TMP/seed.beta"
"$TMP/beta" < "$TMP/seed.beta" > "$TMP/seed.tape"

MEASUREMENTS=$(CDPATH= cd -- "$OMEGA_REPO_ROOT" && \
    python3 "$GATE_DIR/measure.py" "$TMP/seed.beta" "$HAND_EVALUATOR")
EXPECTED='generated_lines=3230
generated_instructions=2842
generated_calls=1678
generated_call_fraction=0.590
generated_tokenizer_instructions=168
generated_tokenizer_calls=88
hand_tokenizer_instructions=38
hand_tokenizer_calls=0
tokenizer_instruction_ratio=4.421
cell_helper_instructions=184
text_helper_instructions=350'

[ "$MEASUREMENTS" = "$EXPECTED" ] || {
    printf '%s\n' "$MEASUREMENTS"
    echo "Direct Beta feasibility measurements changed"
    exit 1
}
[ "$(wc -c < "$TMP/seed.beta" | tr -d ' ')" -eq 79175 ]
[ "$(wc -c < "$TMP/seed.tape" | tr -d ' ')" -eq 22762 ]

echo "Direct Beta feasibility: generated tokenizer is 4.421x hand Beta; seed is 59.0% calls"
