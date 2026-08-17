#!/usr/bin/env sh
# Q7 canonical semantic-ledger Gamma spike.
#
# The typed Gamma program decodes current PSITERM bytes, constructs and audits
# the bounded semantic ledger, and is evaluated by both the canonical
# Beta-written interpreter and the independent Python oracle.
set -eu
cd "$(dirname "$0")"
. ../alpha/seed_env.sh

SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
FIXTURES=../psi-rs/semantics/psi-terminal-codec/tests/fixtures
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || {
  echo "terminal ledger spike: bc build failed" >&2
  exit 1
}

build_beta_program() {
  source=$1
  output=$2
  ../beta-lang-rs/build/bc.exe < "$source" > "$T/program.asm"
  "$ASM" < "$T/program.asm" > "$T/program.tape"
  stamp_seed "$T/program.tape" "$SEED" "$output" >/dev/null 2>&1
}

build_beta_program typeck.beta "$T/typeck.exe"
build_beta_program interp.beta "$T/interp.exe"

cat canonical-bytes/types.gamma \
    terminal-ledger-spike/types.gamma \
    canonical-bytes/decode.gamma \
    terminal-ledger-spike/decode.gamma \
    terminal-ledger-spike/schema.gamma \
    terminal-ledger-spike/ledger.gamma > "$T/typed.gamma"

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
fixture_expr "$FIXTURES/terminal_ledger_spike.hex" --set-byte 0 0 > "$T/bad-magic.expr"
fixture_expr "$FIXTURES/terminal_ledger_spike.hex" --drop-last > "$T/truncated.expr"
fixture_expr "$FIXTURES/terminal_ledger_spike.hex" --append-byte 0 > "$T/trailing.expr"
fixture_expr "$FIXTURES/terminal_ledger_spike.hex" --set-byte 179 10 > "$T/duplicate-result.expr"

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
run_function asymmetric-join run_spike "$T/asymmetric.expr" 0
run_function bad-magic run_spike "$T/bad-magic.expr" 0
run_function truncated run_spike "$T/truncated.expr" 0
run_function trailing-byte run_spike "$T/trailing.expr" 0
run_function duplicate-result run_spike "$T/duplicate-result.expr" 0

make_program measure_spike "$T/matching.expr"
metrics=$("$T/interp.exe" < "$T/run.gamma")
if [ "$metrics" != "(Metrics 35 2414 2052)" ]; then
  echo "terminal ledger spike: metrics drifted: $metrics" >&2
  exit 1
fi
echo "terminal ledger spike: $metrics (rows / ledger bytes / prospective certificate bytes)"
