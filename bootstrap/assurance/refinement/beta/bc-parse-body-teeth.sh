#!/usr/bin/env sh
# Phase-isolated canaries for the parse_proc.genbody implication checker.

parse_body_build_tooth() { # name exact-old exact-new
  parse_body_tooth_name=$1
  parse_body_tooth_old=$2
  parse_body_tooth_new=$3
  parse_body_tooth_count=$(grep -F -c -- "$parse_body_tooth_old" \
    "$T/parse-body.alpha" || true)
  if [ "$parse_body_tooth_count" != 1 ]; then
    echo "bc block control FAIL — $parse_body_tooth_name anchor count $parse_body_tooth_count" >&2
    exit 1
  fi
  awk -v old="$parse_body_tooth_old" -v new="$parse_body_tooth_new" '
    {
      at = index($0, old)
      if (at != 0) {
        $0 = substr($0, 1, at - 1) new substr($0, at + length(old))
      }
      print
    }
  ' "$T/parse-body.alpha" > "$T/$parse_body_tooth_name.alpha"
  "$ASM" < "$T/$parse_body_tooth_name.alpha" \
    > "$T/$parse_body_tooth_name.tape"
  stamp_seed "$T/$parse_body_tooth_name.tape" "$SEED" \
    "$T/$parse_body_tooth_name" >/dev/null
}

parse_body_build_teeth() {
  parse_body_build_tooth parse-body-wrong-pfxs-schema \
    'imm r2, 1398294096             ; PFXS tag' \
    'imm r2, 1398294095             ; PFXS tag'
  parse_body_build_tooth parse-body-wrong-spub-implication \
    'imm r2, 1112887379             ; SPUB tag: A=>SREL' \
    'imm r2, 1112887378             ; SPUB tag: A=>SREL'
  parse_body_build_tooth parse-body-drop-statement-discharge \
    'imm r2, 1397310547             ; SDIS, hypothetical SREL introduction' \
    'imm r2, 1397310546             ; SDIS, hypothetical SREL introduction'
  parse_body_build_tooth parse-body-alias-srel-to-sgfp \
    'imm r2, 1279611475             ; full SREL consequent tag' \
    'imm r2, 1346783059             ; full SREL consequent tag'
  parse_body_build_tooth parse-body-wrong-gen-stmts-target \
    'imm r22, 44066                 ; gen_stmts' \
    'imm r22, 44067                 ; gen_stmts'
  parse_body_build_tooth parse-body-wrong-epilogue-target \
    'imm r22, 31970                 ; emit_epilogue' \
    'imm r22, 31971                 ; emit_epilogue'
  parse_body_build_tooth parse-body-wrong-source-return \
    'imm r23, 50992' \
    'imm r23, 50993'
  parse_body_build_tooth parse-body-duplicate-prefix-term \
    'imm r23, 2                      ; C' \
    'imm r23, 1                      ; C'
  parse_body_build_tooth parse-body-short-epilogue \
    'imm r23, 49                     ; exact suffix bytes' \
    'imm r23, 48                     ; exact suffix bytes'
  parse_body_build_tooth parse-body-wrong-resource-status \
    'imm r23, 252                   ; exact numeric resource status' \
    'imm r23, 251                   ; exact numeric resource status'
  parse_body_build_tooth parse-body-collapse-resource-provenance \
    'imm r24, 3                     ; opaque exact child-guard provenance' \
    'imm r24, 1                     ; opaque exact child-guard provenance'
  parse_body_build_tooth parse-body-skip-epilogue-after-resource \
    'imm r25, 5                     ; numeric252 still appends outer E' \
    'imm r25, 4                     ; numeric252 still appends outer E'
  parse_body_build_tooth parse-body-add-d64-div \
    'imm r11, 2                     ; immediate depth exhaustion Ret252' \
    'imm r11, 3                     ; immediate depth exhaustion Ret252'
  parse_body_build_tooth parse-body-wrong-depth-count \
    'imm r2, 65                     ; exact D=0..64 contexts' \
    'imm r2, 64                     ; exact D=0..64 contexts'
  parse_body_build_tooth parse-body-div-reaches-epilogue \
    'imm r26, 2                     ; event601 unreachable' \
    'imm r26, 1                     ; event601 unreachable'
  parse_body_build_tooth parse-body-div-restores-frame \
    'imm r28, 2                     ; p68 and child resumptions live' \
    'imm r28, 1                     ; p68 and child resumptions live'
  parse_body_build_tooth parse-body-div-decrements-depth \
    'imm r29, 2                     ; live child depth, no decrement' \
    'imm r29, 1                     ; live child depth, no decrement'
  parse_body_build_tooth parse-body-truncate-maximal-prefix \
    'store r20, r1                  ; +104 maximal/exact trace' \
    'store r20, r2                  ; +104 maximal/exact trace'
  parse_body_build_tooth parse-body-add-cursor-productivity \
    'store r20, r1                  ; +112 no cursor productivity premise' \
    'store r20, r2                  ; +112 no cursor productivity premise'
  parse_body_build_tooth parse-body-add-output-productivity \
    'store r20, r1                  ; +120 no stdout productivity premise' \
    'store r20, r2                  ; +120 no stdout productivity premise'
  parse_body_build_tooth parse-body-infer-resource-kind \
    'store r20, r1                  ; +128 no ResourceKind inference' \
    'store r20, r2                  ; +128 no ResourceKind inference'
  parse_body_build_tooth parse-body-float-depth-context \
    'store r20, r1                  ; +144 PFXS depth D = selected SREL_D' \
    'store r20, r2                  ; +144 PFXS depth D = selected SREL_D'
  parse_body_build_tooth parse-body-drop-child-state-custody \
    'store r20, r27                 ; +152 finite final/Div state+cursor' \
    'store r20, r2                  ; +152 finite final/Div state+cursor'
  parse_body_build_tooth parse-body-drop-provenance-identity \
    'store r20, r1                  ; +160 child rho = final rho exactly' \
    'store r20, r2                  ; +160 child rho = final rho exactly'
  parse_body_build_tooth parse-body-wrong-publication \
    'imm r2, 1146045008             ; PBOD conditional body theorem' \
    'imm r2, 1146045007             ; PBOD conditional body theorem'
}

parse_body_reject_teeth() {
  for parse_body_tooth_name in \
    parse-body-wrong-pfxs-schema \
    parse-body-wrong-spub-implication \
    parse-body-drop-statement-discharge \
    parse-body-alias-srel-to-sgfp \
    parse-body-wrong-gen-stmts-target \
    parse-body-wrong-epilogue-target \
    parse-body-wrong-source-return \
    parse-body-duplicate-prefix-term \
    parse-body-short-epilogue \
    parse-body-wrong-resource-status \
    parse-body-collapse-resource-provenance \
    parse-body-skip-epilogue-after-resource \
    parse-body-add-d64-div \
    parse-body-wrong-depth-count \
    parse-body-div-reaches-epilogue \
    parse-body-div-restores-frame \
    parse-body-div-decrements-depth \
    parse-body-truncate-maximal-prefix \
    parse-body-add-cursor-productivity \
    parse-body-add-output-productivity \
    parse-body-infer-resource-kind \
    parse-body-float-depth-context \
    parse-body-drop-child-state-custody \
    parse-body-drop-provenance-identity \
    parse-body-wrong-publication
  do
    set +e
    "$T/$parse_body_tooth_name" < "$T/control.bundle" > "$T/stdout"
    parse_body_tooth_status=$?
    set -e
    if [ "$parse_body_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $parse_body_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
