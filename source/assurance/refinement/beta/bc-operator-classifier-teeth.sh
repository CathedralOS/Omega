#!/usr/bin/env sh
# Phase-isolated canaries for procedures53..54 operator classifiers.

operator_classifier_build_tooth() { # name sed-expression
  operator_classifier_tooth_name=$1
  operator_classifier_tooth_sed=$2
  sed "$operator_classifier_tooth_sed" "$T/control-check.alpha" \
    > "$T/$operator_classifier_tooth_name.alpha"
  "$ASM" < "$T/$operator_classifier_tooth_name.alpha" \
    > "$T/$operator_classifier_tooth_name.tape"
  stamp_seed "$T/$operator_classifier_tooth_name.tape" "$SEED" \
    "$T/$operator_classifier_tooth_name" >/dev/null
}

operator_classifier_build_teeth() {
  operator_classifier_build_tooth operator-classifier-wrong-mul-block \
    's/imm r21, 35028/imm r21, 35029/'
  operator_classifier_build_tooth operator-classifier-wrong-plus-transition \
    's/imm r24, 35650                ; checked plus-true guard/imm r24, 35651                ; checked plus-true guard/'
  operator_classifier_build_tooth operator-classifier-wrong-false-return \
    's/imm r23, 35444                ; is_muldiv false return/imm r23, 35445                ; is_muldiv false return/'
  operator_classifier_build_tooth operator-classifier-wrong-frame-slots \
    's/imm r22, 1                    ; one total frame slot/imm r22, 2                    ; one total frame slot/'
  operator_classifier_build_tooth operator-classifier-local-undercount \
    's/imm r23, 149                 ; checked exclusive local row/imm r23, 148                 ; checked exclusive local row/'
  operator_classifier_build_tooth operator-classifier-transition-undercount \
    's/imm r27, 186                 ; checked exclusive transition row/imm r27, 185                 ; checked exclusive transition row/'
  operator_classifier_build_tooth operator-classifier-event-undercount \
    's/imm r29, 383                 ; checked exclusive event row/imm r29, 382                 ; checked exclusive event row/'
  operator_classifier_build_tooth operator-classifier-primitive-undercount \
    's/imm r23, 586                 ; checked exclusive primitive row/imm r23, 585                 ; checked exclusive primitive row/'
  operator_classifier_build_tooth operator-classifier-wrong-equality-code \
    's/imm r23, 10                   ; full-word ==/imm r23, 9                   ; full-word ==/'
  operator_classifier_build_tooth operator-classifier-push-undercount \
    's/imm r25, 172                 ; checked exclusive binary-push row/imm r25, 171                 ; checked exclusive binary-push row/'
  operator_classifier_build_tooth operator-classifier-ret-undercount \
    's/imm r23, 6                   ; four explicit plus two synthetic returns/imm r23, 5                   ; four explicit plus two synthetic returns/'
  operator_classifier_build_tooth operator-classifier-store-undercount \
    's/imm r24, 9                   ; prologue\/parameter\/binary stores/imm r24, 8                   ; prologue\/parameter\/binary stores/'

  operator_classifier_build_tooth operator-classifier-wrong-star \
    's/imm r3, 42                   ; checked source star discriminator/imm r3, 41                   ; checked source star discriminator/'
  operator_classifier_build_tooth operator-classifier-wrong-slash \
    's/imm r3, 47                   ; checked source slash discriminator/imm r3, 46                   ; checked source slash discriminator/'
  operator_classifier_build_tooth operator-classifier-wrong-percent \
    's/imm r3, 37                   ; checked source percent discriminator/imm r3, 36                   ; checked source percent discriminator/'
  operator_classifier_build_tooth operator-classifier-wrong-plus \
    's/imm r3, 43                   ; checked source plus discriminator/imm r3, 42                   ; checked source plus discriminator/'
  operator_classifier_build_tooth operator-classifier-wrong-minus \
    's/imm r3, 45                   ; checked source minus discriminator/imm r3, 44                   ; checked source minus discriminator/'
  operator_classifier_build_tooth operator-classifier-wrong-mul-count \
    's/imm r21, 3                    ; exactly/imm r21, 2                    ; exactly/'
  operator_classifier_build_tooth operator-classifier-wrong-add-complement \
    's/imm r21, 254                  ; exact byte complement/imm r21, 253                  ; exact byte complement/'
  operator_classifier_build_tooth operator-classifier-drop-byte-completion \
    's/imm r2, 1                    ; checked complete 256-byte sweep/imm r2, 0                    ; checked complete 256-byte sweep/'
  operator_classifier_build_tooth operator-classifier-drop-nonbyte-complement \
    's/imm r2, 1                    ; checked non-byte Word complement is zero/imm r2, 0                    ; checked non-byte Word complement is zero/'
  operator_classifier_build_tooth operator-classifier-drop-parameter-provenance \
    's/imm r2, 1                    ; checked parameter-slot provenance/imm r2, 0                    ; checked parameter-slot provenance/'
  operator_classifier_build_tooth operator-classifier-drop-quiet-frame \
    's/store r1, r2                  ; checked quiet\/restored relation/store r1, r1                  ; checked quiet\/restored relation/'
  operator_classifier_build_tooth operator-classifier-drop-word-total \
    's/imm r2, 1                    ; exhaustive Word partition complete/imm r2, 0                    ; exhaustive Word partition complete/'
}

operator_classifier_reject_teeth() {
  for operator_classifier_tooth_name in \
    operator-classifier-wrong-mul-block \
    operator-classifier-wrong-plus-transition \
    operator-classifier-wrong-false-return \
    operator-classifier-wrong-frame-slots \
    operator-classifier-local-undercount \
    operator-classifier-transition-undercount \
    operator-classifier-event-undercount \
    operator-classifier-primitive-undercount \
    operator-classifier-wrong-equality-code \
    operator-classifier-push-undercount \
    operator-classifier-ret-undercount \
    operator-classifier-store-undercount \
    operator-classifier-wrong-star \
    operator-classifier-wrong-slash \
    operator-classifier-wrong-percent \
    operator-classifier-wrong-plus \
    operator-classifier-wrong-minus \
    operator-classifier-wrong-mul-count \
    operator-classifier-wrong-add-complement \
    operator-classifier-drop-byte-completion \
    operator-classifier-drop-nonbyte-complement \
    operator-classifier-drop-parameter-provenance \
    operator-classifier-drop-quiet-frame \
    operator-classifier-drop-word-total
  do
    set +e
    "$T/$operator_classifier_tooth_name" < "$T/control.bundle" > "$T/stdout"
    operator_classifier_tooth_status=$?
    set -e
    if [ "$operator_classifier_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $operator_classifier_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
