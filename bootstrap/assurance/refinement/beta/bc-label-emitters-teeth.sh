#!/usr/bin/env sh
# Phase-isolated canaries for Checker E's label/string/comparison family.

label_emitters_build_tooth() { # name exact-old exact-new
  label_emitters_tooth_name=$1
  label_emitters_tooth_old=$2
  label_emitters_tooth_new=$3
  label_emitters_tooth_count=$(grep -F -c -- "$label_emitters_tooth_old" \
    "$T/label-emitters-check.alpha" || true)
  if [ "$label_emitters_tooth_count" != 1 ]; then
    echo "bc block control FAIL — $label_emitters_tooth_name anchor count $label_emitters_tooth_count" >&2
    exit 1
  fi
  awk -v old="$label_emitters_tooth_old" -v new="$label_emitters_tooth_new" '
    {
      at = index($0, old)
      if (at != 0) {
        $0 = substr($0, 1, at - 1) new substr($0, at + length(old))
      }
      print
    }
  ' "$T/label-emitters-check.alpha" > "$T/$label_emitters_tooth_name.alpha"
  "$ASM" < "$T/$label_emitters_tooth_name.alpha" \
    > "$T/$label_emitters_tooth_name.tape"
  stamp_seed "$T/$label_emitters_tooth_name.tape" "$SEED" \
    "$T/$label_emitters_tooth_name" >/dev/null
}

label_emitters_build_teeth() {
  # The cursor family is conditional on this exact successful source segment.
  label_emitters_build_tooth label-emitters-drop-source-premise \
    'imm r2, 1                    ; conditional successful source segment' \
    'imm r2, 0                    ; conditional successful source segment'

  # NLBL is a modular full-Word theorem, including WORD_MAX -> 0.
  label_emitters_build_tooth label-emitters-break-word-max-wrap \
    'sub r10, r11                 ; WORD_MAX = 0-1' \
    'add r10, r11                 ; WORD_MAX = 0-1'
  label_emitters_build_tooth label-emitters-drop-word-max-endpoint \
    'imm r2, 2                    ; zero and WORD_MAX endpoints checked' \
    'imm r2, 1                    ; zero and WORD_MAX endpoints checked'
  label_emitters_build_tooth label-emitters-wrong-nlbl-publication \
    'imm r2, 1279413326            ; NLBL' \
    'imm r2, 1279413325            ; NLBL'

  # LREF retains DECW's signed full-Word behavior, including the -8 remainder.
  label_emitters_build_tooth label-emitters-break-lref-int-min \
    'add r10, r14                 ; INT64_MIN = INT64_MAX+1' \
    'sub r10, r14                 ; INT64_MIN = INT64_MAX+1'
  label_emitters_build_tooth label-emitters-break-lref-minus-eight \
    'sub r13, r14                 ; signed remainder -8' \
    'add r13, r14                 ; signed remainder -8'
  label_emitters_build_tooth label-emitters-wrong-lref-int-min-byte \
    'imm r13, 40' \
    'imm r13, 41'
  label_emitters_build_tooth label-emitters-weaken-lref-word-relation \
    'imm r2, 3                    ; exact relation, not canonical-for-all' \
    'imm r2, 2                    ; exact relation, not canonical-for-all'
  label_emitters_build_tooth label-emitters-drop-lref-prefix-order \
    'imm r2, 1                    ; helper completion precedes child output' \
    'imm r2, 0                    ; helper completion precedes child output'

  # STRB is deliberately permissive at the opening and exact at malformed tails.
  label_emitters_build_tooth label-emitters-drop-strb-blind-opening \
    'store r1, r2                  ; exact event183 blindly calls adv' \
    'store r1, r1                  ; exact event183 blindly calls adv'
  label_emitters_build_tooth label-emitters-invent-strb-quote-premise \
    'store r1, r2                  ; k=i+1<=LEN+1, no quote premise' \
    'store r1, r1                  ; k=i+1<=LEN+1, no quote premise'
  label_emitters_build_tooth label-emitters-drop-strb-escaped-nul \
    'imm r2, 1                    ; escaped NUL copied and continues' \
    'imm r2, 0                    ; escaped NUL copied and continues'
  label_emitters_build_tooth label-emitters-undercount-strb-second-bytes \
    'imm r1, 535776' \
    'imm r1, 535777'
  label_emitters_build_tooth label-emitters-shorten-strb-escape-trace \
    'imm r2, 3                    ; trace = [92,next] || child trace' \
    'imm r2, 2                    ; trace = [92,next] || child trace'
  label_emitters_build_tooth label-emitters-double-strb-escape-length \
    'imm r2, 2                    ; logical len = 1+child, never +2' \
    'imm r2, 3                    ; logical len = 1+child, never +2'
  label_emitters_build_tooth label-emitters-drop-strb-trailing-pair \
    'imm r2, 1                    ; emitted exact [92,0]' \
    'imm r2, 0                    ; emitted exact [92,0]'
  label_emitters_build_tooth label-emitters-wrong-strb-trailing-length \
    'imm r2, 2                    ; one logical unit + zero child len' \
    'imm r2, 1                    ; one logical unit + zero child len'
  label_emitters_build_tooth label-emitters-drop-strb-unchecked-opening \
    'store r1, r2                  ; opening byte remains unchecked' \
    'store r1, r1                  ; opening byte remains unchecked'
  label_emitters_build_tooth label-emitters-drop-strb-tail-summary \
    'store r1, r2                  ; trailing escape retained in theorem' \
    'store r1, r1                  ; trailing escape retained in theorem'
  label_emitters_build_tooth label-emitters-drop-strb-nul-summary \
    'store r1, r2                  ; escaped NUL continuation retained' \
    'store r1, r1                  ; escaped NUL continuation retained'
  label_emitters_build_tooth label-emitters-undercount-strb-events \
    'imm r29, 198                 ; exclusive event row' \
    'imm r29, 197                 ; exclusive event row'

  # GEMS fixes all eleven fragments, four LREF roles, and both final cursor classes.
  label_emitters_build_tooth label-emitters-drop-gen-fixed-total \
    'imm r1, 536952' \
    'imm r1, 536953'
  label_emitters_build_tooth label-emitters-drop-gen-fixed-event-count \
    'imm r1, 537088' \
    'imm r1, 537089'
  label_emitters_build_tooth label-emitters-swap-gen-first-label-role \
    'imm r1, 537008' \
    'imm r1, 537009'
  label_emitters_build_tooth label-emitters-drop-gen-phase-order \
    'imm r1, 536944' \
    'imm r1, 536945'
  label_emitters_build_tooth label-emitters-drop-gen-final-exps \
    'imm r1, 536888' \
    'imm r1, 536889'
  label_emitters_build_tooth label-emitters-drop-gen-final-expx \
    'imm r1, 536896' \
    'imm r1, 536897'
  label_emitters_build_tooth label-emitters-drop-gen-final-partition \
    'imm r1, 536904' \
    'imm r1, 536905'
  label_emitters_build_tooth label-emitters-undercount-gen-events \
    'imm r29, 221                  ; exclusive event row' \
    'imm r29, 220                  ; exclusive event row'
  label_emitters_build_tooth label-emitters-wrong-gen-final-expect-event \
    "imm r21, 20540                ; expect(')')" \
    "imm r21, 20541                ; expect(')')"

  # ECMP covers the invalid-code complement, signed high-bit split, and labels.
  label_emitters_build_tooth label-emitters-invent-invalid-cmp-operator \
    'imm r2, 70                    ; 42 prelude + 28 post, no operator' \
    'imm r2, 80                    ; 42 prelude + 28 post, no operator'
  label_emitters_build_tooth label-emitters-drop-cmp-false-complement \
    'imm r2, 2                    ; complete six-false complement' \
    'imm r2, 1                    ; complete six-false complement'
  label_emitters_build_tooth label-emitters-break-cmp-high-bit-lt3 \
    'add r1, r2                   ; INT64_MIN' \
    'sub r1, r2                   ; INT64_MIN'
  label_emitters_build_tooth label-emitters-drop-cmp-label-order \
    'store r1, r2                  ; exact set,done,set,done child order' \
    'store r1, r1                  ; exact set,done,set,done child order'
  label_emitters_build_tooth label-emitters-wrong-cmp-signed-transition \
    'imm r24, 42377               ; signed code<3' \
    'imm r24, 42378               ; signed code<3'
  label_emitters_build_tooth label-emitters-wrong-family-publication \
    'imm r2, 1263547717           ; E5PK' \
    'imm r2, 1263547716           ; E5PK'
}

label_emitters_reject_teeth() {
  for label_emitters_tooth_name in \
    label-emitters-drop-source-premise \
    label-emitters-break-word-max-wrap \
    label-emitters-drop-word-max-endpoint \
    label-emitters-wrong-nlbl-publication \
    label-emitters-break-lref-int-min \
    label-emitters-break-lref-minus-eight \
    label-emitters-wrong-lref-int-min-byte \
    label-emitters-weaken-lref-word-relation \
    label-emitters-drop-lref-prefix-order \
    label-emitters-drop-strb-blind-opening \
    label-emitters-invent-strb-quote-premise \
    label-emitters-drop-strb-escaped-nul \
    label-emitters-undercount-strb-second-bytes \
    label-emitters-shorten-strb-escape-trace \
    label-emitters-double-strb-escape-length \
    label-emitters-drop-strb-trailing-pair \
    label-emitters-wrong-strb-trailing-length \
    label-emitters-drop-strb-unchecked-opening \
    label-emitters-drop-strb-tail-summary \
    label-emitters-drop-strb-nul-summary \
    label-emitters-undercount-strb-events \
    label-emitters-drop-gen-fixed-total \
    label-emitters-drop-gen-fixed-event-count \
    label-emitters-swap-gen-first-label-role \
    label-emitters-drop-gen-phase-order \
    label-emitters-drop-gen-final-exps \
    label-emitters-drop-gen-final-expx \
    label-emitters-drop-gen-final-partition \
    label-emitters-undercount-gen-events \
    label-emitters-wrong-gen-final-expect-event \
    label-emitters-invent-invalid-cmp-operator \
    label-emitters-drop-cmp-false-complement \
    label-emitters-break-cmp-high-bit-lt3 \
    label-emitters-drop-cmp-label-order \
    label-emitters-wrong-cmp-signed-transition \
    label-emitters-wrong-family-publication
  do
    set +e
    "$T/$label_emitters_tooth_name" < "$T/control.bundle" > "$T/stdout"
    label_emitters_tooth_status=$?
    set -e
    if [ "$label_emitters_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $label_emitters_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
