#!/usr/bin/env sh
# Q7 canonical semantic-ledger Gamma spike.
#
# The typed Gamma program decodes current PSITERM bytes, constructs and audits
# the bounded semantic ledger, and is evaluated by both the canonical
# Beta-written interpreter and the independent Python oracle.
set -eu
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?
cd "$OMEGA_GATE_DIR"
. "${OMEGA_PATH_ALPHA}"/seed_env.sh

SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_BETA_ASSEMBLER}"/$BETA_SEED
FIXTURES="${OMEGA_PATH_PSI_PRODUCT}"/semantics/psi-terminal-codec/tests/fixtures
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

( cd "${OMEGA_PATH_BETA_RUST}" && sh build.sh "${OMEGA_PATH_BETA}"/bc.beta >/dev/null ) || {
  echo "terminal ledger spike: bc build failed" >&2
  exit 1
}

build_beta_program() {
  source=$1
  output=$2
  "${OMEGA_PATH_BETA_RUST}"/build/bc.exe < "$source" > "$T/program.asm"
  "$ASM" < "$T/program.asm" > "$T/program.tape"
  stamp_seed "$T/program.tape" "$SEED" "$output" >/dev/null 2>&1
}

build_beta_program typeck.beta "$T/typeck.exe"
build_beta_program interp.beta "$T/interp.exe"

cat canonical-bytes/types.gamma \
    terminal-codec-primitives/types.gamma \
    terminal-ledger-spike/types.gamma \
    canonical-bytes/decode.gamma \
    terminal-codec-primitives/header.gamma \
    terminal-codec-primitives/scalars.gamma \
    terminal-codec-primitives/semantic_ids.gamma \
    terminal-codec-primitives/scalar_types.gamma \
    terminal-codec-primitives/integer_values.gamma \
    terminal-codec-primitives/utf8.gamma \
    terminal-ledger-spike/scalar_types.gamma \
    terminal-ledger-spike/decode.gamma \
    terminal-ledger-spike/schema.gamma \
    terminal-ledger-spike/call_composition.gamma \
    terminal-ledger-spike/ledger.gamma \
    terminal-ledger-spike/structural_effect_decode.gamma \
    terminal-ledger-spike/structural_effect.gamma \
    terminal-ledger-spike/call_decode.gamma \
    terminal-ledger-spike/call_ledger.gamma > "$T/typed.gamma"

set +e
"$T/typeck.exe" < "$T/typed.gamma"
type_status=$?
set -e
if [ "$type_status" != 1 ]; then
  echo "terminal ledger spike: typed Gamma program was rejected (status $type_status)" >&2
  exit 1
fi

python3 erase_types.py < "$T/typed.gamma" > "$T/spike.gamma"

fixture_expr() {
  python3 terminal-ledger-spike/bytes_to_gamma.py "$@"
}

fixture_expr "$FIXTURES/terminal_ledger_spike.hex" > "$T/matching.expr"
fixture_expr "$FIXTURES/terminal_ledger_spike_asymmetric.hex" > "$T/asymmetric.expr"
fixture_expr "$FIXTURES/terminal_ledger_structural_effect.hex" > "$T/structural-effect.expr"
fixture_expr "$FIXTURES/terminal_ledger_call_composition.hex" > "$T/call-composition.expr"
fixture_expr "$FIXTURES/terminal_ledger_call_composition.hex" --set-byte 36 0 \
  > "$T/call-type-identity-drift.expr"
fixture_expr "$FIXTURES/terminal_ledger_call_composition.hex" --set-byte 96 11 \
  > "$T/call-domain-carrier-drift.expr"
fixture_expr "$FIXTURES/terminal_ledger_call_composition.hex" --set-byte 203 11 \
  > "$T/call-boundary-requirement-drift.expr"
fixture_expr "$FIXTURES/terminal_ledger_call_composition.hex" --set-byte 395 21 \
  > "$T/call-unit-target-drift.expr"
fixture_expr "$FIXTURES/terminal_ledger_call_composition.hex" --set-byte 407 20 \
  > "$T/call-unit-argument-drift.expr"
fixture_expr "$FIXTURES/terminal_ledger_call_composition.hex" --set-byte 423 2 \
  > "$T/call-unit-transfer-drift.expr"
fixture_expr "$FIXTURES/terminal_ledger_call_composition.hex" --set-byte 620 11 \
  > "$T/call-boundary-target-drift.expr"
fixture_expr "$FIXTURES/terminal_ledger_call_composition.hex" --set-byte 632 10 \
  > "$T/call-boundary-argument-drift.expr"
fixture_expr "$FIXTURES/terminal_ledger_call_composition.hex" --set-byte 648 2 \
  > "$T/call-boundary-receipt-drift.expr"
fixture_expr "$FIXTURES/terminal_ledger_call_composition.hex" --set-byte 656 1 \
  > "$T/call-boundary-receipt-index-drift.expr"
fixture_expr "$FIXTURES/terminal_ledger_call_composition.hex" --drop-last \
  > "$T/call-truncated.expr"
fixture_expr "$FIXTURES/terminal_ledger_call_composition.hex" --append-byte 0 \
  > "$T/call-trailing.expr"
fixture_expr "$FIXTURES/terminal_ledger_structural_effect.hex" --set-byte 74 0 \
  > "$T/structural-erased-field.expr"
fixture_expr "$FIXTURES/terminal_ledger_structural_effect.hex" --set-byte 36 128 \
  > "$T/structural-invalid-utf8.expr"
fixture_expr "$FIXTURES/terminal_ledger_structural_effect.hex" --set-byte 338 2 \
  > "$T/structural-field-drift.expr"
fixture_expr "$FIXTURES/terminal_ledger_structural_effect.hex" --set-byte 356 2 \
  > "$T/structural-service-drift.expr"
fixture_expr "$FIXTURES/terminal_ledger_structural_effect.hex" --set-byte 364 249 \
  > "$T/structural-port-drift.expr"
fixture_expr "$FIXTURES/terminal_ledger_structural_effect.hex" --set-byte 405 2 \
  > "$T/structural-cleanup-drift.expr"
fixture_expr "$FIXTURES/terminal_ledger_structural_effect.hex" --set-byte 539 1 \
  > "$T/structural-establish-target-drift.expr"
fixture_expr "$FIXTURES/terminal_ledger_structural_effect.hex" --set-byte 556 0 \
  > "$T/structural-missing-discard.expr"
fixture_expr "$FIXTURES/terminal_ledger_spike.hex" --set-byte 0 0 > "$T/bad-magic.expr"
fixture_expr "$FIXTURES/terminal_ledger_spike.hex" --set-byte 8 10 > "$T/bad-format.expr"
fixture_expr "$FIXTURES/terminal_ledger_spike.hex" --set-byte 10 15 \
  > "$T/bad-vocabulary.expr"
fixture_expr "$FIXTURES/terminal_ledger_spike.hex" --drop-last > "$T/truncated.expr"
fixture_expr "$FIXTURES/terminal_ledger_spike.hex" --append-byte 0 > "$T/trailing.expr"
fixture_expr "$FIXTURES/terminal_ledger_spike.hex" --set-byte 195 10 > "$T/duplicate-result.expr"
fixture_expr "$FIXTURES/terminal_ledger_spike.hex" --set-byte 1216 2 > "$T/invalid-boolean.expr"
fixture_expr "$FIXTURES/terminal_ledger_spike.hex" --set-byte 1953 2 \
  > "$T/unsigned-i8-payload.expr"
fixture_expr "$FIXTURES/terminal_ledger_spike.hex" --set-byte 1956 0 \
  > "$T/invalid-i8-sign-extension.expr"
fixture_expr "$FIXTURES/terminal_ledger_spike.hex" --set-byte 1236 41 > "$T/boolean-type-drift.expr"
fixture_expr "$FIXTURES/terminal_ledger_spike.hex" --set-byte 1368 8 > "$T/widen-result-drift.expr"
fixture_expr "$FIXTURES/terminal_ledger_spike.hex" --set-byte 1401 10 > "$T/cast-operand-drift.expr"
fixture_expr "$FIXTURES/terminal_ledger_spike.hex" --set-byte 1409 0 > "$T/missing-cast-obligation.expr"

make_program() {
  function=$1
  expression=$2
  cat "$T/spike.gamma" > "$T/run.gamma"
  printf '\n(%s ' "$function" >> "$T/run.gamma"
  cat "$expression" >> "$T/run.gamma"
  printf ')\n' >> "$T/run.gamma"
}

run_function() {
  name=$1
  function=$2
  expression=$3
  expected=$4
  make_program "$function" "$expression"

  set +e
  beta_output=$("$T/interp.exe" < "$T/run.gamma")
  beta_status=$?
  python_output=$(python3 gamma_ref.py < "$T/run.gamma")
  python_status=$?
  set -e

  if [ "$beta_status" != "$expected" ] || [ "$beta_output" != "$expected" ]; then
    echo "terminal ledger spike: $name Beta result was '$beta_output'/$beta_status, expected $expected" >&2
    exit 1
  fi
  if [ "$python_status" != "$expected" ] || [ "$python_output" != "$expected" ]; then
    echo "terminal ledger spike: $name Python result was '$python_output'/$python_status, expected $expected" >&2
    exit 1
  fi
  echo "terminal ledger spike: $name -> $expected (Beta/Python agree)"
}

run_function matching run_spike "$T/matching.expr" 1
run_function schema-mutations schema_mutation_self_test "$T/matching.expr" 1
run_function call-composition call_schema_mutation_self_test "$T/matching.expr" 1
run_function asymmetric-join run_spike "$T/asymmetric.expr" 0
run_function bad-magic run_spike "$T/bad-magic.expr" 0
run_function bad-format run_spike "$T/bad-format.expr" 0
run_function bad-vocabulary run_spike "$T/bad-vocabulary.expr" 0
run_function truncated run_spike "$T/truncated.expr" 0
run_function trailing-byte run_spike "$T/trailing.expr" 0
run_function duplicate-result run_spike "$T/duplicate-result.expr" 0
run_function invalid-boolean run_spike "$T/invalid-boolean.expr" 0
run_function unsigned-i8-payload run_spike "$T/unsigned-i8-payload.expr" 0
run_function invalid-i8-sign-extension run_spike "$T/invalid-i8-sign-extension.expr" 0
run_function boolean-type-drift run_spike "$T/boolean-type-drift.expr" 0
run_function widen-result-drift run_spike "$T/widen-result-drift.expr" 0
run_function cast-operand-drift run_spike "$T/cast-operand-drift.expr" 0
run_function missing-cast-obligation run_spike "$T/missing-cast-obligation.expr" 0
run_function structural-effect run_structural_effect_spike "$T/structural-effect.expr" 1
run_function structural-effect-schema-mutations \
  structural_effect_schema_mutation_self_test "$T/structural-effect.expr" 1
run_function structural-invalid-utf8 run_structural_effect_spike \
  "$T/structural-invalid-utf8.expr" 0
run_function structural-erased-field run_structural_effect_spike \
  "$T/structural-erased-field.expr" 0
run_function structural-field-drift run_structural_effect_spike \
  "$T/structural-field-drift.expr" 0
run_function structural-service-drift run_structural_effect_spike \
  "$T/structural-service-drift.expr" 0
run_function structural-port-drift run_structural_effect_spike \
  "$T/structural-port-drift.expr" 0
run_function structural-cleanup-drift run_structural_effect_spike \
  "$T/structural-cleanup-drift.expr" 0
run_function structural-establish-target-drift run_structural_effect_spike \
  "$T/structural-establish-target-drift.expr" 0
run_function structural-missing-discard run_structural_effect_spike \
  "$T/structural-missing-discard.expr" 0
run_function call-composition-bytes run_call_composition_spike \
  "$T/call-composition.expr" 1
run_function call-composition-byte-schema-mutations \
  call_composition_byte_schema_mutation_self_test "$T/call-composition.expr" 1
run_function call-type-identity-drift run_call_composition_spike \
  "$T/call-type-identity-drift.expr" 0
run_function call-domain-carrier-drift run_call_composition_spike \
  "$T/call-domain-carrier-drift.expr" 0
run_function call-boundary-requirement-drift run_call_composition_spike \
  "$T/call-boundary-requirement-drift.expr" 0
run_function call-unit-target-drift run_call_composition_spike \
  "$T/call-unit-target-drift.expr" 0
run_function call-unit-argument-drift run_call_composition_spike \
  "$T/call-unit-argument-drift.expr" 0
run_function call-unit-transfer-drift run_call_composition_spike \
  "$T/call-unit-transfer-drift.expr" 0
run_function call-boundary-target-drift run_call_composition_spike \
  "$T/call-boundary-target-drift.expr" 0
run_function call-boundary-argument-drift run_call_composition_spike \
  "$T/call-boundary-argument-drift.expr" 0
run_function call-boundary-receipt-drift run_call_composition_spike \
  "$T/call-boundary-receipt-drift.expr" 0
run_function call-boundary-receipt-index-drift run_call_composition_spike \
  "$T/call-boundary-receipt-index-drift.expr" 0
run_function call-truncated run_call_composition_spike "$T/call-truncated.expr" 0
run_function call-trailing run_call_composition_spike "$T/call-trailing.expr" 0

make_program measure_spike "$T/matching.expr"
metrics=$("$T/interp.exe" < "$T/run.gamma")
if [ "$metrics" != "(Metrics 54 3607 2984)" ]; then
  echo "terminal ledger spike: metrics drifted: $metrics" >&2
  exit 1
fi
echo "terminal ledger spike: $metrics (rows / ledger bytes / prospective certificate bytes)"

make_program measure_structural_effect_spike "$T/structural-effect.expr"
structural_metrics=$("$T/interp.exe" < "$T/run.gamma")
if [ "$structural_metrics" != "(Metrics 3 185 164)" ]; then
  echo "terminal ledger spike: structural/effect metrics drifted: $structural_metrics" >&2
  exit 1
fi
echo "terminal ledger spike: $structural_metrics (structural/effect rows / ledger bytes / prospective certificate bytes)"
