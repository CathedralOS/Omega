#!/usr/bin/env sh
# Canonical whole-source/artifact maximal-observation check for bc.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OBLIGATION_DIR="$GATE_DIR/obligations"
WITNESS_DIR="$GATE_DIR/witnesses"
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" ]; do
  OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$OMEGA_PATH_PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
done
unset OMEGA_PATH_PARENT
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/lattice/paths.sh"
. "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh"
. "$OMEGA_PATH_ALPHA_CHECKER/artifact_env.sh"

ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
SOURCE="$OMEGA_PATH_BETA_COMPILER/bc.beta"
ARTIFACT="$OMEGA_PATH_BETA_COMPILER/artifacts/bc.tape"
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
bc_timing_start() {
  BC_TIMING_PHASE=$1
  BC_TIMING_STARTED=$(date +%s)
  echo "bc timing: $BC_TIMING_PHASE started"
}

bc_timing_finish() {
  BC_TIMING_FINISHED=$(date +%s)
  echo "bc timing: $BC_TIMING_PHASE $((BC_TIMING_FINISHED - BC_TIMING_STARTED))s"
}

u32_file() {
  python3 -c 'import struct,sys; sys.stdout.buffer.write(struct.pack("<I", int(sys.argv[1])))' "$1" > "$2"
}

case_run() {
  set +e
  "$T/control-check" < "$3" > "$T/stdout"
  got=$?
  set -e
  [ "$got" = "$2" ] && [ ! -s "$T/stdout" ] || {
    echo "bc admission FAIL — $1: expected $2/empty, got $got/$(wc -c < "$T/stdout" | tr -d ' ') bytes" >&2
    exit 1
  }
}

bc_timing_start setup-and-witnesses

# The persisted compiler supplies only a location hint.  Require its Alpha text
# to assemble to the exact committed artifact before deriving that hint.
stamp_beta_compiler "$T/bc" >/dev/null
"$T/bc" < "$SOURCE" > "$T/fixed.alpha"
"$ASM" < "$T/fixed.alpha" > "$T/fixed.tape"
cmp "$ARTIFACT" "$T/fixed.tape"

python3 "$WITNESS_DIR/bc_block_control_map.py" \
  --repo "$OMEGA_REPO_ROOT" \
  --source "$SOURCE" \
  --assembly "$T/fixed.alpha" \
  --tape "$ARTIFACT" \
  --output "$T/control.witness"

# The untrusted mapper never writes the source/tape portion of checker input.
# Assemble every bundle here from the exact repository source and artifact.
SOURCE_LEN=$(wc -c < "$SOURCE" | tr -d ' ')
TAPE_LEN=$(wc -c < "$ARTIFACT" | tr -d ' ')
u32_file "$SOURCE_LEN" "$T/source.len"
u32_file "$TAPE_LEN" "$T/tape.len"
python3 "$WITNESS_DIR/bc_call_bounds.py" \
  --repo "$OMEGA_REPO_ROOT" \
  --source "$SOURCE" \
  --output "$T/call-bounds.witness"
make_bundle() { # tape witness output
  cat "$T/source.len" "$SOURCE" "$T/tape.len" "$1" "$2" \
    "$T/call-bounds.witness" > "$3"
}
make_bundle "$ARTIFACT" "$T/control.witness" "$T/control.bundle"
CONTROL_BUNDLE_CKSUM=$(cksum < "$T/control.bundle")
require_control_bundle_unchanged() {
  control_bundle_now=$(cksum < "$T/control.bundle")
  if [ "$control_bundle_now" != "$CONTROL_BUNDLE_CKSUM" ]; then
    echo "bc block control FAIL — canonical control.bundle changed between owners" >&2
    exit 1
  fi
  return 0
}

# The root observable excludes invalid-opcode execution only after the exact
# persisted artifact has passed the independent reachable-structure checker.
# Establish the structural owner before building the canonical conjunction.
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
bc_timing_finish

emit_stack_checker_prefix() {
  cat "$OBLIGATION_DIR/bc-block-control.alpha" \
    "$OBLIGATION_DIR/bc-effect-sites.alpha" \
    "$OBLIGATION_DIR/bc-exact-shape-helpers.alpha" \
    "$OBLIGATION_DIR/bc-procedure-inventory.alpha" \
    "$OBLIGATION_DIR/bc-event-identity.alpha" \
    "$OBLIGATION_DIR/bc-frame-shape.alpha" \
    "$OBLIGATION_DIR/bc-local-access.alpha" \
    "$OBLIGATION_DIR/bc-memory-sites.alpha" \
    "$OBLIGATION_DIR/bc-expr-primitives.alpha" \
    "$OBLIGATION_DIR/bc-stack-pushes.alpha" \
    "$OBLIGATION_DIR/bc-expression-census-prefix.alpha" \
    "$OBLIGATION_DIR/bc-effect-census-prefix.alpha" \
    "$OBLIGATION_DIR/bc-expr-composition.alpha" \
    "$OBLIGATION_DIR/bc-raw-load-families.alpha" \
    "$OBLIGATION_DIR/bc-call-bounds.alpha" \
    "$OBLIGATION_DIR/bc-stack-register-custody.alpha" \
    "$OBLIGATION_DIR/bc-ranged-store-bounds.alpha" \
    "$OBLIGATION_DIR/bc-frame-summary.alpha" \
    "$OBLIGATION_DIR/bc-ranged-store-transfer.alpha" \
    "$OBLIGATION_DIR/bc-counter-transfer.alpha" \
    "$OBLIGATION_DIR/bc-stack-potential-lift.alpha"
}

emit_name_eq_checker_prefix() {
  emit_stack_checker_prefix
  cat \
    "$OBLIGATION_DIR/bc-post-stack-name-eq.alpha" \
    "$OBLIGATION_DIR/bc-name-table-domain.alpha" \
    "$OBLIGATION_DIR/bc-name-eq-control-shape.alpha" \
    "$OBLIGATION_DIR/bc-name-eq-data-shape.alpha" \
    "$OBLIGATION_DIR/bc-name-eq-summary.alpha"
}

build_lookup_checker() {
  {
    emit_name_eq_checker_prefix
    cat "$OBLIGATION_DIR/bc-post-name-eq-lookup.alpha" \
      "$OBLIGATION_DIR/bc-lookup-control-shape.alpha" \
      "$OBLIGATION_DIR/bc-lookup-data-shape.alpha" \
      "$OBLIGATION_DIR/bc-lookup-summary.alpha"
  } > "$T/lookup-check.alpha"
  "$ASM" < "$T/lookup-check.alpha" > "$T/lookup-check.tape"
  stamp_seed "$T/lookup-check.tape" "$SEED" "$T/lookup-check" >/dev/null
}

build_bounded_emitters_checker() {
  {
    emit_stack_checker_prefix
    cat "$OBLIGATION_DIR/bc-post-stack-bounded-emitters.alpha" \
      "$OBLIGATION_DIR/bc-write-str-event-helper.alpha" \
      "$OBLIGATION_DIR/bc-write-str-summary.alpha" \
      "$OBLIGATION_DIR/bc-post-write-str-bounded-emitters.alpha" \
      "$OBLIGATION_DIR/bc-cursor-leaf-summary.alpha" \
      "$OBLIGATION_DIR/bc-skip-ws-summary.alpha" \
      "$OBLIGATION_DIR/bc-post-skip-ws-bounded-emitters.alpha" \
      "$OBLIGATION_DIR/bc-expect-shape.alpha" \
      "$OBLIGATION_DIR/bc-expect-summary.alpha" \
      "$OBLIGATION_DIR/bc-post-expect-bounded-emitters.alpha" \
      "$OBLIGATION_DIR/bc-emit-dec-shape.alpha" \
      "$OBLIGATION_DIR/bc-emit-dec-summary.alpha" \
      "$OBLIGATION_DIR/bc-post-emit-dec-bounded-emitters.alpha" \
      "$OBLIGATION_DIR/bc-bounded-emitters-control-shape.alpha" \
      "$OBLIGATION_DIR/bc-bounded-emitters-data-shape.alpha" \
      "$OBLIGATION_DIR/bc-bounded-emitters-summary.alpha" \
      "$OBLIGATION_DIR/bc-bounded-emitters-slot-summary.alpha" \
      "$OBLIGATION_DIR/bc-bounded-emitters-publication.alpha"
  } > "$T/bounded-emitters-check.alpha"
  "$ASM" < "$T/bounded-emitters-check.alpha" > "$T/bounded-emitters-check.tape"
  stamp_seed "$T/bounded-emitters-check.tape" "$SEED" \
    "$T/bounded-emitters-check" >/dev/null
}

build_emit_dec_word_checker() {
  {
    emit_stack_checker_prefix
    cat "$OBLIGATION_DIR/bc-post-stack-emit-dec-word.alpha" \
      "$OBLIGATION_DIR/bc-emit-dec-shape.alpha" \
      "$OBLIGATION_DIR/bc-emit-dec-word-domain.alpha" \
      "$OBLIGATION_DIR/bc-emit-dec-word-summary.alpha" \
      "$OBLIGATION_DIR/bc-emit-dec-word-publication.alpha"
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
    label_emitters_module_bytes=$(wc -c < "$OBLIGATION_DIR/$label_emitters_module" | tr -d ' ')
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
    cat "$OBLIGATION_DIR/bc-post-stack-label-emitters.alpha" \
      "$OBLIGATION_DIR/bc-write-str-event-helper.alpha" \
      "$OBLIGATION_DIR/bc-write-str-summary.alpha" \
      "$OBLIGATION_DIR/bc-post-write-str-label-emitters.alpha" \
      "$OBLIGATION_DIR/bc-cursor-leaf-summary.alpha" \
      "$OBLIGATION_DIR/bc-skip-ws-summary.alpha" \
      "$OBLIGATION_DIR/bc-post-skip-ws-label-emitters.alpha" \
      "$OBLIGATION_DIR/bc-expect-shape.alpha" \
      "$OBLIGATION_DIR/bc-expect-summary.alpha" \
      "$OBLIGATION_DIR/bc-post-expect-label-emitters.alpha" \
      "$OBLIGATION_DIR/bc-emit-dec-shape.alpha" \
      "$OBLIGATION_DIR/bc-emit-dec-word-domain.alpha" \
      "$OBLIGATION_DIR/bc-emit-dec-word-summary.alpha" \
      "$OBLIGATION_DIR/bc-emit-dec-word-label-publication.alpha" \
      "$OBLIGATION_DIR/bc-cursor-tail-summary.alpha" \
      "$OBLIGATION_DIR/bc-label-core-shape.alpha" \
      "$OBLIGATION_DIR/bc-label-counter-summary.alpha" \
      "$OBLIGATION_DIR/bc-label-ref-summary.alpha" \
      "$OBLIGATION_DIR/bc-emit-str-body-shape.alpha" \
      "$OBLIGATION_DIR/bc-emit-str-body-cases.alpha" \
      "$OBLIGATION_DIR/bc-emit-str-body-summary.alpha" \
      "$OBLIGATION_DIR/bc-gen-emit-shape.alpha" \
      "$OBLIGATION_DIR/bc-gen-emit-summary.alpha" \
      "$OBLIGATION_DIR/bc-emit-cmp-control-shape.alpha" \
      "$OBLIGATION_DIR/bc-emit-cmp-data-shape.alpha" \
      "$OBLIGATION_DIR/bc-emit-cmp-cases.alpha" \
      "$OBLIGATION_DIR/bc-emit-cmp-summary.alpha" \
      "$OBLIGATION_DIR/bc-label-emitters-publication.alpha" \
      "$OBLIGATION_DIR/bc-post-label-emitters-base.alpha"
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
  cat "$OBLIGATION_DIR/bc-block-control.alpha" \
    "$OBLIGATION_DIR/bc-effect-sites.alpha" \
    "$OBLIGATION_DIR/bc-exact-shape-helpers.alpha" \
    "$OBLIGATION_DIR/bc-procedure-inventory.alpha" \
    "$OBLIGATION_DIR/bc-event-identity.alpha" \
    "$OBLIGATION_DIR/bc-frame-shape.alpha" \
    "$OBLIGATION_DIR/bc-local-access.alpha" \
    "$OBLIGATION_DIR/bc-memory-sites.alpha" \
    "$OBLIGATION_DIR/bc-expr-primitives.alpha" \
    "$OBLIGATION_DIR/bc-stack-pushes.alpha" \
    "$OBLIGATION_DIR/bc-expression-census-prefix.alpha" \
    "$OBLIGATION_DIR/bc-effect-census-prefix.alpha" \
    "$OBLIGATION_DIR/bc-expr-composition.alpha" \
    "$OBLIGATION_DIR/bc-raw-load-families.alpha" \
    "$OBLIGATION_DIR/bc-call-bounds.alpha"
}

expression_family_require_module_budgets() {
  for expression_family_module in \
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
    expression_family_module_bytes=$(wc -c < "$OBLIGATION_DIR/$expression_family_module" | tr -d ' ')
    if [ "$expression_family_module_bytes" -ge 20000 ]; then
      echo "bc block control FAIL — $expression_family_module is ${expression_family_module_bytes} bytes (20KB module cap)" >&2
      exit 1
    fi
  done
}

build_expression_family_shape_checker() {
  {
    emit_expression_table_prefix
    cat "$OBLIGATION_DIR/bc-expression-shape-root.alpha" \
      "$OBLIGATION_DIR/bc-expression-leaf-shape.alpha" \
      "$OBLIGATION_DIR/bc-expression-call-control-shape.alpha" \
      "$OBLIGATION_DIR/bc-expression-call-data-shape.alpha" \
      "$OBLIGATION_DIR/bc-expression-factor-control-shape.alpha" \
      "$OBLIGATION_DIR/bc-expression-factor-data-shape.alpha" \
      "$OBLIGATION_DIR/bc-expression-levels-shape.alpha" \
      "$OBLIGATION_DIR/bc-expression-gen-expr-shape.alpha" \
      "$OBLIGATION_DIR/bc-expression-family-shape.alpha"
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
    cat "$OBLIGATION_DIR/bc-expression-root.alpha" \
      "$OBLIGATION_DIR/bc-write-str-event-helper.alpha" \
      "$OBLIGATION_DIR/bc-write-str-summary.alpha" \
      "$OBLIGATION_DIR/bc-post-write-str-label-emitters.alpha" \
      "$OBLIGATION_DIR/bc-cursor-leaf-summary.alpha" \
      "$OBLIGATION_DIR/bc-skip-ws-summary.alpha" \
      "$OBLIGATION_DIR/bc-post-skip-ws-label-emitters.alpha" \
      "$OBLIGATION_DIR/bc-expect-shape.alpha" \
      "$OBLIGATION_DIR/bc-expect-summary.alpha" \
      "$OBLIGATION_DIR/bc-post-expect-expression.alpha" \
      "$OBLIGATION_DIR/bc-emit-dec-shape.alpha" \
      "$OBLIGATION_DIR/bc-emit-dec-word-domain.alpha" \
      "$OBLIGATION_DIR/bc-emit-dec-word-summary.alpha" \
      "$OBLIGATION_DIR/bc-cursor-tail-summary.alpha" \
      "$OBLIGATION_DIR/bc-label-core-shape.alpha" \
      "$OBLIGATION_DIR/bc-label-counter-summary.alpha" \
      "$OBLIGATION_DIR/bc-label-ref-summary.alpha" \
      "$OBLIGATION_DIR/bc-emit-str-body-shape.alpha" \
      "$OBLIGATION_DIR/bc-emit-str-body-cases.alpha" \
      "$OBLIGATION_DIR/bc-emit-str-body-summary.alpha" \
      "$OBLIGATION_DIR/bc-gen-emit-shape.alpha" \
      "$OBLIGATION_DIR/bc-gen-emit-summary.alpha" \
      "$OBLIGATION_DIR/bc-emit-cmp-control-shape.alpha" \
      "$OBLIGATION_DIR/bc-emit-cmp-data-shape.alpha" \
      "$OBLIGATION_DIR/bc-emit-cmp-cases.alpha" \
      "$OBLIGATION_DIR/bc-emit-cmp-summary.alpha" \
      "$OBLIGATION_DIR/bc-label-emitters-publication.alpha" \
      "$OBLIGATION_DIR/bc-post-label-emitters-expression.alpha" \
      "$OBLIGATION_DIR/bc-classifier-shape.alpha" \
      "$OBLIGATION_DIR/bc-classifier-summary.alpha" \
      "$OBLIGATION_DIR/bc-read-ident-shape.alpha" \
      "$OBLIGATION_DIR/bc-read-ident-summary.alpha" \
      "$OBLIGATION_DIR/bc-emit-ident-shape.alpha" \
      "$OBLIGATION_DIR/bc-emit-ident-summary.alpha" \
      "$OBLIGATION_DIR/bc-expression-id-char.alpha" \
      "$OBLIGATION_DIR/bc-fixed-keyword-shape-core.alpha" \
      "$OBLIGATION_DIR/bc-fixed-keyword-data-shape.alpha" \
      "$OBLIGATION_DIR/bc-fixed-keyword-cases.alpha" \
      "$OBLIGATION_DIR/bc-fixed-keyword-summary.alpha" \
      "$OBLIGATION_DIR/bc-literal-skip-shape.alpha" \
      "$OBLIGATION_DIR/bc-literal-skip-summary.alpha" \
      "$OBLIGATION_DIR/bc-post-literal-skip-expression.alpha" \
      "$OBLIGATION_DIR/bc-parse-number-shape.alpha" \
      "$OBLIGATION_DIR/bc-parse-number-summary.alpha" \
      "$OBLIGATION_DIR/bc-parse-char-shape.alpha" \
      "$OBLIGATION_DIR/bc-parse-char-cases.alpha" \
      "$OBLIGATION_DIR/bc-parse-char-summary.alpha" \
      "$OBLIGATION_DIR/bc-operator-classifier-shape.alpha" \
      "$OBLIGATION_DIR/bc-operator-classifier-summary.alpha" \
      "$OBLIGATION_DIR/bc-cmp-op-shape.alpha" \
      "$OBLIGATION_DIR/bc-cmp-op-cases.alpha" \
      "$OBLIGATION_DIR/bc-cmp-op-summary.alpha" \
      "$OBLIGATION_DIR/bc-name-table-domain.alpha" \
      "$OBLIGATION_DIR/bc-name-eq-control-shape.alpha" \
      "$OBLIGATION_DIR/bc-name-eq-data-shape.alpha" \
      "$OBLIGATION_DIR/bc-name-eq-summary.alpha" \
      "$OBLIGATION_DIR/bc-post-name-eq-lookup.alpha" \
      "$OBLIGATION_DIR/bc-lookup-control-shape.alpha" \
      "$OBLIGATION_DIR/bc-lookup-data-shape.alpha" \
      "$OBLIGATION_DIR/bc-lookup-summary.alpha" \
      "$OBLIGATION_DIR/bc-emit-dec-summary.alpha" \
      "$OBLIGATION_DIR/bc-post-emit-dec-bounded-emitters.alpha" \
      "$OBLIGATION_DIR/bc-bounded-emitters-control-shape.alpha" \
      "$OBLIGATION_DIR/bc-bounded-emitters-data-shape.alpha" \
      "$OBLIGATION_DIR/bc-bounded-emitters-summary.alpha" \
      "$OBLIGATION_DIR/bc-bounded-emitters-slot-summary.alpha" \
      "$OBLIGATION_DIR/bc-bounded-emitters-publication.alpha" \
      "$OBLIGATION_DIR/bc-expression-prerequisites.alpha" \
      "$OBLIGATION_DIR/bc-expression-resource-domain.alpha" \
      "$OBLIGATION_DIR/bc-expression-tail-rules.alpha" \
      "$OBLIGATION_DIR/bc-expression-leaf-rules.alpha" \
      "$OBLIGATION_DIR/bc-expression-call-rules.alpha" \
      "$OBLIGATION_DIR/bc-expression-factor-rules.alpha" \
      "$OBLIGATION_DIR/bc-expression-levels-rules.alpha" \
      "$OBLIGATION_DIR/bc-expression-gen-expr-rules.alpha" \
      "$OBLIGATION_DIR/bc-expression-family-publication.alpha"
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
    statement_family_module_bytes=$(wc -c < "$OBLIGATION_DIR/$statement_family_module" | tr -d ' ')
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
    cat "$OBLIGATION_DIR/bc-statement-family-shape-root.alpha" \
      "$OBLIGATION_DIR/bc-statement-emit-epilogue-shape.alpha" \
      "$OBLIGATION_DIR/bc-statement-gen-store-shape.alpha" \
      "$OBLIGATION_DIR/bc-gen-stmts-boundary-shape.alpha" \
      "$OBLIGATION_DIR/bc-statement-gen-block-shape.alpha" \
      "$OBLIGATION_DIR/bc-statement-emit-state-label-shape.alpha" \
      "$OBLIGATION_DIR/bc-statement-gen-state-shape.alpha" \
      "$OBLIGATION_DIR/bc-statement-gen-to-shape.alpha" \
      "$OBLIGATION_DIR/bc-statement-gen-stmt-shape.alpha" \
      "$OBLIGATION_DIR/bc-statement-gen-stmt-data-shape.alpha" \
      "$OBLIGATION_DIR/bc-statement-family-shape.alpha"
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
    cat "$OBLIGATION_DIR/bc-statement-semantic-root.alpha" \
      "$OBLIGATION_DIR/bc-statement-antecedents.alpha" \
      "$OBLIGATION_DIR/bc-write-str-event-helper.alpha" \
      "$OBLIGATION_DIR/bc-statement-emit-epilogue-shape.alpha" \
      "$OBLIGATION_DIR/bc-statement-emit-epilogue-rules.alpha" \
      "$OBLIGATION_DIR/bc-statement-gen-store-shape.alpha" \
      "$OBLIGATION_DIR/bc-statement-gen-store-rules.alpha" \
      "$OBLIGATION_DIR/bc-statement-emit-state-label-shape.alpha" \
      "$OBLIGATION_DIR/bc-statement-state-label-rules.alpha" \
      "$OBLIGATION_DIR/bc-statement-gen-to-rules.alpha" \
      "$OBLIGATION_DIR/bc-statement-gen-stmt-rules.alpha" \
      "$OBLIGATION_DIR/bc-statement-gen-stmt-fallback-rules.alpha" \
      "$OBLIGATION_DIR/bc-statement-wrapper-rules.alpha" \
      "$OBLIGATION_DIR/bc-statement-gfp-rules.alpha" \
      "$OBLIGATION_DIR/bc-statement-family-publication.alpha"
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
    parse_body_module_bytes=$(wc -c < "$OBLIGATION_DIR/$parse_body_module" | tr -d ' ')
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
    cat "$OBLIGATION_DIR/bc-parse-body-root.alpha" \
      "$OBLIGATION_DIR/bc-parse-body-antecedents.alpha" \
      "$OBLIGATION_DIR/bc-parse-body-shape.alpha" \
      "$OBLIGATION_DIR/bc-parse-body-rules.alpha"
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
    resource_module_bytes=$(wc -c < "$OBLIGATION_DIR/$resource_module" | tr -d ' ')
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
    cat "$OBLIGATION_DIR/bc-resource-classification-root.alpha" \
      "$OBLIGATION_DIR/bc-resource-classification-shape.alpha" \
      "$OBLIGATION_DIR/bc-resource-classification-antecedents.alpha" \
      "$OBLIGATION_DIR/bc-resource-profile.alpha" \
      "$OBLIGATION_DIR/bc-resource-classification.alpha"
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
      < "$OBLIGATION_DIR/$declaration_budget_module" | tr -d ' ')
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
    cat "$OBLIGATION_DIR/bc-declaration-budget-root.alpha" \
      "$OBLIGATION_DIR/bc-declaration-budget-antecedents.alpha" \
      "$OBLIGATION_DIR/bc-declaration-budget-shape.alpha" \
      "$OBLIGATION_DIR/bc-declaration-budget-rules.alpha" \
      "$OBLIGATION_DIR/bc-declaration-budget-publication.alpha"
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
    parse_proc_module_bytes=$(wc -c < "$OBLIGATION_DIR/$parse_proc_module" | tr -d ' ')
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
    cat "$OBLIGATION_DIR/bc-parse-proc-root.alpha" \
      "$OBLIGATION_DIR/bc-parse-proc-antecedents.alpha" \
      "$OBLIGATION_DIR/bc-parse-proc-entry-shape.alpha" \
      "$OBLIGATION_DIR/bc-parse-proc-entry-semantics.alpha" \
      "$OBLIGATION_DIR/bc-parse-proc-outcomes.alpha" \
      "$OBLIGATION_DIR/bc-parse-proc-publication.alpha"
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
    bc-root-observation-publication-payloads.alpha \
    bc-fol-resource-cleanup-ledger.alpha
  do
    root_observation_module_bytes=$(wc -c \
      < "$OBLIGATION_DIR/$root_observation_module" | tr -d ' ')
    if [ "$root_observation_module_bytes" -ge 20000 ]; then
      echo "bc block control FAIL — $root_observation_module is ${root_observation_module_bytes} bytes (20KB module cap)" >&2
      exit 1
    fi
  done
}

emit_root_observation_prefix() {
  emit_expression_table_prefix
  cat "$OBLIGATION_DIR/bc-root-observation-root.alpha" \
    "$OBLIGATION_DIR/bc-root-observation-antecedents.alpha" \
    "$OBLIGATION_DIR/bc-root-observation-shape.alpha" \
    "$OBLIGATION_DIR/bc-root-observation-gfp.alpha" \
    "$OBLIGATION_DIR/bc-root-observation-resource-join.alpha" \
    "$OBLIGATION_DIR/bc-root-observation-memory-safety.alpha" \
    "$OBLIGATION_DIR/bc-root-observation-maximal.alpha"
}

emit_fol_resource_prefix() {
  emit_expression_table_prefix
  cat "$OBLIGATION_DIR/bc-root-observation-root.alpha" \
    "$OBLIGATION_DIR/bc-root-observation-antecedents.alpha" \
    "$OBLIGATION_DIR/bc-root-observation-shape.alpha" \
    "$OBLIGATION_DIR/bc-root-observation-resource-join.alpha"
}

build_root_observation_checker() {
  root_observation_require_module_budgets
  {
    emit_root_observation_prefix
    cat "$OBLIGATION_DIR/bc-root-observation-publication.alpha" \
      "$OBLIGATION_DIR/bc-root-observation-publication-payloads.alpha"
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

build_fol_resource_cleanup_ledger() {
  {
    emit_fol_resource_prefix
    cat "$OBLIGATION_DIR/bc-fol-resource-cleanup-ledger.alpha"
  } > "$T/fol-resource-ledger.alpha"
  python3 "$OMEGA_PATH_ALPHA_ASSEMBLER/asm_ref.py" \
    < "$T/fol-resource-ledger.alpha" > "$T/fol-resource-ledger-ref.tape"
  "$ASM" < "$T/fol-resource-ledger.alpha" > "$T/fol-resource-ledger.tape"
  cmp -s "$T/fol-resource-ledger.tape" "$T/fol-resource-ledger-ref.tape" || {
    echo "bc block control FAIL — FOL resource ledger assembler diamond disagrees" >&2
    exit 1
  }
  fol_resource_ledger_tape_bytes=$(wc -c \
    < "$T/fol-resource-ledger.tape" | tr -d ' ')
  if [ "$fol_resource_ledger_tape_bytes" -gt 100000 ]; then
    echo "bc block control FAIL — FOL resource ledger tape is ${fol_resource_ledger_tape_bytes} bytes (100000-byte engineering budget)" >&2
    exit 1
  fi
  stamp_seed "$T/fol-resource-ledger.tape" "$SEED" \
    "$T/fol-resource-ledger" >/dev/null
  stamp_proof_checker "$T/proof-checker" >/dev/null
}

smoke_fol_resource_cleanup_ledger() {
  set +e
  "$T/fol-resource-ledger" < "$T/control.bundle" \
    > "$T/fol-resource-owner-prefix"
  fol_resource_ledger_status=$?
  set -e
  if [ "$fol_resource_ledger_status" != 0 ] || \
      [ ! -s "$T/fol-resource-owner-prefix" ]; then
    echo "bc block control FAIL — FOL resource ledger did not publish its canonical prefix" >&2
    exit 1
  fi
  python3 "$OMEGA_PATH_ALPHA_CHECKER/tools/elab.py" \
    < "$GATE_DIR/fol/bc-main-resource-refinement.elab" \
    > "$T/fol-resource-candidate.raw"
  python3 "$GATE_DIR/fol/trace_refinement_seam.py" --split \
    "$T/fol-resource-candidate.raw" \
    "$T/fol-resource-candidate-prefix" \
    "$T/fol-resource-candidate-proof"
  cmp -s "$T/fol-resource-owner-prefix" \
    "$T/fol-resource-candidate-prefix" || {
    echo "bc block control FAIL — proof producer changed FOL resource declarations or goal" >&2
    exit 1
  }
  cat "$T/fol-resource-owner-prefix" "$T/fol-resource-candidate-proof" \
    > "$T/fol-resource-certificate"
  set +e
  fol_resource_verdict=$("$T/proof-checker" \
    < "$T/fol-resource-certificate")
  fol_resource_checker_status=$?
  set -e
  if [ "$fol_resource_verdict" != accept ]; then
    echo "bc block control FAIL — rooted checker rejected FOL resource cleanup (status $fol_resource_checker_status)" >&2
    exit 1
  fi

  # The exact indices participate in normalization rather than appearing as
  # ceremonial equalities. Mutate one use in the owner-fixed goal, plus the
  # dynamic-ret successor rule, and require the same proof to fail.
  for fol_resource_binding_case in subject profile observation ret-successor
  do
    case "$fol_resource_binding_case" in
      subject)
        fol_resource_binding_old='(k 41)'
        fol_resource_binding_new='(k 42)'
        ;;
      profile)
        fol_resource_binding_old='(k 43)'
        fol_resource_binding_new='(k 44)'
        ;;
      observation)
        fol_resource_binding_old='(k 44)'
        fol_resource_binding_new='(k 43)'
        ;;
      ret-successor)
        fol_resource_binding_old='(fun 35 21 (k 12 (k 22 (v 0))))'
        fol_resource_binding_new='(fun 35 21 (k 12 (k 16 (v 0))))'
        ;;
    esac
    python3 -c 'import pathlib,sys
p = pathlib.Path(sys.argv[1]); out = pathlib.Path(sys.argv[2])
raw = p.read_bytes(); old = sys.argv[3].encode(); new = sys.argv[4].encode()
at = raw.rfind(old)
if at < 0: raise SystemExit("binding tooth did not find its target")
changed = raw[:at] + new + raw[at + len(old):]
out.write_bytes(changed)' \
      "$T/fol-resource-candidate-prefix" \
      "$T/fol-resource-$fol_resource_binding_case-prefix" \
      "$fol_resource_binding_old" "$fol_resource_binding_new"
    cat "$T/fol-resource-$fol_resource_binding_case-prefix" \
      "$T/fol-resource-candidate-proof" \
      > "$T/fol-resource-$fol_resource_binding_case.raw"
    set +e
    fol_resource_binding_verdict=$("$T/proof-checker" \
      < "$T/fol-resource-$fol_resource_binding_case.raw")
    set -e
    if [ "$fol_resource_binding_verdict" != reject ]; then
      echo "bc block control FAIL — FOL $fol_resource_binding_case mutation was not rejected" >&2
      exit 1
    fi
    if cmp -s "$T/fol-resource-owner-prefix" \
        "$T/fol-resource-$fol_resource_binding_case-prefix"; then
      echo "bc block control FAIL — owner accepted changed FOL $fol_resource_binding_case" >&2
      exit 1
    fi
  done
  require_control_bundle_unchanged
  # Keep the function's status independent of the final conditional inside the
  # bundle-integrity helper on shells that propagate its false test status.
  return 0
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
  build_fol_resource_cleanup_ledger
  smoke_fol_resource_cleanup_ledger
}

bc_timing_start canonical-prerequisites
procedure_inventory_module_bytes=$(wc -c \
  < "$OBLIGATION_DIR/bc-procedure-inventory.alpha" | tr -d ' ')
if [ "$procedure_inventory_module_bytes" -ge 20000 ]; then
  echo "bc block control FAIL — bc-procedure-inventory.alpha is ${procedure_inventory_module_bytes} bytes (20KB module cap)" >&2
  exit 1
fi
effect_census_prefix_module_bytes=$(wc -c \
  < "$OBLIGATION_DIR/bc-effect-census-prefix.alpha" | tr -d ' ')
if [ "$effect_census_prefix_module_bytes" -ge 20000 ]; then
  echo "bc block control FAIL — bc-effect-census-prefix.alpha is ${effect_census_prefix_module_bytes} bytes (20KB module cap)" >&2
  exit 1
fi
build_emit_dec_word_checker
smoke_emit_dec_word_checker
build_label_emitters_checker
smoke_label_emitters_checker
build_expression_family_checkers
smoke_expression_family_checkers
build_statement_family_shape_checker
smoke_statement_family_shape_checker
bc_timing_finish

bc_timing_start checker-a-canonical
cat "$OBLIGATION_DIR/bc-block-control.alpha" \
  "$OBLIGATION_DIR/bc-effect-sites.alpha" \
  "$OBLIGATION_DIR/bc-exact-shape-helpers.alpha" \
  "$OBLIGATION_DIR/bc-procedure-inventory.alpha" \
  "$OBLIGATION_DIR/bc-event-identity.alpha" \
  "$OBLIGATION_DIR/bc-frame-shape.alpha" \
  "$OBLIGATION_DIR/bc-local-access.alpha" \
  "$OBLIGATION_DIR/bc-memory-sites.alpha" \
  "$OBLIGATION_DIR/bc-expr-primitives.alpha" \
  "$OBLIGATION_DIR/bc-stack-pushes.alpha" \
  "$OBLIGATION_DIR/bc-expression-census-prefix.alpha" \
  "$OBLIGATION_DIR/bc-effect-census-prefix.alpha" \
  "$OBLIGATION_DIR/bc-expr-composition.alpha" \
  "$OBLIGATION_DIR/bc-raw-load-families.alpha" \
  "$OBLIGATION_DIR/bc-call-bounds.alpha" \
  "$OBLIGATION_DIR/bc-stack-register-custody.alpha" \
  "$OBLIGATION_DIR/bc-ranged-store-bounds.alpha" \
  "$OBLIGATION_DIR/bc-frame-summary.alpha" \
  "$OBLIGATION_DIR/bc-ranged-store-transfer.alpha" \
  "$OBLIGATION_DIR/bc-counter-transfer.alpha" \
  "$OBLIGATION_DIR/bc-stack-potential-lift.alpha" \
  "$OBLIGATION_DIR/bc-post-stack-fixed.alpha" \
  "$OBLIGATION_DIR/bc-slurp-summary.alpha" \
  "$OBLIGATION_DIR/bc-main-slurp-bridge.alpha" \
  "$OBLIGATION_DIR/bc-write-str-event-helper.alpha" \
  "$OBLIGATION_DIR/bc-write-str-summary.alpha" \
  "$OBLIGATION_DIR/bc-fixed-emitter-summary.alpha" \
  "$OBLIGATION_DIR/bc-cursor-leaf-summary.alpha" \
  "$OBLIGATION_DIR/bc-skip-ws-summary.alpha" \
  "$OBLIGATION_DIR/bc-main-ready-summary.alpha" \
  "$OBLIGATION_DIR/bc-main-loop-entry-summary.alpha" \
  "$OBLIGATION_DIR/bc-classifier-shape.alpha" \
  "$OBLIGATION_DIR/bc-classifier-summary.alpha" \
  "$OBLIGATION_DIR/bc-read-ident-shape.alpha" \
  "$OBLIGATION_DIR/bc-read-ident-summary.alpha" \
  "$OBLIGATION_DIR/bc-expect-shape.alpha" \
  "$OBLIGATION_DIR/bc-expect-summary.alpha" \
  "$OBLIGATION_DIR/bc-declare-shape.alpha" \
  "$OBLIGATION_DIR/bc-declare-summary.alpha" \
  "$OBLIGATION_DIR/bc-let-keyword-shape.alpha" \
  "$OBLIGATION_DIR/bc-let-keyword-summary.alpha" \
  "$OBLIGATION_DIR/bc-literal-skip-shape.alpha" \
  "$OBLIGATION_DIR/bc-literal-skip-summary.alpha" \
  "$OBLIGATION_DIR/bc-count-lets-control-shape.alpha" \
  "$OBLIGATION_DIR/bc-count-lets-data-shape.alpha" \
  "$OBLIGATION_DIR/bc-count-lets-cases.alpha" \
  "$OBLIGATION_DIR/bc-count-lets-summary.alpha" \
  "$OBLIGATION_DIR/bc-parse-params-control-shape.alpha" \
  "$OBLIGATION_DIR/bc-parse-params-data-shape.alpha" \
  "$OBLIGATION_DIR/bc-parse-parameter-summary.alpha" \
  "$OBLIGATION_DIR/bc-parse-capacity-summary.alpha" \
  "$OBLIGATION_DIR/bc-emit-ident-shape.alpha" \
  "$OBLIGATION_DIR/bc-emit-ident-summary.alpha" \
  "$OBLIGATION_DIR/bc-emit-dec-shape.alpha" \
  "$OBLIGATION_DIR/bc-emit-dec-summary.alpha" \
  "$OBLIGATION_DIR/bc-fixed-decimal-emitters-shape.alpha" \
  "$OBLIGATION_DIR/bc-fixed-decimal-emitters-summary.alpha" \
  "$OBLIGATION_DIR/bc-parse-output-prefix-shape.alpha" \
  "$OBLIGATION_DIR/bc-parse-output-prefix-summary.alpha" \
  "$OBLIGATION_DIR/bc-gen-stmts-boundary-shape.alpha" \
  "$OBLIGATION_DIR/bc-gen-stmts-boundary-summary.alpha" \
  "$OBLIGATION_DIR/bc-parse-number-shape.alpha" \
  "$OBLIGATION_DIR/bc-parse-number-summary.alpha" \
  "$OBLIGATION_DIR/bc-parse-char-shape.alpha" \
  "$OBLIGATION_DIR/bc-parse-char-cases.alpha" \
  "$OBLIGATION_DIR/bc-parse-char-summary.alpha" \
  "$OBLIGATION_DIR/bc-operator-classifier-shape.alpha" \
  "$OBLIGATION_DIR/bc-operator-classifier-summary.alpha" \
  "$OBLIGATION_DIR/bc-cmp-op-shape.alpha" \
  "$OBLIGATION_DIR/bc-cmp-op-cases.alpha" \
  "$OBLIGATION_DIR/bc-cmp-op-summary.alpha" \
  "$OBLIGATION_DIR/bc-fixed-keyword-shape-core.alpha" \
  "$OBLIGATION_DIR/bc-fixed-keyword-data-shape.alpha" \
  "$OBLIGATION_DIR/bc-fixed-keyword-cases.alpha" \
  "$OBLIGATION_DIR/bc-fixed-keyword-summary.alpha" > "$T/control-check.alpha"
checker_a_source_bytes=$(wc -c < "$T/control-check.alpha" | tr -d ' ')
if [ "$checker_a_source_bytes" -gt 1048576 ]; then
  echo "bc block control FAIL — Checker A source is ${checker_a_source_bytes} bytes (1048576-byte assembler input limit)" >&2
  exit 1
fi
"$ASM" < "$T/control-check.alpha" > "$T/control-check.tape"
checker_a_tape_bytes=$(wc -c < "$T/control-check.tape" | tr -d ' ')
checker_a_seed_payload_limit=$((HOLE_SIZE - 4))
if [ "$checker_a_tape_bytes" -gt "$checker_a_seed_payload_limit" ]; then
  echo "bc block control FAIL — Checker A tape is ${checker_a_tape_bytes} bytes (${checker_a_seed_payload_limit}-byte seed payload limit)" >&2
  exit 1
fi
stamp_seed "$T/control-check.tape" "$SEED" "$T/control-check" >/dev/null

# Fail fast on Checker A before composing the later canonical owners.
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
bc_timing_finish

# Eight format/identity-level teeth retain fail-closed input binding without
# generating checker-source permutations. The canonical admission executables
# decide each one. The selected event-PC mutation specifically proves that a
# witness coordinate cannot redefine emit_dec's source-owned call identity.
cp "$T/control.bundle" "$T/wrong-source.bundle"
printf '\161' | dd of="$T/wrong-source.bundle" bs=1 seek=4 conv=notrunc status=none

tape_offset=$((4 + SOURCE_LEN + 4))
cp "$T/control.bundle" "$T/wrong-tape.bundle"
printf '\377' | dd of="$T/wrong-tape.bundle" bs=1 seek="$tape_offset" conv=notrunc status=none

bundle_size=$(wc -c < "$T/control.bundle" | tr -d ' ')
dd if="$T/control.bundle" of="$T/truncated.bundle" bs=1 count=$((bundle_size - 1)) status=none
cp "$T/control.bundle" "$T/wrong-length.bundle"
u32_file $((SOURCE_LEN - 1)) "$T/wrong-source.len"
dd if="$T/wrong-source.len" of="$T/wrong-length.bundle" bs=1 seek=0 conv=notrunc status=none

control_witness_offset=$((4 + SOURCE_LEN + 4 + TAPE_LEN))
# BC11 header (28 u32s), 359 block PCs, 293 transition PCs, then event row 308.
emit_dec_call_pc_offset=$((control_witness_offset + 4 * (28 + 359 + 293 + 308)))
cp "$T/control.bundle" "$T/wrong-event-pc.bundle"
printf '\377' | dd of="$T/wrong-event-pc.bundle" bs=1 seek="$emit_dec_call_pc_offset" conv=notrunc status=none

# These two emit_param_store calls have the same complete semantic key. Swap
# their otherwise valid witness PCs: exact-cardinality occurrence selection
# must retain their distinct ordered continuations rather than aliasing them.
emit_param_first_pc_offset=$((control_witness_offset + 4 * (28 + 359 + 293 + 328)))
emit_param_second_pc_offset=$((control_witness_offset + 4 * (28 + 359 + 293 + 332)))
cp "$T/control.bundle" "$T/swapped-event-occurrence.bundle"
dd if="$T/swapped-event-occurrence.bundle" of="$T/first-event-pc" bs=1 skip="$emit_param_first_pc_offset" count=4 status=none
dd if="$T/swapped-event-occurrence.bundle" of="$T/second-event-pc" bs=1 skip="$emit_param_second_pc_offset" count=4 status=none
dd if="$T/second-event-pc" of="$T/swapped-event-occurrence.bundle" bs=1 seek="$emit_param_first_pc_offset" conv=notrunc status=none
dd if="$T/first-event-pc" of="$T/swapped-event-occurrence.bundle" bs=1 seek="$emit_param_second_pc_offset" conv=notrunc status=none

# gen_emit's three standalone newlines have one complete semantic key. Swap two
# otherwise valid witness PCs: the checker-owned exact cardinality, lexical
# occurrence, and artifact-order rejoin must reject the inverted pair.
gen_emit_first_newline_pc_offset=$((control_witness_offset + 4 * (28 + 359 + 293 + 204)))
gen_emit_second_newline_pc_offset=$((control_witness_offset + 4 * (28 + 359 + 293 + 214)))
cp "$T/control.bundle" "$T/swapped-emit-occurrence.bundle"
dd if="$T/swapped-emit-occurrence.bundle" of="$T/first-emit-pc" bs=1 skip="$gen_emit_first_newline_pc_offset" count=4 status=none
dd if="$T/swapped-emit-occurrence.bundle" of="$T/second-emit-pc" bs=1 skip="$gen_emit_second_newline_pc_offset" count=4 status=none
dd if="$T/second-emit-pc" of="$T/swapped-emit-occurrence.bundle" bs=1 seek="$gen_emit_first_newline_pc_offset" conv=notrunc status=none
dd if="$T/first-emit-pc" of="$T/swapped-emit-occurrence.bundle" bs=1 seek="$gen_emit_second_newline_pc_offset" conv=notrunc status=none

# Memory rows 67 and 69 are word loads in the same procedure and block. Swap
# their valid PCs; their independently checked address-literal joins must keep
# RESOURCE_FAIL and BLOCKDEPTH distinct.
first_memory_pc_offset=$((control_witness_offset + 4 * (28 + 359 + 293 + 617 + 244 + 67)))
second_memory_pc_offset=$((control_witness_offset + 4 * (28 + 359 + 293 + 617 + 244 + 69)))
cp "$T/control.bundle" "$T/swapped-memory-identity.bundle"
dd if="$T/swapped-memory-identity.bundle" of="$T/first-memory-pc" bs=1 skip="$first_memory_pc_offset" count=4 status=none
dd if="$T/swapped-memory-identity.bundle" of="$T/second-memory-pc" bs=1 skip="$second_memory_pc_offset" count=4 status=none
dd if="$T/second-memory-pc" of="$T/swapped-memory-identity.bundle" bs=1 seek="$first_memory_pc_offset" conv=notrunc status=none
dd if="$T/first-memory-pc" of="$T/swapped-memory-identity.bundle" bs=1 seek="$second_memory_pc_offset" conv=notrunc status=none

root_case_run() {
  set +e
  "$T/root-observation" < "$2" > "$T/stdout"
  root_case_status=$?
  set -e
  if [ "$root_case_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc admission FAIL — $1 was not rejected" >&2
    exit 1
  fi
}
fol_resource_case_run() {
  set +e
  "$T/fol-resource-ledger" < "$2" > "$T/stdout"
  fol_resource_case_status=$?
  set -e
  if [ "$fol_resource_case_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc admission FAIL — $1 was not rejected by the FOL instruction owner" >&2
    exit 1
  fi
}
label_emitters_case_run() {
  set +e
  "$T/label-emitters-check" < "$2" > "$T/stdout"
  label_emitters_case_status=$?
  set -e
  if [ "$label_emitters_case_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc admission FAIL — $1 was not rejected by the label-emitter owner" >&2
    exit 1
  fi
}
root_case_run wrong-source "$T/wrong-source.bundle"
root_case_run wrong-tape "$T/wrong-tape.bundle"
root_case_run truncated-witness "$T/truncated.bundle"
root_case_run wrong-length "$T/wrong-length.bundle"
root_case_run wrong-event-pc "$T/wrong-event-pc.bundle"
root_case_run swapped-event-occurrence "$T/swapped-event-occurrence.bundle"
root_case_run swapped-memory-identity "$T/swapped-memory-identity.bundle"
fol_resource_case_run wrong-source "$T/wrong-source.bundle"
fol_resource_case_run wrong-tape "$T/wrong-tape.bundle"
label_emitters_case_run swapped-emit-occurrence "$T/swapped-emit-occurrence.bundle"

root_tape_bytes=$(wc -c < "$T/root-observation.tape" | tr -d ' ')
root_tape_sha256=$(shasum -a 256 "$T/root-observation.tape" | cut -d ' ' -f 1)
echo "bc admission: exact B_bc1 maximal observation + 8 format/identity-binding teeth + 2 FOL subject-bundle teeth + 4 FOL semantic-binding teeth + 4 expression-prefix teeth + 4 effect-prefix teeth passed (${root_tape_bytes}-byte ROOT tape, sha256 ${root_tape_sha256})"
