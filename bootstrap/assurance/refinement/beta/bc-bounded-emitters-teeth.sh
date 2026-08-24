#!/usr/bin/env sh
# Phase-isolated canaries for Checker C's eight bounded emitter theorems.

bounded_emitters_build_tooth() { # name exact-old exact-new
  bounded_emitters_tooth_name=$1
  bounded_emitters_tooth_old=$2
  bounded_emitters_tooth_new=$3
  bounded_emitters_tooth_count=$(grep -F -c -- "$bounded_emitters_tooth_old" \
    "$T/bounded-emitters-check.alpha" || true)
  if [ "$bounded_emitters_tooth_count" != 1 ]; then
    echo "bc block control FAIL — $bounded_emitters_tooth_name anchor count $bounded_emitters_tooth_count" >&2
    exit 1
  fi
  sed "s|$bounded_emitters_tooth_old|$bounded_emitters_tooth_new|" \
    "$T/bounded-emitters-check.alpha" > "$T/$bounded_emitters_tooth_name.alpha"
  "$ASM" < "$T/$bounded_emitters_tooth_name.alpha" \
    > "$T/$bounded_emitters_tooth_name.tape"
  stamp_seed "$T/$bounded_emitters_tooth_name.tape" "$SEED" \
    "$T/$bounded_emitters_tooth_name" >/dev/null
}

bounded_emitters_build_teeth() {
  bounded_emitters_build_tooth bounded-emitters-wrong-stack-bridge \
    'imm r2, 1431261267             ; SPOU' \
    'imm r2, 1431261266             ; SPOU'
  bounded_emitters_build_tooth bounded-emitters-drop-source-premise \
    'imm r2, 1                       ; conditional successful source segment' \
    'imm r2, 0                       ; conditional successful source segment'
  bounded_emitters_build_tooth bounded-emitters-wrong-wstr-bridge \
    'imm r2, 1430467159               ; W2CU' \
    'imm r2, 1430467158               ; W2CU'
  bounded_emitters_build_tooth bounded-emitters-wrong-skip-bridge \
    'imm r2, 1480929875                ; S2EX' \
    'imm r2, 1480929874                ; S2EX'
  bounded_emitters_build_tooth bounded-emitters-wrong-expect-bridge \
    'imm r2, 1162097221                ; E2DE' \
    'imm r2, 1162097220                ; E2DE'
  bounded_emitters_build_tooth bounded-emitters-wrong-decimal-bridge \
    'imm r2, 1161966148                 ; D2BE' \
    'imm r2, 1161966147                 ; D2BE'

  bounded_emitters_build_tooth bounded-emitters-wrong-slot-frame \
    'imm r23, 24                   ; emit_slot_addr frame' \
    'imm r23, 16                   ; emit_slot_addr frame'
  bounded_emitters_build_tooth bounded-emitters-wrong-expect-target \
    'imm r24, 17441               ; first expect target' \
    'imm r24, 17442               ; first expect target'
  bounded_emitters_build_tooth bounded-emitters-wrong-second-expect-continuation \
    'imm r25, 21347               ; exact second call continuation' \
    'imm r25, 21346               ; exact second call continuation'
  bounded_emitters_build_tooth bounded-emitters-wrong-mnemonic-guard \
    'imm r24, 32589               ; add-guard control PC' \
    'imm r24, 32590               ; add-guard control PC'
  bounded_emitters_build_tooth bounded-emitters-wrong-mod-event \
    'imm r22, 33482               ; mod mnemonic event PC' \
    'imm r22, 33483               ; mod mnemonic event PC'
  bounded_emitters_build_tooth bounded-emitters-wrong-slot-call-count \
    'imm r22, 9                   ; emit_slot_addr calls' \
    'imm r22, 8                   ; emit_slot_addr calls'
  bounded_emitters_build_tooth bounded-emitters-wrong-mnemonic-return-count \
    'imm r23, 7                   ; emit_mnemonic returns' \
    'imm r23, 6                   ; emit_mnemonic returns'
  bounded_emitters_build_tooth bounded-emitters-wrong-slot-store-count \
    'imm r24, 8                   ; emit_slot_addr stores' \
    'imm r24, 7                   ; emit_slot_addr stores'
  bounded_emitters_build_tooth bounded-emitters-wrong-open-delimiter \
    'imm r23, 40                  ; gen_read_byte open delimiter' \
    'imm r23, 41                  ; gen_read_byte open delimiter'
  bounded_emitters_build_tooth bounded-emitters-wrong-plus-literal \
    'imm r23, 43                  ; plus Word' \
    'imm r23, 44                  ; plus Word'
  bounded_emitters_build_tooth bounded-emitters-wrong-slot-add \
    'imm r23, 3                   ; exact offset add opcode' \
    'imm r23, 5                   ; exact offset add opcode'
  bounded_emitters_build_tooth bounded-emitters-wrong-slot-multiply \
    'imm r23, 5                   ; exact offset multiply opcode' \
    'imm r23, 3                   ; exact offset multiply opcode'
  bounded_emitters_build_tooth bounded-emitters-wrong-load-selector \
    'imm r23, 0                   ; emit_load_slot selector' \
    'imm r23, 1                   ; emit_load_slot selector'
  bounded_emitters_build_tooth bounded-emitters-wrong-store-selector \
    'imm r23, 1                   ; emit_store_slot selector' \
    'imm r23, 0                   ; emit_store_slot selector'
  bounded_emitters_build_tooth bounded-emitters-wrong-slot-argument-push \
    'imm r23, 34287               ; offset argument push' \
    'imm r23, 34288               ; offset argument push'

  bounded_emitters_build_tooth bounded-emitters-drop-d2be-import \
    'imm r21, 1161966148            ; D2BE' \
    'imm r21, 1161966147            ; D2BE'
  bounded_emitters_build_tooth bounded-emitters-omit-k-fourth-case \
    'imm r1, 4                    ; exact k-domain endpoint' \
    'imm r1, 3                    ; exact k-domain endpoint'
  bounded_emitters_build_tooth bounded-emitters-undercount-domain-pairs \
    'imm r1, 2048                 ; exact reg/slot pair count' \
    'imm r1, 2047                 ; exact reg/slot pair count'
  bounded_emitters_build_tooth bounded-emitters-wrong-max-offset \
    'imm r1, 8192                 ; exact maximum bounded offset' \
    'imm r1, 8191                 ; exact maximum bounded offset'
  bounded_emitters_build_tooth bounded-emitters-break-offset-formula \
    'add r13, r1                   ; exact bounded slot offset' \
    'add r13, r13                  ; exact bounded slot offset'
  bounded_emitters_build_tooth bounded-emitters-drop-cursor-thread \
    'store r1, r12                  ; second call consumes first successor' \
    'store r1, r1                   ; second call consumes first successor'
  bounded_emitters_build_tooth bounded-emitters-wrong-read-length \
    'imm r21, 8                    ; exact read-r0 literal length' \
    'imm r21, 7                    ; exact read-r0 literal length'
  bounded_emitters_build_tooth bounded-emitters-drop-selected-k \
    'store r1, r10                  ; selected exact k' \
    'store r1, r1                   ; selected exact k'
  bounded_emitters_build_tooth bounded-emitters-wrong-pop-fixed-total \
    'imm r21, 31                   ; fixed bytes excluding dec(k)' \
    'imm r21, 30                   ; fixed bytes excluding dec(k)'
  bounded_emitters_build_tooth bounded-emitters-drop-dead-result \
    'store r1, r2                   ; synthetic result is dead/unclaimed' \
    'store r1, r1                   ; synthetic result is dead/unclaimed'
  bounded_emitters_build_tooth bounded-emitters-drop-word-complement \
    'store r1, r2                   ; full-Word mnemonic complement' \
    'store r1, r1                   ; full-Word mnemonic complement'
  bounded_emitters_build_tooth bounded-emitters-wrong-mnemonic-count \
    'imm r1, 7                    ; six total mnemonic partitions endpoint' \
    'imm r1, 6                    ; six total mnemonic partitions endpoint'
  bounded_emitters_build_tooth bounded-emitters-reorder-combine-child \
    'store r1, r2                   ; child completed before suffix' \
    'store r1, r1                   ; child completed before suffix'
  bounded_emitters_build_tooth bounded-emitters-drop-repeated-register \
    'store r1, r2                   ; first and third child use same reg' \
    'store r1, r1                   ; first and third child use same reg'
  bounded_emitters_build_tooth bounded-emitters-drop-load-register \
    'store r1, r2                   ; event375 first argument is exact zero' \
    'store r1, r1                   ; event375 first argument is exact zero'
  bounded_emitters_build_tooth bounded-emitters-drop-store-register \
    'store r1, r2                   ; event377 first argument is exact one' \
    'store r1, r1                   ; event377 first argument is exact one'
  bounded_emitters_build_tooth bounded-emitters-drop-first-delimiter-argument \
    'imm r1, 532408' 'imm r1, 532400'
  bounded_emitters_build_tooth bounded-emitters-drop-second-delimiter-argument \
    'imm r1, 532440' 'imm r1, 532400'
  bounded_emitters_build_tooth bounded-emitters-drop-mnemonic-false-prefix \
    'store r1, r2                   ; false-prefix length' \
    'store r1, r1                   ; false-prefix length'
  bounded_emitters_build_tooth bounded-emitters-wrong-parameterized-mnemonic \
    'imm r2, 1347308869            ; EMNP, parameterized EMNE(OP)' \
    'imm r2, 1347308868            ; EMNP, parameterized EMNE(OP)'
  bounded_emitters_build_tooth bounded-emitters-drop-combine-op-copy \
    'store r1, r2                   ; event364 argument is the same OP' \
    'store r1, r1                   ; event364 argument is the same OP'
  bounded_emitters_build_tooth bounded-emitters-drop-combine-class-copy \
    'store r1, r2                   ; child output uses that same partition' \
    'store r1, r1                   ; child output uses that same partition'
  bounded_emitters_build_tooth bounded-emitters-drop-slot-parameter \
    'store r1, r11                  ; parameterized clause retains s' \
    'store r1, r1                   ; parameterized clause retains s'
  bounded_emitters_build_tooth bounded-emitters-drop-slot-offset-child \
    'store r1, r13                  ; same offset passed to child' \
    'store r1, r1                   ; same offset passed to child'
  bounded_emitters_build_tooth bounded-emitters-drop-first-slot-register \
    'store r1, r10                  ; first child reg' \
    'store r1, r1                   ; first child reg'
  bounded_emitters_build_tooth bounded-emitters-wrong-esla-zero-clause \
    'imm r2, 1093686085            ; ES0A: forall slot, ESLA(0,slot)' \
    'imm r2, 1093686084            ; ES0A: forall slot, ESLA(0,slot)'
  bounded_emitters_build_tooth bounded-emitters-wrong-esla-one-clause \
    'imm r2, 1093751621            ; ES1A: forall slot, ESLA(1,slot)' \
    'imm r2, 1093751620            ; ES1A: forall slot, ESLA(1,slot)'
  bounded_emitters_build_tooth bounded-emitters-drop-load-same-slot \
    'store r1, r10                  ; event375 child receives same s' \
    'store r1, r1                   ; event375 child receives same s'
  bounded_emitters_build_tooth bounded-emitters-drop-store-same-slot \
    'store r1, r10                  ; event377 child receives same s' \
    'store r1, r1                   ; event377 child receives same s'
  bounded_emitters_build_tooth bounded-emitters-break-selected-k-value \
    'load r20, r1                   ; DECS argument consumes selected-k cell' \
    'imm r20, 3                     ; DECS argument consumes selected-k cell'
  bounded_emitters_build_tooth bounded-emitters-wrong-publication \
    'imm r2, 1397572930            ; BEMS' \
    'imm r2, 1397572929            ; BEMS'
}

bounded_emitters_reject_teeth() {
  for bounded_emitters_tooth_name in \
    bounded-emitters-wrong-stack-bridge \
    bounded-emitters-drop-source-premise \
    bounded-emitters-wrong-wstr-bridge \
    bounded-emitters-wrong-skip-bridge \
    bounded-emitters-wrong-expect-bridge \
    bounded-emitters-wrong-decimal-bridge \
    bounded-emitters-wrong-slot-frame \
    bounded-emitters-wrong-expect-target \
    bounded-emitters-wrong-second-expect-continuation \
    bounded-emitters-wrong-mnemonic-guard \
    bounded-emitters-wrong-mod-event \
    bounded-emitters-wrong-slot-call-count \
    bounded-emitters-wrong-mnemonic-return-count \
    bounded-emitters-wrong-slot-store-count \
    bounded-emitters-wrong-open-delimiter \
    bounded-emitters-wrong-plus-literal \
    bounded-emitters-wrong-slot-add \
    bounded-emitters-wrong-slot-multiply \
    bounded-emitters-wrong-load-selector \
    bounded-emitters-wrong-store-selector \
    bounded-emitters-wrong-slot-argument-push \
    bounded-emitters-drop-d2be-import \
    bounded-emitters-omit-k-fourth-case \
    bounded-emitters-undercount-domain-pairs \
    bounded-emitters-wrong-max-offset \
    bounded-emitters-break-offset-formula \
    bounded-emitters-drop-cursor-thread \
    bounded-emitters-wrong-read-length \
    bounded-emitters-drop-selected-k \
    bounded-emitters-wrong-pop-fixed-total \
    bounded-emitters-drop-dead-result \
    bounded-emitters-drop-word-complement \
    bounded-emitters-wrong-mnemonic-count \
    bounded-emitters-reorder-combine-child \
    bounded-emitters-drop-repeated-register \
    bounded-emitters-drop-load-register \
    bounded-emitters-drop-store-register \
    bounded-emitters-drop-first-delimiter-argument \
    bounded-emitters-drop-second-delimiter-argument \
    bounded-emitters-drop-mnemonic-false-prefix \
    bounded-emitters-wrong-parameterized-mnemonic \
    bounded-emitters-drop-combine-op-copy \
    bounded-emitters-drop-combine-class-copy \
    bounded-emitters-drop-slot-parameter \
    bounded-emitters-drop-slot-offset-child \
    bounded-emitters-drop-first-slot-register \
    bounded-emitters-wrong-esla-zero-clause \
    bounded-emitters-wrong-esla-one-clause \
    bounded-emitters-drop-load-same-slot \
    bounded-emitters-drop-store-same-slot \
    bounded-emitters-break-selected-k-value \
    bounded-emitters-wrong-publication
  do
    set +e
    "$T/$bounded_emitters_tooth_name" < "$T/control.bundle" > "$T/stdout"
    bounded_emitters_tooth_status=$?
    set -e
    if [ "$bounded_emitters_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $bounded_emitters_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
