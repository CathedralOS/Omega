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
. "$GATE_DIR/bc-count-lets-teeth.sh"
. "$GATE_DIR/bc-parse-parameter-teeth.sh"
. "$GATE_DIR/bc-parse-capacity-teeth.sh"
. "$GATE_DIR/bc-emit-ident-teeth.sh"
. "$GATE_DIR/bc-emit-dec-teeth.sh"
. "$GATE_DIR/bc-fixed-decimal-emitters-teeth.sh"
. "$GATE_DIR/bc-parse-output-prefix-teeth.sh"
. "$GATE_DIR/bc-gen-stmts-boundary-teeth.sh"
. "$GATE_DIR/bc-parse-number-teeth.sh"
. "$GATE_DIR/bc-parse-char-teeth.sh"
. "$GATE_DIR/bc-operator-classifier-teeth.sh"
. "$GATE_DIR/bc-cmp-op-teeth.sh"
. "$GATE_DIR/bc-fixed-keyword-teeth.sh"
. "$GATE_DIR/bc-checker-split-teeth.sh"
. "$GATE_DIR/bc-name-eq-teeth.sh"
. "$GATE_DIR/bc-lookup-teeth.sh"
. "$GATE_DIR/bc-bounded-emitters-teeth.sh"
. "$GATE_DIR/bc-emit-dec-word-teeth.sh"
. "$GATE_DIR/bc-label-emitters-teeth.sh"
. "$GATE_DIR/bc-expression-family-teeth.sh"
. "$GATE_DIR/bc-statement-family-teeth.sh"
. "$GATE_DIR/bc-statement-family-semantic-teeth.sh"
. "$GATE_DIR/bc-parse-body-teeth.sh"
. "$GATE_DIR/bc-resource-classification-teeth.sh"
. "$GATE_DIR/bc-declaration-budget-teeth.sh"
. "$GATE_DIR/bc-parse-proc-teeth.sh"
. "$GATE_DIR/bc-root-observation-teeth.sh"

bc_timing_start() { # phase
  BC_TIMING_PHASE=$1
  BC_TIMING_STARTED=$(date +%s)
}

bc_timing_finish() {
  BC_TIMING_FINISHED=$(date +%s)
  BC_TIMING_SECONDS=$((BC_TIMING_FINISHED - BC_TIMING_STARTED))
  echo "bc timing: $BC_TIMING_PHASE ${BC_TIMING_SECONDS}s"
}

bc_timing_start setup-and-witnesses

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
CONTROL_BUNDLE_CKSUM=$(cksum < "$T/control.bundle")
require_control_bundle_unchanged() {
  control_bundle_now=$(cksum < "$T/control.bundle")
  if [ "$control_bundle_now" != "$CONTROL_BUNDLE_CKSUM" ]; then
    echo "bc block control FAIL — canonical control.bundle changed between owners" >&2
    exit 1
  fi
}

# The root observable excludes invalid-opcode execution only after the exact
# persisted artifact has passed the independent reachable-structure checker.
# Establish that owner before any focused mode may exit; the historical matrix
# below reuses the same executable for its structurally valid mutations.
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
BC_OWNER_ARTIFACT_STRUCTURE=1
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
bc_timing_finish

emit_stack_checker_prefix() {
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
    "$GATE_DIR/bc-stack-potential-lift.alpha"
}

emit_name_eq_checker_prefix() {
  emit_stack_checker_prefix
  cat \
    "$GATE_DIR/bc-post-stack-name-eq.alpha" \
    "$GATE_DIR/bc-exact-shape-helpers.alpha" \
    "$GATE_DIR/bc-name-table-domain.alpha" \
    "$GATE_DIR/bc-name-eq-control-shape.alpha" \
    "$GATE_DIR/bc-name-eq-data-shape.alpha" \
    "$GATE_DIR/bc-name-eq-summary.alpha"
}

build_name_eq_checker() {
  {
    emit_name_eq_checker_prefix
    cat "$GATE_DIR/bc-post-name-eq-base.alpha"
  } > "$T/name-eq-check.alpha"
  "$ASM" < "$T/name-eq-check.alpha" > "$T/name-eq-check.tape"
  stamp_seed "$T/name-eq-check.tape" "$SEED" "$T/name-eq-check" >/dev/null
}

build_lookup_checker() {
  {
    emit_name_eq_checker_prefix
    cat "$GATE_DIR/bc-post-name-eq-lookup.alpha" \
      "$GATE_DIR/bc-lookup-control-shape.alpha" \
      "$GATE_DIR/bc-lookup-data-shape.alpha" \
      "$GATE_DIR/bc-lookup-summary.alpha"
  } > "$T/lookup-check.alpha"
  "$ASM" < "$T/lookup-check.alpha" > "$T/lookup-check.tape"
  stamp_seed "$T/lookup-check.tape" "$SEED" "$T/lookup-check" >/dev/null
}

build_bounded_emitters_checker() {
  {
    emit_stack_checker_prefix
    cat "$GATE_DIR/bc-post-stack-bounded-emitters.alpha" \
      "$GATE_DIR/bc-exact-shape-helpers.alpha" \
      "$GATE_DIR/bc-write-str-event-helper.alpha" \
      "$GATE_DIR/bc-write-str-summary.alpha" \
      "$GATE_DIR/bc-post-write-str-bounded-emitters.alpha" \
      "$GATE_DIR/bc-cursor-leaf-summary.alpha" \
      "$GATE_DIR/bc-skip-ws-summary.alpha" \
      "$GATE_DIR/bc-post-skip-ws-bounded-emitters.alpha" \
      "$GATE_DIR/bc-expect-shape.alpha" \
      "$GATE_DIR/bc-expect-summary.alpha" \
      "$GATE_DIR/bc-post-expect-bounded-emitters.alpha" \
      "$GATE_DIR/bc-emit-dec-shape.alpha" \
      "$GATE_DIR/bc-emit-dec-summary.alpha" \
      "$GATE_DIR/bc-post-emit-dec-bounded-emitters.alpha" \
      "$GATE_DIR/bc-bounded-emitters-control-shape.alpha" \
      "$GATE_DIR/bc-bounded-emitters-data-shape.alpha" \
      "$GATE_DIR/bc-bounded-emitters-summary.alpha" \
      "$GATE_DIR/bc-bounded-emitters-slot-summary.alpha" \
      "$GATE_DIR/bc-bounded-emitters-publication.alpha"
  } > "$T/bounded-emitters-check.alpha"
  "$ASM" < "$T/bounded-emitters-check.alpha" > "$T/bounded-emitters-check.tape"
  stamp_seed "$T/bounded-emitters-check.tape" "$SEED" \
    "$T/bounded-emitters-check" >/dev/null
}

build_emit_dec_word_checker() {
  {
    emit_stack_checker_prefix
    cat "$GATE_DIR/bc-post-stack-emit-dec-word.alpha" \
      "$GATE_DIR/bc-exact-shape-helpers.alpha" \
      "$GATE_DIR/bc-emit-dec-shape.alpha" \
      "$GATE_DIR/bc-emit-dec-word-domain.alpha" \
      "$GATE_DIR/bc-emit-dec-word-summary.alpha" \
      "$GATE_DIR/bc-emit-dec-word-publication.alpha"
  } > "$T/emit-dec-word-check.alpha"
  "$ASM" < "$T/emit-dec-word-check.alpha" > "$T/emit-dec-word-check.tape"
  stamp_seed "$T/emit-dec-word-check.tape" "$SEED" \
    "$T/emit-dec-word-check" >/dev/null
}

label_emitters_require_module_budgets() {
  for label_emitters_module in \
    bc-cursor-tail-summary.alpha \
    bc-label-core-shape.alpha \
    bc-label-counter-summary.alpha \
    bc-label-ref-summary.alpha \
    bc-emit-str-body-shape.alpha \
    bc-emit-str-body-cases.alpha \
    bc-emit-str-body-summary.alpha \
    bc-gen-emit-shape.alpha \
    bc-gen-emit-summary.alpha \
    bc-emit-cmp-control-shape.alpha \
    bc-emit-cmp-data-shape.alpha \
    bc-emit-cmp-cases.alpha \
    bc-emit-cmp-summary.alpha \
    bc-label-emitters-publication.alpha \
    bc-post-label-emitters-base.alpha
  do
    label_emitters_module_bytes=$(wc -c < "$GATE_DIR/$label_emitters_module" | tr -d ' ')
    if [ "$label_emitters_module_bytes" -ge 20000 ]; then
      echo "bc block control FAIL — $label_emitters_module is ${label_emitters_module_bytes} bytes (20KB module cap)" >&2
      exit 1
    fi
  done
}

build_label_emitters_checker() {
  label_emitters_require_module_budgets
  {
    emit_stack_checker_prefix
    cat "$GATE_DIR/bc-post-stack-label-emitters.alpha" \
      "$GATE_DIR/bc-exact-shape-helpers.alpha" \
      "$GATE_DIR/bc-write-str-event-helper.alpha" \
      "$GATE_DIR/bc-write-str-summary.alpha" \
      "$GATE_DIR/bc-post-write-str-label-emitters.alpha" \
      "$GATE_DIR/bc-cursor-leaf-summary.alpha" \
      "$GATE_DIR/bc-skip-ws-summary.alpha" \
      "$GATE_DIR/bc-post-skip-ws-label-emitters.alpha" \
      "$GATE_DIR/bc-expect-shape.alpha" \
      "$GATE_DIR/bc-expect-summary.alpha" \
      "$GATE_DIR/bc-post-expect-label-emitters.alpha" \
      "$GATE_DIR/bc-emit-dec-shape.alpha" \
      "$GATE_DIR/bc-emit-dec-word-domain.alpha" \
      "$GATE_DIR/bc-emit-dec-word-summary.alpha" \
      "$GATE_DIR/bc-emit-dec-word-label-publication.alpha" \
      "$GATE_DIR/bc-cursor-tail-summary.alpha" \
      "$GATE_DIR/bc-label-core-shape.alpha" \
      "$GATE_DIR/bc-label-counter-summary.alpha" \
      "$GATE_DIR/bc-label-ref-summary.alpha" \
      "$GATE_DIR/bc-emit-str-body-shape.alpha" \
      "$GATE_DIR/bc-emit-str-body-cases.alpha" \
      "$GATE_DIR/bc-emit-str-body-summary.alpha" \
      "$GATE_DIR/bc-gen-emit-shape.alpha" \
      "$GATE_DIR/bc-gen-emit-summary.alpha" \
      "$GATE_DIR/bc-emit-cmp-control-shape.alpha" \
      "$GATE_DIR/bc-emit-cmp-data-shape.alpha" \
      "$GATE_DIR/bc-emit-cmp-cases.alpha" \
      "$GATE_DIR/bc-emit-cmp-summary.alpha" \
      "$GATE_DIR/bc-label-emitters-publication.alpha" \
      "$GATE_DIR/bc-post-label-emitters-base.alpha"
  } > "$T/label-emitters-check.alpha"
  label_emitters_checker_source_bytes=$(wc -c < "$T/label-emitters-check.alpha" | tr -d ' ')
  if [ "$label_emitters_checker_source_bytes" -ge 900000 ]; then
    echo "bc block control FAIL — Checker E source is ${label_emitters_checker_source_bytes} bytes (900KB budget)" >&2
    exit 1
  fi
  "$ASM" < "$T/label-emitters-check.alpha" > "$T/label-emitters-check.tape"
  python3 "$OMEGA_PATH_ALPHA_ASSEMBLER/asm_ref.py" \
    < "$T/label-emitters-check.alpha" > "$T/label-emitters-check-ref.tape"
  if ! cmp -s "$T/label-emitters-check.tape" "$T/label-emitters-check-ref.tape"; then
    echo "bc block control FAIL — Checker E assembler diamond disagrees" >&2
    exit 1
  fi
  stamp_seed "$T/label-emitters-check.tape" "$SEED" \
    "$T/label-emitters-check" >/dev/null
}

emit_expression_table_prefix() {
  cat "$GATE_DIR/bc-block-control.alpha" \
    "$GATE_DIR/bc-effect-sites.alpha" \
    "$GATE_DIR/bc-frame-shape.alpha" \
    "$GATE_DIR/bc-local-access.alpha" \
    "$GATE_DIR/bc-memory-sites.alpha" \
    "$GATE_DIR/bc-expr-primitives.alpha" \
    "$GATE_DIR/bc-stack-pushes.alpha" \
    "$GATE_DIR/bc-expr-composition.alpha" \
    "$GATE_DIR/bc-raw-load-families.alpha" \
    "$GATE_DIR/bc-call-bounds.alpha"
}

expression_family_require_module_budgets() {
  for expression_family_module in \
    bc-expression-selected-row-helpers.alpha \
    bc-expression-leaf-shape.alpha \
    bc-expression-call-control-shape.alpha \
    bc-expression-call-data-shape.alpha \
    bc-expression-factor-control-shape.alpha \
    bc-expression-factor-data-shape.alpha \
    bc-expression-levels-shape.alpha \
    bc-expression-gen-expr-shape.alpha \
    bc-expression-leaf-rules.alpha \
    bc-expression-call-rules.alpha \
    bc-expression-factor-rules.alpha \
    bc-expression-levels-rules.alpha \
    bc-expression-gen-expr-rules.alpha \
    bc-expression-family-publication.alpha
  do
    expression_family_module_bytes=$(wc -c < "$GATE_DIR/$expression_family_module" | tr -d ' ')
    if [ "$expression_family_module_bytes" -ge 20000 ]; then
      echo "bc block control FAIL — $expression_family_module is ${expression_family_module_bytes} bytes (20KB module cap)" >&2
      exit 1
    fi
  done
}

build_expression_family_shape_checker() {
  {
    emit_expression_table_prefix
    cat "$GATE_DIR/bc-expression-shape-root.alpha" \
      "$GATE_DIR/bc-expression-selected-row-helpers.alpha" \
      "$GATE_DIR/bc-exact-shape-helpers.alpha" \
      "$GATE_DIR/bc-expression-leaf-shape.alpha" \
      "$GATE_DIR/bc-expression-call-control-shape.alpha" \
      "$GATE_DIR/bc-expression-call-data-shape.alpha" \
      "$GATE_DIR/bc-expression-factor-control-shape.alpha" \
      "$GATE_DIR/bc-expression-factor-data-shape.alpha" \
      "$GATE_DIR/bc-expression-levels-shape.alpha" \
      "$GATE_DIR/bc-expression-gen-expr-shape.alpha" \
      "$GATE_DIR/bc-expression-family-shape.alpha"
  } > "$T/expression-family-shape.alpha"
  "$ASM" < "$T/expression-family-shape.alpha" > "$T/expression-family-shape.tape"
  python3 "$OMEGA_PATH_ALPHA_ASSEMBLER/asm_ref.py" \
    < "$T/expression-family-shape.alpha" > "$T/expression-family-shape-ref.tape"
  cmp -s "$T/expression-family-shape.tape" "$T/expression-family-shape-ref.tape" || {
    echo "bc block control FAIL — expression shape assembler diamond disagrees" >&2
    exit 1
  }
  stamp_seed "$T/expression-family-shape.tape" "$SEED" \
    "$T/expression-family-shape" >/dev/null
}

build_expression_family_semantic_checker() {
  {
    emit_expression_table_prefix
    cat "$GATE_DIR/bc-expression-root.alpha" \
      "$GATE_DIR/bc-expression-selected-row-helpers.alpha" \
      "$GATE_DIR/bc-exact-shape-helpers.alpha" \
      "$GATE_DIR/bc-write-str-event-helper.alpha" \
      "$GATE_DIR/bc-write-str-summary.alpha" \
      "$GATE_DIR/bc-post-write-str-label-emitters.alpha" \
      "$GATE_DIR/bc-cursor-leaf-summary.alpha" \
      "$GATE_DIR/bc-skip-ws-summary.alpha" \
      "$GATE_DIR/bc-post-skip-ws-label-emitters.alpha" \
      "$GATE_DIR/bc-expect-shape.alpha" \
      "$GATE_DIR/bc-expect-summary.alpha" \
      "$GATE_DIR/bc-post-expect-expression.alpha" \
      "$GATE_DIR/bc-emit-dec-shape.alpha" \
      "$GATE_DIR/bc-emit-dec-word-domain.alpha" \
      "$GATE_DIR/bc-emit-dec-word-summary.alpha" \
      "$GATE_DIR/bc-emit-dec-word-label-publication.alpha" \
      "$GATE_DIR/bc-cursor-tail-summary.alpha" \
      "$GATE_DIR/bc-label-core-shape.alpha" \
      "$GATE_DIR/bc-label-counter-summary.alpha" \
      "$GATE_DIR/bc-label-ref-summary.alpha" \
      "$GATE_DIR/bc-emit-str-body-shape.alpha" \
      "$GATE_DIR/bc-emit-str-body-cases.alpha" \
      "$GATE_DIR/bc-emit-str-body-summary.alpha" \
      "$GATE_DIR/bc-gen-emit-shape.alpha" \
      "$GATE_DIR/bc-gen-emit-summary.alpha" \
      "$GATE_DIR/bc-emit-cmp-control-shape.alpha" \
      "$GATE_DIR/bc-emit-cmp-data-shape.alpha" \
      "$GATE_DIR/bc-emit-cmp-cases.alpha" \
      "$GATE_DIR/bc-emit-cmp-summary.alpha" \
      "$GATE_DIR/bc-label-emitters-publication.alpha" \
      "$GATE_DIR/bc-post-label-emitters-expression.alpha" \
      "$GATE_DIR/bc-classifier-shape.alpha" \
      "$GATE_DIR/bc-classifier-summary.alpha" \
      "$GATE_DIR/bc-read-ident-shape.alpha" \
      "$GATE_DIR/bc-read-ident-summary.alpha" \
      "$GATE_DIR/bc-emit-ident-shape.alpha" \
      "$GATE_DIR/bc-emit-ident-summary.alpha" \
      "$GATE_DIR/bc-expression-id-char.alpha" \
      "$GATE_DIR/bc-fixed-keyword-shape-core.alpha" \
      "$GATE_DIR/bc-fixed-keyword-data-shape.alpha" \
      "$GATE_DIR/bc-fixed-keyword-cases.alpha" \
      "$GATE_DIR/bc-fixed-keyword-summary.alpha" \
      "$GATE_DIR/bc-literal-skip-shape.alpha" \
      "$GATE_DIR/bc-literal-skip-summary.alpha" \
      "$GATE_DIR/bc-post-literal-skip-expression.alpha" \
      "$GATE_DIR/bc-parse-number-shape.alpha" \
      "$GATE_DIR/bc-parse-number-summary.alpha" \
      "$GATE_DIR/bc-parse-char-shape.alpha" \
      "$GATE_DIR/bc-parse-char-cases.alpha" \
      "$GATE_DIR/bc-parse-char-summary.alpha" \
      "$GATE_DIR/bc-operator-classifier-shape.alpha" \
      "$GATE_DIR/bc-operator-classifier-summary.alpha" \
      "$GATE_DIR/bc-cmp-op-shape.alpha" \
      "$GATE_DIR/bc-cmp-op-cases.alpha" \
      "$GATE_DIR/bc-cmp-op-summary.alpha" \
      "$GATE_DIR/bc-name-table-domain.alpha" \
      "$GATE_DIR/bc-name-eq-control-shape.alpha" \
      "$GATE_DIR/bc-name-eq-data-shape.alpha" \
      "$GATE_DIR/bc-name-eq-summary.alpha" \
      "$GATE_DIR/bc-post-name-eq-lookup.alpha" \
      "$GATE_DIR/bc-lookup-control-shape.alpha" \
      "$GATE_DIR/bc-lookup-data-shape.alpha" \
      "$GATE_DIR/bc-lookup-summary.alpha" \
      "$GATE_DIR/bc-emit-dec-summary.alpha" \
      "$GATE_DIR/bc-post-emit-dec-bounded-emitters.alpha" \
      "$GATE_DIR/bc-bounded-emitters-control-shape.alpha" \
      "$GATE_DIR/bc-bounded-emitters-data-shape.alpha" \
      "$GATE_DIR/bc-bounded-emitters-summary.alpha" \
      "$GATE_DIR/bc-bounded-emitters-slot-summary.alpha" \
      "$GATE_DIR/bc-bounded-emitters-publication.alpha" \
      "$GATE_DIR/bc-expression-prerequisites.alpha" \
      "$GATE_DIR/bc-expression-resource-domain.alpha" \
      "$GATE_DIR/bc-expression-tail-rules.alpha" \
      "$GATE_DIR/bc-expression-leaf-rules.alpha" \
      "$GATE_DIR/bc-expression-call-rules.alpha" \
      "$GATE_DIR/bc-expression-factor-rules.alpha" \
      "$GATE_DIR/bc-expression-levels-rules.alpha" \
      "$GATE_DIR/bc-expression-gen-expr-rules.alpha" \
      "$GATE_DIR/bc-expression-family-publication.alpha"
  } > "$T/expression-family-semantic.alpha"
  expression_semantic_source_bytes=$(wc -c < "$T/expression-family-semantic.alpha" | tr -d ' ')
  if [ "$expression_semantic_source_bytes" -ge 1040000 ]; then
    echo "bc block control FAIL — expression semantic source is ${expression_semantic_source_bytes} bytes (1040KB budget)" >&2
    exit 1
  fi
  python3 "$OMEGA_PATH_ALPHA_ASSEMBLER/asm_ref.py" \
    < "$T/expression-family-semantic.alpha" > "$T/expression-family-semantic-ref.tape"
  "$ASM" < "$T/expression-family-semantic.alpha" > "$T/expression-family-semantic.tape"
  expression_semantic_tape_bytes=$(wc -c < "$T/expression-family-semantic.tape" | tr -d ' ')
  if [ "$expression_semantic_tape_bytes" -gt 262140 ]; then
    echo "bc block control FAIL — expression semantic tape is ${expression_semantic_tape_bytes} bytes (262140-byte limit)" >&2
    exit 1
  fi
  cmp -s "$T/expression-family-semantic.tape" "$T/expression-family-semantic-ref.tape" || {
    echo "bc block control FAIL — expression semantic assembler diamond disagrees" >&2
    cmp -l "$T/expression-family-semantic.tape" \
      "$T/expression-family-semantic-ref.tape" | head -8 >&2 || true
    exit 1
  }
  stamp_seed "$T/expression-family-semantic.tape" "$SEED" \
    "$T/expression-family-semantic" >/dev/null
}

build_expression_family_checkers() {
  expression_family_require_module_budgets
  build_expression_family_shape_checker
  build_expression_family_semantic_checker
}

statement_family_require_module_budgets() {
  for statement_family_module in \
    bc-statement-family-shape-root.alpha \
    bc-statement-emit-epilogue-shape.alpha \
    bc-statement-gen-store-shape.alpha \
    bc-gen-stmts-boundary-shape.alpha \
    bc-statement-gen-block-shape.alpha \
    bc-statement-emit-state-label-shape.alpha \
    bc-statement-gen-state-shape.alpha \
    bc-statement-gen-to-shape.alpha \
    bc-statement-gen-stmt-shape.alpha \
    bc-statement-gen-stmt-data-shape.alpha \
    bc-statement-family-shape.alpha \
    bc-statement-semantic-root.alpha \
    bc-statement-antecedents.alpha \
    bc-write-str-event-helper.alpha \
    bc-statement-emit-epilogue-rules.alpha \
    bc-statement-gen-store-rules.alpha \
    bc-statement-state-label-rules.alpha \
    bc-statement-gen-to-rules.alpha \
    bc-statement-gen-stmt-rules.alpha \
    bc-statement-gen-stmt-fallback-rules.alpha \
    bc-statement-wrapper-rules.alpha \
    bc-statement-gfp-rules.alpha \
    bc-statement-family-publication.alpha
  do
    statement_family_module_bytes=$(wc -c < "$GATE_DIR/$statement_family_module" | tr -d ' ')
    if [ "$statement_family_module_bytes" -ge 20000 ]; then
      echo "bc block control FAIL — $statement_family_module is ${statement_family_module_bytes} bytes (20KB module cap)" >&2
      exit 1
    fi
  done
}

build_statement_family_shape_checker() {
  statement_family_require_module_budgets
  {
    emit_expression_table_prefix
    cat "$GATE_DIR/bc-statement-family-shape-root.alpha" \
      "$GATE_DIR/bc-expression-selected-row-helpers.alpha" \
      "$GATE_DIR/bc-exact-shape-helpers.alpha" \
      "$GATE_DIR/bc-statement-emit-epilogue-shape.alpha" \
      "$GATE_DIR/bc-statement-gen-store-shape.alpha" \
      "$GATE_DIR/bc-gen-stmts-boundary-shape.alpha" \
      "$GATE_DIR/bc-statement-gen-block-shape.alpha" \
      "$GATE_DIR/bc-statement-emit-state-label-shape.alpha" \
      "$GATE_DIR/bc-statement-gen-state-shape.alpha" \
      "$GATE_DIR/bc-statement-gen-to-shape.alpha" \
      "$GATE_DIR/bc-statement-gen-stmt-shape.alpha" \
      "$GATE_DIR/bc-statement-gen-stmt-data-shape.alpha" \
      "$GATE_DIR/bc-statement-family-shape.alpha"
  } > "$T/statement-family-shape.alpha"
  "$ASM" < "$T/statement-family-shape.alpha" > "$T/statement-family-shape.tape"
  python3 "$OMEGA_PATH_ALPHA_ASSEMBLER/asm_ref.py" \
    < "$T/statement-family-shape.alpha" > "$T/statement-family-shape-ref.tape"
  cmp -s "$T/statement-family-shape.tape" "$T/statement-family-shape-ref.tape" || {
    echo "bc block control FAIL — statement shape assembler diamond disagrees" >&2
    exit 1
  }
  statement_shape_tape_bytes=$(wc -c < "$T/statement-family-shape.tape" | tr -d ' ')
  if [ "$statement_shape_tape_bytes" -gt 262140 ]; then
    echo "bc block control FAIL — statement shape tape is ${statement_shape_tape_bytes} bytes (262140-byte limit)" >&2
    exit 1
  fi
  stamp_seed "$T/statement-family-shape.tape" "$SEED" \
    "$T/statement-family-shape" >/dev/null
}

build_statement_family_semantic_checker() {
  statement_family_require_module_budgets
  {
    emit_expression_table_prefix
    cat "$GATE_DIR/bc-statement-semantic-root.alpha" \
      "$GATE_DIR/bc-expression-selected-row-helpers.alpha" \
      "$GATE_DIR/bc-exact-shape-helpers.alpha" \
      "$GATE_DIR/bc-statement-antecedents.alpha" \
      "$GATE_DIR/bc-write-str-event-helper.alpha" \
      "$GATE_DIR/bc-statement-emit-epilogue-shape.alpha" \
      "$GATE_DIR/bc-statement-emit-epilogue-rules.alpha" \
      "$GATE_DIR/bc-statement-gen-store-shape.alpha" \
      "$GATE_DIR/bc-statement-gen-store-rules.alpha" \
      "$GATE_DIR/bc-statement-emit-state-label-shape.alpha" \
      "$GATE_DIR/bc-statement-state-label-rules.alpha" \
      "$GATE_DIR/bc-statement-gen-to-rules.alpha" \
      "$GATE_DIR/bc-statement-gen-stmt-rules.alpha" \
      "$GATE_DIR/bc-statement-gen-stmt-fallback-rules.alpha" \
      "$GATE_DIR/bc-statement-wrapper-rules.alpha" \
      "$GATE_DIR/bc-statement-gfp-rules.alpha" \
      "$GATE_DIR/bc-statement-family-publication.alpha"
  } > "$T/statement-family-semantic.alpha"
  python3 "$OMEGA_PATH_ALPHA_ASSEMBLER/asm_ref.py" \
    < "$T/statement-family-semantic.alpha" \
    > "$T/statement-family-semantic-ref.tape"
  "$ASM" < "$T/statement-family-semantic.alpha" \
    > "$T/statement-family-semantic.tape"
  cmp -s "$T/statement-family-semantic.tape" \
    "$T/statement-family-semantic-ref.tape" || {
    echo "bc block control FAIL — statement semantic assembler diamond disagrees" >&2
    exit 1
  }
  statement_semantic_tape_bytes=$(wc -c \
    < "$T/statement-family-semantic.tape" | tr -d ' ')
  if [ "$statement_semantic_tape_bytes" -gt 235000 ]; then
    echo "bc block control FAIL — statement semantic tape is ${statement_semantic_tape_bytes} bytes (235000-byte engineering budget)" >&2
    exit 1
  fi
  stamp_seed "$T/statement-family-semantic.tape" "$SEED" \
    "$T/statement-family-semantic" >/dev/null
}

smoke_name_eq_checker() {
  set +e
  "$T/name-eq-check" < "$T/control.bundle" > "$T/stdout"
  name_eq_smoke_status=$?
  set -e
  if [ "$name_eq_smoke_status" != 0 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — name_eq canonical smoke: expected 0/empty, got $name_eq_smoke_status/$(wc -c < "$T/stdout" | tr -d ' ') bytes" >&2
    exit 1
  fi
}

smoke_lookup_checker() {
  set +e
  "$T/lookup-check" < "$T/control.bundle" > "$T/stdout"
  lookup_smoke_status=$?
  set -e
  if [ "$lookup_smoke_status" != 0 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — lookup canonical smoke: expected 0/empty, got $lookup_smoke_status/$(wc -c < "$T/stdout" | tr -d ' ') bytes" >&2
    exit 1
  fi
  require_control_bundle_unchanged
  BC_OWNER_LOOKUP=1
}

smoke_bounded_emitters_checker() {
  set +e
  "$T/bounded-emitters-check" < "$T/control.bundle" > "$T/stdout"
  bounded_emitters_smoke_status=$?
  set -e
  if [ "$bounded_emitters_smoke_status" != 0 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — bounded emitters canonical smoke: expected 0/empty, got $bounded_emitters_smoke_status/$(wc -c < "$T/stdout" | tr -d ' ') bytes" >&2
    exit 1
  fi
  require_control_bundle_unchanged
  BC_OWNER_BOUNDED_EMITTERS=1
}

smoke_emit_dec_word_checker() {
  set +e
  "$T/emit-dec-word-check" < "$T/control.bundle" > "$T/stdout"
  emit_dec_word_smoke_status=$?
  set -e
  if [ "$emit_dec_word_smoke_status" != 0 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — emit_dec Word canonical smoke: expected 0/empty, got $emit_dec_word_smoke_status/$(wc -c < "$T/stdout" | tr -d ' ') bytes" >&2
    exit 1
  fi
  require_control_bundle_unchanged
  BC_OWNER_EMIT_DEC_WORD=1
}

smoke_label_emitters_checker() {
  set +e
  "$T/label-emitters-check" < "$T/control.bundle" > "$T/stdout"
  label_emitters_smoke_status=$?
  set -e
  if [ "$label_emitters_smoke_status" != 0 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — label emitters canonical smoke: expected 0/empty, got $label_emitters_smoke_status/$(wc -c < "$T/stdout" | tr -d ' ') bytes" >&2
    exit 1
  fi
  require_control_bundle_unchanged
  BC_OWNER_LABEL_EMITTERS=1
}

smoke_expression_family_checkers() {
  for expression_checker_kind in shape semantic
  do
    set +e
    "$T/expression-family-$expression_checker_kind" \
      < "$T/control.bundle" > "$T/stdout"
    expression_checker_status=$?
    set -e
    if [ "$expression_checker_status" != 0 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — expression $expression_checker_kind canonical smoke: expected 0/empty, got $expression_checker_status/$(wc -c < "$T/stdout" | tr -d ' ') bytes" >&2
      exit 1
    fi
  done
  require_control_bundle_unchanged
  BC_OWNER_EXPRESSION_FAMILY=1
}

smoke_statement_family_shape_checker() {
  set +e
  "$T/statement-family-shape" < "$T/control.bundle" > "$T/stdout"
  statement_shape_status=$?
  set -e
  if [ "$statement_shape_status" != 0 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — statement shape canonical smoke: expected 0/empty, got $statement_shape_status/$(wc -c < "$T/stdout" | tr -d ' ') bytes" >&2
    exit 1
  fi
  require_control_bundle_unchanged
  BC_OWNER_STATEMENT_SHAPE=1
}

smoke_statement_family_semantic_checker() {
  set +e
  "$T/statement-family-semantic" < "$T/control.bundle" > "$T/stdout"
  statement_semantic_status=$?
  set -e
  if [ "$statement_semantic_status" != 0 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — statement semantic canonical smoke: expected 0/empty, got $statement_semantic_status/$(wc -c < "$T/stdout" | tr -d ' ') bytes" >&2
    exit 1
  fi
  require_control_bundle_unchanged
  BC_OWNER_STATEMENT_IMPLICATION=1
}

parse_body_require_module_budgets() {
  for parse_body_module in \
    bc-parse-body-root.alpha \
    bc-parse-body-antecedents.alpha \
    bc-parse-body-shape.alpha \
    bc-parse-body-rules.alpha
  do
    parse_body_module_bytes=$(wc -c < "$GATE_DIR/$parse_body_module" | tr -d ' ')
    if [ "$parse_body_module_bytes" -ge 20000 ]; then
      echo "bc block control FAIL — $parse_body_module is ${parse_body_module_bytes} bytes (20KB module cap)" >&2
      exit 1
    fi
  done
}

build_parse_body_checker() {
  parse_body_require_module_budgets
  {
    emit_expression_table_prefix
    cat "$GATE_DIR/bc-parse-body-root.alpha" \
      "$GATE_DIR/bc-expression-selected-row-helpers.alpha" \
      "$GATE_DIR/bc-exact-shape-helpers.alpha" \
      "$GATE_DIR/bc-parse-body-antecedents.alpha" \
      "$GATE_DIR/bc-parse-body-shape.alpha" \
      "$GATE_DIR/bc-parse-body-rules.alpha"
  } > "$T/parse-body.alpha"
  python3 "$OMEGA_PATH_ALPHA_ASSEMBLER/asm_ref.py" \
    < "$T/parse-body.alpha" > "$T/parse-body-ref.tape"
  "$ASM" < "$T/parse-body.alpha" > "$T/parse-body.tape"
  cmp -s "$T/parse-body.tape" "$T/parse-body-ref.tape" || {
    echo "bc block control FAIL — parse body assembler diamond disagrees" >&2
    exit 1
  }
  parse_body_tape_bytes=$(wc -c < "$T/parse-body.tape" | tr -d ' ')
  if [ "$parse_body_tape_bytes" -gt 100000 ]; then
    echo "bc block control FAIL — parse body tape is ${parse_body_tape_bytes} bytes (100000-byte engineering budget)" >&2
    exit 1
  fi
  stamp_seed "$T/parse-body.tape" "$SEED" "$T/parse-body" >/dev/null
}

smoke_parse_body_checker() {
  set +e
  "$T/parse-body" < "$T/control.bundle" > "$T/stdout"
  parse_body_status=$?
  set -e
  if [ "$parse_body_status" != 0 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — parse body canonical smoke: expected 0/empty, got $parse_body_status/$(wc -c < "$T/stdout" | tr -d ' ') bytes" >&2
    exit 1
  fi
}

# Meta-level modus ponens over one immutable bundle. The parse executable proves
# (PFXS and SREL)=>PBOD; it does not import SPUB as though SPUB were SREL.
# These flags are set only after the canonical owner processes have accepted.
parse_body_discharge_statement_relation() {
  bc_require_owner label "${BC_OWNER_LABEL_EMITTERS:-0}"
  bc_require_owner expression "${BC_OWNER_EXPRESSION_FAMILY:-0}"
  bc_require_owner statement-shape "${BC_OWNER_STATEMENT_SHAPE:-0}"
  bc_require_owner checker-a "${BC_OWNER_CHECKER_A:-0}"
  bc_require_owner lookup "${BC_OWNER_LOOKUP:-0}"
  bc_require_owner bounded-emitters "${BC_OWNER_BOUNDED_EMITTERS:-0}"
  bc_require_owner statement-implication \
    "${BC_OWNER_STATEMENT_IMPLICATION:-0}"
}

establish_parse_body_canonical() {
  parse_body_discharge_statement_relation
  build_parse_body_checker
  smoke_parse_body_checker
  require_control_bundle_unchanged
  BC_OWNER_PARSE_BODY=1
}

bc_require_owner() { # name accepted
  if [ "$2" != 1 ]; then
    echo "bc block control FAIL — missing discharged owner $1" >&2
    exit 1
  fi
}

resource_classification_require_module_budgets() {
  for resource_module in \
    bc-resource-classification-root.alpha \
    bc-resource-classification-shape.alpha \
    bc-resource-classification-antecedents.alpha \
    bc-resource-profile.alpha \
    bc-resource-classification.alpha
  do
    resource_module_bytes=$(wc -c < "$GATE_DIR/$resource_module" | tr -d ' ')
    if [ "$resource_module_bytes" -ge 20000 ]; then
      echo "bc block control FAIL — $resource_module is ${resource_module_bytes} bytes (20KB module cap)" >&2
      exit 1
    fi
  done
}

build_resource_classification_checker() {
  resource_classification_require_module_budgets
  {
    emit_expression_table_prefix
    cat "$GATE_DIR/bc-resource-classification-root.alpha" \
      "$GATE_DIR/bc-expression-selected-row-helpers.alpha" \
      "$GATE_DIR/bc-exact-shape-helpers.alpha" \
      "$GATE_DIR/bc-resource-classification-shape.alpha" \
      "$GATE_DIR/bc-resource-classification-antecedents.alpha" \
      "$GATE_DIR/bc-resource-profile.alpha" \
      "$GATE_DIR/bc-resource-classification.alpha"
  } > "$T/resource-classification.alpha"
  python3 "$OMEGA_PATH_ALPHA_ASSEMBLER/asm_ref.py" \
    < "$T/resource-classification.alpha" > "$T/resource-classification-ref.tape"
  "$ASM" < "$T/resource-classification.alpha" \
    > "$T/resource-classification.tape"
  cmp -s "$T/resource-classification.tape" \
    "$T/resource-classification-ref.tape" || {
    echo "bc block control FAIL — resource classification assembler diamond disagrees" >&2
    exit 1
  }
  resource_tape_bytes=$(wc -c < "$T/resource-classification.tape" | tr -d ' ')
  if [ "$resource_tape_bytes" -gt 100000 ]; then
    echo "bc block control FAIL — resource classification tape is ${resource_tape_bytes} bytes (100000-byte engineering budget)" >&2
    exit 1
  fi
  stamp_seed "$T/resource-classification.tape" "$SEED" \
    "$T/resource-classification" >/dev/null
}

smoke_resource_classification_checker() {
  set +e
  "$T/resource-classification" < "$T/control.bundle" > "$T/stdout"
  resource_smoke_status=$?
  set -e
  if [ "$resource_smoke_status" != 0 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — resource classification canonical smoke: expected 0/empty, got $resource_smoke_status/$(wc -c < "$T/stdout" | tr -d ' ') bytes" >&2
    exit 1
  fi
  require_control_bundle_unchanged
  BC_OWNER_RESOURCE_CLASSIFICATION=1
}

# Meta-level conjunction over the same immutable bundle. RCLS is a conditional
# implication; its process-local antecedent cells are not imported as owner
# conclusions. Checker A owns source/declare/block/parse guards, while the
# expression-family process owns call/expression guards and XRSC.
resource_classification_discharge_owners() {
  bc_require_owner checker-a "${BC_OWNER_CHECKER_A:-0}"
  bc_require_owner expression "${BC_OWNER_EXPRESSION_FAMILY:-0}"
}

establish_resource_classification_canonical() {
  resource_classification_discharge_owners
  build_resource_classification_checker
  smoke_resource_classification_checker
}

declaration_budget_require_module_budgets() {
  for declaration_budget_module in \
    bc-declaration-budget-root.alpha \
    bc-declaration-budget-antecedents.alpha \
    bc-declaration-budget-shape.alpha \
    bc-declaration-budget-rules.alpha \
    bc-declaration-budget-publication.alpha
  do
    declaration_budget_module_bytes=$(wc -c \
      < "$GATE_DIR/$declaration_budget_module" | tr -d ' ')
    if [ "$declaration_budget_module_bytes" -ge 20000 ]; then
      echo "bc block control FAIL — $declaration_budget_module is ${declaration_budget_module_bytes} bytes (20KB module cap)" >&2
      exit 1
    fi
  done
}

build_declaration_budget_checker() {
  declaration_budget_require_module_budgets
  {
    emit_expression_table_prefix
    cat "$GATE_DIR/bc-declaration-budget-root.alpha" \
      "$GATE_DIR/bc-expression-selected-row-helpers.alpha" \
      "$GATE_DIR/bc-exact-shape-helpers.alpha" \
      "$GATE_DIR/bc-declaration-budget-antecedents.alpha" \
      "$GATE_DIR/bc-declaration-budget-shape.alpha" \
      "$GATE_DIR/bc-declaration-budget-rules.alpha" \
      "$GATE_DIR/bc-declaration-budget-publication.alpha"
  } > "$T/declaration-budget.alpha"
  python3 "$OMEGA_PATH_ALPHA_ASSEMBLER/asm_ref.py" \
    < "$T/declaration-budget.alpha" > "$T/declaration-budget-ref.tape"
  "$ASM" < "$T/declaration-budget.alpha" > "$T/declaration-budget.tape"
  cmp -s "$T/declaration-budget.tape" "$T/declaration-budget-ref.tape" || {
    echo "bc block control FAIL — declaration budget assembler diamond disagrees" >&2
    exit 1
  }
  declaration_budget_tape_bytes=$(wc -c \
    < "$T/declaration-budget.tape" | tr -d ' ')
  if [ "$declaration_budget_tape_bytes" -gt 100000 ]; then
    echo "bc block control FAIL — declaration budget tape is ${declaration_budget_tape_bytes} bytes (100000-byte engineering budget)" >&2
    exit 1
  fi
  stamp_seed "$T/declaration-budget.tape" "$SEED" \
    "$T/declaration-budget" >/dev/null
}

smoke_declaration_budget_checker() {
  set +e
  "$T/declaration-budget" < "$T/control.bundle" > "$T/stdout"
  declaration_budget_status=$?
  set -e
  if [ "$declaration_budget_status" != 0 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — declaration budget canonical smoke: expected 0/empty, got $declaration_budget_status/$(wc -c < "$T/stdout" | tr -d ' ') bytes" >&2
    exit 1
  fi
  require_control_bundle_unchanged
  BC_OWNER_DECLARATION_BUDGET=1
}

declaration_budget_discharge_owners() {
  bc_require_owner checker-a "${BC_OWNER_CHECKER_A:-0}"
  bc_require_owner parse-body "${BC_OWNER_PARSE_BODY:-0}"
}

establish_declaration_budget_canonical() {
  declaration_budget_discharge_owners
  build_declaration_budget_checker
  smoke_declaration_budget_checker
}

parse_proc_require_module_budgets() {
  for parse_proc_module in \
    bc-parse-proc-root.alpha \
    bc-parse-proc-antecedents.alpha \
    bc-parse-proc-entry-shape.alpha \
    bc-parse-proc-entry-semantics.alpha \
    bc-parse-proc-outcomes.alpha \
    bc-parse-proc-publication.alpha
  do
    parse_proc_module_bytes=$(wc -c < "$GATE_DIR/$parse_proc_module" | tr -d ' ')
    if [ "$parse_proc_module_bytes" -ge 20000 ]; then
      echo "bc block control FAIL — $parse_proc_module is ${parse_proc_module_bytes} bytes (20KB module cap)" >&2
      exit 1
    fi
  done
}

build_parse_proc_checker() {
  parse_proc_require_module_budgets
  {
    emit_expression_table_prefix
    cat "$GATE_DIR/bc-parse-proc-root.alpha" \
      "$GATE_DIR/bc-expression-selected-row-helpers.alpha" \
      "$GATE_DIR/bc-exact-shape-helpers.alpha" \
      "$GATE_DIR/bc-parse-proc-antecedents.alpha" \
      "$GATE_DIR/bc-parse-proc-entry-shape.alpha" \
      "$GATE_DIR/bc-parse-proc-entry-semantics.alpha" \
      "$GATE_DIR/bc-parse-proc-outcomes.alpha" \
      "$GATE_DIR/bc-parse-proc-publication.alpha"
  } > "$T/parse-proc.alpha"
  python3 "$OMEGA_PATH_ALPHA_ASSEMBLER/asm_ref.py" \
    < "$T/parse-proc.alpha" > "$T/parse-proc-ref.tape"
  "$ASM" < "$T/parse-proc.alpha" > "$T/parse-proc.tape"
  cmp -s "$T/parse-proc.tape" "$T/parse-proc-ref.tape" || {
    echo "bc block control FAIL — complete parse_proc assembler diamond disagrees" >&2
    exit 1
  }
  parse_proc_tape_bytes=$(wc -c < "$T/parse-proc.tape" | tr -d ' ')
  if [ "$parse_proc_tape_bytes" -gt 100000 ]; then
    echo "bc block control FAIL — complete parse_proc tape is ${parse_proc_tape_bytes} bytes (100000-byte engineering budget)" >&2
    exit 1
  fi
  stamp_seed "$T/parse-proc.tape" "$SEED" "$T/parse-proc" >/dev/null
}

smoke_parse_proc_checker() {
  set +e
  "$T/parse-proc" < "$T/control.bundle" > "$T/stdout"
  parse_proc_status=$?
  set -e
  if [ "$parse_proc_status" != 0 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — complete parse_proc canonical smoke: expected 0/empty, got $parse_proc_status/$(wc -c < "$T/stdout" | tr -d ' ') bytes" >&2
    exit 1
  fi
  require_control_bundle_unchanged
  BC_OWNER_PARSE_PROC=1
}

parse_proc_discharge_owners() {
  bc_require_owner checker-a "${BC_OWNER_CHECKER_A:-0}"
  bc_require_owner parse-body "${BC_OWNER_PARSE_BODY:-0}"
  bc_require_owner declaration-budget "${BC_OWNER_DECLARATION_BUDGET:-0}"
}

establish_parse_proc_canonical() {
  parse_proc_discharge_owners
  build_parse_proc_checker
  smoke_parse_proc_checker
}

root_observation_require_module_budgets() {
  for root_observation_module in \
    bc-root-observation-root.alpha \
    bc-root-observation-antecedents.alpha \
    bc-root-observation-shape.alpha \
    bc-root-observation-gfp.alpha \
    bc-root-observation-resource-join.alpha \
    bc-root-observation-memory-safety.alpha \
    bc-root-observation-maximal.alpha \
    bc-root-observation-publication.alpha \
    bc-root-observation-publication-payloads.alpha
  do
    root_observation_module_bytes=$(wc -c \
      < "$GATE_DIR/$root_observation_module" | tr -d ' ')
    if [ "$root_observation_module_bytes" -ge 20000 ]; then
      echo "bc block control FAIL — $root_observation_module is ${root_observation_module_bytes} bytes (20KB module cap)" >&2
      exit 1
    fi
  done
}

build_root_observation_checker() {
  root_observation_require_module_budgets
  {
    emit_expression_table_prefix
    cat "$GATE_DIR/bc-root-observation-root.alpha" \
      "$GATE_DIR/bc-expression-selected-row-helpers.alpha" \
      "$GATE_DIR/bc-exact-shape-helpers.alpha" \
      "$GATE_DIR/bc-root-observation-antecedents.alpha" \
      "$GATE_DIR/bc-root-observation-shape.alpha" \
      "$GATE_DIR/bc-root-observation-gfp.alpha" \
      "$GATE_DIR/bc-root-observation-resource-join.alpha" \
      "$GATE_DIR/bc-root-observation-memory-safety.alpha" \
      "$GATE_DIR/bc-root-observation-maximal.alpha" \
      "$GATE_DIR/bc-root-observation-publication.alpha" \
      "$GATE_DIR/bc-root-observation-publication-payloads.alpha"
  } > "$T/root-observation.alpha"
  python3 "$OMEGA_PATH_ALPHA_ASSEMBLER/asm_ref.py" \
    < "$T/root-observation.alpha" > "$T/root-observation-ref.tape"
  "$ASM" < "$T/root-observation.alpha" > "$T/root-observation.tape"
  cmp -s "$T/root-observation.tape" "$T/root-observation-ref.tape" || {
    echo "bc block control FAIL — root observation assembler diamond disagrees" >&2
    exit 1
  }
  root_observation_tape_bytes=$(wc -c \
    < "$T/root-observation.tape" | tr -d ' ')
  if [ "$root_observation_tape_bytes" -gt 100000 ]; then
    echo "bc block control FAIL — root observation tape is ${root_observation_tape_bytes} bytes (100000-byte engineering budget)" >&2
    exit 1
  fi
  stamp_seed "$T/root-observation.tape" "$SEED" \
    "$T/root-observation" >/dev/null
}

smoke_root_observation_checker() {
  set +e
  "$T/root-observation" < "$T/control.bundle" > "$T/stdout"
  root_observation_status=$?
  set -e
  if [ "$root_observation_status" != 0 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — root observation canonical smoke: expected 0/empty, got $root_observation_status/$(wc -c < "$T/stdout" | tr -d ' ') bytes" >&2
    exit 1
  fi
  require_control_bundle_unchanged
  BC_OWNER_ROOT_OBSERVATION=1
}

root_observation_discharge_owners() {
  bc_require_owner artifact-structure "${BC_OWNER_ARTIFACT_STRUCTURE:-0}"
  bc_require_owner checker-a "${BC_OWNER_CHECKER_A:-0}"
  bc_require_owner lookup "${BC_OWNER_LOOKUP:-0}"
  bc_require_owner emit-dec-word "${BC_OWNER_EMIT_DEC_WORD:-0}"
  bc_require_owner parse-body "${BC_OWNER_PARSE_BODY:-0}"
  bc_require_owner resource-classification \
    "${BC_OWNER_RESOURCE_CLASSIFICATION:-0}"
  bc_require_owner declaration-budget "${BC_OWNER_DECLARATION_BUDGET:-0}"
  bc_require_owner parse-proc "${BC_OWNER_PARSE_PROC:-0}"
}

establish_root_observation_canonical() {
  root_observation_discharge_owners
  build_root_observation_checker
  smoke_root_observation_checker
}

# Statement-family focus continues through the canonical prerequisite owners
# below before testing its conditional semantic implication. No process-local
# GSBD/XPUB/LOOK/BEMS/E5PK marker is copied between executables.

# Exact family shape and the 65-row semantic induction are intentionally two
# independent processes over one canonical bundle. Acceptance is conjunction.
if [ "${BC_BLOCK_FOCUS:-}" = expression-family ]; then
  bc_timing_start expression-family-focus
  build_expression_family_checkers
  smoke_expression_family_checkers
  expression_family_build_teeth
  expression_family_reject_teeth
  bc_timing_finish
  echo "bc expression family: focused canonical + 16 phase-isolated teeth passed ($(wc -c < "$T/expression-family-shape.tape" | tr -d ' ')-byte shape, $(wc -c < "$T/expression-family-semantic.tape" | tr -d ' ')-byte semantic tapes)"
  exit 0
fi

# Checker E independently rebuilds WSTR/cursor/expect/full-Word DECW before
# composing the label, string-body, gen_emit, and comparison emitter family.
if [ "${BC_BLOCK_FOCUS:-}" = label-emitters ]; then
  bc_timing_start label-emitters-focus
  build_label_emitters_checker
  smoke_label_emitters_checker
  label_emitters_build_teeth
  label_emitters_reject_teeth
  bc_timing_finish
  echo "bc label emitters: focused canonical + 37 phase-isolated teeth passed ($(wc -c < "$T/label-emitters-check.tape" | tr -d ' ')-byte checker tape)"
  exit 0
fi

# Checker D independently re-executes exact procedure-40 shape and proves the
# honest signed full-Word relation without importing bounded DECS.
if [ "${BC_BLOCK_FOCUS:-}" = emit-dec-word ]; then
  bc_timing_start emit-dec-word-focus
  build_emit_dec_word_checker
  smoke_emit_dec_word_checker
  emit_dec_word_build_teeth
  emit_dec_word_reject_teeth
  bc_timing_finish
  echo "bc emit_dec Word: focused canonical + 36 phase-isolated teeth passed ($(wc -c < "$T/emit-dec-word-check.tape" | tr -d ' ')-byte checker tape)"
  exit 0
fi

# The name_eq focus is an independent tranche: it deliberately skips Checker A
# and imports no process-local theorem cells from it.
if [ "${BC_BLOCK_FOCUS:-}" = name-eq ]; then
  bc_timing_start name-eq-focus
  build_name_eq_checker
  smoke_name_eq_checker
  checker_split_build_name_tooth
  name_eq_build_teeth
  checker_split_reject_name_tooth
  name_eq_reject_teeth
  bc_timing_finish
  echo "bc name_eq: focused canonical + 32 phase-isolated teeth passed ($(wc -c < "$T/name-eq-check.tape" | tr -d ' ')-byte checker tape)"
  exit 0
fi

if [ "${BC_BLOCK_FOCUS:-}" = lookup ]; then
  bc_timing_start lookup-focus
  build_lookup_checker
  smoke_lookup_checker
  lookup_build_teeth
  lookup_reject_teeth
  bc_timing_finish
  echo "bc lookup: focused canonical + 35 phase-isolated teeth passed ($(wc -c < "$T/lookup-check.tape" | tr -d ' ')-byte checker tape)"
  exit 0
fi

# Checker C independently re-executes the lower-rooted WSTR, cursor/expect,
# and bounded-DECS prerequisites.  Its focus never constructs Checker A or B.
if [ "${BC_BLOCK_FOCUS:-}" = bounded-emitters ]; then
  bc_timing_start bounded-emitters-focus
  build_bounded_emitters_checker
  smoke_bounded_emitters_checker
  bounded_emitters_build_teeth
  bounded_emitters_reject_teeth
  bc_timing_finish
  echo "bc bounded emitters: focused canonical + 52 phase-isolated teeth passed ($(wc -c < "$T/bounded-emitters-check.tape" | tr -d ' ')-byte checker tape)"
  exit 0
fi

# Statement focus runs only the canonical external owners here. The unfocused
# gate retains their full historical teeth before constructing Checker A.
if [ "${BC_BLOCK_FOCUS:-}" = resource-classification ]; then
  bc_timing_start resource-classification-prerequisites
  build_expression_family_checkers
  smoke_expression_family_checkers
  bc_timing_finish
elif [ "${BC_BLOCK_FOCUS:-}" = statement-family ] || \
   [ "${BC_BLOCK_FOCUS:-}" = parse-proc-body ] || \
   [ "${BC_BLOCK_FOCUS:-}" = declaration-budget ] || \
   [ "${BC_BLOCK_FOCUS:-}" = parse-proc ] || \
   [ "${BC_BLOCK_FOCUS:-}" = root-observation ]; then
  bc_timing_start statement-family-prerequisites
  if [ "${BC_BLOCK_FOCUS:-}" = root-observation ]; then
    build_emit_dec_word_checker
    smoke_emit_dec_word_checker
  fi
  build_label_emitters_checker
  smoke_label_emitters_checker
  build_expression_family_checkers
  smoke_expression_family_checkers
  build_statement_family_shape_checker
  smoke_statement_family_shape_checker
  bc_timing_finish
else
  bc_timing_start emit-dec-word-tranche
  build_emit_dec_word_checker
  smoke_emit_dec_word_checker
  emit_dec_word_build_teeth
  emit_dec_word_reject_teeth
  bc_timing_finish

  bc_timing_start label-emitters-tranche
  build_label_emitters_checker
  smoke_label_emitters_checker
  label_emitters_build_teeth
  label_emitters_reject_teeth
  bc_timing_finish

  bc_timing_start expression-family-tranche
  build_expression_family_checkers
  smoke_expression_family_checkers
  expression_family_build_teeth
  expression_family_reject_teeth
  bc_timing_finish

  bc_timing_start statement-family-shape-tranche
  build_statement_family_shape_checker
  smoke_statement_family_shape_checker
  statement_family_build_teeth
  statement_family_reject_teeth
  bc_timing_finish
fi

bc_timing_start checker-a-canonical
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
  "$GATE_DIR/bc-stack-potential-lift.alpha" \
  "$GATE_DIR/bc-post-stack-fixed.alpha" \
  "$GATE_DIR/bc-slurp-summary.alpha" \
  "$GATE_DIR/bc-main-slurp-bridge.alpha" \
  "$GATE_DIR/bc-write-str-event-helper.alpha" \
  "$GATE_DIR/bc-write-str-summary.alpha" \
  "$GATE_DIR/bc-fixed-emitter-summary.alpha" \
  "$GATE_DIR/bc-cursor-leaf-summary.alpha" \
  "$GATE_DIR/bc-skip-ws-summary.alpha" \
  "$GATE_DIR/bc-main-ready-summary.alpha" \
  "$GATE_DIR/bc-summary-combinators.alpha" \
  "$GATE_DIR/bc-exact-shape-helpers.alpha" \
  "$GATE_DIR/bc-main-loop-entry-summary.alpha" \
  "$GATE_DIR/bc-classifier-shape.alpha" \
  "$GATE_DIR/bc-classifier-summary.alpha" \
  "$GATE_DIR/bc-read-ident-shape.alpha" \
  "$GATE_DIR/bc-read-ident-summary.alpha" \
  "$GATE_DIR/bc-expect-shape.alpha" \
  "$GATE_DIR/bc-expect-summary.alpha" \
  "$GATE_DIR/bc-declare-shape.alpha" \
  "$GATE_DIR/bc-declare-summary.alpha" \
  "$GATE_DIR/bc-let-keyword-shape.alpha" \
  "$GATE_DIR/bc-let-keyword-summary.alpha" \
  "$GATE_DIR/bc-literal-skip-shape.alpha" \
  "$GATE_DIR/bc-literal-skip-summary.alpha" \
  "$GATE_DIR/bc-count-lets-control-shape.alpha" \
  "$GATE_DIR/bc-count-lets-data-shape.alpha" \
  "$GATE_DIR/bc-count-lets-cases.alpha" \
  "$GATE_DIR/bc-count-lets-summary.alpha" \
  "$GATE_DIR/bc-parse-params-control-shape.alpha" \
  "$GATE_DIR/bc-parse-params-data-shape.alpha" \
  "$GATE_DIR/bc-parse-parameter-summary.alpha" \
  "$GATE_DIR/bc-parse-capacity-summary.alpha" \
  "$GATE_DIR/bc-emit-ident-shape.alpha" \
  "$GATE_DIR/bc-emit-ident-summary.alpha" \
  "$GATE_DIR/bc-emit-dec-shape.alpha" \
  "$GATE_DIR/bc-emit-dec-summary.alpha" \
  "$GATE_DIR/bc-fixed-decimal-emitters-shape.alpha" \
  "$GATE_DIR/bc-fixed-decimal-emitters-summary.alpha" \
  "$GATE_DIR/bc-parse-output-prefix-shape.alpha" \
  "$GATE_DIR/bc-parse-output-prefix-summary.alpha" \
  "$GATE_DIR/bc-gen-stmts-boundary-shape.alpha" \
  "$GATE_DIR/bc-gen-stmts-boundary-summary.alpha" \
  "$GATE_DIR/bc-parse-number-shape.alpha" \
  "$GATE_DIR/bc-parse-number-summary.alpha" \
  "$GATE_DIR/bc-parse-char-shape.alpha" \
  "$GATE_DIR/bc-parse-char-cases.alpha" \
  "$GATE_DIR/bc-parse-char-summary.alpha" \
  "$GATE_DIR/bc-operator-classifier-shape.alpha" \
  "$GATE_DIR/bc-operator-classifier-summary.alpha" \
  "$GATE_DIR/bc-cmp-op-shape.alpha" \
  "$GATE_DIR/bc-cmp-op-cases.alpha" \
  "$GATE_DIR/bc-cmp-op-summary.alpha" \
  "$GATE_DIR/bc-fixed-keyword-shape-core.alpha" \
  "$GATE_DIR/bc-fixed-keyword-data-shape.alpha" \
  "$GATE_DIR/bc-fixed-keyword-cases.alpha" \
  "$GATE_DIR/bc-fixed-keyword-summary.alpha" > "$T/control-check.alpha"
"$ASM" < "$T/control-check.alpha" > "$T/control-check.tape"
checker_a_tape_bytes=$(wc -c < "$T/control-check.tape" | tr -d ' ')
checker_a_seed_payload_limit=$((HOLE_SIZE - 4))
if [ "$checker_a_tape_bytes" -gt "$checker_a_seed_payload_limit" ]; then
  echo "bc block control FAIL — Checker A tape is ${checker_a_tape_bytes} bytes (${checker_a_seed_payload_limit}-byte seed payload limit)" >&2
  exit 1
fi
stamp_seed "$T/control-check.tape" "$SEED" "$T/control-check" >/dev/null

# Fail fast on the canonical proof before spending most of the gate assembling
# its mutation matrix. The same valid run used to occur only after every tooth
# binary had been built, obscuring simple theorem-integration failures.
set +e
"$T/control-check" < "$T/control.bundle" > "$T/stdout"
control_smoke_status=$?
set -e
if [ "$control_smoke_status" != 0 ] || [ -s "$T/stdout" ]; then
  echo "bc block control FAIL — canonical proof smoke: expected 0/empty, got $control_smoke_status/$(wc -c < "$T/stdout" | tr -d ' ') bytes" >&2
  exit 1
fi
require_control_bundle_unchanged
BC_OWNER_CHECKER_A=1
bc_timing_finish

# Focused development mode avoids the historical mutation matrix while still
# using the exact canonical source/artifact bundle and every prerequisite proof.
case "${BC_BLOCK_FOCUS:-}" in
  resource-classification)
    bc_timing_start resource-classification
    establish_resource_classification_canonical
    resource_classification_build_teeth
    resource_classification_reject_teeth
    bc_timing_finish
    echo "bc checked resources: seven exact guards classified into five ResourceKinds without status inversion + 36 phase-isolated teeth passed ($(wc -c < "$T/resource-classification.tape" | tr -d ' ')-byte tape)"
    exit 0
    ;;
  parse-proc-body)
    bc_timing_start parse-proc-body-conditional-semantics
    build_lookup_checker
    smoke_lookup_checker
    build_bounded_emitters_checker
    smoke_bounded_emitters_checker
    build_statement_family_semantic_checker
    smoke_statement_family_semantic_checker
    establish_parse_body_canonical
    parse_body_build_teeth
    parse_body_reject_teeth
    bc_timing_finish
    echo "bc parse_proc genbody: same-bundle statement implication discharged; D=0..64 maximal Ret-or-Div composition through unconditional epilogue/return + 25 teeth passed ($(wc -c < "$T/parse-body.tape" | tr -d ' ')-byte tape)"
    exit 0
    ;;
  declaration-budget)
    bc_timing_start declaration-budget
    build_lookup_checker
    smoke_lookup_checker
    build_bounded_emitters_checker
    smoke_bounded_emitters_checker
    build_statement_family_semantic_checker
    smoke_statement_family_semantic_checker
    establish_parse_body_canonical
    establish_declaration_budget_canonical
    declaration_budget_build_teeth
    declaration_budget_reject_teeth
    bc_timing_finish
    echo "bc declaration budget: count_lets/PCAP/SREL occurrence rank excludes root declare exhaustion + 14 phase-isolated teeth passed ($(wc -c < "$T/declaration-budget.tape" | tr -d ' ')-byte tape)"
    exit 0
    ;;
  parse-proc)
    bc_timing_start complete-parse-proc
    build_lookup_checker
    smoke_lookup_checker
    build_bounded_emitters_checker
    smoke_bounded_emitters_checker
    build_statement_family_semantic_checker
    smoke_statement_family_semantic_checker
    establish_parse_body_canonical
    establish_declaration_budget_canonical
    establish_parse_proc_canonical
    parse_proc_build_teeth
    parse_proc_reject_teeth
    bc_timing_finish
    echo "bc complete parse_proc: exact permissive entry and PLOP/PCAP/PFXS/PBOD(D0) compose to Ret0/Ret252/Div with origins1/3 excluded + 16 phase-isolated teeth passed ($(wc -c < "$T/parse-proc.tape" | tr -d ' ')-byte tape)"
    exit 0
    ;;
  root-observation)
    bc_timing_start whole-root-observation
    build_lookup_checker
    smoke_lookup_checker
    build_bounded_emitters_checker
    smoke_bounded_emitters_checker
    build_statement_family_semantic_checker
    smoke_statement_family_semantic_checker
    establish_parse_body_canonical
    establish_resource_classification_canonical
    establish_declaration_budget_canonical
    establish_parse_proc_canonical
    establish_root_observation_canonical
    root_observation_build_teeth
    root_observation_reject_teeth
    bc_timing_finish
    echo "bc root observable: exact maximal stdout and Halt/Trap/Exhaust/Diverge equality over every finite source and supported resource profile + root teeth passed ($(wc -c < "$T/root-observation.tape" | tr -d ' ')-byte tape)"
    exit 0
    ;;
  statement-family)
    bc_timing_start statement-family-conditional-semantics
    build_lookup_checker
    smoke_lookup_checker
    build_bounded_emitters_checker
    smoke_bounded_emitters_checker
    build_statement_family_semantic_checker
    smoke_statement_family_semantic_checker
    statement_family_build_teeth
    statement_family_reject_teeth
    statement_family_semantic_build_teeth
    statement_family_semantic_reject_teeth
    bc_timing_finish
    echo "bc statement family: focused prerequisite conjunction + 12 shape and 22 semantic teeth passed ($(wc -c < "$T/statement-family-shape.tape" | tr -d ' ')-byte shape, $(wc -c < "$T/statement-family-semantic.tape" | tr -d ' ')-byte semantic tapes)"
    exit 0
    ;;
  operator-classifier)
    operator_classifier_build_teeth
    operator_classifier_reject_teeth
    echo "bc operator classifiers: focused canonical + 24 phase-isolated teeth passed ($(wc -c < "$T/control-check.tape" | tr -d ' ')-byte checker tape)"
    exit 0
    ;;
  cmp-op)
    cmp_op_build_teeth
    cmp_op_reject_teeth
    echo "bc cmp_op: focused canonical + 41 phase-isolated teeth passed ($(wc -c < "$T/control-check.tape" | tr -d ' ')-byte checker tape)"
    exit 0
    ;;
  fixed-keyword)
    fixed_keyword_build_teeth
    checker_split_build_fixed_tooth
    fixed_keyword_reject_teeth
    checker_split_reject_fixed_tooth
    echo "bc fixed keywords: focused canonical + 43 phase-isolated teeth passed ($(wc -c < "$T/control-check.tape" | tr -d ' ')-byte checker tape)"
    exit 0
    ;;
  parse-char)
    parse_char_build_teeth
    parse_char_reject_teeth
    echo "bc parse_char: focused canonical + 38 phase-isolated teeth passed ($(wc -c < "$T/control-check.tape" | tr -d ' ')-byte checker tape)"
    exit 0
    ;;
  "")
    ;;
  *)
    echo "bc block control FAIL — unknown BC_BLOCK_FOCUS: $BC_BLOCK_FOCUS" >&2
    exit 2
    ;;
esac

# Establish the independent canonical owners, then close the statement
# implication before spending time on their historical mutation families.
bc_timing_start independent-canonical-tranches
build_name_eq_checker
smoke_name_eq_checker
build_lookup_checker
smoke_lookup_checker
build_bounded_emitters_checker
smoke_bounded_emitters_checker
bc_timing_finish

bc_timing_start statement-family-semantic-tranche
build_statement_family_semantic_checker
smoke_statement_family_semantic_checker
statement_family_semantic_build_teeth
statement_family_semantic_reject_teeth
bc_timing_finish

bc_timing_start parse-proc-body-tranche
establish_parse_body_canonical
parse_body_build_teeth
parse_body_reject_teeth
bc_timing_finish

bc_timing_start resource-classification-tranche
establish_resource_classification_canonical
resource_classification_build_teeth
resource_classification_reject_teeth
bc_timing_finish

bc_timing_start declaration-budget-tranche
establish_declaration_budget_canonical
declaration_budget_build_teeth
declaration_budget_reject_teeth
bc_timing_finish

bc_timing_start complete-parse-proc-tranche
establish_parse_proc_canonical
parse_proc_build_teeth
parse_proc_reject_teeth
bc_timing_finish

bc_timing_start root-observation-tranche
establish_root_observation_canonical
root_observation_build_teeth
root_observation_reject_teeth
bc_timing_finish

bc_timing_start independent-historical-teeth
bounded_emitters_build_teeth
bounded_emitters_reject_teeth
checker_split_build_fixed_tooth
checker_split_build_name_tooth
name_eq_build_teeth
checker_split_reject_fixed_tooth
checker_split_reject_name_tooth
name_eq_reject_teeth
lookup_build_teeth
lookup_reject_teeth
bc_timing_finish

bc_timing_start checker-a-historical-mutations

# Phase-isolated fixed-load tooth: keep the exact source, artifact, witness,
# grammar counts, and every prior phase unchanged while omitting one load-class
# owner.  The exhaustive 95-row family scan must find the unclassified load.
sed '/call composition_load_parse_fixed/{n;s/imm r6, 1/imm r6, 0/;}' \
  "$T/control-check.alpha" > "$T/load-missing-class.alpha"
"$ASM" < "$T/load-missing-class.alpha" > "$T/load-missing-class.tape"
stamp_seed "$T/load-missing-class.tape" "$SEED" "$T/load-missing-class" >/dev/null

# Phase-isolated slurp-summary teeth mutate only the checker proof script. The
# first derives the endpoint payload from n instead of c, the second admits a
# zero rank delta, the third breaks backedge renaming, and the fourth feeds zero
# rather than n to LEN.
sed 's/imm r20, 4                    ; derived c payload kind/imm r20, 2                    ; derived c payload kind/' \
  "$T/control-check.alpha" > "$T/slurp-wrong-payload.alpha"
"$ASM" < "$T/slurp-wrong-payload.alpha" > "$T/slurp-wrong-payload.tape"
stamp_seed "$T/slurp-wrong-payload.tape" "$SEED" "$T/slurp-wrong-payload" >/dev/null
sed 's/imm r3, 1                     ; checked inverse-successor delta/imm r3, 0                     ; checked inverse-successor delta/' \
  "$T/control-check.alpha" > "$T/slurp-zero-rank.alpha"
"$ASM" < "$T/slurp-zero-rank.alpha" > "$T/slurp-zero-rank.tape"
stamp_seed "$T/slurp-zero-rank.tape" "$SEED" "$T/slurp-zero-rank" >/dev/null
sed 's/imm r2, 2                    ; checked renamed cursor successor/imm r2, 1                    ; checked renamed cursor successor/' \
  "$T/control-check.alpha" > "$T/slurp-wrong-rename.alpha"
"$ASM" < "$T/slurp-wrong-rename.alpha" > "$T/slurp-wrong-rename.tape"
stamp_seed "$T/slurp-wrong-rename.tape" "$SEED" "$T/slurp-wrong-rename" >/dev/null
sed 's/call slurp_sv_load_n             ; checked LEN payload flow/call slurp_sv_zero               ; checked LEN payload flow/' \
  "$T/control-check.alpha" > "$T/slurp-wrong-len.alpha"
"$ASM" < "$T/slurp-wrong-len.alpha" > "$T/slurp-wrong-len.tape"
stamp_seed "$T/slurp-wrong-len.tape" "$SEED" "$T/slurp-wrong-len" >/dev/null

# Phase-isolated main/slurp bridge teeth retain the exact source, artifact,
# slurp theorem, and every preceding phase.  They respectively sever the
# returned-r0/local association, admit zero==one, and relabel status 253 as 252.
sed 's/call main_slurp_value_load_local             ; checked returned-r0 flow/call main_slurp_value_one                   ; checked returned-r0 flow/' \
  "$T/control-check.alpha" > "$T/main-slurp-wrong-local.alpha"
"$ASM" < "$T/main-slurp-wrong-local.alpha" > "$T/main-slurp-wrong-local.tape"
stamp_seed "$T/main-slurp-wrong-local.tape" "$SEED" "$T/main-slurp-wrong-local" >/dev/null
sed 's/imm r21, 0                    ; checked zero != one result/imm r21, 1                    ; checked zero != one result/' \
  "$T/control-check.alpha" > "$T/main-slurp-wrong-branch.alpha"
"$ASM" < "$T/main-slurp-wrong-branch.alpha" > "$T/main-slurp-wrong-branch.tape"
stamp_seed "$T/main-slurp-wrong-branch.tape" "$SEED" "$T/main-slurp-wrong-branch" >/dev/null
sed 's/imm r20, 253                  ; checked concrete failure value/imm r20, 252                  ; checked concrete failure value/' \
  "$T/control-check.alpha" > "$T/main-slurp-wrong-status.alpha"
"$ASM" < "$T/main-slurp-wrong-status.alpha" > "$T/main-slurp-wrong-status.tape"
stamp_seed "$T/main-slurp-wrong-status.tape" "$SEED" "$T/main-slurp-wrong-status" >/dev/null
sed 's/imm r1, 525744                ; import success clause/imm r1, 525752                ; import success clause/' \
  "$T/control-check.alpha" > "$T/main-slurp-wrong-clause.alpha"
"$ASM" < "$T/main-slurp-wrong-clause.alpha" > "$T/main-slurp-wrong-clause.tape"
stamp_seed "$T/main-slurp-wrong-clause.tape" "$SEED" "$T/main-slurp-wrong-clause" >/dev/null

# Phase-isolated __write_str-summary teeth keep the exact helper and all 113
# emit macros unchanged while breaking byte provenance, rank, backedge rename,
# or exhaustive literal-byte accounting inside only the new relational phase.
sed 's/imm r2, 1                    ; checked load endpoint K/imm r2, 2                    ; checked load endpoint K/' \
  "$T/control-check.alpha" > "$T/write-str-wrong-byte.alpha"
"$ASM" < "$T/write-str-wrong-byte.alpha" > "$T/write-str-wrong-byte.tape"
stamp_seed "$T/write-str-wrong-byte.tape" "$SEED" "$T/write-str-wrong-byte" >/dev/null
sed 's/imm r3, 1                    ; checked rank delta/imm r3, 0                    ; checked rank delta/' \
  "$T/control-check.alpha" > "$T/write-str-zero-rank.alpha"
"$ASM" < "$T/write-str-zero-rank.alpha" > "$T/write-str-zero-rank.tape"
stamp_seed "$T/write-str-zero-rank.tape" "$SEED" "$T/write-str-zero-rank" >/dev/null
sed 's/imm r2, 1                    ; checked renamed output segment/imm r2, 2                    ; checked renamed output segment/' \
  "$T/control-check.alpha" > "$T/write-str-wrong-rename.alpha"
"$ASM" < "$T/write-str-wrong-rename.alpha" > "$T/write-str-wrong-rename.tape"
stamp_seed "$T/write-str-wrong-rename.tape" "$SEED" "$T/write-str-wrong-rename" >/dev/null
sed '/write_str_emit_byte_count:/,/write_str_emit_done:/{s/imm r1, 829/imm r1, 828/;}' \
  "$T/control-check.alpha" > "$T/write-str-wrong-total.alpha"
"$ASM" < "$T/write-str-wrong-total.alpha" > "$T/write-str-wrong-total.tape"
stamp_seed "$T/write-str-wrong-total.tape" "$SEED" "$T/write-str-wrong-total" >/dev/null
sed 's/imm r20, 70                   ; checked positive cost step/imm r20, 69                   ; checked positive cost step/' \
  "$T/control-check.alpha" > "$T/write-str-wrong-cost.alpha"
"$ASM" < "$T/write-str-wrong-cost.alpha" > "$T/write-str-wrong-cost.tape"
stamp_seed "$T/write-str-wrong-cost.tape" "$SEED" "$T/write-str-wrong-cost" >/dev/null

# Phase-isolated fixed-emitter teeth preserve every per-event helper clause but
# break source-order selection, the inter-event call continuation, or one exact
# procedure total in only the new concatenation phase.
sed 's/imm r20, 311                  ; checked first prelude event/imm r20, 312                  ; checked first prelude event/' \
  "$T/control-check.alpha" > "$T/fixed-emit-wrong-row.alpha"
"$ASM" < "$T/fixed-emit-wrong-row.alpha" > "$T/fixed-emit-wrong-row.tape"
stamp_seed "$T/fixed-emit-wrong-row.tape" "$SEED" "$T/fixed-emit-wrong-row" >/dev/null
sed 's/imm r3, 9                    ; checked call continuation width/imm r3, 8                    ; checked call continuation width/' \
  "$T/control-check.alpha" > "$T/fixed-emit-wrong-continuation.alpha"
"$ASM" < "$T/fixed-emit-wrong-continuation.alpha" > "$T/fixed-emit-wrong-continuation.tape"
stamp_seed "$T/fixed-emit-wrong-continuation.tape" "$SEED" "$T/fixed-emit-wrong-continuation" >/dev/null
sed 's/imm r22, 55                   ; checked prelude byte total/imm r22, 54                   ; checked prelude byte total/' \
  "$T/control-check.alpha" > "$T/fixed-emit-wrong-total.alpha"
"$ASM" < "$T/fixed-emit-wrong-total.alpha" > "$T/fixed-emit-wrong-total.tape"
stamp_seed "$T/fixed-emit-wrong-total.tape" "$SEED" "$T/fixed-emit-wrong-total" >/dev/null
sed 's/imm r23, 21226                ; checked exclusive end/imm r23, 21225                ; checked exclusive end/' \
  "$T/control-check.alpha" > "$T/fixed-emit-wrong-end.alpha"
"$ASM" < "$T/fixed-emit-wrong-end.alpha" > "$T/fixed-emit-wrong-end.tape"
stamp_seed "$T/fixed-emit-wrong-end.tape" "$SEED" "$T/fixed-emit-wrong-end" >/dev/null

# Phase-isolated cursor-leaf teeth preserve the exact procedures and preceding
# summaries while severing source-index provenance, reversing the cbyte miss
# partition, admitting a zero cursor delta, dropping CR, or classifying the
# complement as whitespace in only the new relational phase.
sed 's/imm r21, 1                    ; checked SRC index from local i/imm r21, 2                    ; checked SRC index from local i/' \
  "$T/control-check.alpha" > "$T/cursor-cbyte-wrong-index.alpha"
"$ASM" < "$T/cursor-cbyte-wrong-index.alpha" > "$T/cursor-cbyte-wrong-index.tape"
stamp_seed "$T/cursor-cbyte-wrong-index.tape" "$SEED" "$T/cursor-cbyte-wrong-index" >/dev/null
sed 's/imm r2, 2                    ; checked miss relation LEN<=CUR/imm r2, 1                    ; checked miss relation LEN<=CUR/' \
  "$T/control-check.alpha" > "$T/cursor-cbyte-wrong-boundary.alpha"
"$ASM" < "$T/cursor-cbyte-wrong-boundary.alpha" > "$T/cursor-cbyte-wrong-boundary.tape"
stamp_seed "$T/cursor-cbyte-wrong-boundary.tape" "$SEED" "$T/cursor-cbyte-wrong-boundary" >/dev/null
sed 's/imm r2, 1                    ; checked CUR increment delta/imm r2, 0                    ; checked CUR increment delta/' \
  "$T/control-check.alpha" > "$T/cursor-adv-zero-delta.alpha"
"$ASM" < "$T/cursor-adv-zero-delta.alpha" > "$T/cursor-adv-zero-delta.tape"
stamp_seed "$T/cursor-adv-zero-delta.tape" "$SEED" "$T/cursor-adv-zero-delta" >/dev/null
sed 's/imm r21, 1                    ; checked CR is whitespace/imm r21, 0                    ; checked CR is whitespace/' \
  "$T/control-check.alpha" > "$T/cursor-space-drop-cr.alpha"
"$ASM" < "$T/cursor-space-drop-cr.alpha" > "$T/cursor-space-drop-cr.tape"
stamp_seed "$T/cursor-space-drop-cr.tape" "$SEED" "$T/cursor-space-drop-cr" >/dev/null
sed 's/imm r20, 2                    ; checked other-result kind/imm r20, 1                    ; checked other-result kind/' \
  "$T/control-check.alpha" > "$T/cursor-space-zero-is-space.alpha"
"$ASM" < "$T/cursor-space-zero-is-space.alpha" > "$T/cursor-space-zero-is-space.tape"
stamp_seed "$T/cursor-space-zero-is-space.tape" "$SEED" "$T/cursor-space-zero-is-space" >/dev/null
sed 's/imm r23, 17                   ; checked exclusive local row/imm r23, 16                   ; checked exclusive local row/' \
  "$T/control-check.alpha" > "$T/cursor-effect-undercount.alpha"
"$ASM" < "$T/cursor-effect-undercount.alpha" > "$T/cursor-effect-undercount.tape"
stamp_seed "$T/cursor-effect-undercount.tape" "$SEED" "$T/cursor-effect-undercount" >/dev/null

# Phase-isolated skip_ws teeth retain both exact procedures and every imported
# cursor-leaf clause while breaking one continuation, value handoff, same-cursor
# fact, terminal result, inner/outer progress fact, or exhaustive event census.
sed 's/imm r22, 4305                 ; checked skip call continuation/imm r22, 4306                 ; checked skip call continuation/' \
  "$T/control-check.alpha" > "$T/skip-ws-wrong-continuation.alpha"
"$ASM" < "$T/skip-ws-wrong-continuation.alpha" > "$T/skip-ws-wrong-continuation.tape"
stamp_seed "$T/skip-ws-wrong-continuation.tape" "$SEED" "$T/skip-ws-wrong-continuation" >/dev/null
sed 's/imm r1, 526408                ; checked is_space argument handoff/imm r1, 526416                ; checked is_space argument handoff/' \
  "$T/control-check.alpha" > "$T/skip-ws-wrong-argument.alpha"
"$ASM" < "$T/skip-ws-wrong-argument.alpha" > "$T/skip-ws-wrong-argument.tape"
stamp_seed "$T/skip-ws-wrong-argument.tape" "$SEED" "$T/skip-ws-wrong-argument" >/dev/null
sed 's/imm r2, 1                    ; checked same-cursor cbyte pair/imm r2, 2                    ; checked same-cursor cbyte pair/' \
  "$T/control-check.alpha" > "$T/skip-ws-wrong-cursor.alpha"
"$ASM" < "$T/skip-ws-wrong-cursor.alpha" > "$T/skip-ws-wrong-cursor.tape"
stamp_seed "$T/skip-ws-wrong-cursor.tape" "$SEED" "$T/skip-ws-wrong-cursor" >/dev/null
sed 's/imm r20, 1                    ; checked comment-newline result/imm r20, 0                    ; checked comment-newline result/' \
  "$T/control-check.alpha" > "$T/skip-ws-wrong-newline.alpha"
"$ASM" < "$T/skip-ws-wrong-newline.alpha" > "$T/skip-ws-wrong-newline.tape"
stamp_seed "$T/skip-ws-wrong-newline.tape" "$SEED" "$T/skip-ws-wrong-newline" >/dev/null
sed 's/imm r20, 0                    ; checked comment-zero result/imm r20, 1                    ; checked comment-zero result/' \
  "$T/control-check.alpha" > "$T/skip-ws-wrong-zero.alpha"
"$ASM" < "$T/skip-ws-wrong-zero.alpha" > "$T/skip-ws-wrong-zero.tape"
stamp_seed "$T/skip-ws-wrong-zero.tape" "$SEED" "$T/skip-ws-wrong-zero" >/dev/null
sed 's/imm r2, 1                    ; checked comment rank delta/imm r2, 0                    ; checked comment rank delta/' \
  "$T/control-check.alpha" > "$T/skip-ws-zero-inner-rank.alpha"
"$ASM" < "$T/skip-ws-zero-inner-rank.alpha" > "$T/skip-ws-zero-inner-rank.tape"
stamp_seed "$T/skip-ws-zero-inner-rank.tape" "$SEED" "$T/skip-ws-zero-inner-rank" >/dev/null
sed 's/imm r21, 2                    ; checked result-one cursor progress/imm r21, 1                    ; checked result-one cursor progress/' \
  "$T/control-check.alpha" > "$T/skip-ws-no-step-progress.alpha"
"$ASM" < "$T/skip-ws-no-step-progress.alpha" > "$T/skip-ws-no-step-progress.tape"
stamp_seed "$T/skip-ws-no-step-progress.tape" "$SEED" "$T/skip-ws-no-step-progress" >/dev/null
sed 's/call skip_outer_exit          ; checked ordinary result-zero exit/call skip_outer_backedge      ; checked ordinary result-zero exit/' \
  "$T/control-check.alpha" > "$T/skip-ws-zero-backedge.alpha"
"$ASM" < "$T/skip-ws-zero-backedge.alpha" > "$T/skip-ws-zero-backedge.tape"
stamp_seed "$T/skip-ws-zero-backedge.tape" "$SEED" "$T/skip-ws-zero-backedge" >/dev/null
sed 's/imm r2, 1                    ; checked outer rank decrease/imm r2, 0                    ; checked outer rank decrease/' \
  "$T/control-check.alpha" > "$T/skip-ws-zero-outer-rank.alpha"
"$ASM" < "$T/skip-ws-zero-outer-rank.alpha" > "$T/skip-ws-zero-outer-rank.tape"
stamp_seed "$T/skip-ws-zero-outer-rank.tape" "$SEED" "$T/skip-ws-zero-outer-rank" >/dev/null
sed 's/imm r29, 33                   ; checked exclusive step event row/imm r29, 32                   ; checked exclusive step event row/' \
  "$T/control-check.alpha" > "$T/skip-ws-event-undercount.alpha"
"$ASM" < "$T/skip-ws-event-undercount.alpha" > "$T/skip-ws-event-undercount.tape"
stamp_seed "$T/skip-ws-event-undercount.tape" "$SEED" "$T/skip-ws-event-undercount" >/dev/null
sed 's/imm r1, 526320                ; checked domain-preserving reset bound/imm r1, 526328                ; checked domain-preserving reset bound/' \
  "$T/control-check.alpha" > "$T/skip-ws-drop-domain.alpha"
"$ASM" < "$T/skip-ws-drop-domain.alpha" > "$T/skip-ws-drop-domain.tape"
stamp_seed "$T/skip-ws-drop-domain.tape" "$SEED" "$T/skip-ws-drop-domain" >/dev/null
sed 's/imm r1, 526248                ; checked opening local c provenance/imm r1, 526256                ; checked opening local c provenance/' \
  "$T/control-check.alpha" > "$T/skip-ws-wrong-opening.alpha"
"$ASM" < "$T/skip-ws-wrong-opening.alpha" > "$T/skip-ws-wrong-opening.tape"
stamp_seed "$T/skip-ws-wrong-opening.tape" "$SEED" "$T/skip-ws-wrong-opening" >/dev/null
sed 's/imm r2, 1                    ; checked inner rank premise LEN-CUR/imm r2, 0                    ; checked inner rank premise LEN-CUR/' \
  "$T/control-check.alpha" > "$T/skip-ws-zero-inner-premise.alpha"
"$ASM" < "$T/skip-ws-zero-inner-premise.alpha" > "$T/skip-ws-zero-inner-premise.tape"
stamp_seed "$T/skip-ws-zero-inner-premise.tape" "$SEED" "$T/skip-ws-zero-inner-premise" >/dev/null
sed 's/imm r2, 1                    ; checked renamed comment cursor/imm r2, 2                    ; checked renamed comment cursor/' \
  "$T/control-check.alpha" > "$T/skip-ws-wrong-inner-rename.alpha"
"$ASM" < "$T/skip-ws-wrong-inner-rename.alpha" > "$T/skip-ws-wrong-inner-rename.tape"
stamp_seed "$T/skip-ws-wrong-inner-rename.tape" "$SEED" "$T/skip-ws-wrong-inner-rename" >/dev/null
sed 's/imm r2, 1                    ; checked outer rank premise LEN-CUR/imm r2, 0                    ; checked outer rank premise LEN-CUR/' \
  "$T/control-check.alpha" > "$T/skip-ws-zero-outer-premise.alpha"
"$ASM" < "$T/skip-ws-zero-outer-premise.alpha" > "$T/skip-ws-zero-outer-premise.tape"
stamp_seed "$T/skip-ws-zero-outer-premise.tape" "$SEED" "$T/skip-ws-zero-outer-premise" >/dev/null
sed 's/imm r2, 1                    ; checked renamed outer cursor/imm r2, 2                    ; checked renamed outer cursor/' \
  "$T/control-check.alpha" > "$T/skip-ws-wrong-outer-rename.alpha"
"$ASM" < "$T/skip-ws-wrong-outer-rename.alpha" > "$T/skip-ws-wrong-outer-rename.tape"
stamp_seed "$T/skip-ws-wrong-outer-rename.tape" "$SEED" "$T/skip-ws-wrong-outer-rename" >/dev/null

# Phase-isolated main.ready teeth preserve the successful root/slurp bridge and
# all three imported callee summaries while breaking one exact cutpoint, call,
# trace, cursor, transition, or exhaustive-effect join in only this phase.
sed 's/imm r20, 526440               ; checked imported ready clause/imm r20, 526448               ; checked imported ready clause/' \
  "$T/control-check.alpha" > "$T/main-ready-wrong-clause.alpha"
"$ASM" < "$T/main-ready-wrong-clause.alpha" > "$T/main-ready-wrong-clause.tape"
stamp_seed "$T/main-ready-wrong-clause.tape" "$SEED" "$T/main-ready-wrong-clause" >/dev/null
sed 's/imm r22, 51235               ; checked first continuation/imm r22, 51236               ; checked first continuation/' \
  "$T/control-check.alpha" > "$T/main-ready-wrong-continuation.alpha"
"$ASM" < "$T/main-ready-wrong-continuation.alpha" > "$T/main-ready-wrong-continuation.tape"
stamp_seed "$T/main-ready-wrong-continuation.tape" "$SEED" "$T/main-ready-wrong-continuation" >/dev/null
sed 's/imm r20, 525952               ; checked second theorem import/imm r20, 525944               ; checked second theorem import/' \
  "$T/control-check.alpha" > "$T/main-ready-wrong-summary.alpha"
"$ASM" < "$T/main-ready-wrong-summary.alpha" > "$T/main-ready-wrong-summary.tape"
stamp_seed "$T/main-ready-wrong-summary.tape" "$SEED" "$T/main-ready-wrong-summary" >/dev/null
sed 's/imm r3, 187                   ; checked composed prefix length/imm r3, 186                   ; checked composed prefix length/' \
  "$T/control-check.alpha" > "$T/main-ready-wrong-length.alpha"
"$ASM" < "$T/main-ready-wrong-length.alpha" > "$T/main-ready-wrong-length.tape"
stamp_seed "$T/main-ready-wrong-length.tape" "$SEED" "$T/main-ready-wrong-length" >/dev/null
sed 's/imm r21, 2                     ; checked skip emits epsilon/imm r21, 1                     ; checked skip emits epsilon/' \
  "$T/control-check.alpha" > "$T/main-ready-wrong-order.alpha"
"$ASM" < "$T/main-ready-wrong-order.alpha" > "$T/main-ready-wrong-order.tape"
stamp_seed "$T/main-ready-wrong-order.tape" "$SEED" "$T/main-ready-wrong-order" >/dev/null
sed 's/imm r2, 51262                 ; checked ready->loop target/imm r2, 51263                 ; checked ready->loop target/' \
  "$T/control-check.alpha" > "$T/main-ready-wrong-target.alpha"
"$ASM" < "$T/main-ready-wrong-target.alpha" > "$T/main-ready-wrong-target.tape"
stamp_seed "$T/main-ready-wrong-target.tape" "$SEED" "$T/main-ready-wrong-target" >/dev/null
sed 's/imm r29, 608                 ; checked exclusive ready event row/imm r29, 607                 ; checked exclusive ready event row/' \
  "$T/control-check.alpha" > "$T/main-ready-event-undercount.alpha"
"$ASM" < "$T/main-ready-event-undercount.alpha" > "$T/main-ready-event-undercount.tape"
stamp_seed "$T/main-ready-event-undercount.tape" "$SEED" "$T/main-ready-event-undercount" >/dev/null

# Phase-isolated main.loop-entry teeth retain the complete root prefix and
# cbyte theorem while breaking the call, expression, guarded split, terminal
# payload, body cutpoint, or exhaustive row inventory only in this phase.
sed 's/imm r26, 51271               ; checked loop cbyte continuation/imm r26, 51272               ; checked loop cbyte continuation/' \
  "$T/control-check.alpha" > "$T/main-loop-wrong-continuation.alpha"
"$ASM" < "$T/main-loop-wrong-continuation.alpha" > "$T/main-loop-wrong-continuation.tape"
stamp_seed "$T/main-loop-wrong-continuation.tape" "$SEED" "$T/main-loop-wrong-continuation" >/dev/null
sed 's/imm r23, 11                  ; !=/imm r23, 10                  ; !=/' \
  "$T/control-check.alpha" > "$T/main-loop-wrong-comparison.alpha"
"$ASM" < "$T/main-loop-wrong-comparison.alpha" > "$T/main-loop-wrong-comparison.tape"
stamp_seed "$T/main-loop-wrong-comparison.tape" "$SEED" "$T/main-loop-wrong-comparison" >/dev/null
sed 's/imm r21, 51375               ; checked zero continuation/imm r21, 51376               ; checked zero continuation/' \
  "$T/control-check.alpha" > "$T/main-loop-wrong-zero-target.alpha"
"$ASM" < "$T/main-loop-wrong-zero-target.alpha" > "$T/main-loop-wrong-zero-target.tape"
stamp_seed "$T/main-loop-wrong-zero-target.tape" "$SEED" "$T/main-loop-wrong-zero-target" >/dev/null
sed 's/imm r2, 2                    ; logical-end relation CUR=LEN/imm r2, 1                    ; logical-end relation CUR=LEN/' \
  "$T/control-check.alpha" > "$T/main-loop-wrong-end-clause.alpha"
"$ASM" < "$T/main-loop-wrong-end-clause.alpha" > "$T/main-loop-wrong-end-clause.tape"
stamp_seed "$T/main-loop-wrong-end-clause.tape" "$SEED" "$T/main-loop-wrong-end-clause" >/dev/null
sed 's/imm r2, 1                    ; nonzero hit relation CUR<LEN/imm r2, 2                    ; nonzero hit relation CUR<LEN/' \
  "$T/control-check.alpha" > "$T/main-loop-wrong-nonzero-clause.alpha"
"$ASM" < "$T/main-loop-wrong-nonzero-clause.alpha" > "$T/main-loop-wrong-nonzero-clause.tape"
stamp_seed "$T/main-loop-wrong-nonzero-clause.tape" "$SEED" "$T/main-loop-wrong-nonzero-clause" >/dev/null
sed 's/imm r2, 0                    ; checked zero != zero result/imm r2, 1                    ; checked zero != zero result/' \
  "$T/control-check.alpha" > "$T/main-loop-wrong-zero-result.alpha"
"$ASM" < "$T/main-loop-wrong-zero-result.alpha" > "$T/main-loop-wrong-zero-result.tape"
stamp_seed "$T/main-loop-wrong-zero-result.tape" "$SEED" "$T/main-loop-wrong-zero-result" >/dev/null
sed 's/imm r2, 0                    ; checked concrete halt payload/imm r2, 1                    ; checked concrete halt payload/' \
  "$T/control-check.alpha" > "$T/main-loop-wrong-halt.alpha"
"$ASM" < "$T/main-loop-wrong-halt.alpha" > "$T/main-loop-wrong-halt.tape"
stamp_seed "$T/main-loop-wrong-halt.tape" "$SEED" "$T/main-loop-wrong-halt" >/dev/null
sed 's/imm r2, 51405                 ; checked main.body cutpoint/imm r2, 51406                 ; checked main.body cutpoint/' \
  "$T/control-check.alpha" > "$T/main-loop-wrong-body.alpha"
"$ASM" < "$T/main-loop-wrong-body.alpha" > "$T/main-loop-wrong-body.tape"
stamp_seed "$T/main-loop-wrong-body.tape" "$SEED" "$T/main-loop-wrong-body" >/dev/null
sed 's/imm r23, 812                 ; checked exclusive loop primitive row/imm r23, 811                 ; checked exclusive loop primitive row/' \
  "$T/control-check.alpha" > "$T/main-loop-primitive-undercount.alpha"
"$ASM" < "$T/main-loop-primitive-undercount.alpha" > "$T/main-loop-primitive-undercount.tape"
stamp_seed "$T/main-loop-primitive-undercount.tape" "$SEED" "$T/main-loop-primitive-undercount" >/dev/null
sed 's/imm r29, 610                 ; checked exclusive loop event row/imm r29, 609                 ; checked exclusive loop event row/' \
  "$T/control-check.alpha" > "$T/main-loop-event-undercount.alpha"
"$ASM" < "$T/main-loop-event-undercount.alpha" > "$T/main-loop-event-undercount.tape"
stamp_seed "$T/main-loop-event-undercount.tape" "$SEED" "$T/main-loop-event-undercount" >/dev/null
sed 's/imm r21, 1347636301            ; required conditional MLSP/imm r21, 1297238352            ; required conditional MLSP/' \
  "$T/control-check.alpha" > "$T/main-loop-wrong-generic.alpha"
"$ASM" < "$T/main-loop-wrong-generic.alpha" > "$T/main-loop-wrong-generic.tape"
stamp_seed "$T/main-loop-wrong-generic.tape" "$SEED" "$T/main-loop-wrong-generic" >/dev/null
sed 's/imm r27, 1                     ; checked root source bridge token/imm r27, 2                     ; checked root source bridge token/' \
  "$T/control-check.alpha" > "$T/main-loop-wrong-source-bridge.alpha"
"$ASM" < "$T/main-loop-wrong-source-bridge.alpha" > "$T/main-loop-wrong-source-bridge.tape"
stamp_seed "$T/main-loop-wrong-source-bridge.tape" "$SEED" "$T/main-loop-wrong-source-bridge" >/dev/null

# Phase-isolated byte-classifier teeth keep the exact artifact and prior root
# summaries fixed while breaking the byte premise, independent interval
# specification, call/argument joins, exact source boundary/opcode, or one
# whole-table census in the new shape/meaning pair.
sed 's/imm r2, 1                    ; checked 0<=c<=255 premise/imm r2, 0                    ; checked 0<=c<=255 premise/' \
  "$T/control-check.alpha" > "$T/classifier-drop-domain.alpha"
"$ASM" < "$T/classifier-drop-domain.alpha" > "$T/classifier-drop-domain.tape"
stamp_seed "$T/classifier-drop-domain.tape" "$SEED" "$T/classifier-drop-domain" >/dev/null
sed 's/imm r3, 58                   ; checked digit spec exclusive upper/imm r3, 57                   ; checked digit spec exclusive upper/' \
  "$T/control-check.alpha" > "$T/classifier-digit-spec-bound.alpha"
"$ASM" < "$T/classifier-digit-spec-bound.alpha" > "$T/classifier-digit-spec-bound.tape"
stamp_seed "$T/classifier-digit-spec-bound.tape" "$SEED" "$T/classifier-digit-spec-bound" >/dev/null
sed 's/imm r3, 91                   ; checked alpha spec uppercase exclusive/imm r3, 90                   ; checked alpha spec uppercase exclusive/' \
  "$T/control-check.alpha" > "$T/classifier-alpha-spec-bound.alpha"
"$ASM" < "$T/classifier-alpha-spec-bound.alpha" > "$T/classifier-alpha-spec-bound.tape"
stamp_seed "$T/classifier-alpha-spec-bound.tape" "$SEED" "$T/classifier-alpha-spec-bound" >/dev/null
sed 's/imm r2, 1                    ; checked digit handoff relation/imm r2, 2                    ; checked digit handoff relation/' \
  "$T/control-check.alpha" > "$T/classifier-wrong-handoff.alpha"
"$ASM" < "$T/classifier-wrong-handoff.alpha" > "$T/classifier-wrong-handoff.tape"
stamp_seed "$T/classifier-wrong-handoff.tape" "$SEED" "$T/classifier-wrong-handoff" >/dev/null
sed 's/imm r23, 12                  ; checked uppercase <=/imm r23, 8                   ; checked uppercase <=/' \
  "$T/control-check.alpha" > "$T/classifier-wrong-upper-op.alpha"
"$ASM" < "$T/classifier-wrong-upper-op.alpha" > "$T/classifier-wrong-upper-op.tape"
stamp_seed "$T/classifier-wrong-upper-op.tape" "$SEED" "$T/classifier-wrong-upper-op" >/dev/null
sed 's/imm r26, 3393               ; checked alpha continuation/imm r26, 3394               ; checked alpha continuation/' \
  "$T/control-check.alpha" > "$T/classifier-wrong-continuation.alpha"
"$ASM" < "$T/classifier-wrong-continuation.alpha" > "$T/classifier-wrong-continuation.tape"
stamp_seed "$T/classifier-wrong-continuation.tape" "$SEED" "$T/classifier-wrong-continuation" >/dev/null
sed 's/imm r23, 3516               ; checked digit argument handoff/imm r23, 3517               ; checked digit argument handoff/' \
  "$T/control-check.alpha" > "$T/classifier-wrong-argument.alpha"
"$ASM" < "$T/classifier-wrong-argument.alpha" > "$T/classifier-wrong-argument.tape"
stamp_seed "$T/classifier-wrong-argument.tape" "$SEED" "$T/classifier-wrong-argument" >/dev/null
sed 's/imm r23, 61                  ; checked exclusive classifier primitive row/imm r23, 60                  ; checked exclusive classifier primitive row/' \
  "$T/control-check.alpha" > "$T/classifier-primitive-undercount.alpha"
"$ASM" < "$T/classifier-primitive-undercount.alpha" > "$T/classifier-primitive-undercount.tape"
stamp_seed "$T/classifier-primitive-undercount.tape" "$SEED" "$T/classifier-primitive-undercount" >/dev/null
sed 's/imm r29, 19                  ; checked exclusive classifier event row/imm r29, 18                  ; checked exclusive classifier event row/' \
  "$T/control-check.alpha" > "$T/classifier-event-undercount.alpha"
"$ASM" < "$T/classifier-event-undercount.alpha" > "$T/classifier-event-undercount.tape"
stamp_seed "$T/classifier-event-undercount.tape" "$SEED" "$T/classifier-event-undercount" >/dev/null
sed 's/imm r23, 95                  ; checked underscore/imm r23, 94                  ; checked underscore/' \
  "$T/control-check.alpha" > "$T/classifier-wrong-underscore.alpha"
"$ASM" < "$T/classifier-wrong-underscore.alpha" > "$T/classifier-wrong-underscore.tape"
stamp_seed "$T/classifier-wrong-underscore.tape" "$SEED" "$T/classifier-wrong-underscore" >/dev/null

# Phase-isolated read_ident teeth break exact calls/argument flow, fixed-global
# addresses, subtraction, table closure, or the terminating scan relation.
sed 's/imm r26, 5303               ; checked read_ident cbyte continuation/imm r26, 5304               ; checked read_ident cbyte continuation/' \
  "$T/control-check.alpha" > "$T/read-ident-wrong-cbyte-continuation.alpha"
"$ASM" < "$T/read-ident-wrong-cbyte-continuation.alpha" > "$T/read-ident-wrong-cbyte-continuation.tape"
stamp_seed "$T/read-ident-wrong-cbyte-continuation.tape" "$SEED" "$T/read-ident-wrong-cbyte-continuation" >/dev/null
sed 's/imm r26, 5344               ; checked read_ident alnum continuation/imm r26, 5345               ; checked read_ident alnum continuation/' \
  "$T/control-check.alpha" > "$T/read-ident-wrong-alnum-continuation.alpha"
"$ASM" < "$T/read-ident-wrong-alnum-continuation.alpha" > "$T/read-ident-wrong-alnum-continuation.tape"
stamp_seed "$T/read-ident-wrong-alnum-continuation.tape" "$SEED" "$T/read-ident-wrong-alnum-continuation" >/dev/null
sed 's/imm r23, 5303               ; checked cbyte-to-alnum argument/imm r23, 5304               ; checked cbyte-to-alnum argument/' \
  "$T/control-check.alpha" > "$T/read-ident-wrong-argument.alpha"
"$ASM" < "$T/read-ident-wrong-argument.alpha" > "$T/read-ident-wrong-argument.tape"
stamp_seed "$T/read-ident-wrong-argument.tape" "$SEED" "$T/read-ident-wrong-argument" >/dev/null
sed 's/imm r23, 2097120             ; checked IDOFF address/imm r23, 2097121             ; checked IDOFF address/' \
  "$T/control-check.alpha" > "$T/read-ident-wrong-idoff.alpha"
"$ASM" < "$T/read-ident-wrong-idoff.alpha" > "$T/read-ident-wrong-idoff.tape"
stamp_seed "$T/read-ident-wrong-idoff.tape" "$SEED" "$T/read-ident-wrong-idoff" >/dev/null
sed 's/imm r23, 2097112             ; checked IDLEN address/imm r23, 2097113             ; checked IDLEN address/' \
  "$T/control-check.alpha" > "$T/read-ident-wrong-idlen.alpha"
"$ASM" < "$T/read-ident-wrong-idlen.alpha" > "$T/read-ident-wrong-idlen.tape"
stamp_seed "$T/read-ident-wrong-idlen.tape" "$SEED" "$T/read-ident-wrong-idlen" >/dev/null
sed 's/imm r23, 4                   ; checked CUR-IDOFF subtraction/imm r23, 3                   ; checked CUR-IDOFF subtraction/' \
  "$T/control-check.alpha" > "$T/read-ident-wrong-subtraction.alpha"
"$ASM" < "$T/read-ident-wrong-subtraction.alpha" > "$T/read-ident-wrong-subtraction.tape"
stamp_seed "$T/read-ident-wrong-subtraction.tape" "$SEED" "$T/read-ident-wrong-subtraction" >/dev/null
sed 's/imm r25, 21                  ; checked exclusive read_ident memory row/imm r25, 20                  ; checked exclusive read_ident memory row/' \
  "$T/control-check.alpha" > "$T/read-ident-memory-undercount.alpha"
"$ASM" < "$T/read-ident-memory-undercount.alpha" > "$T/read-ident-memory-undercount.tape"
stamp_seed "$T/read-ident-memory-undercount.tape" "$SEED" "$T/read-ident-memory-undercount" >/dev/null
sed 's/imm r29, 37                  ; checked exclusive read_ident event row/imm r29, 36                  ; checked exclusive read_ident event row/' \
  "$T/control-check.alpha" > "$T/read-ident-event-undercount.alpha"
"$ASM" < "$T/read-ident-event-undercount.alpha" > "$T/read-ident-event-undercount.tape"
stamp_seed "$T/read-ident-event-undercount.tape" "$SEED" "$T/read-ident-event-undercount" >/dev/null
sed 's/imm r23, 95                  ; checked exclusive read_ident primitive row/imm r23, 94                  ; checked exclusive read_ident primitive row/' \
  "$T/control-check.alpha" > "$T/read-ident-primitive-undercount.alpha"
"$ASM" < "$T/read-ident-primitive-undercount.alpha" > "$T/read-ident-primitive-undercount.tape"
stamp_seed "$T/read-ident-primitive-undercount.tape" "$SEED" "$T/read-ident-primitive-undercount" >/dev/null
sed 's/imm r2, 1                    ; checked read_ident rank decrease/imm r2, 0                    ; checked read_ident rank decrease/' \
  "$T/control-check.alpha" > "$T/read-ident-zero-rank.alpha"
"$ASM" < "$T/read-ident-zero-rank.alpha" > "$T/read-ident-zero-rank.tape"
stamp_seed "$T/read-ident-zero-rank.tape" "$SEED" "$T/read-ident-zero-rank" >/dev/null
sed 's/imm r2, 2                    ; checked read_ident successor renaming/imm r2, 1                    ; checked read_ident successor renaming/' \
  "$T/control-check.alpha" > "$T/read-ident-wrong-rename.alpha"
"$ASM" < "$T/read-ident-wrong-rename.alpha" > "$T/read-ident-wrong-rename.tape"
stamp_seed "$T/read-ident-wrong-rename.tape" "$SEED" "$T/read-ident-wrong-rename" >/dev/null
sed 's/imm r2, 1                    ; checked first non-alnum\/end\/NUL stop/imm r2, 2                    ; checked first non-alnum\/end\/NUL stop/' \
  "$T/control-check.alpha" > "$T/read-ident-wrong-stop.alpha"
"$ASM" < "$T/read-ident-wrong-stop.alpha" > "$T/read-ident-wrong-stop.tape"
stamp_seed "$T/read-ident-wrong-stop.tape" "$SEED" "$T/read-ident-wrong-stop" >/dev/null

# Phase-isolated expect teeth sever one call/local/comparison/census join or one
# side of the nonzero-delimiter cursor relation.
sed 's/imm r26, 17501               ; checked expect skip_ws continuation/imm r26, 17502               ; checked expect skip_ws continuation/' \
  "$T/control-check.alpha" > "$T/expect-wrong-skip-continuation.alpha"
"$ASM" < "$T/expect-wrong-skip-continuation.alpha" > "$T/expect-wrong-skip-continuation.tape"
stamp_seed "$T/expect-wrong-skip-continuation.tape" "$SEED" "$T/expect-wrong-skip-continuation" >/dev/null
sed 's/imm r26, 17510               ; checked expect cbyte continuation/imm r26, 17511               ; checked expect cbyte continuation/' \
  "$T/control-check.alpha" > "$T/expect-wrong-cbyte-continuation.alpha"
"$ASM" < "$T/expect-wrong-cbyte-continuation.alpha" > "$T/expect-wrong-cbyte-continuation.tape"
stamp_seed "$T/expect-wrong-cbyte-continuation.tape" "$SEED" "$T/expect-wrong-cbyte-continuation" >/dev/null
sed 's/imm r26, 17662               ; checked expect adv continuation/imm r26, 17663               ; checked expect adv continuation/' \
  "$T/control-check.alpha" > "$T/expect-wrong-adv-continuation.alpha"
"$ASM" < "$T/expect-wrong-adv-continuation.alpha" > "$T/expect-wrong-adv-continuation.tape"
stamp_seed "$T/expect-wrong-adv-continuation.tape" "$SEED" "$T/expect-wrong-adv-continuation" >/dev/null
sed 's/imm r23, 0                   ; checked expect ch slot/imm r23, 1                   ; checked expect ch slot/' \
  "$T/control-check.alpha" > "$T/expect-wrong-slot.alpha"
"$ASM" < "$T/expect-wrong-slot.alpha" > "$T/expect-wrong-slot.tape"
stamp_seed "$T/expect-wrong-slot.tape" "$SEED" "$T/expect-wrong-slot" >/dev/null
sed 's/imm r23, 10                  ; checked expect equality/imm r23, 11                  ; checked expect equality/' \
  "$T/control-check.alpha" > "$T/expect-wrong-comparison.alpha"
"$ASM" < "$T/expect-wrong-comparison.alpha" > "$T/expect-wrong-comparison.tape"
stamp_seed "$T/expect-wrong-comparison.tape" "$SEED" "$T/expect-wrong-comparison" >/dev/null
sed 's/imm r29, 165                 ; checked exclusive expect event row/imm r29, 164                 ; checked exclusive expect event row/' \
  "$T/control-check.alpha" > "$T/expect-event-undercount.alpha"
"$ASM" < "$T/expect-event-undercount.alpha" > "$T/expect-event-undercount.tape"
stamp_seed "$T/expect-event-undercount.tape" "$SEED" "$T/expect-event-undercount" >/dev/null
sed 's/imm r23, 356                 ; checked exclusive expect primitive row/imm r23, 355                 ; checked exclusive expect primitive row/' \
  "$T/control-check.alpha" > "$T/expect-primitive-undercount.alpha"
"$ASM" < "$T/expect-primitive-undercount.alpha" > "$T/expect-primitive-undercount.tape"
stamp_seed "$T/expect-primitive-undercount.tape" "$SEED" "$T/expect-primitive-undercount" >/dev/null
sed 's/imm r1, 526976/imm r1, 526968/' \
  "$T/control-check.alpha" > "$T/expect-drop-delimiter-premise.alpha"
"$ASM" < "$T/expect-drop-delimiter-premise.alpha" > "$T/expect-drop-delimiter-premise.tape"
stamp_seed "$T/expect-drop-delimiter-premise.tape" "$SEED" "$T/expect-drop-delimiter-premise" >/dev/null
sed 's/imm r2, 1                    ; checked mismatch preserves normalized CUR/imm r2, 2                    ; checked mismatch preserves normalized CUR/' \
  "$T/control-check.alpha" > "$T/expect-wrong-mismatch-cursor.alpha"
"$ASM" < "$T/expect-wrong-mismatch-cursor.alpha" > "$T/expect-wrong-mismatch-cursor.tape"
stamp_seed "$T/expect-wrong-mismatch-cursor.tape" "$SEED" "$T/expect-wrong-mismatch-cursor" >/dev/null
sed 's/imm r2, 2                    ; checked match consumes exactly one byte/imm r2, 1                    ; checked match consumes exactly one byte/' \
  "$T/control-check.alpha" > "$T/expect-wrong-match-cursor.alpha"
"$ASM" < "$T/expect-wrong-match-cursor.alpha" > "$T/expect-wrong-match-cursor.tape"
stamp_seed "$T/expect-wrong-match-cursor.tape" "$SEED" "$T/expect-wrong-match-cursor" >/dev/null
sed 's/imm r2, 1                    ; checked nonzero match entails CUR<LEN/imm r2, 0                    ; checked nonzero match entails CUR<LEN/' \
  "$T/control-check.alpha" > "$T/expect-drop-match-range.alpha"
"$ASM" < "$T/expect-drop-match-range.alpha" > "$T/expect-drop-match-range.tape"
stamp_seed "$T/expect-drop-match-range.tape" "$SEED" "$T/expect-drop-match-range" >/dev/null

# Phase-isolated declare teeth break exact guard/value/table closure or one
# branch of the conditional insertion/resource relation.
sed 's/imm r24, 24313               ; checked declare room guard/imm r24, 24314               ; checked declare room guard/' \
  "$T/control-check.alpha" > "$T/declare-wrong-guard.alpha"
"$ASM" < "$T/declare-wrong-guard.alpha" > "$T/declare-wrong-guard.tape"
stamp_seed "$T/declare-wrong-guard.tape" "$SEED" "$T/declare-wrong-guard" >/dev/null
sed 's/imm r24, 24190               ; checked NLOC snapshot into s/imm r24, 24191               ; checked NLOC snapshot into s/' \
  "$T/control-check.alpha" > "$T/declare-wrong-snapshot.alpha"
"$ASM" < "$T/declare-wrong-snapshot.alpha" > "$T/declare-wrong-snapshot.tape"
stamp_seed "$T/declare-wrong-snapshot.tape" "$SEED" "$T/declare-wrong-snapshot" >/dev/null
sed 's/imm r23, 1024                ; checked declare capacity/imm r23, 1023                ; checked declare capacity/' \
  "$T/control-check.alpha" > "$T/declare-wrong-capacity.alpha"
"$ASM" < "$T/declare-wrong-capacity.alpha" > "$T/declare-wrong-capacity.tape"
stamp_seed "$T/declare-wrong-capacity.tape" "$SEED" "$T/declare-wrong-capacity" >/dev/null
sed 's/imm r23, 252                 ; checked declare exhaustion status/imm r23, 253                 ; checked declare exhaustion status/' \
  "$T/control-check.alpha" > "$T/declare-wrong-status.alpha"
"$ASM" < "$T/declare-wrong-status.alpha" > "$T/declare-wrong-status.tape"
stamp_seed "$T/declare-wrong-status.tape" "$SEED" "$T/declare-wrong-status" >/dev/null
sed 's/imm r23, 2097120             ; IDOFF payload/imm r23, 2097128             ; IDOFF payload/' \
  "$T/control-check.alpha" > "$T/declare-wrong-idoff.alpha"
"$ASM" < "$T/declare-wrong-idoff.alpha" > "$T/declare-wrong-idoff.tape"
stamp_seed "$T/declare-wrong-idoff.tape" "$SEED" "$T/declare-wrong-idoff" >/dev/null
sed 's/imm r23, 2097112             ; IDLEN payload/imm r23, 2097120             ; IDLEN payload/' \
  "$T/control-check.alpha" > "$T/declare-wrong-idlen.alpha"
"$ASM" < "$T/declare-wrong-idlen.alpha" > "$T/declare-wrong-idlen.tape"
stamp_seed "$T/declare-wrong-idlen.tape" "$SEED" "$T/declare-wrong-idlen" >/dev/null
sed 's/imm r25, 44                  ; checked exclusive declare memory row/imm r25, 43                  ; checked exclusive declare memory row/' \
  "$T/control-check.alpha" > "$T/declare-memory-undercount.alpha"
"$ASM" < "$T/declare-memory-undercount.alpha" > "$T/declare-memory-undercount.tape"
stamp_seed "$T/declare-memory-undercount.tape" "$SEED" "$T/declare-memory-undercount" >/dev/null
sed 's/imm r23, 451                 ; checked exclusive declare primitive row/imm r23, 450                 ; checked exclusive declare primitive row/' \
  "$T/control-check.alpha" > "$T/declare-primitive-undercount.alpha"
"$ASM" < "$T/declare-primitive-undercount.alpha" > "$T/declare-primitive-undercount.tape"
stamp_seed "$T/declare-primitive-undercount.tape" "$SEED" "$T/declare-primitive-undercount" >/dev/null
sed 's/imm r2, 1                    ; checked full-table return zero/imm r2, 2                    ; checked full-table return zero/' \
  "$T/control-check.alpha" > "$T/declare-wrong-full-return.alpha"
"$ASM" < "$T/declare-wrong-full-return.alpha" > "$T/declare-wrong-full-return.tape"
stamp_seed "$T/declare-wrong-full-return.tape" "$SEED" "$T/declare-wrong-full-return" >/dev/null
sed 's/imm r2, 1                    ; checked 0<=s<=1023 table index/imm r2, 0                    ; checked 0<=s<=1023 table index/' \
  "$T/control-check.alpha" > "$T/declare-drop-table-bound.alpha"
"$ASM" < "$T/declare-drop-table-bound.alpha" > "$T/declare-drop-table-bound.tape"
stamp_seed "$T/declare-drop-table-bound.tape" "$SEED" "$T/declare-drop-table-bound" >/dev/null
sed 's/imm r2, 2                    ; checked NLOC=s+1 in \[1,1024\]/imm r2, 1                    ; checked NLOC=s+1 in [1,1024]/' \
  "$T/control-check.alpha" > "$T/declare-wrong-nloc-update.alpha"
"$ASM" < "$T/declare-wrong-nloc-update.alpha" > "$T/declare-wrong-nloc-update.tape"
stamp_seed "$T/declare-wrong-nloc-update.tape" "$SEED" "$T/declare-wrong-nloc-update" >/dev/null
sed 's/imm r2, 2                    ; checked successful return is s/imm r2, 1                    ; checked successful return is s/' \
  "$T/control-check.alpha" > "$T/declare-wrong-room-return.alpha"
"$ASM" < "$T/declare-wrong-room-return.alpha" > "$T/declare-wrong-room-return.tape"
stamp_seed "$T/declare-wrong-room-return.tape" "$SEED" "$T/declare-wrong-room-return" >/dev/null

# Phase-isolated let-keyword teeth break id_char bounds/addressing, one exact
# short-circuit row, or an exhaustive keyword theorem branch.
sed 's/imm r24, 5963               ; checked is_let length guard/imm r24, 5964               ; checked is_let length guard/' \
  "$T/control-check.alpha" > "$T/let-keyword-wrong-length-guard.alpha"
"$ASM" < "$T/let-keyword-wrong-length-guard.alpha" > "$T/let-keyword-wrong-length-guard.tape"
stamp_seed "$T/let-keyword-wrong-length-guard.tape" "$SEED" "$T/let-keyword-wrong-length-guard" >/dev/null
sed 's/imm r26, 6063               ; checked let\[0\] continuation/imm r26, 6064               ; checked let[0] continuation/' \
  "$T/control-check.alpha" > "$T/let-keyword-wrong-continuation.alpha"
"$ASM" < "$T/let-keyword-wrong-continuation.alpha" > "$T/let-keyword-wrong-continuation.tape"
stamp_seed "$T/let-keyword-wrong-continuation.tape" "$SEED" "$T/let-keyword-wrong-continuation" >/dev/null
sed 's/imm r23, 2097120             ; checked id_char IDOFF/imm r23, 2097112             ; checked id_char IDOFF/' \
  "$T/control-check.alpha" > "$T/let-keyword-wrong-idoff.alpha"
"$ASM" < "$T/let-keyword-wrong-idoff.alpha" > "$T/let-keyword-wrong-idoff.tape"
stamp_seed "$T/let-keyword-wrong-idoff.tape" "$SEED" "$T/let-keyword-wrong-idoff" >/dev/null
sed 's/imm r23, 2097112             ; checked is_let IDLEN/imm r23, 2097120             ; checked is_let IDLEN/' \
  "$T/control-check.alpha" > "$T/let-keyword-wrong-idlen.alpha"
"$ASM" < "$T/let-keyword-wrong-idlen.alpha" > "$T/let-keyword-wrong-idlen.tape"
stamp_seed "$T/let-keyword-wrong-idlen.tape" "$SEED" "$T/let-keyword-wrong-idlen" >/dev/null
sed 's/imm r23, 0                   ; checked let index zero/imm r23, 1                   ; checked let index zero/' \
  "$T/control-check.alpha" > "$T/let-keyword-wrong-index.alpha"
"$ASM" < "$T/let-keyword-wrong-index.alpha" > "$T/let-keyword-wrong-index.tape"
stamp_seed "$T/let-keyword-wrong-index.tape" "$SEED" "$T/let-keyword-wrong-index" >/dev/null
sed 's/imm r23, 108                 ; '\''l'\''/imm r23, 107                 ; '\''l'\''/' \
  "$T/control-check.alpha" > "$T/let-keyword-wrong-l.alpha"
"$ASM" < "$T/let-keyword-wrong-l.alpha" > "$T/let-keyword-wrong-l.tape"
stamp_seed "$T/let-keyword-wrong-l.tape" "$SEED" "$T/let-keyword-wrong-l" >/dev/null
sed 's/imm r23, 6392               ; checked let\[2\] argument/imm r23, 6393               ; checked let[2] argument/' \
  "$T/control-check.alpha" > "$T/let-keyword-wrong-argument.alpha"
"$ASM" < "$T/let-keyword-wrong-argument.alpha" > "$T/let-keyword-wrong-argument.tape"
stamp_seed "$T/let-keyword-wrong-argument.tape" "$SEED" "$T/let-keyword-wrong-argument" >/dev/null
sed 's/imm r29, 46                  ; checked exclusive is_let event row/imm r29, 45                  ; checked exclusive is_let event row/' \
  "$T/control-check.alpha" > "$T/let-keyword-event-undercount.alpha"
"$ASM" < "$T/let-keyword-event-undercount.alpha" > "$T/let-keyword-event-undercount.tape"
stamp_seed "$T/let-keyword-event-undercount.tape" "$SEED" "$T/let-keyword-event-undercount" >/dev/null
sed 's/imm r23, 116                 ; checked exclusive let primitive row/imm r23, 115                 ; checked exclusive let primitive row/' \
  "$T/control-check.alpha" > "$T/let-keyword-primitive-undercount.alpha"
"$ASM" < "$T/let-keyword-primitive-undercount.alpha" > "$T/let-keyword-primitive-undercount.tape"
stamp_seed "$T/let-keyword-primitive-undercount.tape" "$SEED" "$T/let-keyword-primitive-undercount" >/dev/null
sed 's/imm r2, 1                    ; checked call-specific k bound/imm r2, 0                    ; checked call-specific k bound/' \
  "$T/control-check.alpha" > "$T/let-keyword-drop-k-bound.alpha"
"$ASM" < "$T/let-keyword-drop-k-bound.alpha" > "$T/let-keyword-drop-k-bound.tape"
stamp_seed "$T/let-keyword-drop-k-bound.tape" "$SEED" "$T/let-keyword-drop-k-bound" >/dev/null
sed 's/imm r2, 1                    ; checked IDLEN != 3 short circuit/imm r2, 2                    ; checked IDLEN != 3 short circuit/' \
  "$T/control-check.alpha" > "$T/let-keyword-wrong-length-clause.alpha"
"$ASM" < "$T/let-keyword-wrong-length-clause.alpha" > "$T/let-keyword-wrong-length-clause.tape"
stamp_seed "$T/let-keyword-wrong-length-clause.tape" "$SEED" "$T/let-keyword-wrong-length-clause" >/dev/null
sed 's/store r1, r2                  ; checked exact let returns one/store r1, r1                  ; checked exact let returns one/' \
  "$T/control-check.alpha" > "$T/let-keyword-wrong-result.alpha"
"$ASM" < "$T/let-keyword-wrong-result.alpha" > "$T/let-keyword-wrong-result.tape"
stamp_seed "$T/let-keyword-wrong-result.tape" "$SEED" "$T/let-keyword-wrong-result" >/dev/null

# Phase-isolated literal-skip teeth sever exact CFG/call/census facts, the
# bounded ADVX consequence, malformed-tail deltas, or a string fixed-point
# case. Each leaves the exact source, artifact, witness, and prior phases intact.
sed 's/imm r24, 26809              ; checked char backslash guard/imm r24, 26810              ; checked char backslash guard/' \
  "$T/control-check.alpha" > "$T/literal-skip-wrong-char-guard.alpha"
"$ASM" < "$T/literal-skip-wrong-char-guard.alpha" > "$T/literal-skip-wrong-char-guard.tape"
stamp_seed "$T/literal-skip-wrong-char-guard.tape" "$SEED" "$T/literal-skip-wrong-char-guard" >/dev/null
sed 's/imm r26, 26873              ; checked final char advance continuation/imm r26, 26874              ; checked final char advance continuation/' \
  "$T/control-check.alpha" > "$T/literal-skip-wrong-char-continuation.alpha"
"$ASM" < "$T/literal-skip-wrong-char-continuation.alpha" > "$T/literal-skip-wrong-char-continuation.tape"
stamp_seed "$T/literal-skip-wrong-char-continuation.tape" "$SEED" "$T/literal-skip-wrong-char-continuation" >/dev/null
sed 's/imm r26, 27365              ; checked escape tail continuation/imm r26, 27366              ; checked escape tail continuation/' \
  "$T/control-check.alpha" > "$T/literal-skip-wrong-escape-continuation.alpha"
"$ASM" < "$T/literal-skip-wrong-escape-continuation.alpha" > "$T/literal-skip-wrong-escape-continuation.tape"
stamp_seed "$T/literal-skip-wrong-escape-continuation.tape" "$SEED" "$T/literal-skip-wrong-escape-continuation" >/dev/null
sed 's/imm r23, 34                  ; checked closing quote/imm r23, 35                  ; checked closing quote/' \
  "$T/control-check.alpha" > "$T/literal-skip-wrong-closing-quote.alpha"
"$ASM" < "$T/literal-skip-wrong-closing-quote.alpha" > "$T/literal-skip-wrong-closing-quote.tape"
stamp_seed "$T/literal-skip-wrong-closing-quote.tape" "$SEED" "$T/literal-skip-wrong-closing-quote" >/dev/null
sed 's/imm r29, 297                 ; checked exclusive string event row/imm r29, 296                 ; checked exclusive string event row/' \
  "$T/control-check.alpha" > "$T/literal-skip-event-undercount.alpha"
"$ASM" < "$T/literal-skip-event-undercount.alpha" > "$T/literal-skip-event-undercount.tape"
stamp_seed "$T/literal-skip-event-undercount.tape" "$SEED" "$T/literal-skip-event-undercount" >/dev/null
sed 's/imm r23, 496                 ; checked exclusive string primitive row/imm r23, 495                 ; checked exclusive string primitive row/' \
  "$T/control-check.alpha" > "$T/literal-skip-primitive-undercount.alpha"
"$ASM" < "$T/literal-skip-primitive-undercount.alpha" > "$T/literal-skip-primitive-undercount.tape"
stamp_seed "$T/literal-skip-primitive-undercount.tape" "$SEED" "$T/literal-skip-primitive-undercount" >/dev/null
sed 's/imm r2, 1                    ; checked 0<=CUR<=CAP+1/imm r2, 0                    ; checked 0<=CUR<=CAP+1/' \
  "$T/control-check.alpha" > "$T/literal-skip-drop-advx-bound.alpha"
"$ASM" < "$T/literal-skip-drop-advx-bound.alpha" > "$T/literal-skip-drop-advx-bound.tape"
stamp_seed "$T/literal-skip-drop-advx-bound.tape" "$SEED" "$T/literal-skip-drop-advx-bound" >/dev/null
sed 's/store r1, r2                  ; checked exact CUR+1<=CAP+2/store r1, r1                  ; checked exact CUR+1<=CAP+2/' \
  "$T/control-check.alpha" > "$T/literal-skip-wrong-advx-successor.alpha"
"$ASM" < "$T/literal-skip-wrong-advx-successor.alpha" > "$T/literal-skip-wrong-advx-successor.tape"
stamp_seed "$T/literal-skip-wrong-advx-successor.tape" "$SEED" "$T/literal-skip-wrong-advx-successor" >/dev/null
sed 's/imm r2, 3                    ; checked ordinary total delta/imm r2, 4                    ; checked ordinary total delta/' \
  "$T/control-check.alpha" > "$T/literal-skip-wrong-char-delta.alpha"
"$ASM" < "$T/literal-skip-wrong-char-delta.alpha" > "$T/literal-skip-wrong-char-delta.tape"
stamp_seed "$T/literal-skip-wrong-char-delta.tape" "$SEED" "$T/literal-skip-wrong-char-delta" >/dev/null
sed 's/imm r2, 1                    ; checked ordinary final CUR<=LEN+2/imm r2, 2                    ; checked ordinary final CUR<=LEN+2/' \
  "$T/control-check.alpha" > "$T/literal-skip-wrong-char-bound.alpha"
"$ASM" < "$T/literal-skip-wrong-char-bound.alpha" > "$T/literal-skip-wrong-char-bound.tape"
stamp_seed "$T/literal-skip-wrong-char-bound.tape" "$SEED" "$T/literal-skip-wrong-char-bound" >/dev/null
sed 's/imm r2, 2                    ; checked cursor preserved/imm r2, 1                    ; checked cursor preserved/' \
  "$T/control-check.alpha" > "$T/literal-skip-wrong-zero-cursor.alpha"
"$ASM" < "$T/literal-skip-wrong-zero-cursor.alpha" > "$T/literal-skip-wrong-zero-cursor.tape"
stamp_seed "$T/literal-skip-wrong-zero-cursor.tape" "$SEED" "$T/literal-skip-wrong-zero-cursor" >/dev/null
sed 's/imm r2, 1                    ; checked rank decrease by one/imm r2, 0                    ; checked rank decrease by one/' \
  "$T/control-check.alpha" > "$T/literal-skip-zero-ordinary-rank.alpha"
"$ASM" < "$T/literal-skip-zero-ordinary-rank.alpha" > "$T/literal-skip-zero-ordinary-rank.tape"
stamp_seed "$T/literal-skip-zero-ordinary-rank.tape" "$SEED" "$T/literal-skip-zero-ordinary-rank" >/dev/null
sed 's/imm r2, 2                    ; checked rank decrease by two/imm r2, 1                    ; checked rank decrease by two/' \
  "$T/control-check.alpha" > "$T/literal-skip-wrong-escape-rank.alpha"
"$ASM" < "$T/literal-skip-wrong-escape-rank.alpha" > "$T/literal-skip-wrong-escape-rank.tape"
stamp_seed "$T/literal-skip-wrong-escape-rank.tape" "$SEED" "$T/literal-skip-wrong-escape-rank" >/dev/null
sed 's/store r1, r2                  ; checked smaller rank renamed/store r1, r1                  ; checked smaller rank renamed/' \
  "$T/control-check.alpha" > "$T/literal-skip-wrong-backedge-rename.alpha"
"$ASM" < "$T/literal-skip-wrong-backedge-rename.alpha" > "$T/literal-skip-wrong-backedge-rename.tape"
stamp_seed "$T/literal-skip-wrong-backedge-rename.tape" "$SEED" "$T/literal-skip-wrong-backedge-rename" >/dev/null
count_lets_build_teeth
parse_parameter_build_teeth
parse_capacity_build_teeth
emit_ident_build_teeth
emit_dec_build_teeth
fixed_decimal_emitters_build_teeth
parse_output_prefix_build_teeth
gen_stmts_boundary_build_teeth
parse_number_build_teeth
parse_char_build_teeth
operator_classifier_build_teeth
cmp_op_build_teeth
fixed_keyword_build_teeth

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
  "$GATE_DIR/bc-stack-potential-lift.alpha" \
  "$GATE_DIR/bc-post-stack-fixed.alpha" \
  "$GATE_DIR/bc-slurp-summary.alpha" \
  "$GATE_DIR/bc-main-slurp-bridge.alpha" \
  "$GATE_DIR/bc-write-str-event-helper.alpha" \
  "$GATE_DIR/bc-write-str-summary.alpha" \
  "$GATE_DIR/bc-fixed-emitter-summary.alpha" \
  "$GATE_DIR/bc-cursor-leaf-summary.alpha" \
  "$GATE_DIR/bc-skip-ws-summary.alpha" \
  "$GATE_DIR/bc-main-ready-summary.alpha" \
  "$GATE_DIR/bc-summary-combinators.alpha" \
  "$GATE_DIR/bc-exact-shape-helpers.alpha" \
  "$GATE_DIR/bc-main-loop-entry-summary.alpha" \
  "$GATE_DIR/bc-classifier-shape.alpha" \
  "$GATE_DIR/bc-classifier-summary.alpha" \
  "$GATE_DIR/bc-read-ident-shape.alpha" \
  "$GATE_DIR/bc-read-ident-summary.alpha" \
  "$GATE_DIR/bc-expect-shape.alpha" \
  "$GATE_DIR/bc-expect-summary.alpha" \
  "$GATE_DIR/bc-declare-shape.alpha" \
  "$GATE_DIR/bc-declare-summary.alpha" \
  "$GATE_DIR/bc-let-keyword-shape.alpha" \
  "$GATE_DIR/bc-let-keyword-summary.alpha" \
  "$GATE_DIR/bc-literal-skip-shape.alpha" \
  "$GATE_DIR/bc-literal-skip-summary.alpha" \
  "$GATE_DIR/bc-count-lets-control-shape.alpha" \
  "$GATE_DIR/bc-count-lets-data-shape.alpha" \
  "$GATE_DIR/bc-count-lets-cases.alpha" \
  "$GATE_DIR/bc-count-lets-summary.alpha" \
  "$GATE_DIR/bc-parse-params-control-shape.alpha" \
  "$GATE_DIR/bc-parse-params-data-shape.alpha" \
  "$GATE_DIR/bc-parse-parameter-summary.alpha" \
  "$GATE_DIR/bc-parse-capacity-summary.alpha" \
  "$GATE_DIR/bc-emit-ident-shape.alpha" \
  "$GATE_DIR/bc-emit-ident-summary.alpha" \
  "$GATE_DIR/bc-emit-dec-shape.alpha" \
  "$GATE_DIR/bc-emit-dec-summary.alpha" \
  "$GATE_DIR/bc-fixed-decimal-emitters-shape.alpha" \
  "$GATE_DIR/bc-fixed-decimal-emitters-summary.alpha" \
  "$GATE_DIR/bc-parse-output-prefix-shape.alpha" \
  "$GATE_DIR/bc-parse-output-prefix-summary.alpha" \
  "$GATE_DIR/bc-gen-stmts-boundary-shape.alpha" \
  "$GATE_DIR/bc-gen-stmts-boundary-summary.alpha" \
  "$GATE_DIR/bc-parse-number-shape.alpha" \
  "$GATE_DIR/bc-parse-number-summary.alpha" \
  "$GATE_DIR/bc-parse-char-shape.alpha" \
  "$GATE_DIR/bc-parse-char-cases.alpha" \
  "$GATE_DIR/bc-parse-char-summary.alpha" \
  "$GATE_DIR/bc-operator-classifier-shape.alpha" \
  "$GATE_DIR/bc-operator-classifier-summary.alpha" \
  "$GATE_DIR/bc-cmp-op-shape.alpha" \
  "$GATE_DIR/bc-cmp-op-cases.alpha" \
  "$GATE_DIR/bc-cmp-op-summary.alpha" \
  "$GATE_DIR/bc-fixed-keyword-shape-core.alpha" \
  "$GATE_DIR/bc-fixed-keyword-data-shape.alpha" \
  "$GATE_DIR/bc-fixed-keyword-cases.alpha" \
  "$GATE_DIR/bc-fixed-keyword-summary.alpha" > "$T/flat-check.alpha"
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

set +e
"$T/load-missing-class" < "$T/control.bundle" > "$T/stdout"
load_missing_class_status=$?
set -e
if [ "$load_missing_class_status" != 1 ] || [ -s "$T/stdout" ]; then
  echo "bc block control FAIL — missing fixed raw-load class was not rejected" >&2
  exit 1
fi
for slurp_tooth in slurp-wrong-payload slurp-zero-rank slurp-wrong-rename slurp-wrong-len; do
  set +e
  "$T/$slurp_tooth" < "$T/control.bundle" > "$T/stdout"
  slurp_tooth_status=$?
  set -e
  if [ "$slurp_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $slurp_tooth was not rejected" >&2
    exit 1
  fi
done
for main_slurp_tooth in main-slurp-wrong-local main-slurp-wrong-branch main-slurp-wrong-status main-slurp-wrong-clause; do
  set +e
  "$T/$main_slurp_tooth" < "$T/control.bundle" > "$T/stdout"
  main_slurp_tooth_status=$?
  set -e
  if [ "$main_slurp_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $main_slurp_tooth was not rejected" >&2
    exit 1
  fi
done
for write_str_tooth in write-str-wrong-byte write-str-zero-rank write-str-wrong-rename write-str-wrong-total write-str-wrong-cost; do
  set +e
  "$T/$write_str_tooth" < "$T/control.bundle" > "$T/stdout"
  write_str_tooth_status=$?
  set -e
  if [ "$write_str_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $write_str_tooth was not rejected" >&2
    exit 1
  fi
done
for fixed_emit_tooth in fixed-emit-wrong-row fixed-emit-wrong-continuation fixed-emit-wrong-total fixed-emit-wrong-end; do
  set +e
  "$T/$fixed_emit_tooth" < "$T/control.bundle" > "$T/stdout"
  fixed_emit_tooth_status=$?
  set -e
  if [ "$fixed_emit_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $fixed_emit_tooth was not rejected" >&2
    exit 1
  fi
done
for cursor_leaf_tooth in cursor-cbyte-wrong-index cursor-cbyte-wrong-boundary cursor-adv-zero-delta cursor-space-drop-cr cursor-space-zero-is-space cursor-effect-undercount; do
  set +e
  "$T/$cursor_leaf_tooth" < "$T/control.bundle" > "$T/stdout"
  cursor_leaf_tooth_status=$?
  set -e
  if [ "$cursor_leaf_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $cursor_leaf_tooth was not rejected" >&2
    exit 1
  fi
done
for skip_ws_tooth in skip-ws-wrong-continuation skip-ws-wrong-argument skip-ws-wrong-cursor skip-ws-wrong-newline skip-ws-wrong-zero skip-ws-zero-inner-rank skip-ws-no-step-progress skip-ws-zero-backedge skip-ws-zero-outer-rank skip-ws-event-undercount skip-ws-drop-domain skip-ws-wrong-opening skip-ws-zero-inner-premise skip-ws-wrong-inner-rename skip-ws-zero-outer-premise skip-ws-wrong-outer-rename; do
  set +e
  "$T/$skip_ws_tooth" < "$T/control.bundle" > "$T/stdout"
  skip_ws_tooth_status=$?
  set -e
  if [ "$skip_ws_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $skip_ws_tooth was not rejected" >&2
    exit 1
  fi
done
for main_ready_tooth in main-ready-wrong-clause main-ready-wrong-continuation main-ready-wrong-summary main-ready-wrong-length main-ready-wrong-order main-ready-wrong-target main-ready-event-undercount; do
  set +e
  "$T/$main_ready_tooth" < "$T/control.bundle" > "$T/stdout"
  main_ready_tooth_status=$?
  set -e
  if [ "$main_ready_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $main_ready_tooth was not rejected" >&2
    exit 1
  fi
done
for main_loop_tooth in main-loop-wrong-continuation main-loop-wrong-comparison main-loop-wrong-zero-target main-loop-wrong-end-clause main-loop-wrong-nonzero-clause main-loop-wrong-zero-result main-loop-wrong-halt main-loop-wrong-body main-loop-primitive-undercount main-loop-event-undercount main-loop-wrong-generic main-loop-wrong-source-bridge; do
  set +e
  "$T/$main_loop_tooth" < "$T/control.bundle" > "$T/stdout"
  main_loop_tooth_status=$?
  set -e
  if [ "$main_loop_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $main_loop_tooth was not rejected" >&2
    exit 1
  fi
done
for classifier_tooth in classifier-drop-domain classifier-digit-spec-bound classifier-alpha-spec-bound classifier-wrong-handoff classifier-wrong-upper-op classifier-wrong-continuation classifier-wrong-argument classifier-primitive-undercount classifier-event-undercount classifier-wrong-underscore; do
  set +e
  "$T/$classifier_tooth" < "$T/control.bundle" > "$T/stdout"
  classifier_tooth_status=$?
  set -e
  if [ "$classifier_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $classifier_tooth was not rejected" >&2
    exit 1
  fi
done
for read_ident_tooth in read-ident-wrong-cbyte-continuation read-ident-wrong-alnum-continuation read-ident-wrong-argument read-ident-wrong-idoff read-ident-wrong-idlen read-ident-wrong-subtraction read-ident-memory-undercount read-ident-event-undercount read-ident-primitive-undercount read-ident-zero-rank read-ident-wrong-rename read-ident-wrong-stop; do
  set +e
  "$T/$read_ident_tooth" < "$T/control.bundle" > "$T/stdout"
  read_ident_tooth_status=$?
  set -e
  if [ "$read_ident_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $read_ident_tooth was not rejected" >&2
    exit 1
  fi
done
for expect_tooth in expect-wrong-skip-continuation expect-wrong-cbyte-continuation expect-wrong-adv-continuation expect-wrong-slot expect-wrong-comparison expect-event-undercount expect-primitive-undercount expect-drop-delimiter-premise expect-wrong-mismatch-cursor expect-wrong-match-cursor expect-drop-match-range; do
  set +e
  "$T/$expect_tooth" < "$T/control.bundle" > "$T/stdout"
  expect_tooth_status=$?
  set -e
  if [ "$expect_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $expect_tooth was not rejected" >&2
    exit 1
  fi
done
for declare_tooth in declare-wrong-guard declare-wrong-snapshot declare-wrong-capacity declare-wrong-status declare-wrong-idoff declare-wrong-idlen declare-memory-undercount declare-primitive-undercount declare-wrong-full-return declare-drop-table-bound declare-wrong-nloc-update declare-wrong-room-return; do
  set +e
  "$T/$declare_tooth" < "$T/control.bundle" > "$T/stdout"
  declare_tooth_status=$?
  set -e
  if [ "$declare_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $declare_tooth was not rejected" >&2
    exit 1
  fi
done
for let_keyword_tooth in let-keyword-wrong-length-guard let-keyword-wrong-continuation let-keyword-wrong-idoff let-keyword-wrong-idlen let-keyword-wrong-index let-keyword-wrong-l let-keyword-wrong-argument let-keyword-event-undercount let-keyword-primitive-undercount let-keyword-drop-k-bound let-keyword-wrong-length-clause let-keyword-wrong-result; do
  set +e
  "$T/$let_keyword_tooth" < "$T/control.bundle" > "$T/stdout"
  let_keyword_tooth_status=$?
  set -e
  if [ "$let_keyword_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $let_keyword_tooth was not rejected" >&2
    exit 1
  fi
done
for literal_skip_tooth in literal-skip-wrong-char-guard literal-skip-wrong-char-continuation literal-skip-wrong-escape-continuation literal-skip-wrong-closing-quote literal-skip-event-undercount literal-skip-primitive-undercount literal-skip-drop-advx-bound literal-skip-wrong-advx-successor literal-skip-wrong-char-delta literal-skip-wrong-char-bound literal-skip-wrong-zero-cursor literal-skip-zero-ordinary-rank literal-skip-wrong-escape-rank literal-skip-wrong-backedge-rename; do
  set +e
  "$T/$literal_skip_tooth" < "$T/control.bundle" > "$T/stdout"
  literal_skip_tooth_status=$?
  set -e
  if [ "$literal_skip_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $literal_skip_tooth was not rejected" >&2
    exit 1
  fi
done
count_lets_reject_teeth
parse_parameter_reject_teeth
parse_capacity_reject_teeth
emit_ident_reject_teeth
emit_dec_reject_teeth
fixed_decimal_emitters_reject_teeth
parse_output_prefix_reject_teeth
gen_stmts_boundary_reject_teeth
parse_number_reject_teeth
parse_char_reject_teeth
operator_classifier_reject_teeth
cmp_op_reject_teeth
fixed_keyword_reject_teeth
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

# Show that the negative control has teeth beyond the early structural owner:
# its changed target is another real instruction boundary, so the generic Alpha
# CFG checker still accepts it.
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

bc_timing_finish

echo "bc block control/effects: 70 proc / 355 block / 291 transition; 613 effect sites / 829 fixed emit bytes; 113 __write_str calls instantiated from one length-ranked exact-output summary; main.ready composes emit_prelude/write_str/skip_ws into the exact 187-byte prefix, then a reusable main.loop split sends normalized zero to halt(0) and nonzero to main.body without consuming it; byte classifiers digit/alpha/alnum are exact over all 256 cbyte values, terminating read_ident returns their maximal prefix, parse_number returns the exact maximal digit fold modulo 2^64 after same-cursor observations and one-byte ranked steps, parse_char exhausts ordinary/escape byte mappings with exact bounded malformed-tail cursor outcomes and no closing-quote premise, is_muldiv/is_addsub are total quiet Word predicates for */% and +-, cmp_op returns exact operator codes/deltas including restored single = and unchecked ADVX-bounded ! tail, and nine fixed keyword recognizers return one exactly on their bounded identifier spellings with length-first/first-mismatch short circuit; id_char/is_let recognize the exact let slice, literal skippers terminate honestly through bounded malformed tails, count_lets terminates with exact nested-body let count and restored entry CUR, the bounded parse_proc parameter loop records at most four exact slices or returns numeric 252 before output, pdone composes nparams+count_lets without wrap and reaches slotsready at <=1024 or returns numeric 252 before output, fixed-decimal emitter summaries append exact bounded prologue and parameter-store text, and the PCAP bridge composes saved name/nslots/nparams through the exact at-most-four store loop to genbody; procedure62's root-independent boundary closes depth64/resource/close/zero returns and the unexecuted gen_stmt cutpoint; nonzero expect normalizes then conditionally consumes one delimiter, and declare either appends the identifier slot or records numeric status 252 at capacity; cbyte/adv/is_space leaf summaries compose through terminating skip_ws_step/skip_ws loops; 78 frame slots / 27 parameter stores / 134 call pops; 169 local loads / 73 local stores; 61 raw loads = 54 fixed-safe + 5 SRC-indexed + 2 table-indexed / 34 raw stores; cursor-zero slurp segment/value/termination summary composed from root through main.ready or halt(253); 581 literals / 55 arithmetic / 180 comparison primitives; 235 binary / 134 argument / 34 store-address pushes; syntax-directed composition / relative temporary peak 2; three ranged Alpha operands transferred; all 607 stores partitioned / 70 call-cut frames summarized; 64-row counter contexts; absolute B_bc1 stack <=12720 explicit bytes / <=662 hidden returns; all 2630 explicit-stack effects and 687 artifact effects owned ($(wc -c < "$T/control-check.tape" | tr -d ' ')-byte Alpha checker tape)"
echo "bc independent conditional tranches: name_eq, lookup, eight bounded emitters, full-Word emit_dec, label/string/comparison emitters, and the seven-procedure expression family passed canonical and phase-isolated mutation gates"
