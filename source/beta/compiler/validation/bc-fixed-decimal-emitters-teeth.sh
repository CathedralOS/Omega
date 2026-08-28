#!/usr/bin/env sh
# Phase-isolated canaries for fixed-decimal emitter shape and meaning.

fixed_decimal_emitters_build_tooth() { # name sed-expression
  fixed_decimal_tooth_name=$1
  fixed_decimal_tooth_sed=$2
  sed "$fixed_decimal_tooth_sed" "$T/control-check.alpha" \
    > "$T/$fixed_decimal_tooth_name.alpha"
  "$ASM" < "$T/$fixed_decimal_tooth_name.alpha" \
    > "$T/$fixed_decimal_tooth_name.tape"
  stamp_seed "$T/$fixed_decimal_tooth_name.tape" "$SEED" \
    "$T/$fixed_decimal_tooth_name" >/dev/null
}

fixed_decimal_emitters_build_teeth() {
  fixed_decimal_emitters_build_tooth fixed-decimal-wrong-guard \
    's/imm r24, 30256              ; checked nslots>0 guard/imm r24, 30257              ; checked nslots>0 guard/'
  fixed_decimal_emitters_build_tooth fixed-decimal-wrong-prologue-continuation \
    's/imm r26, 30458              ; checked prologue decimal continuation/imm r26, 30459              ; checked prologue decimal continuation/'
  fixed_decimal_emitters_build_tooth fixed-decimal-wrong-offset-continuation \
    's/imm r26, 30897              ; checked offset-decimal continuation/imm r26, 30898              ; checked offset-decimal continuation/'
  fixed_decimal_emitters_build_tooth fixed-decimal-wrong-register-continuation \
    's/imm r26, 31092              ; checked register-decimal continuation/imm r26, 31093              ; checked register-decimal continuation/'
  fixed_decimal_emitters_build_tooth fixed-decimal-wrong-first-param-emit \
    's/imm r22, 30687/imm r22, 30688/'
  fixed_decimal_emitters_build_tooth fixed-decimal-wrong-offset-add \
    's/imm r23, 3                   ; 8 + 8\*k/imm r23, 4                   ; 8 + 8*k/'
  fixed_decimal_emitters_build_tooth fixed-decimal-wrong-register-push \
    's/imm r23, 31051/imm r23, 31052/'
  fixed_decimal_emitters_build_tooth fixed-decimal-event-undercount \
    's/imm r29, 326                 ; exclusive prologue event row/imm r29, 325                 ; exclusive prologue event row/'
  fixed_decimal_emitters_build_tooth fixed-decimal-primitive-undercount \
    's/imm r23, 542                 ; exclusive parameter primitive row/imm r23, 541                 ; exclusive parameter primitive row/'
  fixed_decimal_emitters_build_tooth fixed-decimal-store-undercount \
    's/imm r24, 6                   ; frame\/parameter\/push stores/imm r24, 5                   ; frame\/parameter\/push stores/'

  fixed_decimal_emitters_build_tooth fixed-decimal-omit-nslots-1024 \
    's/imm r1, 1025                 ; checked nslots sweep includes 1024/imm r1, 1024                 ; checked nslots sweep includes 1024/'
  fixed_decimal_emitters_build_tooth fixed-decimal-omit-k-3 \
    's/imm r1, 4                    ; checked k sweep includes 3/imm r1, 3                    ; checked k sweep includes 3/'
  fixed_decimal_emitters_build_tooth fixed-decimal-drop-prologue-argument \
    's/imm r2, 1                    ; row535\/push316 pass exact 8\*nslots/imm r2, 0                    ; row535\/push316 pass exact 8*nslots/'
  fixed_decimal_emitters_build_tooth fixed-decimal-drop-offset-argument \
    's/imm r2, 2                    ; rows539\/541 pass exact 8+8\*k/imm r2, 0                    ; rows539\/541 pass exact 8+8*k/'
  fixed_decimal_emitters_build_tooth fixed-decimal-drop-register-argument \
    's/imm r2, 3                    ; local123\/push318 pass exact k/imm r2, 0                    ; local123\/push318 pass exact k/'
  fixed_decimal_emitters_build_tooth fixed-decimal-reverse-child-order \
    's/imm r2, 2                    ; both children in source order/imm r2, 1                    ; both children in source order/'
  fixed_decimal_emitters_build_tooth fixed-decimal-wrong-prologue-total \
    's/imm r21, 65                  ; fixed bytes, excluding decimal digits/imm r21, 64                  ; fixed bytes, excluding decimal digits/'
  fixed_decimal_emitters_build_tooth fixed-decimal-wrong-parameter-total \
    's/imm r21, 40                  ; fixed bytes, excluding decimal digits/imm r21, 39                  ; fixed bytes, excluding decimal digits/'
  fixed_decimal_emitters_build_tooth fixed-decimal-drop-parameter-frame \
    's/store r1, r2                  ; synthetic epilogue restores caller/store r1, r1                  ; synthetic epilogue restores caller/'
}

fixed_decimal_emitters_reject_teeth() {
  for fixed_decimal_tooth_name in \
    fixed-decimal-wrong-guard \
    fixed-decimal-wrong-prologue-continuation \
    fixed-decimal-wrong-offset-continuation \
    fixed-decimal-wrong-register-continuation \
    fixed-decimal-wrong-first-param-emit \
    fixed-decimal-wrong-offset-add \
    fixed-decimal-wrong-register-push \
    fixed-decimal-event-undercount \
    fixed-decimal-primitive-undercount \
    fixed-decimal-store-undercount \
    fixed-decimal-omit-nslots-1024 \
    fixed-decimal-omit-k-3 \
    fixed-decimal-drop-prologue-argument \
    fixed-decimal-drop-offset-argument \
    fixed-decimal-drop-register-argument \
    fixed-decimal-reverse-child-order \
    fixed-decimal-wrong-prologue-total \
    fixed-decimal-wrong-parameter-total \
    fixed-decimal-drop-parameter-frame
  do
    set +e
    "$T/$fixed_decimal_tooth_name" < "$T/control.bundle" > "$T/stdout"
    fixed_decimal_tooth_status=$?
    set -e
    if [ "$fixed_decimal_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $fixed_decimal_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
