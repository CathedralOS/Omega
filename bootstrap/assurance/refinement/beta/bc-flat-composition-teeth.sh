#!/usr/bin/env sh
# Diagnostic flat-custody projection for composition-order mutations.

flat_composition_build_teeth() {
  # Preserve a projection of the immediately preceding flat-custody phase. The
  # same-valued composition witnesses must pass it and fail only after recursive
  # grammar-directed ordering is enabled.
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
}

flat_composition_case() { # label input
  set +e
  "$T/flat-check" < "$2" > "$T/stdout"
  flat_composition_status=$?
  set -e
  if [ "$flat_composition_status" != 0 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $1 did not preserve flat custody" >&2
    exit 1
  fi
}

flat_composition_reject_teeth() {
  flat_composition_case "recursive-order witness" "$T/composition-order.bundle"
  flat_composition_case "argument-order witness" "$T/composition-argument-order.bundle"
  flat_composition_case "store-order witness" "$T/composition-store-order.bundle"
}
