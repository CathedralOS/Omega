#!/usr/bin/env sh
# Phase-isolated canaries for the split exact-shape/semantic expression proof.

expression_family_build_tooth() { # kind name exact-old exact-new
  expression_tooth_kind=$1
  expression_tooth_name=$2
  expression_tooth_old=$3
  expression_tooth_new=$4
  expression_tooth_source="$T/expression-family-$expression_tooth_kind.alpha"
  expression_tooth_count=$(grep -F -c -- "$expression_tooth_old" \
    "$expression_tooth_source" || true)
  if [ "$expression_tooth_count" != 1 ]; then
    echo "bc block control FAIL — $expression_tooth_name anchor count $expression_tooth_count" >&2
    exit 1
  fi
  awk -v old="$expression_tooth_old" -v new="$expression_tooth_new" '
    {
      at = index($0, old)
      if (at != 0) {
        $0 = substr($0, 1, at - 1) new substr($0, at + length(old))
      }
      print
    }
  ' "$expression_tooth_source" > "$T/$expression_tooth_name.alpha"
  "$ASM" < "$T/$expression_tooth_name.alpha" > "$T/$expression_tooth_name.tape"
  stamp_seed "$T/$expression_tooth_name.tape" "$SEED" \
    "$T/$expression_tooth_name" >/dev/null
}

expression_family_build_teeth() {
  expression_family_build_tooth shape expression-shape-wrong-family-count \
    'imm r2, 7                       ; exact procedure family cardinality' \
    'imm r2, 6                       ; exact procedure family cardinality'
  expression_family_build_tooth shape expression-shape-wrong-gen-expr-proc \
    'imm r21, 61                    ; exact gen_expr procedure id' \
    'imm r21, 60                    ; exact gen_expr procedure id'
  expression_family_build_tooth shape expression-shape-wrong-gen-expr-entry \
    'imm r21, 43134                 ; exact gen_expr entry pc' \
    'imm r21, 43135                 ; exact gen_expr entry pc'
  expression_family_build_tooth shape expression-shape-wrong-depth-store \
    'imm r24, 43373' 'imm r24, 43374'
  expression_family_build_tooth shape expression-shape-wrong-comparison-call \
    'imm r24, 40739' 'imm r24, 40740'
  expression_family_build_tooth shape expression-shape-wrong-return-event \
    'imm r20, 497                   ; comparison return event' \
    'imm r20, 498                   ; comparison return event'
  expression_family_build_tooth shape expression-shape-wrong-memory-census \
    'imm r25, 64                    ; exclusive memory row bound' \
    'imm r25, 63                    ; exclusive memory row bound'
  expression_family_build_tooth shape expression-shape-wrong-shape-token \
    'imm r2, 1213417541              ; XSHL' \
    'imm r2, 1213417540              ; XSHL'

  expression_family_build_tooth semantic expression-semantic-drop-writer-domain \
    'imm r21, 1129534036             ; XRSC: exhaustive protected writers' \
    'imm r21, 1129534035             ; XRSC: exhaustive protected writers'
  expression_family_build_tooth semantic expression-semantic-wrong-left-sum-site \
    'imm r24, 38910                 ; gen_sum' \
    'imm r24, 38911                 ; gen_sum'
  expression_family_build_tooth semantic expression-semantic-wrong-depth-guard \
    'imm r24, 43243' 'imm r24, 43244'
  expression_family_build_tooth semantic expression-semantic-break-depth-formula \
    'imm r13, 64' 'imm r13, 63'
  expression_family_build_tooth semantic expression-semantic-drop-base-provenance \
    'imm r2, 3                     ; three base provenance cases' \
    'imm r2, 2                     ; three base provenance cases'
  expression_family_build_tooth semantic expression-semantic-drop-step-provenance \
    'imm r2, 9                     ; nine step provenance products' \
    'imm r2, 8                     ; nine step provenance products'
  expression_family_build_tooth semantic expression-semantic-break-root-row \
    'imm r21, 1                    ; exact completed child context' \
    'imm r21, 2                    ; exact completed child context'
  expression_family_build_tooth semantic expression-semantic-wrong-publication \
    'imm r2, 1112559704             ; XPUB' \
    'imm r2, 1112559703             ; XPUB'
}

expression_family_reject_teeth() {
  for expression_tooth_name in \
    expression-shape-wrong-family-count \
    expression-shape-wrong-gen-expr-proc \
    expression-shape-wrong-gen-expr-entry \
    expression-shape-wrong-depth-store \
    expression-shape-wrong-comparison-call \
    expression-shape-wrong-return-event \
    expression-shape-wrong-memory-census \
    expression-shape-wrong-shape-token \
    expression-semantic-drop-writer-domain \
    expression-semantic-wrong-left-sum-site \
    expression-semantic-wrong-depth-guard \
    expression-semantic-break-depth-formula \
    expression-semantic-drop-base-provenance \
    expression-semantic-drop-step-provenance \
    expression-semantic-break-root-row \
    expression-semantic-wrong-publication
  do
    set +e
    "$T/$expression_tooth_name" < "$T/control.bundle" > "$T/stdout"
    expression_tooth_status=$?
    set -e
    if [ "$expression_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $expression_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
