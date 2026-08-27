#!/usr/bin/env sh
# OMGRFN16 immutable-frame R1--R5 composition for recursive CKIR14 arithmetic.
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$HERE
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT")
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRFN16 same-frame composite: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN16 same-frame composite: skipped ($TOOL absent)"; exit 0;
  }
done

R=$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT
C=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER
T=$(mktemp -d)
if [ "${OMEGA_KEEP_OMGRFN16_COMPOSITE_TEMP:-0}" = 1 ]; then
  echo "OMGRFN16 same-frame composite: retained $T" >&2
else
  trap 'rm -rf "$T"' EXIT
fi

CHECKERS='r1 r2 r3 r4-lowering r4-source-result r5-structure r5-result r5-elf'
MATRIX=${OMGRFN16_MATRIX:-focused}
case "$MATRIX" in
  focused|exhaustive) ;;
  *) echo "OMGRFN16 same-frame composite: OMGRFN16_MATRIX must be focused or exhaustive" >&2; exit 2 ;;
esac
STARTED_AT=$(date +%s)
PHASE_AT=$STARTED_AT
phase() {
  PHASE_NOW=$(date +%s)
  echo "OMGRFN16 same-frame composite: $1 $((PHASE_NOW - PHASE_AT))s (total $((PHASE_NOW - STARTED_AT))s)" >&2
  PHASE_AT=$PHASE_NOW
}
BETA_PAIRS=0
observe_python() {
  CHECKER=$1 EXPECTED=$2 FRAME=$3 LABEL=$4
  set +e
  PYTHONPATH="$R:$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES" \
    python3 -B "$R/omgrfn16-$CHECKER.py" < "$FRAME" > "$T/$LABEL.out" 2> "$T/$LABEL.err"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] && [ ! -s "$T/$LABEL.out" ] || {
    echo "OMGRFN16 same-frame composite: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,8p' "$T/$LABEL.err" >&2
    exit 1
  }
}
observe_beta() {
  OWNER=$1 MODE=$2 EXPECTED=$3 FRAME=$4 LABEL=$5
  set +e
  "$T/$OWNER.$MODE" < "$FRAME" > "$T/$LABEL.out" 2> "$T/$LABEL.err"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] && [ ! -s "$T/$LABEL.out" ] || {
    echo "OMGRFN16 same-frame composite: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,8p' "$T/$LABEL.err" >&2
    exit 1
  }
}
run_beta_pair() {
  OWNER=$1 FRAME=$2 EXPECTED=$3 LABEL=$4
  observe_beta "$OWNER" native "$EXPECTED" "$FRAME" "$LABEL-native"
  observe_beta "$OWNER" self "$EXPECTED" "$FRAME" "$LABEL-self"
  BETA_PAIRS=$((BETA_PAIRS + 1))
}

PYTHONPATH="$R:$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES" \
  python3 -B "$R/omgrfn16-materialize-r1-r2.py" "$T/checkers"
PYTHONPATH="$R:$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES" \
  python3 -B "$R/omgrfn16-materialize-r3-r5.py" "$T/checkers"
PYTHONPATH="$R:$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES" \
  python3 -B "$R/omgrfn16-materialize-r4.py" "$T/checkers"
SEED=$OMEGA_PATH_ALPHA/$ALPHA_SEED
ASM=$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED
stamp_beta_compiler "$T/bc0" >/dev/null
"$T/bc0" < "$OMEGA_PATH_BETA/bc.beta" > "$T/bc1.asm"
"$ASM" < "$T/bc1.asm" > "$T/bc1.tape"
stamp_seed "$T/bc1.tape" "$SEED" "$T/bc1" >/dev/null 2>&1
for OWNER in $CHECKERS; do
  "$T/bc0" < "$T/checkers/$OWNER.beta" > "$T/$OWNER.native.asm"
  "$T/bc1" < "$T/checkers/$OWNER.beta" > "$T/$OWNER.self.asm"
  cmp "$T/$OWNER.native.asm" "$T/$OWNER.self.asm" >/dev/null
  "$ASM" < "$T/$OWNER.native.asm" > "$T/$OWNER.tape"
  TAPE_BYTES=$(wc -c < "$T/$OWNER.tape" | tr -d ' ')
  [ "$TAPE_BYTES" -le 262140 ] || {
    echo "OMGRFN16 same-frame composite: $OWNER tape $TAPE_BYTES exceeds ceiling" >&2
    exit 1
  }
  stamp_seed "$T/$OWNER.tape" "$SEED" "$T/$OWNER.native" >/dev/null 2>&1
  stamp_seed "$T/$OWNER.tape" "$SEED" "$T/$OWNER.self" >/dev/null 2>&1
done
phase "materialization and persisted-Beta fixed points"

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-resolve.alp" "$T/resolver" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-resolved-to-ckir4.alp" "$T/lowerer" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-checked-ir-v5-to-elf.alp" "$T/backend" >/dev/null
PYTHONPATH="$R:$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES" python3 -B "$R/omgrfn16_gate.py" \
  produce "$T/resolver" "$T/lowerer" "$T/backend" "$T/profiles"
PYTHONPATH="$R:$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES" python3 -B "$R/omgrfn16_gate.py" \
  controls "$T/profiles"
phase "producer-backed profiles and controls"

TAB=$(printf '\t')
COUNT=0
while IFS="$TAB" read -r PROFILE OUTCOME; do
  [ -n "$PROFILE" ] || continue
  COUNT=$((COUNT + 1))
  for CHECKER in $CHECKERS; do
    observe_python "$CHECKER" 0 "$T/profiles/$PROFILE.rfn" "$PROFILE-$CHECKER-python"
    case "$MATRIX:$PROFILE" in
      exhaustive:*|focused:mixed-success|focused:view-composition-success|focused:add-overflow)
        run_beta_pair "$CHECKER" "$T/profiles/$PROFILE.rfn" 0 "$PROFILE-$CHECKER-beta"
        ;;
    esac
  done
done < "$T/profiles/profiles.tsv"
phase "positive profile matrix"

observe_python r1 251 "$T/profiles/control-retired-outer15.rfn" retired-outer15-python
observe_python r1 251 "$T/profiles/control-flags0.rfn" flags0-python
observe_python r1 251 "$T/profiles/control-flags2.rfn" flags2-python
observe_python r1 251 "$T/profiles/control-unknown-flags.rfn" unknown-flags-python
# A complete successful u32::MAX header is valid R1 framing even though the
# other owners reject this deliberately false component/result proposition.
observe_python r1 0 "$T/profiles/control-u32-max-success-framing.rfn" max-success-r1-python
observe_python r5-result 251 "$T/profiles/control-u32-max-success-framing.rfn" max-success-false-result-python
observe_python r2 251 "$T/profiles/control-retired-witness6.rfn" retired-witness6-python
observe_python r3 251 "$T/profiles/control-retired-ckir13.rfn" retired-ckir13-r3-python
observe_python r5-structure 251 "$T/profiles/control-retired-ckir13.rfn" retired-ckir13-r5-python
observe_python r4-source-result 251 "$T/profiles/control-claim71.rfn" claim71-source-python
observe_python r5-result 251 "$T/profiles/control-claim71.rfn" claim71-ckir-python
for OWNER in r1 r2 r3 r4-lowering r5-structure r5-elf; do
  observe_python "$OWNER" 0 "$T/profiles/control-claim71.rfn" "$OWNER-claim-opacity-python"
done
observe_python r4-source-result 251 "$T/profiles/control-trap-as-result.rfn" trap-as-result-source-python
observe_python r5-result 251 "$T/profiles/control-trap-as-result.rfn" trap-as-result-ckir-python
observe_python r4-lowering 251 "$T/profiles/control-source-ckir-cross.rfn" source-ckir-cross-python
observe_python r5-elf 251 "$T/profiles/control-ckir-elf-cross.rfn" ckir-elf-cross-python
observe_python r4-lowering 251 "$T/profiles/control-source-operator.rfn" source-operator-python
observe_python r4-source-result 251 "$T/profiles/control-source-operator.rfn" source-operator-result-python
observe_python r2 0 "$T/profiles/control-source-operator.rfn" source-operator-r2-opacity-python
observe_python r4-lowering 251 "$T/profiles/control-source-leaf-name.rfn" source-leaf-name-python
observe_python r4-source-result 251 "$T/profiles/control-source-leaf-name.rfn" source-leaf-name-result-python
observe_python r2 0 "$T/profiles/control-source-leaf-name.rfn" source-leaf-name-r2-opacity-python
observe_python r2 251 "$T/profiles/control-source-grown-stale-witness.rfn" source-grown-stale-witness-python
observe_python r2 252 "$T/profiles/control-source-depth-nine.rfn" source-depth-nine-r2-python
observe_python r4-lowering 252 "$T/profiles/control-source-depth-nine.rfn" source-depth-nine-lowering-python
observe_python r4-source-result 252 "$T/profiles/control-source-depth-nine.rfn" source-depth-nine-result-python
observe_python r4-lowering 251 "$T/profiles/control-source-view-literal.rfn" source-view-literal-python
observe_python r4-source-result 251 "$T/profiles/control-source-view-literal.rfn" source-view-literal-result-python
observe_python r2 0 "$T/profiles/control-source-view-literal.rfn" source-view-literal-r2-opacity-python
observe_python r2 0 "$T/profiles/control-source-transition-sibling.rfn" transition-sibling-r2-opacity-python
observe_python r4-lowering 251 "$T/profiles/control-source-transition-sibling.rfn" transition-sibling-lowering-python
observe_python r4-source-result 251 "$T/profiles/control-source-transition-sibling.rfn" transition-sibling-result-python
observe_python r2 251 "$T/profiles/control-witness-high-word.rfn" witness-high-word-python
observe_python r4-lowering 251 "$T/profiles/control-retired-witness6.rfn" retired-witness6-lowering-python
observe_python r5-elf 251 "$T/profiles/control-elf-instruction.rfn" elf-instruction-python
observe_python r5-elf 251 "$T/profiles/control-elf-case-tag.rfn" elf-case-tag-python
observe_python r5-elf 251 "$T/profiles/control-elf-dispatch-bound.rfn" elf-dispatch-bound-python
observe_python r5-elf 251 "$T/profiles/control-elf-trailing.rfn" elf-trailing-python
observe_python r1 251 "$T/profiles/control-malformed-omgcomp.rfn" malformed-omgcomp-python
for RESOURCE in omgcomp witness ckir elf whole-frame; do
  observe_python r1 252 "$T/profiles/control-$RESOURCE-resource.rfn" "$RESOURCE-resource-python"
done
phase "responsibility-local Python control matrix"

if [ "$MATRIX" = exhaustive ]; then
  for OWNER in $CHECKERS; do
    run_beta_pair "$OWNER" "$T/profiles/control-retired-outer15.rfn" 251 "$OWNER-retired-outer15"
  done
  run_beta_pair r1 "$T/profiles/control-flags0.rfn" 251 r1-flags0
  run_beta_pair r1 "$T/profiles/control-flags2.rfn" 251 r1-flags2
  run_beta_pair r1 "$T/profiles/control-unknown-flags.rfn" 251 r1-unknown-flags
  run_beta_pair r1 "$T/profiles/control-u32-max-success-framing.rfn" 0 r1-max-success-framing
  run_beta_pair r5-result "$T/profiles/control-u32-max-success-framing.rfn" 251 r5-result-max-false
  run_beta_pair r2 "$T/profiles/control-retired-witness6.rfn" 251 r2-retired-witness6
  run_beta_pair r3 "$T/profiles/control-retired-ckir13.rfn" 251 r3-retired-ckir13
  run_beta_pair r5-structure "$T/profiles/control-retired-ckir13.rfn" 251 r5-retired-ckir13
  run_beta_pair r4-source-result "$T/profiles/control-claim71.rfn" 251 r4-source-claim71
  run_beta_pair r5-result "$T/profiles/control-claim71.rfn" 251 r5-result-claim71
  for OWNER in r1 r2 r3 r4-lowering r5-structure r5-elf; do
    run_beta_pair "$OWNER" "$T/profiles/control-claim71.rfn" 0 "$OWNER-claim-opacity"
  done
  run_beta_pair r4-source-result "$T/profiles/control-trap-as-result.rfn" 251 r4-source-trap-as-result
  run_beta_pair r5-result "$T/profiles/control-trap-as-result.rfn" 251 r5-result-trap-as-result
  run_beta_pair r4-lowering "$T/profiles/control-source-ckir-cross.rfn" 251 r4-source-ckir-cross
  run_beta_pair r5-elf "$T/profiles/control-ckir-elf-cross.rfn" 251 r5-ckir-elf-cross
  run_beta_pair r4-lowering "$T/profiles/control-source-operator.rfn" 251 r4-source-operator
  run_beta_pair r4-source-result "$T/profiles/control-source-operator.rfn" 251 r4-source-operator-result
  run_beta_pair r2 "$T/profiles/control-source-operator.rfn" 0 r2-source-operator-opacity
  run_beta_pair r4-lowering "$T/profiles/control-source-leaf-name.rfn" 251 r4-source-leaf-name
  run_beta_pair r4-source-result "$T/profiles/control-source-leaf-name.rfn" 251 r4-source-leaf-name-result
  run_beta_pair r2 "$T/profiles/control-source-leaf-name.rfn" 0 r2-source-leaf-name-opacity
  run_beta_pair r2 "$T/profiles/control-source-grown-stale-witness.rfn" 251 r2-source-grown-stale-witness
  run_beta_pair r2 "$T/profiles/control-source-depth-nine.rfn" 252 r2-source-depth-nine
  run_beta_pair r4-lowering "$T/profiles/control-source-depth-nine.rfn" 252 r4-source-depth-nine
  run_beta_pair r4-source-result "$T/profiles/control-source-depth-nine.rfn" 252 r4-source-result-depth-nine
  run_beta_pair r4-lowering "$T/profiles/control-source-view-literal.rfn" 251 r4-source-view-literal
  run_beta_pair r4-source-result "$T/profiles/control-source-view-literal.rfn" 251 r4-source-result-view-literal
  run_beta_pair r2 "$T/profiles/control-source-view-literal.rfn" 0 r2-source-view-literal-opacity
  run_beta_pair r2 "$T/profiles/control-source-transition-sibling.rfn" 0 r2-transition-sibling-opacity
  run_beta_pair r4-lowering "$T/profiles/control-source-transition-sibling.rfn" 251 r4-transition-sibling
  run_beta_pair r4-source-result "$T/profiles/control-source-transition-sibling.rfn" 251 r4-source-result-transition-sibling
  run_beta_pair r2 "$T/profiles/control-witness-high-word.rfn" 251 r2-witness-high-word
  run_beta_pair r4-lowering "$T/profiles/control-retired-witness6.rfn" 251 r4-retired-witness6
  run_beta_pair r5-elf "$T/profiles/control-elf-instruction.rfn" 251 r5-elf-instruction
  run_beta_pair r5-elf "$T/profiles/control-elf-case-tag.rfn" 251 r5-elf-case-tag
  run_beta_pair r5-elf "$T/profiles/control-elf-dispatch-bound.rfn" 251 r5-elf-dispatch-bound
  run_beta_pair r5-elf "$T/profiles/control-elf-trailing.rfn" 251 r5-elf-trailing
  run_beta_pair r1 "$T/profiles/control-malformed-omgcomp.rfn" 251 r1-malformed-omgcomp
  for RESOURCE in omgcomp witness ckir elf whole-frame; do
    run_beta_pair r1 "$T/profiles/control-$RESOURCE-resource.rfn" 252 "r1-$RESOURCE-resource"
  done
else
  # The Python owners retain the exhaustive responsibility-local mutation
  # matrix above. These native/self pairs establish that every persisted-Beta
  # owner accepts representative recursive/view/trap carriers and rejects one
  # control belonging to its own responsibility, without multiplying every
  # case by every independent checker implementation.
  run_beta_pair r1 "$T/profiles/control-retired-outer15.rfn" 251 r1-retired-outer15
  run_beta_pair r2 "$T/profiles/control-retired-witness6.rfn" 251 r2-retired-witness6
  run_beta_pair r3 "$T/profiles/control-retired-ckir13.rfn" 251 r3-retired-ckir13
  run_beta_pair r4-lowering "$T/profiles/control-source-ckir-cross.rfn" 251 r4-source-ckir-cross
  run_beta_pair r4-source-result "$T/profiles/control-claim71.rfn" 251 r4-source-claim71
  run_beta_pair r5-structure "$T/profiles/control-retired-ckir13.rfn" 251 r5-retired-ckir13
  run_beta_pair r5-result "$T/profiles/control-trap-as-result.rfn" 251 r5-result-trap-as-result
  run_beta_pair r5-elf "$T/profiles/control-elf-instruction.rfn" 251 r5-elf-instruction
fi
phase "$MATRIX persisted-Beta control matrix"

echo "OMGRFN16 same-frame composite: $COUNT producer-backed result/trap profiles passed exhaustive responsibility-local Python R1--R5; $BETA_PAIRS $MATRIX native/self persisted-Beta pairs passed; recursive postorder, full-u32 literals/widening, first traps, CKIR12 view composition, ownership, cross-pair, claim, instruction, and EOF controls passed"
