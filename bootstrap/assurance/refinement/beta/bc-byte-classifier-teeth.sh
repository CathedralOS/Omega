#!/usr/bin/env sh
# Extracted Checker-A historical canaries for byte-classifier.

byte_classifier_build_teeth() {
# Phase-isolated byte-classifier teeth keep the exact artifact and prior root
# summaries fixed while breaking the byte premise, independent interval
# specification, call/argument joins, exact source boundary/opcode, or one
# whole-table census in the new shape/meaning pair.
sed 's/imm r2, 1                    ; checked 0<=c<=255 premise/imm r2, 0                    ; checked 0<=c<=255 premise/' \
  "$T/control-check.alpha" > "$T/classifier-drop-domain.alpha"
"$ASM" < "$T/classifier-drop-domain.alpha" > "$T/classifier-drop-domain.tape"
stamp_seed "$T/classifier-drop-domain.tape" "$SEED" "$T/classifier-drop-domain" >/dev/null
sed 's/imm r3, 58                   ; checked digit spec exclusive upper/imm r3, 57                   ; checked digit spec exclusive upper/' \
  "$T/control-check.alpha" > "$T/classifier-digit-spec-bound.alpha"
"$ASM" < "$T/classifier-digit-spec-bound.alpha" > "$T/classifier-digit-spec-bound.tape"
stamp_seed "$T/classifier-digit-spec-bound.tape" "$SEED" "$T/classifier-digit-spec-bound" >/dev/null
sed 's/imm r3, 91                   ; checked alpha spec uppercase exclusive/imm r3, 90                   ; checked alpha spec uppercase exclusive/' \
  "$T/control-check.alpha" > "$T/classifier-alpha-spec-bound.alpha"
"$ASM" < "$T/classifier-alpha-spec-bound.alpha" > "$T/classifier-alpha-spec-bound.tape"
stamp_seed "$T/classifier-alpha-spec-bound.tape" "$SEED" "$T/classifier-alpha-spec-bound" >/dev/null
sed 's/imm r2, 1                    ; checked digit handoff relation/imm r2, 2                    ; checked digit handoff relation/' \
  "$T/control-check.alpha" > "$T/classifier-wrong-handoff.alpha"
"$ASM" < "$T/classifier-wrong-handoff.alpha" > "$T/classifier-wrong-handoff.tape"
stamp_seed "$T/classifier-wrong-handoff.tape" "$SEED" "$T/classifier-wrong-handoff" >/dev/null
sed 's/imm r23, 12                  ; checked uppercase <=/imm r23, 8                   ; checked uppercase <=/' \
  "$T/control-check.alpha" > "$T/classifier-wrong-upper-op.alpha"
"$ASM" < "$T/classifier-wrong-upper-op.alpha" > "$T/classifier-wrong-upper-op.tape"
stamp_seed "$T/classifier-wrong-upper-op.tape" "$SEED" "$T/classifier-wrong-upper-op" >/dev/null
sed 's/imm r26, 3393               ; checked alpha continuation/imm r26, 3394               ; checked alpha continuation/' \
  "$T/control-check.alpha" > "$T/classifier-wrong-continuation.alpha"
"$ASM" < "$T/classifier-wrong-continuation.alpha" > "$T/classifier-wrong-continuation.tape"
stamp_seed "$T/classifier-wrong-continuation.tape" "$SEED" "$T/classifier-wrong-continuation" >/dev/null
sed 's/imm r23, 3516               ; checked digit argument handoff/imm r23, 3517               ; checked digit argument handoff/' \
  "$T/control-check.alpha" > "$T/classifier-wrong-argument.alpha"
"$ASM" < "$T/classifier-wrong-argument.alpha" > "$T/classifier-wrong-argument.tape"
stamp_seed "$T/classifier-wrong-argument.tape" "$SEED" "$T/classifier-wrong-argument" >/dev/null
sed 's/imm r23, 61                  ; checked exclusive classifier primitive row/imm r23, 60                  ; checked exclusive classifier primitive row/' \
  "$T/control-check.alpha" > "$T/classifier-primitive-undercount.alpha"
"$ASM" < "$T/classifier-primitive-undercount.alpha" > "$T/classifier-primitive-undercount.tape"
stamp_seed "$T/classifier-primitive-undercount.tape" "$SEED" "$T/classifier-primitive-undercount" >/dev/null
sed 's/imm r29, 19                  ; checked exclusive classifier event row/imm r29, 18                  ; checked exclusive classifier event row/' \
  "$T/control-check.alpha" > "$T/classifier-event-undercount.alpha"
"$ASM" < "$T/classifier-event-undercount.alpha" > "$T/classifier-event-undercount.tape"
stamp_seed "$T/classifier-event-undercount.tape" "$SEED" "$T/classifier-event-undercount" >/dev/null
sed 's/imm r23, 95                  ; checked underscore/imm r23, 94                  ; checked underscore/' \
  "$T/control-check.alpha" > "$T/classifier-wrong-underscore.alpha"
"$ASM" < "$T/classifier-wrong-underscore.alpha" > "$T/classifier-wrong-underscore.tape"
stamp_seed "$T/classifier-wrong-underscore.tape" "$SEED" "$T/classifier-wrong-underscore" >/dev/null

}

byte_classifier_reject_teeth() {
for classifier_tooth in classifier-drop-domain classifier-digit-spec-bound classifier-alpha-spec-bound classifier-wrong-handoff classifier-wrong-upper-op classifier-wrong-continuation classifier-wrong-argument classifier-primitive-undercount classifier-event-undercount classifier-wrong-underscore; do
  set +e
  "$T/$classifier_tooth" < "$T/control.bundle" > "$T/stdout"
  classifier_tooth_status=$?
  set -e
  if [ "$classifier_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $classifier_tooth was not rejected" >&2
    exit 1
  fi
done
}
