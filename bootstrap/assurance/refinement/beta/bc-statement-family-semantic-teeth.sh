#!/usr/bin/env sh
# Phase-isolated canaries for the conditional statement-family semantics.

statement_semantic_build_tooth() { # name exact-old exact-new
  statement_semantic_tooth_name=$1
  statement_semantic_tooth_old=$2
  statement_semantic_tooth_new=$3
  statement_semantic_tooth_count=$(grep -F -c -- \
    "$statement_semantic_tooth_old" \
    "$T/statement-family-semantic.alpha" || true)
  if [ "$statement_semantic_tooth_count" != 1 ]; then
    echo "bc block control FAIL — $statement_semantic_tooth_name anchor count $statement_semantic_tooth_count" >&2
    exit 1
  fi
  awk -v old="$statement_semantic_tooth_old" \
      -v new="$statement_semantic_tooth_new" '
    {
      at = index($0, old)
      if (at != 0) {
        $0 = substr($0, 1, at - 1) new substr($0, at + length(old))
      }
      print
    }
  ' "$T/statement-family-semantic.alpha" \
    > "$T/$statement_semantic_tooth_name.alpha"
  "$ASM" < "$T/$statement_semantic_tooth_name.alpha" \
    > "$T/$statement_semantic_tooth_name.tape"
  stamp_seed "$T/$statement_semantic_tooth_name.tape" "$SEED" \
    "$T/$statement_semantic_tooth_name" >/dev/null
}

statement_family_semantic_build_teeth() {
  statement_semantic_build_tooth statement-semantic-wrong-antecedent-count \
    'imm r21, 15                     ; exact external antecedent count' \
    'imm r21, 14                     ; exact external antecedent count'
  statement_semantic_build_tooth statement-semantic-wrong-epilogue-length \
    'imm r2, 49                    ; exact concatenated byte count' \
    'imm r2, 48                    ; exact concatenated byte count'
  statement_semantic_build_tooth statement-semantic-drop-store-product \
    'imm r2, 3                     ; exact sticky status product' \
    'imm r2, 2                     ; exact sticky status product'
  statement_semantic_build_tooth statement-semantic-wrong-label-order \
    'imm r2, 3                     ; current, separator, target' \
    'imm r2, 2                     ; current, separator, target'
  statement_semantic_build_tooth statement-semantic-restore-id-globals \
    'imm r2, 1                    ; plain restore address is CUR only' \
    'imm r2, 2                    ; plain restore address is CUR only'
  statement_semantic_build_tooth statement-semantic-wrong-dispatch-count \
    'imm r2, 7                        ; all seven ordered destinations' \
    'imm r2, 6                        ; all seven ordered destinations'
  statement_semantic_build_tooth statement-semantic-drop-let-suffix \
    'imm r2, 2                        ; ESTS(n) executed in both products' \
    'imm r2, 1                        ; ESTS(n) executed in both products'
  statement_semantic_build_tooth statement-semantic-collapse-lookup-zero \
    'imm r2, 2                        ; numeric-zero alias has two provenances' \
    'imm r2, 1                        ; numeric-zero alias has two provenances'
  statement_semantic_build_tooth statement-semantic-collapse-state-div \
    'imm r2, 1                        ; Div trace/frame lifted, no return' \
    'imm r2, 0                        ; Div trace/frame lifted, no return'
  statement_semantic_build_tooth statement-semantic-collapse-wrapper-div \
    'store r1, r2                  ; Div child -> Div, frames remain live' \
    'store r1, r0                  ; Div child -> Div, frames remain live'
  statement_semantic_build_tooth statement-semantic-wrong-depth-row-count \
    'imm r2, 65' \
    'imm r2, 64'
  statement_semantic_build_tooth statement-semantic-drop-depth-base \
    'store r10, r2                    ; one terminal: depth exhaustion' \
    'store r10, r0                    ; one terminal: depth exhaustion'
  statement_semantic_build_tooth statement-semantic-drop-terminal-case \
    'store r12, r2                    ; resource/close/eof terminals' \
    'store r12, r1                    ; resource/close/eof terminals'
  statement_semantic_build_tooth statement-semantic-skip-post-child-ws \
    'store r12, r2                    ; post-Ret SWSQ is mandatory' \
    'store r12, r0                    ; post-Ret SWSQ is mandatory'
  statement_semantic_build_tooth statement-semantic-cursor-progress-guard \
    'store r12, r2                    ; no cursor-productivity premise' \
    'store r12, r0                    ; no cursor-productivity premise'
  statement_semantic_build_tooth statement-semantic-output-progress-guard \
    'store r12, r2                    ; no stdout-productivity premise' \
    'store r12, r0                    ; no stdout-productivity premise'
  statement_semantic_build_tooth statement-semantic-use-least-fixed-point \
    'store r12, r2                    ; nu, never least fixed point' \
    'store r12, r1                    ; nu, never least fixed point'
  statement_semantic_build_tooth statement-semantic-decrement-divergence \
    'store r12, r2                    ; divergent activation no decrement' \
    'store r12, r0                    ; divergent activation no decrement'
  statement_semantic_build_tooth statement-semantic-truncate-prefix \
    'store r12, r2                    ; maximal Ret/Div trace prefix' \
    'store r12, r0                    ; maximal Ret/Div trace prefix'
  statement_semantic_build_tooth statement-semantic-drop-tau-constructor \
    'store r1, r2                    ; Ret/Tau/Put constructors' \
    'store r1, r0                    ; Ret/Tau/Put constructors'
  statement_semantic_build_tooth statement-semantic-wrong-publication \
    'imm r2, 1112887379             ; SPUB conditional statement theorem' \
    'imm r2, 1112887378             ; SPUB conditional statement theorem'
}

statement_family_semantic_reject_teeth() {
  for statement_semantic_tooth_name in \
    statement-semantic-wrong-antecedent-count \
    statement-semantic-wrong-epilogue-length \
    statement-semantic-drop-store-product \
    statement-semantic-wrong-label-order \
    statement-semantic-restore-id-globals \
    statement-semantic-wrong-dispatch-count \
    statement-semantic-drop-let-suffix \
    statement-semantic-collapse-lookup-zero \
    statement-semantic-collapse-state-div \
    statement-semantic-collapse-wrapper-div \
    statement-semantic-wrong-depth-row-count \
    statement-semantic-drop-depth-base \
    statement-semantic-drop-terminal-case \
    statement-semantic-skip-post-child-ws \
    statement-semantic-cursor-progress-guard \
    statement-semantic-output-progress-guard \
    statement-semantic-use-least-fixed-point \
    statement-semantic-decrement-divergence \
    statement-semantic-truncate-prefix \
    statement-semantic-drop-tau-constructor \
    statement-semantic-wrong-publication
  do
    set +e
    "$T/$statement_semantic_tooth_name" \
      < "$T/control.bundle" > "$T/stdout"
    statement_semantic_tooth_status=$?
    set -e
    if [ "$statement_semantic_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $statement_semantic_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
