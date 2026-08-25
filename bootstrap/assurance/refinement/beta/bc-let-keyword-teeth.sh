#!/usr/bin/env sh
# Extracted Checker-A historical canaries for let-keyword.

let_keyword_build_teeth() {
# Phase-isolated let-keyword teeth break id_char bounds/addressing, one exact
# short-circuit row, or an exhaustive keyword theorem branch.
sed 's/imm r24, 5963               ; checked is_let length guard/imm r24, 5964               ; checked is_let length guard/' \
  "$T/control-check.alpha" > "$T/let-keyword-wrong-length-guard.alpha"
"$ASM" < "$T/let-keyword-wrong-length-guard.alpha" > "$T/let-keyword-wrong-length-guard.tape"
stamp_seed "$T/let-keyword-wrong-length-guard.tape" "$SEED" "$T/let-keyword-wrong-length-guard" >/dev/null
sed 's/imm r26, 6063               ; checked let\[0\] continuation/imm r26, 6064               ; checked let[0] continuation/' \
  "$T/control-check.alpha" > "$T/let-keyword-wrong-continuation.alpha"
"$ASM" < "$T/let-keyword-wrong-continuation.alpha" > "$T/let-keyword-wrong-continuation.tape"
stamp_seed "$T/let-keyword-wrong-continuation.tape" "$SEED" "$T/let-keyword-wrong-continuation" >/dev/null
sed 's/imm r23, 2097120             ; checked id_char IDOFF/imm r23, 2097112             ; checked id_char IDOFF/' \
  "$T/control-check.alpha" > "$T/let-keyword-wrong-idoff.alpha"
"$ASM" < "$T/let-keyword-wrong-idoff.alpha" > "$T/let-keyword-wrong-idoff.tape"
stamp_seed "$T/let-keyword-wrong-idoff.tape" "$SEED" "$T/let-keyword-wrong-idoff" >/dev/null
sed 's/imm r23, 2097112             ; checked is_let IDLEN/imm r23, 2097120             ; checked is_let IDLEN/' \
  "$T/control-check.alpha" > "$T/let-keyword-wrong-idlen.alpha"
"$ASM" < "$T/let-keyword-wrong-idlen.alpha" > "$T/let-keyword-wrong-idlen.tape"
stamp_seed "$T/let-keyword-wrong-idlen.tape" "$SEED" "$T/let-keyword-wrong-idlen" >/dev/null
sed 's/imm r23, 0                   ; checked let index zero/imm r23, 1                   ; checked let index zero/' \
  "$T/control-check.alpha" > "$T/let-keyword-wrong-index.alpha"
"$ASM" < "$T/let-keyword-wrong-index.alpha" > "$T/let-keyword-wrong-index.tape"
stamp_seed "$T/let-keyword-wrong-index.tape" "$SEED" "$T/let-keyword-wrong-index" >/dev/null
sed 's/imm r23, 108                 ; '\''l'\''/imm r23, 107                 ; '\''l'\''/' \
  "$T/control-check.alpha" > "$T/let-keyword-wrong-l.alpha"
"$ASM" < "$T/let-keyword-wrong-l.alpha" > "$T/let-keyword-wrong-l.tape"
stamp_seed "$T/let-keyword-wrong-l.tape" "$SEED" "$T/let-keyword-wrong-l" >/dev/null
sed 's/imm r23, 6392               ; checked let\[2\] argument/imm r23, 6393               ; checked let[2] argument/' \
  "$T/control-check.alpha" > "$T/let-keyword-wrong-argument.alpha"
"$ASM" < "$T/let-keyword-wrong-argument.alpha" > "$T/let-keyword-wrong-argument.tape"
stamp_seed "$T/let-keyword-wrong-argument.tape" "$SEED" "$T/let-keyword-wrong-argument" >/dev/null
sed 's/imm r29, 46                  ; checked exclusive is_let event row/imm r29, 45                  ; checked exclusive is_let event row/' \
  "$T/control-check.alpha" > "$T/let-keyword-event-undercount.alpha"
"$ASM" < "$T/let-keyword-event-undercount.alpha" > "$T/let-keyword-event-undercount.tape"
stamp_seed "$T/let-keyword-event-undercount.tape" "$SEED" "$T/let-keyword-event-undercount" >/dev/null
sed 's/imm r23, 116                 ; checked exclusive let primitive row/imm r23, 115                 ; checked exclusive let primitive row/' \
  "$T/control-check.alpha" > "$T/let-keyword-primitive-undercount.alpha"
"$ASM" < "$T/let-keyword-primitive-undercount.alpha" > "$T/let-keyword-primitive-undercount.tape"
stamp_seed "$T/let-keyword-primitive-undercount.tape" "$SEED" "$T/let-keyword-primitive-undercount" >/dev/null
sed 's/imm r2, 1                    ; checked call-specific k bound/imm r2, 0                    ; checked call-specific k bound/' \
  "$T/control-check.alpha" > "$T/let-keyword-drop-k-bound.alpha"
"$ASM" < "$T/let-keyword-drop-k-bound.alpha" > "$T/let-keyword-drop-k-bound.tape"
stamp_seed "$T/let-keyword-drop-k-bound.tape" "$SEED" "$T/let-keyword-drop-k-bound" >/dev/null
sed 's/imm r2, 1                    ; checked IDLEN != 3 short circuit/imm r2, 2                    ; checked IDLEN != 3 short circuit/' \
  "$T/control-check.alpha" > "$T/let-keyword-wrong-length-clause.alpha"
"$ASM" < "$T/let-keyword-wrong-length-clause.alpha" > "$T/let-keyword-wrong-length-clause.tape"
stamp_seed "$T/let-keyword-wrong-length-clause.tape" "$SEED" "$T/let-keyword-wrong-length-clause" >/dev/null
sed 's/store r1, r2                  ; checked exact let returns one/store r1, r1                  ; checked exact let returns one/' \
  "$T/control-check.alpha" > "$T/let-keyword-wrong-result.alpha"
"$ASM" < "$T/let-keyword-wrong-result.alpha" > "$T/let-keyword-wrong-result.tape"
stamp_seed "$T/let-keyword-wrong-result.tape" "$SEED" "$T/let-keyword-wrong-result" >/dev/null

}

let_keyword_reject_teeth() {
for let_keyword_tooth in let-keyword-wrong-length-guard let-keyword-wrong-continuation let-keyword-wrong-idoff let-keyword-wrong-idlen let-keyword-wrong-index let-keyword-wrong-l let-keyword-wrong-argument let-keyword-event-undercount let-keyword-primitive-undercount let-keyword-drop-k-bound let-keyword-wrong-length-clause let-keyword-wrong-result; do
  set +e
  "$T/$let_keyword_tooth" < "$T/control.bundle" > "$T/stdout"
  let_keyword_tooth_status=$?
  set -e
  if [ "$let_keyword_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $let_keyword_tooth was not rejected" >&2
    exit 1
  fi
done
}
