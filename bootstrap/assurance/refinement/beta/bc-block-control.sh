#!/usr/bin/env sh
# Bounded source-to-artifact control correspondence for the whole bc compiler.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$OMEGA_PATH_PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
done
unset OMEGA_PATH_PARENT
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"

ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
SOURCE="$OMEGA_PATH_BETA/bc.beta"
ARTIFACT="$OMEGA_PATH_BETA/artifacts/bc.tape"
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

# The persisted compiler supplies only a location hint.  Require its Alpha text
# to assemble to the exact committed artifact before deriving that hint.
stamp_beta_compiler "$T/bc" >/dev/null
"$T/bc" < "$SOURCE" > "$T/fixed.alpha"
"$ASM" < "$T/fixed.alpha" > "$T/fixed.tape"
cmp "$ARTIFACT" "$T/fixed.tape"

python3 "$GATE_DIR/bc_block_control_map.py" \
  --repo "$OMEGA_REPO_ROOT" \
  --source "$SOURCE" \
  --assembly "$T/fixed.alpha" \
  --tape "$ARTIFACT" \
  --output "$T/control.witness" \
  --retarget-patch-output "$T/retarget.patch" \
  --operand-witness-output "$T/operand.witness" \
  --duplicate-witness-output "$T/duplicate.witness" \
  --missing-witness-output "$T/missing.witness" \
  --noncanonical-witness-output "$T/noncanonical.witness"

# The untrusted mapper never writes the source/tape portion of checker input.
# Assemble every bundle here from the exact repository source and artifact.
u32_file() { # value output
  python3 -c 'import struct,sys; sys.stdout.buffer.write(struct.pack("<I", int(sys.argv[1])))' "$1" > "$2"
}
SOURCE_LEN=$(wc -c < "$SOURCE" | tr -d ' ')
TAPE_LEN=$(wc -c < "$ARTIFACT" | tr -d ' ')
u32_file "$SOURCE_LEN" "$T/source.len"
u32_file "$TAPE_LEN" "$T/tape.len"
make_bundle() { # tape witness output
  cat "$T/source.len" "$SOURCE" "$T/tape.len" "$1" "$2" > "$3"
}
make_bundle "$ARTIFACT" "$T/control.witness" "$T/control.bundle"
make_bundle "$ARTIFACT" "$T/operand.witness" "$T/operand.bundle"
make_bundle "$ARTIFACT" "$T/duplicate.witness" "$T/duplicate.bundle"
make_bundle "$ARTIFACT" "$T/missing.witness" "$T/missing.bundle"
make_bundle "$ARTIFACT" "$T/noncanonical.witness" "$T/noncanonical.bundle"

cp "$ARTIFACT" "$T/retarget.tape"
RETARGET_OFFSET=$(dd if="$T/retarget.patch" bs=1 count=4 2>/dev/null | od -An -tu4 | tr -d ' ')
dd if="$T/retarget.patch" of="$T/retarget.tape" bs=1 skip=4 seek="$RETARGET_OFFSET" count=8 conv=notrunc 2>/dev/null
make_bundle "$T/retarget.tape" "$T/control.witness" "$T/retarget.bundle"

"$ASM" < "$GATE_DIR/bc-block-control.alpha" > "$T/control-check.tape"
stamp_seed "$T/control-check.tape" "$SEED" "$T/control-check" >/dev/null

case_run() { # label expected-status input
  set +e
  "$T/control-check" < "$3" > "$T/stdout"
  got=$?
  set -e
  if [ "$got" != "$2" ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $1: expected $2/empty, got $got/$(wc -c < "$T/stdout" | tr -d ' ') bytes" >&2
    exit 1
  fi
}

case_run "whole compiler control skeleton" 0 "$T/control.bundle"
case_run "valid-boundary transition retarget" 1 "$T/retarget.bundle"
case_run "block pc into opcode-looking operand" 1 "$T/operand.bundle"
case_run "duplicate block location" 1 "$T/duplicate.bundle"
case_run "missing transition location" 1 "$T/missing.bundle"
case_run "noncanonical transition order" 1 "$T/noncanonical.bundle"

# Show that the negative control has teeth beyond the pre-existing structural
# obligation: its changed target is another real instruction boundary, so the
# generic Alpha CFG checker still accepts it.
"$ASM" < "$GATE_DIR/bc-artifact-structure.alpha" > "$T/structure-check.tape"
stamp_seed "$T/structure-check.tape" "$SEED" "$T/structure-check" >/dev/null
set +e
"$T/structure-check" < "$ARTIFACT" > "$T/stdout"
artifact_structure_status=$?
set -e
if [ "$artifact_structure_status" != 0 ] || [ -s "$T/stdout" ]; then
  echo "bc block control FAIL — persisted artifact failed its structural prerequisite" >&2
  exit 1
fi
set +e
"$T/structure-check" < "$T/retarget.tape" > "$T/stdout"
structure_status=$?
set -e
if [ "$structure_status" != 0 ] || [ -s "$T/stdout" ]; then
  echo "bc block control FAIL — retarget was not a structurally valid boundary mutation" >&2
  exit 1
fi

echo "bc block control: source completeness 70 proc / 355 block / 291 transition; valid-boundary retarget rejected ($(wc -c < "$T/control-check.tape" | tr -d ' ')-byte Alpha checker tape)"
