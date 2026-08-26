#!/usr/bin/env sh
# Rust-free Gamma observation for the bounded OMGCOMP2 -> OMGRSW6 relation.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
cd "$OMEGA_REPO_ROOT"

for TOOL in python3 cmp; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRSW6 meaning: skipped ($TOOL absent)"
    exit 0
  }
done

RESOLVER=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp
REFERENCE=$GATE_DIR/omgrsw6_provider_resolution_reference.py
RUNNER=$GATE_DIR/delta-ckir4-meaning-runner.py
DECODER=$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/decode-gamma-output.py
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

stamp_beta_compiler "$T/bc.exe" >/dev/null || {
  echo "OMGRSW6 meaning FAIL - Beta compiler artifact" >&2
  exit 1
}
ASM=$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED
SEED=$OMEGA_PATH_ALPHA/$ALPHA_SEED
build_beta() {
  "$T/bc.exe" < "$1" > "$T/program.asm" 2>/dev/null \
    && "$ASM" < "$T/program.asm" > "$T/program.tape" 2>/dev/null \
    && stamp_seed "$T/program.tape" "$SEED" "$2" >/dev/null 2>&1
}
build_beta "$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/omega2gamma.beta" "$T/elaborate.exe" || {
  echo "OMGRSW6 meaning FAIL - omega2gamma build" >&2
  exit 1
}
build_beta "$OMEGA_PATH_GAMMA/interp.beta" "$T/interp.exe" || {
  echo "OMGRSW6 meaning FAIL - Gamma interpreter build" >&2
  exit 1
}

python3 -B "$RUNNER" elaborate "$T/elaborate.exe" "$RESOLVER" \
  "$T/resolver.gamma" "$T/timings.tsv" "OMGRSW6 meaning" 120 1048576
python3 -B "$REFERENCE" build "$T/reference" >/dev/null

run_case() { # label input status expected-stdout timeout
  LABEL=$1 INPUT=$2 EXPECTED=$3 EXPECTED_STDOUT=$4 TIMEOUT=$5
  python3 -B "$RUNNER" run "$T/interp.exe" "$T/resolver.gamma" "$INPUT" \
    "$T/$LABEL.observation" "$T/timings.tsv" "OMGRSW6 meaning $LABEL" "$TIMEOUT"
  STATUS=$(python3 -B "$DECODER" "$T/$LABEL.observation" "$T/$LABEL.stdout")
  [ "$STATUS" -eq "$EXPECTED" ] || {
    echo "OMGRSW6 meaning FAIL - $LABEL status $STATUS, expected $EXPECTED" >&2
    exit 1
  }
  cmp "$T/$LABEL.stdout" "$EXPECTED_STDOUT" >/dev/null || {
    echo "OMGRSW6 meaning FAIL - $LABEL stdout differs" >&2
    exit 1
  }
}

: > "$T/empty"
run_case canonical "$T/reference/canonical.omgc" 0 \
  "$T/reference/canonical.omgrsw6" 240
run_case semantic-251 "$T/reference/wrong-call-target.omgc" 251 "$T/empty" 180
run_case resource-252 "$T/reference/resource-identifier-adjacent.omgc" 252 "$T/empty" 180

echo "OMGRSW6 meaning: Rust-free canonical 0, semantic 251, resource 252, and exact publication PASS"
