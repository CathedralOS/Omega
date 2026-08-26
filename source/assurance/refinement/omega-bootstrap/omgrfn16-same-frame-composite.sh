#!/usr/bin/env sh
# Provisional OMGRFN16 Python-oracle R1--R5 same-frame composition.
# The final lower-rooted gate additionally requires persisted-Beta owners.
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$HERE
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT")
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
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
observe() {
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

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-resolve.alp" "$T/resolver" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-resolved-to-ckir4.alp" "$T/lowerer" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-checked-ir-v5-to-elf.alp" "$T/backend" >/dev/null
PYTHONPATH="$R:$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES" python3 -B "$R/omgrfn16_gate.py" \
  produce "$T/resolver" "$T/lowerer" "$T/backend" "$T/profiles"
PYTHONPATH="$R:$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES" python3 -B "$R/omgrfn16_gate.py" \
  controls "$T/profiles"

TAB=$(printf '\t')
COUNT=0
while IFS="$TAB" read -r PROFILE OUTCOME; do
  [ -n "$PROFILE" ] || continue
  COUNT=$((COUNT + 1))
  for CHECKER in $CHECKERS; do
    observe "$CHECKER" 0 "$T/profiles/$PROFILE.rfn" "$PROFILE-$CHECKER"
  done
done < "$T/profiles/profiles.tsv"

observe r1 251 "$T/profiles/control-retired-outer15.rfn" retired-outer15
observe r1 251 "$T/profiles/control-flags0.rfn" flags0
observe r1 251 "$T/profiles/control-flags2.rfn" flags2
observe r1 251 "$T/profiles/control-unknown-flags.rfn" unknown-flags
# A complete successful u32::MAX header is valid R1 framing even though the
# other owners reject this deliberately false component/result proposition.
observe r1 0 "$T/profiles/control-u32-max-success-framing.rfn" max-success-r1
observe r5-result 251 "$T/profiles/control-u32-max-success-framing.rfn" max-success-false-result
observe r2 251 "$T/profiles/control-retired-witness6.rfn" retired-witness6
observe r3 251 "$T/profiles/control-retired-ckir13.rfn" retired-ckir13-r3
observe r5-structure 251 "$T/profiles/control-retired-ckir13.rfn" retired-ckir13-r5
observe r4-source-result 251 "$T/profiles/control-claim71.rfn" claim71-source
observe r5-result 251 "$T/profiles/control-claim71.rfn" claim71-ckir
observe r4-source-result 251 "$T/profiles/control-trap-as-result.rfn" trap-as-result-source
observe r5-result 251 "$T/profiles/control-trap-as-result.rfn" trap-as-result-ckir
observe r4-lowering 251 "$T/profiles/control-source-ckir-cross.rfn" source-ckir-cross
observe r5-elf 251 "$T/profiles/control-ckir-elf-cross.rfn" ckir-elf-cross
observe r4-lowering 251 "$T/profiles/control-source-operator.rfn" source-operator
observe r2 251 "$T/profiles/control-witness-high-word.rfn" witness-high-word
observe r5-elf 251 "$T/profiles/control-elf-instruction.rfn" elf-instruction
observe r5-elf 251 "$T/profiles/control-elf-trailing.rfn" elf-trailing

echo "OMGRFN16 provisional Python oracle: $COUNT producer-backed result/trap profiles passed independent R1--R5 models; add/subtract/multiply success and first traps, recursive postorder, signed/full-u32 literals, exact widening, CKIR12 view composition, cross-version/cross-pair/claim/instruction/EOF controls, and byte-exact independent ELF reconstruction passed; persisted-Beta closure remains required"
