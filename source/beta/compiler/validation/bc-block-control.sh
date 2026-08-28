#!/usr/bin/env sh
# Bounded source-to-artifact control correspondence for the whole bc compiler.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$OMEGA_PATH_PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
done
unset OMEGA_PATH_PARENT
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh"

ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
SOURCE="$OMEGA_PATH_BETA_COMPILER/bc.beta"
ARTIFACT="$OMEGA_PATH_BETA_COMPILER/artifacts/bc.tape"
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
. "$GATE_DIR/bc-mutation-cache.sh"
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
. "$GATE_DIR/bc-raw-load-family-teeth.sh"
. "$GATE_DIR/bc-slurp-summary-teeth.sh"
. "$GATE_DIR/bc-main-slurp-teeth.sh"
. "$GATE_DIR/bc-write-str-teeth.sh"
. "$GATE_DIR/bc-fixed-emitter-teeth.sh"
. "$GATE_DIR/bc-cursor-leaf-teeth.sh"
. "$GATE_DIR/bc-skip-ws-teeth.sh"
. "$GATE_DIR/bc-main-ready-teeth.sh"
. "$GATE_DIR/bc-main-loop-teeth.sh"
. "$GATE_DIR/bc-byte-classifier-teeth.sh"
. "$GATE_DIR/bc-read-ident-teeth.sh"
. "$GATE_DIR/bc-expect-teeth.sh"
. "$GATE_DIR/bc-declare-teeth.sh"
. "$GATE_DIR/bc-let-keyword-teeth.sh"
. "$GATE_DIR/bc-literal-skip-teeth.sh"
. "$GATE_DIR/bc-stack-owner-teeth.sh"
. "$GATE_DIR/bc-ranged-static-teeth.sh"
. "$GATE_DIR/bc-ranged-transfer-teeth.sh"
. "$GATE_DIR/bc-frame-summary-teeth.sh"
. "$GATE_DIR/bc-counter-potential-teeth.sh"
. "$GATE_DIR/bc-flat-composition-teeth.sh"
. "$GATE_DIR/bc-coherent-ranged-teeth.sh"
. "$GATE_DIR/bc-call-bounds-teeth.sh"
. "$GATE_DIR/bc-artifact-control-flow-teeth.sh"
. "$GATE_DIR/bc-artifact-effect-emitter-teeth.sh"
. "$GATE_DIR/bc-artifact-frame-call-teeth.sh"
. "$GATE_DIR/bc-artifact-local-access-teeth.sh"
. "$GATE_DIR/bc-artifact-raw-memory-teeth.sh"
. "$GATE_DIR/bc-artifact-primitive-composition-teeth.sh"
. "$GATE_DIR/bc-artifact-comparison-teeth.sh"
. "$GATE_DIR/bc-artifact-stack-push-teeth.sh"
. "$GATE_DIR/bc-artifact-structural-survival-teeth.sh"
. "$GATE_DIR/bc-checker-a-shards.sh"

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
  bc_timing_start emit-dec-word-canonical
  build_emit_dec_word_checker
  smoke_emit_dec_word_checker
  bc_timing_finish
  bc_run_cached_teeth emit-dec-word '36 cases' \
    emit_dec_word_build_teeth emit_dec_word_reject_teeth \
    "$T/emit-dec-word-check.alpha" "$T/control.bundle" \
    "$GATE_DIR/bc-emit-dec-word-teeth.sh" \
    "$GATE_DIR/bc-mutation-cache.sh" "$ARTIFACT" "$ASM" "$SEED" \
    "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" "$OMEGA_PATH_ALPHA/seed_env.sh"

  bc_timing_start label-emitters-canonical
  build_label_emitters_checker
  smoke_label_emitters_checker
  bc_timing_finish
  bc_run_cached_teeth label-emitters '37 cases' \
    label_emitters_build_teeth label_emitters_reject_teeth \
    "$T/label-emitters-check.alpha" "$T/control.bundle" \
    "$GATE_DIR/bc-label-emitters-teeth.sh" \
    "$GATE_DIR/bc-mutation-cache.sh" "$ARTIFACT" "$ASM" "$SEED" \
    "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" "$OMEGA_PATH_ALPHA/seed_env.sh"

  bc_timing_start expression-family-canonical
  build_expression_family_checkers
  smoke_expression_family_checkers
  bc_timing_finish
  bc_run_cached_teeth expression-family '16 cases' \
    expression_family_build_teeth expression_family_reject_teeth \
    "$T/expression-family-shape.alpha" \
    "$T/expression-family-semantic.alpha" "$T/control.bundle" \
    "$GATE_DIR/bc-expression-family-teeth.sh" \
    "$GATE_DIR/bc-mutation-cache.sh" "$ARTIFACT" "$ASM" "$SEED" \
    "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" "$OMEGA_PATH_ALPHA/seed_env.sh"

  bc_timing_start statement-family-shape-canonical
  build_statement_family_shape_checker
  smoke_statement_family_shape_checker
  bc_timing_finish
  bc_run_cached_teeth statement-family-shape '12 cases' \
    statement_family_build_teeth statement_family_reject_teeth \
    "$T/statement-family-shape.alpha" "$T/control.bundle" \
    "$GATE_DIR/bc-statement-family-teeth.sh" \
    "$GATE_DIR/bc-mutation-cache.sh" "$ARTIFACT" "$ASM" "$SEED" \
    "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" "$OMEGA_PATH_ALPHA/seed_env.sh"
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
  checker-a-shards)
    checker_a_prepare_shards
    checker_a_build_shards
    checker_a_reject_shards
    echo "bc Checker-A historical shards: exact two-phase mutation inventory passed"
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

bc_timing_start statement-family-semantic-canonical
build_statement_family_semantic_checker
smoke_statement_family_semantic_checker
bc_timing_finish
bc_run_cached_teeth statement-family-semantic '22 cases' \
  statement_family_semantic_build_teeth \
  statement_family_semantic_reject_teeth \
  "$T/statement-family-semantic.alpha" "$T/control.bundle" \
  "$GATE_DIR/bc-statement-family-semantic-teeth.sh" \
  "$GATE_DIR/bc-mutation-cache.sh" "$ARTIFACT" "$ASM" "$SEED" \
  "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" "$OMEGA_PATH_ALPHA/seed_env.sh"

bc_timing_start parse-proc-body-canonical
establish_parse_body_canonical
bc_timing_finish
bc_run_cached_teeth parse-proc-body '25 cases' \
  parse_body_build_teeth parse_body_reject_teeth \
  "$T/parse-body.alpha" "$T/control.bundle" \
  "$GATE_DIR/bc-parse-body-teeth.sh" \
  "$GATE_DIR/bc-mutation-cache.sh" "$ARTIFACT" "$ASM" "$SEED" \
  "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" "$OMEGA_PATH_ALPHA/seed_env.sh"

bc_timing_start resource-classification-canonical
establish_resource_classification_canonical
bc_timing_finish
bc_run_cached_teeth resource-classification '36 cases' \
  resource_classification_build_teeth resource_classification_reject_teeth \
  "$T/resource-classification.alpha" "$T/control.bundle" \
  "$GATE_DIR/bc-resource-classification-teeth.sh" \
  "$GATE_DIR/bc-mutation-cache.sh" "$ARTIFACT" "$ASM" "$SEED" \
  "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" "$OMEGA_PATH_ALPHA/seed_env.sh"

bc_timing_start declaration-budget-canonical
establish_declaration_budget_canonical
bc_timing_finish
bc_run_cached_teeth declaration-budget '14 cases' \
  declaration_budget_build_teeth declaration_budget_reject_teeth \
  "$T/declaration-budget.alpha" "$T/control.bundle" \
  "$GATE_DIR/bc-declaration-budget-teeth.sh" \
  "$GATE_DIR/bc-mutation-cache.sh" "$ARTIFACT" "$ASM" "$SEED" \
  "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" "$OMEGA_PATH_ALPHA/seed_env.sh"

bc_timing_start complete-parse-proc-canonical
establish_parse_proc_canonical
bc_timing_finish
bc_run_cached_teeth complete-parse-proc '16 cases' \
  parse_proc_build_teeth parse_proc_reject_teeth \
  "$T/parse-proc.alpha" "$T/control.bundle" \
  "$GATE_DIR/bc-parse-proc-teeth.sh" \
  "$GATE_DIR/bc-mutation-cache.sh" "$ARTIFACT" "$ASM" "$SEED" \
  "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" "$OMEGA_PATH_ALPHA/seed_env.sh"

bc_timing_start root-observation-canonical
establish_root_observation_canonical
bc_timing_finish
bc_run_cached_teeth root-observation '49 cases' \
  root_observation_build_teeth root_observation_reject_teeth \
  "$T/root-observation.alpha" "$T/control.bundle" \
  "$GATE_DIR/bc-root-observation-teeth.sh" \
  "$GATE_DIR/bc-mutation-cache.sh" "$ARTIFACT" "$ASM" "$SEED" \
  "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" "$OMEGA_PATH_ALPHA/seed_env.sh"

# These families share the freshly reconstructed canonical bundle but own
# disjoint phase-isolated mutations. Their green receipts bind the exact
# canonical checker, bundle, harness, and case inventory. A change to one teeth
# module therefore reruns that family without recompiling its siblings.
bc_run_cached_teeth bounded-emitters '52 cases' \
  bounded_emitters_build_teeth bounded_emitters_reject_teeth \
  "$T/bounded-emitters-check.alpha" "$T/control.bundle" \
  "$GATE_DIR/bc-bounded-emitters-teeth.sh" "$GATE_DIR/bc-mutation-cache.sh" \
  "$ARTIFACT" "$ASM" "$SEED" "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" \
  "$OMEGA_PATH_ALPHA/seed_env.sh"
bc_run_cached_teeth checker-split-fixed '1 case' \
  checker_split_build_fixed_tooth checker_split_reject_fixed_tooth \
  "$T/control-check.alpha" "$T/control.bundle" \
  "$GATE_DIR/bc-checker-split-teeth.sh" "$GATE_DIR/bc-mutation-cache.sh" \
  "$ARTIFACT" "$ASM" "$SEED" "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" \
  "$OMEGA_PATH_ALPHA/seed_env.sh"
bc_run_cached_teeth checker-split-name-eq '1 case' \
  checker_split_build_name_tooth checker_split_reject_name_tooth \
  "$T/name-eq-check.alpha" "$T/control.bundle" \
  "$GATE_DIR/bc-checker-split-teeth.sh" "$GATE_DIR/bc-mutation-cache.sh" \
  "$ARTIFACT" "$ASM" "$SEED" "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" \
  "$OMEGA_PATH_ALPHA/seed_env.sh"
bc_run_cached_teeth name-eq '31 cases' \
  name_eq_build_teeth name_eq_reject_teeth \
  "$T/name-eq-check.alpha" "$T/control.bundle" \
  "$GATE_DIR/bc-name-eq-teeth.sh" "$GATE_DIR/bc-mutation-cache.sh" \
  "$ARTIFACT" "$ASM" "$SEED" "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" \
  "$OMEGA_PATH_ALPHA/seed_env.sh"
bc_run_cached_teeth lookup '35 cases' \
  lookup_build_teeth lookup_reject_teeth \
  "$T/lookup-check.alpha" "$T/control.bundle" \
  "$GATE_DIR/bc-lookup-teeth.sh" "$GATE_DIR/bc-mutation-cache.sh" \
  "$ARTIFACT" "$ASM" "$SEED" "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh" \
  "$OMEGA_PATH_ALPHA/seed_env.sh"

checker_a_prepare_shards
checker_a_build_shards
checker_a_reject_shards

echo "bc block control/effects: 71 procedures / 359 blocks / 293 transitions; complete B_bc1 canonical owners, mutation gates, and checker shards passed ($(wc -c < "$T/control-check.tape" | tr -d " ")-byte Alpha checker tape)"
