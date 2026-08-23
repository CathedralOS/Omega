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
  --noncanonical-memory-witness-output "$T/noncanonical-memory.witness" \
  --literal-value-patch-output "$T/literal-value.patch" \
  --literal-register-patch-output "$T/literal-register.patch" \
  --arithmetic-opcode-patch-output "$T/arithmetic-opcode.patch" \
  --arithmetic-pop-step-patch-output "$T/arithmetic-pop-step.patch" \
  --arithmetic-register-patch-output "$T/arithmetic-register.patch" \
  --duplicate-primitive-witness-output "$T/duplicate-primitive.witness" \
  --noncanonical-primitive-witness-output "$T/noncanonical-primitive.witness" \
  --synthetic-literal-witness-output "$T/synthetic-literal.witness" \
  --composition-order-witness-output "$T/composition-order.witness" \
  --composition-argument-order-witness-output "$T/composition-argument-order.witness" \
  --composition-store-order-witness-output "$T/composition-store-order.witness" \
  --comparison-opcode-patch-output "$T/comparison-opcode.patch" \
  --comparison-operand-patch-output "$T/comparison-operand.patch" \
  --comparison-branch-target-patch-output "$T/comparison-branch-target.patch" \
  --comparison-result-patch-output "$T/comparison-result.patch" \
  --comparison-pop-step-patch-output "$T/comparison-pop-step.patch" \
  --push-step-patch-output "$T/push-step.patch" \
  --push-stack-register-patch-output "$T/push-stack-register.patch" \
  --push-value-register-patch-output "$T/push-value-register.patch" \
  --push-opcode-patch-output "$T/push-opcode.patch" \
  --duplicate-push-witness-output "$T/duplicate-push.witness" \
  --cross-block-push-witness-output "$T/cross-block-push.witness"

# The untrusted mapper never writes the source/tape portion of checker input.
# Assemble every bundle here from the exact repository source and artifact.
u32_file() { # value output
  python3 -c 'import struct,sys; sys.stdout.buffer.write(struct.pack("<I", int(sys.argv[1])))' "$1" > "$2"
}
SOURCE_LEN=$(wc -c < "$SOURCE" | tr -d ' ')
TAPE_LEN=$(wc -c < "$ARTIFACT" | tr -d ' ')
u32_file "$SOURCE_LEN" "$T/source.len"
u32_file "$TAPE_LEN" "$T/tape.len"
python3 "$GATE_DIR/bc_call_bounds.py" \
  --repo "$OMEGA_REPO_ROOT" \
  --source "$SOURCE" \
  --output "$T/call-bounds.witness" \
  --underreport-probe-output "$T/call-bounds-probe.witness" \
  --underreport-root-output "$T/call-bounds-root.witness"
make_bundle() { # tape witness output
  cat "$T/source.len" "$SOURCE" "$T/tape.len" "$1" "$2" \
    "$T/call-bounds.witness" > "$3"
}
make_bounds_bundle() { # bounds-witness output
  cat "$T/source.len" "$SOURCE" "$T/tape.len" "$ARTIFACT" \
    "$T/control.witness" "$1" > "$2"
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
make_bundle "$ARTIFACT" "$T/duplicate-primitive.witness" "$T/duplicate-primitive.bundle"
make_bundle "$ARTIFACT" "$T/noncanonical-primitive.witness" "$T/noncanonical-primitive.bundle"
make_bundle "$ARTIFACT" "$T/synthetic-literal.witness" "$T/synthetic-literal.bundle"
make_bundle "$ARTIFACT" "$T/composition-order.witness" "$T/composition-order.bundle"
make_bundle "$ARTIFACT" "$T/composition-argument-order.witness" "$T/composition-argument-order.bundle"
make_bundle "$ARTIFACT" "$T/composition-store-order.witness" "$T/composition-store-order.bundle"
make_bundle "$ARTIFACT" "$T/duplicate-push.witness" "$T/duplicate-push.bundle"
make_bundle "$ARTIFACT" "$T/cross-block-push.witness" "$T/cross-block-push.bundle"
make_bounds_bundle "$T/call-bounds-probe.witness" "$T/call-bounds-probe.bundle"
make_bounds_bundle "$T/call-bounds-root.witness" "$T/call-bounds-root.bundle"

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
apply_tape_patch literal-value
apply_tape_patch literal-register
apply_tape_patch arithmetic-opcode
apply_tape_patch arithmetic-pop-step
apply_tape_patch arithmetic-register
apply_tape_patch comparison-opcode
apply_tape_patch comparison-operand
apply_tape_patch comparison-branch-target
apply_tape_patch comparison-result
apply_tape_patch comparison-pop-step
apply_tape_patch push-step
apply_tape_patch push-stack-register
apply_tape_patch push-value-register
apply_tape_patch push-opcode

cat "$GATE_DIR/bc-block-control.alpha" \
  "$GATE_DIR/bc-effect-sites.alpha" \
  "$GATE_DIR/bc-frame-shape.alpha" \
  "$GATE_DIR/bc-local-access.alpha" \
  "$GATE_DIR/bc-memory-sites.alpha" \
  "$GATE_DIR/bc-expr-primitives.alpha" \
  "$GATE_DIR/bc-stack-pushes.alpha" \
  "$GATE_DIR/bc-expr-composition.alpha" \
  "$GATE_DIR/bc-raw-load-families.alpha" \
  "$GATE_DIR/bc-call-bounds.alpha" \
  "$GATE_DIR/bc-stack-register-custody.alpha" \
  "$GATE_DIR/bc-ranged-store-bounds.alpha" \
  "$GATE_DIR/bc-frame-summary.alpha" \
  "$GATE_DIR/bc-ranged-store-transfer.alpha" \
  "$GATE_DIR/bc-counter-transfer.alpha" \
  "$GATE_DIR/bc-stack-potential-lift.alpha" > "$T/control-check.alpha"
"$ASM" < "$T/control-check.alpha" > "$T/control-check.tape"
stamp_seed "$T/control-check.tape" "$SEED" "$T/control-check" >/dev/null

# Phase-isolated fixed-load tooth: keep the exact source, artifact, witness,
# grammar counts, and every prior phase unchanged while omitting one load-class
# owner.  The exhaustive 95-row family scan must find the unclassified load.
sed '/call composition_load_parse_fixed/{n;s/imm r6, 1/imm r6, 0/;}' \
  "$T/control-check.alpha" > "$T/load-missing-class.alpha"
"$ASM" < "$T/load-missing-class.alpha" > "$T/load-missing-class.tape"
stamp_seed "$T/load-missing-class.tape" "$SEED" "$T/load-missing-class" >/dev/null

# Phase-isolated tooth: leave the exact source, tape, witness, and every prior
# checker phase unchanged, but underreport the prelude fp owner. Adjust only the
# derived-map subtotal so rejection must come from the exhaustive equality scan.
sed \
  -e '/imm r0, 10/{n;s/call stack_owner_mark/call stack_owner_skip_mark/;}' \
  -e '/stack_owner_count:/,/stack_scan_init/{s/imm r3, 2630/imm r3, 2629/;}' \
  "$T/control-check.alpha" > "$T/stack-missing-owner.alpha"
"$ASM" < "$T/stack-missing-owner.alpha" > "$T/stack-missing-owner.tape"
stamp_seed "$T/stack-missing-owner.tape" "$SEED" "$T/stack-missing-owner" >/dev/null

# Phase-isolated ranged-store teeth. One drops the sole source-range class;
# the other underreports the loop invariant while leaving every prior phase and
# the exact source/artifact/witness unchanged.
sed '/composition_store_source_range:/,/jmp composition_store_ranged_count/{s/imm r6, 2/imm r6, 0/;}' \
  "$T/control-check.alpha" > "$T/ranged-missing-class.alpha"
"$ASM" < "$T/ranged-missing-class.alpha" > "$T/ranged-missing-class.tape"
stamp_seed "$T/ranged-missing-class.tape" "$SEED" "$T/ranged-missing-class" >/dev/null
sed '/ranged_interval_loop_candidate:/,/call ranged_interval_store/{s/imm r22, 1048576/imm r22, 1048575/;}' \
  "$T/control-check.alpha" > "$T/ranged-underreported-loop.alpha"
"$ASM" < "$T/ranged-underreported-loop.alpha" > "$T/ranged-underreported-loop.tape"
stamp_seed "$T/ranged-underreported-loop.tape" "$SEED" "$T/ranged-underreported-loop" >/dev/null

# Phase-isolated transfer teeth: misjoin n's address use to c's PC, or
# underreport the selected procedures' real 32-byte relative frame depth.
sed '/; slurp locals:/,/; declare snapshot/{s/imm r24, 446/imm r24, 503/;}' \
  "$T/control-check.alpha" > "$T/transfer-wrong-local.alpha"
"$ASM" < "$T/transfer-wrong-local.alpha" > "$T/transfer-wrong-local.tape"
stamp_seed "$T/transfer-wrong-local.tape" "$SEED" "$T/transfer-wrong-local" >/dev/null
sed '/transfer_frame_push:/,/jmp transfer_frame_next3/{s/imm r1, 33/imm r1, 25/;}' \
  "$T/control-check.alpha" > "$T/transfer-shallow-frame.alpha"
"$ASM" < "$T/transfer-shallow-frame.alpha" > "$T/transfer-shallow-frame.tape"
stamp_seed "$T/transfer-shallow-frame.tape" "$SEED" "$T/transfer-shallow-frame" >/dev/null
sed '/transfer_value_add_src_now:/,/call transfer_value_set/{s/imm r20, 5/imm r20, 10/;}' \
  "$T/control-check.alpha" > "$T/transfer-wrong-value-tag.alpha"
"$ASM" < "$T/transfer-wrong-value-tag.alpha" > "$T/transfer-wrong-value-tag.tape"
stamp_seed "$T/transfer-wrong-value-tag.tape" "$SEED" "$T/transfer-wrong-value-tag" >/dev/null

# Phase-isolated frame-summary teeth.  The first omits one saved-fp owner while
# adjusting only the derived subtotals, so the independent 607-store scan must
# reject it.  The second underreports every procedure's checked local peak.
sed \
  -e '/fs_store_saved_entry:/,/fs_store_saved_next:/{s/call fs_store_mark/call fs_store_skip_first_saved/;}' \
  -e '/fs_store_counts:/,/fs_store_saved_count:/{s/imm r3, 607/imm r3, 606/;}' \
  -e '/fs_store_saved_count:/,/fs_store_push_count:/{s/imm r3, 70/imm r3, 69/;}' \
  "$T/control-check.alpha" > "$T/frame-missing-store-owner.alpha"
"$ASM" < "$T/frame-missing-store-owner.alpha" > "$T/frame-missing-store-owner.tape"
stamp_seed "$T/frame-missing-store-owner.tape" "$SEED" "$T/frame-missing-store-owner" >/dev/null
sed '/fs_proc_expected_peak:/,/store r1, r4/{s/imm r1, 8/imm r1, 0/;}' \
  "$T/control-check.alpha" > "$T/frame-underreported-peak.alpha"
"$ASM" < "$T/frame-underreported-peak.alpha" > "$T/frame-underreported-peak.tape"
stamp_seed "$T/frame-underreported-peak.tape" "$SEED" "$T/frame-underreported-peak" >/dev/null

# Phase-isolated resource/potential teeth.  The first breaks the checked
# BCS9-row/live-counter relation, the second undercounts one protected writer,
# and the third underreports only the final exact root instantiation.
sed '/counter_context_build_row:/,/store r1, r21/{s/imm r21, 64/imm r21, 63/;}' \
  "$T/control-check.alpha" > "$T/counter-wrong-context.alpha"
"$ASM" < "$T/counter-wrong-context.alpha" > "$T/counter-wrong-context.tape"
stamp_seed "$T/counter-wrong-context.tape" "$SEED" "$T/counter-wrong-context" >/dev/null
sed '/counter_writer_resource_count:/,/counter_writer_counts_ok:/{s/imm r1, 7/imm r1, 6/;}' \
  "$T/control-check.alpha" > "$T/counter-missing-writer.alpha"
"$ASM" < "$T/counter-missing-writer.alpha" > "$T/counter-missing-writer.tape"
stamp_seed "$T/counter-missing-writer.tape" "$SEED" "$T/counter-missing-writer" >/dev/null
sed '/stack_lift_instantiate_main:/,/stack_lift_main_hidden:/{s/imm r1, 12720/imm r1, 12712/;}' \
  "$T/control-check.alpha" > "$T/stack-underreported-root.alpha"
"$ASM" < "$T/stack-underreported-root.alpha" > "$T/stack-underreported-root.tape"
stamp_seed "$T/stack-underreported-root.tape" "$SEED" "$T/stack-underreported-root" >/dev/null

# Preserve a diagnostic projection of the immediately preceding flat-custody
# phase.  The same-valued composition witness below must pass this projection
# and fail only once grammar-directed recursive ordering is enabled.
sed 's/jeq r2, r3, composition_scan_init/jeq r2, r3, scan_owned_effects_init/' \
  "$GATE_DIR/bc-stack-pushes.alpha" > "$T/bc-stack-pushes-flat.alpha"
cat "$GATE_DIR/bc-block-control.alpha" \
  "$GATE_DIR/bc-effect-sites.alpha" \
  "$GATE_DIR/bc-frame-shape.alpha" \
  "$GATE_DIR/bc-local-access.alpha" \
  "$GATE_DIR/bc-memory-sites.alpha" \
  "$GATE_DIR/bc-expr-primitives.alpha" \
  "$T/bc-stack-pushes-flat.alpha" \
  "$GATE_DIR/bc-call-bounds.alpha" \
  "$GATE_DIR/bc-stack-register-custody.alpha" \
  "$GATE_DIR/bc-ranged-store-bounds.alpha" \
  "$GATE_DIR/bc-frame-summary.alpha" \
  "$GATE_DIR/bc-ranged-store-transfer.alpha" \
  "$GATE_DIR/bc-counter-transfer.alpha" \
  "$GATE_DIR/bc-stack-potential-lift.alpha" > "$T/flat-check.alpha"
"$ASM" < "$T/flat-check.alpha" > "$T/flat-check.tape"
stamp_seed "$T/flat-check.tape" "$SEED" "$T/flat-check" >/dev/null

# Projection immediately before the ranged-store induction. Coherent source /
# artifact mutations below must retain every earlier custody and stack-bound
# fact, then fail only after the carried-value phase is enabled.
sed 's/jeq r2, r3, ranged_bounds_init/jeq r2, r3, scan_owned_effects_init/' \
  "$GATE_DIR/bc-stack-register-custody.alpha" > "$T/bc-stack-register-pre-ranged.alpha"
cat "$GATE_DIR/bc-block-control.alpha" \
  "$GATE_DIR/bc-effect-sites.alpha" \
  "$GATE_DIR/bc-frame-shape.alpha" \
  "$GATE_DIR/bc-local-access.alpha" \
  "$GATE_DIR/bc-memory-sites.alpha" \
  "$GATE_DIR/bc-expr-primitives.alpha" \
  "$GATE_DIR/bc-stack-pushes.alpha" \
  "$GATE_DIR/bc-expr-composition.alpha" \
  "$GATE_DIR/bc-raw-load-families.alpha" \
  "$GATE_DIR/bc-call-bounds.alpha" \
  "$T/bc-stack-register-pre-ranged.alpha" > "$T/pre-ranged-check.alpha"
"$ASM" < "$T/pre-ranged-check.alpha" > "$T/pre-ranged-check.tape"
stamp_seed "$T/pre-ranged-check.tape" "$SEED" "$T/pre-ranged-check" >/dev/null

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

coherent_ranged_mutant() { # label
  ranged_label=$1
  case "$ranged_label" in
    slurp-cap)
      sed 's/to full when (n == 1048576)/to full when (n == 1048577)/' \
        "$SOURCE" > "$T/$ranged_label.beta"
      ;;
    declare-cap)
      sed 's/to write when (s < 1024)/to write when (s <= 1024)/' \
        "$SOURCE" > "$T/$ranged_label.beta"
      ;;
    nloc-step)
      sed 's/word\[2097128\] = s + 1/word[2097128] = s - 1/' \
        "$SOURCE" > "$T/$ranged_label.beta"
      ;;
    *)
      exit 2
      ;;
  esac
  "$T/bc" < "$T/$ranged_label.beta" > "$T/$ranged_label.alpha"
  "$ASM" < "$T/$ranged_label.alpha" > "$T/$ranged_label.tape"
  python3 "$GATE_DIR/bc_block_control_map.py" \
    --repo "$OMEGA_REPO_ROOT" \
    --source "$T/$ranged_label.beta" \
    --assembly "$T/$ranged_label.alpha" \
    --tape "$T/$ranged_label.tape" \
    --output "$T/$ranged_label.witness"
  ranged_source_len=$(wc -c < "$T/$ranged_label.beta" | tr -d ' ')
  ranged_tape_len=$(wc -c < "$T/$ranged_label.tape" | tr -d ' ')
  u32_file "$ranged_source_len" "$T/$ranged_label-source.len"
  u32_file "$ranged_tape_len" "$T/$ranged_label-tape.len"
  cat "$T/$ranged_label-source.len" "$T/$ranged_label.beta" \
    "$T/$ranged_label-tape.len" "$T/$ranged_label.tape" \
    "$T/$ranged_label.witness" "$T/call-bounds.witness" \
    > "$T/$ranged_label.bundle"

  set +e
  "$T/pre-ranged-check" < "$T/$ranged_label.bundle" > "$T/stdout"
  ranged_pre_status=$?
  set -e
  if [ "$ranged_pre_status" != 0 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $ranged_label did not preserve the pre-induction projection" >&2
    exit 1
  fi
  case_run "unsafe ranged-store induction: $ranged_label" 1 "$T/$ranged_label.bundle"
}

case_run "whole compiler control skeleton" 0 "$T/control.bundle"
set +e
"$T/load-missing-class" < "$T/control.bundle" > "$T/stdout"
load_missing_class_status=$?
set -e
if [ "$load_missing_class_status" != 1 ] || [ -s "$T/stdout" ]; then
  echo "bc block control FAIL — missing fixed raw-load class was not rejected" >&2
  exit 1
fi
coherent_ranged_mutant slurp-cap
coherent_ranged_mutant declare-cap
coherent_ranged_mutant nloc-step
case_run "underreported rejected recursive probe" 1 "$T/call-bounds-probe.bundle"
case_run "underreported root stack bound" 1 "$T/call-bounds-root.bundle"
set +e
"$T/stack-missing-owner" < "$T/control.bundle" > "$T/stdout"
missing_stack_owner_status=$?
set -e
if [ "$missing_stack_owner_status" != 1 ] || [ -s "$T/stdout" ]; then
  echo "bc block control FAIL — underreported stack owner: expected 1/empty, got $missing_stack_owner_status/$(wc -c < "$T/stdout" | tr -d ' ') bytes" >&2
  exit 1
fi
set +e
"$T/ranged-missing-class" < "$T/control.bundle" > "$T/stdout"
ranged_missing_class_status=$?
set -e
if [ "$ranged_missing_class_status" != 1 ] || [ -s "$T/stdout" ]; then
  echo "bc block control FAIL — missing ranged-store class was not rejected" >&2
  exit 1
fi
set +e
"$T/ranged-underreported-loop" < "$T/control.bundle" > "$T/stdout"
ranged_underreported_loop_status=$?
set -e
if [ "$ranged_underreported_loop_status" != 1 ] || [ -s "$T/stdout" ]; then
  echo "bc block control FAIL — underreported ranged loop invariant was not rejected" >&2
  exit 1
fi
set +e
"$T/transfer-wrong-local" < "$T/control.bundle" > "$T/stdout"
transfer_wrong_local_status=$?
set -e
if [ "$transfer_wrong_local_status" != 1 ] || [ -s "$T/stdout" ]; then
  echo "bc block control FAIL — wrong ranged-store local transfer was not rejected" >&2
  exit 1
fi
set +e
"$T/transfer-shallow-frame" < "$T/control.bundle" > "$T/stdout"
transfer_shallow_frame_status=$?
set -e
if [ "$transfer_shallow_frame_status" != 1 ] || [ -s "$T/stdout" ]; then
  echo "bc block control FAIL — underreported selected frame depth was not rejected" >&2
  exit 1
fi
set +e
"$T/transfer-wrong-value-tag" < "$T/control.bundle" > "$T/stdout"
transfer_wrong_value_tag_status=$?
set -e
if [ "$transfer_wrong_value_tag_status" != 1 ] || [ -s "$T/stdout" ]; then
  echo "bc block control FAIL — wrong ranged-store value tag was not rejected" >&2
  exit 1
fi
set +e
"$T/frame-missing-store-owner" < "$T/control.bundle" > "$T/stdout"
frame_missing_store_owner_status=$?
set -e
if [ "$frame_missing_store_owner_status" != 1 ] || [ -s "$T/stdout" ]; then
  echo "bc block control FAIL — missing frame-summary store owner was not rejected" >&2
  exit 1
fi
set +e
"$T/frame-underreported-peak" < "$T/control.bundle" > "$T/stdout"
frame_underreported_peak_status=$?
set -e
if [ "$frame_underreported_peak_status" != 1 ] || [ -s "$T/stdout" ]; then
  echo "bc block control FAIL — underreported procedure-local frame peak was not rejected" >&2
  exit 1
fi
set +e
"$T/counter-wrong-context" < "$T/control.bundle" > "$T/stdout"
counter_wrong_context_status=$?
set -e
if [ "$counter_wrong_context_status" != 1 ] || [ -s "$T/stdout" ]; then
  echo "bc block control FAIL — wrong counter/potential context relation was not rejected" >&2
  exit 1
fi
set +e
"$T/counter-missing-writer" < "$T/control.bundle" > "$T/stdout"
counter_missing_writer_status=$?
set -e
if [ "$counter_missing_writer_status" != 1 ] || [ -s "$T/stdout" ]; then
  echo "bc block control FAIL — undercounted protected writer was not rejected" >&2
  exit 1
fi
set +e
"$T/stack-underreported-root" < "$T/control.bundle" > "$T/stdout"
stack_underreported_root_status=$?
set -e
if [ "$stack_underreported_root_status" != 1 ] || [ -s "$T/stdout" ]; then
  echo "bc block control FAIL — underreported absolute stack root was not rejected" >&2
  exit 1
fi
flat_case() { # label input
  set +e
  "$T/flat-check" < "$2" > "$T/stdout"
  flat_composition_status=$?
  set -e
  if [ "$flat_composition_status" != 0 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $1 did not preserve flat custody" >&2
    exit 1
  fi
}
flat_case "recursive-order witness" "$T/composition-order.bundle"
flat_case "argument-order witness" "$T/composition-argument-order.bundle"
flat_case "store-order witness" "$T/composition-store-order.bundle"
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
case_run "literal value" 1 "$T/literal-value.bundle"
case_run "literal destination register" 1 "$T/literal-register.bundle"
case_run "arithmetic opcode" 1 "$T/arithmetic-opcode.bundle"
case_run "arithmetic pop step" 1 "$T/arithmetic-pop-step.bundle"
case_run "arithmetic destination register" 1 "$T/arithmetic-register.bundle"
case_run "duplicate expression primitive location" 1 "$T/duplicate-primitive.bundle"
case_run "noncanonical expression primitive order" 1 "$T/noncanonical-primitive.bundle"
case_run "same-valued synthetic literal location" 1 "$T/synthetic-literal.bundle"
case_run "same-valued recursive expression order" 1 "$T/composition-order.bundle"
case_run "ordinary-call argument composition order" 1 "$T/composition-argument-order.bundle"
case_run "store address/value composition order" 1 "$T/composition-store-order.bundle"
case_run "comparison branch opcode" 1 "$T/comparison-opcode.bundle"
case_run "comparison branch operand order" 1 "$T/comparison-operand.bundle"
case_run "comparison branch target" 1 "$T/comparison-branch-target.bundle"
case_run "comparison materialized result" 1 "$T/comparison-result.bundle"
case_run "comparison pop step" 1 "$T/comparison-pop-step.bundle"
case_run "argument push stack step" 1 "$T/push-step.bundle"
case_run "argument push stack register" 1 "$T/push-stack-register.bundle"
case_run "argument push value register" 1 "$T/push-value-register.bundle"
case_run "same-width argument push opcode" 1 "$T/push-opcode.bundle"
case_run "duplicate stack-push location" 1 "$T/duplicate-push.bundle"
case_run "cross-block stack-push location" 1 "$T/cross-block-push.bundle"

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

for mutation in call-retarget read-register write-register helper-write emit-byte emit-length emit-pointer emit-helper orphan-io frame-size saved-fp frame-base param-offset param-register call-pop-order call-pop-step local-load-slot local-store-slot local-base local-load-opcode local-store-opcode memory-load-width memory-store-width memory-load-register memory-store-register memory-pop-step literal-value literal-register arithmetic-opcode arithmetic-pop-step arithmetic-register comparison-opcode comparison-operand comparison-branch-target comparison-result comparison-pop-step push-step push-stack-register push-value-register push-opcode; do
  set +e
  "$T/structure-check" < "$T/$mutation.tape" > "$T/stdout"
  structure_status=$?
  set -e
  if [ "$structure_status" != 0 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $mutation was not a structurally valid mutation" >&2
    exit 1
  fi
done

echo "bc block control/effects: 70 proc / 355 block / 291 transition; 613 effect sites / 829 fixed emit bytes; 78 frame slots / 27 parameter stores / 134 call pops; 169 local loads / 73 local stores; 61 raw loads = 54 fixed-safe + 5 SRC-indexed + 2 table-indexed / 34 raw stores; 581 literals / 55 arithmetic / 180 comparison primitives; 235 binary / 134 argument / 34 store-address pushes; syntax-directed composition / relative temporary peak 2; three ranged Alpha operands transferred; all 607 stores partitioned / 70 call-cut frames summarized; 64-row counter contexts; absolute B_bc1 stack <=12720 explicit bytes / <=662 hidden returns; all 2630 explicit-stack effects and 687 artifact effects owned ($(wc -c < "$T/control-check.tape" | tr -d ' ')-byte Alpha checker tape)"
