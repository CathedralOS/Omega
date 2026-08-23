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
  --noncanonical-witness-output "$T/noncanonical.witness" \
  --call-retarget-patch-output "$T/call-retarget.patch" \
  --read-register-patch-output "$T/read-register.patch" \
  --write-register-patch-output "$T/write-register.patch" \
  --helper-write-patch-output "$T/helper-write.patch" \
  --emit-byte-patch-output "$T/emit-byte.patch" \
  --emit-length-patch-output "$T/emit-length.patch" \
  --emit-pointer-patch-output "$T/emit-pointer.patch" \
  --emit-helper-patch-output "$T/emit-helper.patch" \
  --orphan-io-patch-output "$T/orphan-io.patch" \
  --duplicate-event-witness-output "$T/duplicate-event.witness" \
  --noncanonical-event-witness-output "$T/noncanonical-event.witness" \
  --frame-size-patch-output "$T/frame-size.patch" \
  --saved-fp-patch-output "$T/saved-fp.patch" \
  --frame-base-patch-output "$T/frame-base.patch" \
  --param-offset-patch-output "$T/param-offset.patch" \
  --param-register-patch-output "$T/param-register.patch" \
  --call-pop-order-patch-output "$T/call-pop-order.patch" \
  --call-pop-step-patch-output "$T/call-pop-step.patch" \
  --local-load-slot-patch-output "$T/local-load-slot.patch" \
  --local-store-slot-patch-output "$T/local-store-slot.patch" \
  --local-base-patch-output "$T/local-base.patch" \
  --local-load-opcode-patch-output "$T/local-load-opcode.patch" \
  --local-store-opcode-patch-output "$T/local-store-opcode.patch" \
  --duplicate-local-witness-output "$T/duplicate-local.witness" \
  --noncanonical-local-witness-output "$T/noncanonical-local.witness" \
  --memory-load-width-patch-output "$T/memory-load-width.patch" \
  --memory-store-width-patch-output "$T/memory-store-width.patch" \
  --memory-load-register-patch-output "$T/memory-load-register.patch" \
  --memory-store-register-patch-output "$T/memory-store-register.patch" \
  --memory-pop-step-patch-output "$T/memory-pop-step.patch" \
  --duplicate-memory-witness-output "$T/duplicate-memory.witness" \
  --noncanonical-memory-witness-output "$T/noncanonical-memory.witness"

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
make_bundle "$ARTIFACT" "$T/duplicate-event.witness" "$T/duplicate-event.bundle"
make_bundle "$ARTIFACT" "$T/noncanonical-event.witness" "$T/noncanonical-event.bundle"
make_bundle "$ARTIFACT" "$T/duplicate-local.witness" "$T/duplicate-local.bundle"
make_bundle "$ARTIFACT" "$T/noncanonical-local.witness" "$T/noncanonical-local.bundle"
make_bundle "$ARTIFACT" "$T/duplicate-memory.witness" "$T/duplicate-memory.bundle"
make_bundle "$ARTIFACT" "$T/noncanonical-memory.witness" "$T/noncanonical-memory.bundle"

cp "$ARTIFACT" "$T/retarget.tape"
RETARGET_OFFSET=$(dd if="$T/retarget.patch" bs=1 count=4 2>/dev/null | od -An -tu4 | tr -d ' ')
dd if="$T/retarget.patch" of="$T/retarget.tape" bs=1 skip=4 seek="$RETARGET_OFFSET" count=8 conv=notrunc 2>/dev/null
make_bundle "$T/retarget.tape" "$T/control.witness" "$T/retarget.bundle"

apply_tape_patch() { # label
  cp "$ARTIFACT" "$T/$1.tape"
  PATCH_OFFSET=$(dd if="$T/$1.patch" bs=1 count=4 2>/dev/null | od -An -tu4 | tr -d ' ')
  PATCH_SIZE=$(wc -c < "$T/$1.patch" | tr -d ' ')
  PATCH_SIZE=$((PATCH_SIZE - 4))
  dd if="$T/$1.patch" of="$T/$1.tape" bs=1 skip=4 seek="$PATCH_OFFSET" count="$PATCH_SIZE" conv=notrunc 2>/dev/null
  make_bundle "$T/$1.tape" "$T/control.witness" "$T/$1.bundle"
}
apply_tape_patch call-retarget
apply_tape_patch read-register
apply_tape_patch write-register
apply_tape_patch helper-write
apply_tape_patch emit-byte
apply_tape_patch emit-length
apply_tape_patch emit-pointer
apply_tape_patch emit-helper
apply_tape_patch orphan-io
apply_tape_patch frame-size
apply_tape_patch saved-fp
apply_tape_patch frame-base
apply_tape_patch param-offset
apply_tape_patch param-register
apply_tape_patch call-pop-order
apply_tape_patch call-pop-step
apply_tape_patch local-load-slot
apply_tape_patch local-store-slot
apply_tape_patch local-base
apply_tape_patch local-load-opcode
apply_tape_patch local-store-opcode
apply_tape_patch memory-load-width
apply_tape_patch memory-store-width
apply_tape_patch memory-load-register
apply_tape_patch memory-store-register
apply_tape_patch memory-pop-step

cat "$GATE_DIR/bc-block-control.alpha" \
  "$GATE_DIR/bc-effect-sites.alpha" \
  "$GATE_DIR/bc-frame-shape.alpha" \
  "$GATE_DIR/bc-local-access.alpha" \
  "$GATE_DIR/bc-memory-sites.alpha" > "$T/control-check.alpha"
"$ASM" < "$T/control-check.alpha" > "$T/control-check.tape"
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
case_run "valid-entry source call retarget" 1 "$T/call-retarget.bundle"
case_run "source read register" 1 "$T/read-register.bundle"
case_run "source write register" 1 "$T/write-register.bundle"
case_run "helper write register" 1 "$T/helper-write.bundle"
case_run "jump-skipped emit byte" 1 "$T/emit-byte.bundle"
case_run "emit length" 1 "$T/emit-length.bundle"
case_run "emit pointer" 1 "$T/emit-pointer.bundle"
case_run "emit helper target" 1 "$T/emit-helper.bundle"
case_run "same-width read/write opcode" 1 "$T/orphan-io.bundle"
case_run "duplicate source effect location" 1 "$T/duplicate-event.bundle"
case_run "noncanonical source effect order" 1 "$T/noncanonical-event.bundle"
case_run "frame allocation size" 1 "$T/frame-size.bundle"
case_run "saved frame-pointer register" 1 "$T/saved-fp.bundle"
case_run "frame-base register" 1 "$T/frame-base.bundle"
case_run "parameter slot offset" 1 "$T/param-offset.bundle"
case_run "parameter source register" 1 "$T/param-register.bundle"
case_run "two-argument pop order" 1 "$T/call-pop-order.bundle"
case_run "argument pop stack step" 1 "$T/call-pop-step.bundle"
case_run "valid-slot local load retarget" 1 "$T/local-load-slot.bundle"
case_run "valid-slot local store retarget" 1 "$T/local-store-slot.bundle"
case_run "local frame-base register" 1 "$T/local-base.bundle"
case_run "same-width local load/store opcode" 1 "$T/local-load-opcode.bundle"
case_run "same-width local store/load opcode" 1 "$T/local-store-opcode.bundle"
case_run "duplicate local access location" 1 "$T/duplicate-local.bundle"
case_run "noncanonical local access order" 1 "$T/noncanonical-local.bundle"
case_run "raw memory load width" 1 "$T/memory-load-width.bundle"
case_run "raw memory store width" 1 "$T/memory-store-width.bundle"
case_run "raw memory load register" 1 "$T/memory-load-register.bundle"
case_run "raw memory store register" 1 "$T/memory-store-register.bundle"
case_run "raw memory store pop step" 1 "$T/memory-pop-step.bundle"
case_run "duplicate raw memory location" 1 "$T/duplicate-memory.bundle"
case_run "noncanonical raw memory order" 1 "$T/noncanonical-memory.bundle"

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

for mutation in call-retarget read-register write-register helper-write emit-byte emit-length emit-pointer emit-helper orphan-io frame-size saved-fp frame-base param-offset param-register call-pop-order call-pop-step local-load-slot local-store-slot local-base local-load-opcode local-store-opcode memory-load-width memory-store-width memory-load-register memory-store-register memory-pop-step; do
  set +e
  "$T/structure-check" < "$T/$mutation.tape" > "$T/stdout"
  structure_status=$?
  set -e
  if [ "$structure_status" != 0 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $mutation was not a structurally valid mutation" >&2
    exit 1
  fi
done

echo "bc block control/effects: 70 proc / 355 block / 291 transition; 612 effect sites / 829 fixed emit bytes; 78 frame slots / 27 parameter stores / 134 call pops; 169 local loads / 73 local stores; 62 raw loads / 33 raw stores; all 686 artifact effects owned ($(wc -c < "$T/control-check.tape" | tr -d ' ')-byte Alpha checker tape)"
