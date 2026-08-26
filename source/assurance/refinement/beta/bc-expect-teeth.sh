#!/usr/bin/env sh
# Extracted Checker-A historical canaries for expect.

expect_build_teeth() {
# Phase-isolated expect teeth sever one call/local/comparison/census join or one
# side of the nonzero-delimiter cursor relation.
sed 's/imm r26, 17501               ; checked expect skip_ws continuation/imm r26, 17502               ; checked expect skip_ws continuation/' \
  "$T/control-check.alpha" > "$T/expect-wrong-skip-continuation.alpha"
"$ASM" < "$T/expect-wrong-skip-continuation.alpha" > "$T/expect-wrong-skip-continuation.tape"
stamp_seed "$T/expect-wrong-skip-continuation.tape" "$SEED" "$T/expect-wrong-skip-continuation" >/dev/null
sed 's/imm r26, 17510               ; checked expect cbyte continuation/imm r26, 17511               ; checked expect cbyte continuation/' \
  "$T/control-check.alpha" > "$T/expect-wrong-cbyte-continuation.alpha"
"$ASM" < "$T/expect-wrong-cbyte-continuation.alpha" > "$T/expect-wrong-cbyte-continuation.tape"
stamp_seed "$T/expect-wrong-cbyte-continuation.tape" "$SEED" "$T/expect-wrong-cbyte-continuation" >/dev/null
sed 's/imm r26, 17662               ; checked expect adv continuation/imm r26, 17663               ; checked expect adv continuation/' \
  "$T/control-check.alpha" > "$T/expect-wrong-adv-continuation.alpha"
"$ASM" < "$T/expect-wrong-adv-continuation.alpha" > "$T/expect-wrong-adv-continuation.tape"
stamp_seed "$T/expect-wrong-adv-continuation.tape" "$SEED" "$T/expect-wrong-adv-continuation" >/dev/null
sed 's/imm r23, 0                   ; checked expect ch slot/imm r23, 1                   ; checked expect ch slot/' \
  "$T/control-check.alpha" > "$T/expect-wrong-slot.alpha"
"$ASM" < "$T/expect-wrong-slot.alpha" > "$T/expect-wrong-slot.tape"
stamp_seed "$T/expect-wrong-slot.tape" "$SEED" "$T/expect-wrong-slot" >/dev/null
sed 's/imm r23, 10                  ; checked expect equality/imm r23, 11                  ; checked expect equality/' \
  "$T/control-check.alpha" > "$T/expect-wrong-comparison.alpha"
"$ASM" < "$T/expect-wrong-comparison.alpha" > "$T/expect-wrong-comparison.tape"
stamp_seed "$T/expect-wrong-comparison.tape" "$SEED" "$T/expect-wrong-comparison" >/dev/null
sed 's/imm r29, 165                 ; checked exclusive expect event row/imm r29, 164                 ; checked exclusive expect event row/' \
  "$T/control-check.alpha" > "$T/expect-event-undercount.alpha"
"$ASM" < "$T/expect-event-undercount.alpha" > "$T/expect-event-undercount.tape"
stamp_seed "$T/expect-event-undercount.tape" "$SEED" "$T/expect-event-undercount" >/dev/null
sed 's/imm r23, 356                 ; checked exclusive expect primitive row/imm r23, 355                 ; checked exclusive expect primitive row/' \
  "$T/control-check.alpha" > "$T/expect-primitive-undercount.alpha"
"$ASM" < "$T/expect-primitive-undercount.alpha" > "$T/expect-primitive-undercount.tape"
stamp_seed "$T/expect-primitive-undercount.tape" "$SEED" "$T/expect-primitive-undercount" >/dev/null
sed 's/imm r1, 526976/imm r1, 526968/' \
  "$T/control-check.alpha" > "$T/expect-drop-delimiter-premise.alpha"
"$ASM" < "$T/expect-drop-delimiter-premise.alpha" > "$T/expect-drop-delimiter-premise.tape"
stamp_seed "$T/expect-drop-delimiter-premise.tape" "$SEED" "$T/expect-drop-delimiter-premise" >/dev/null
sed 's/imm r2, 1                    ; checked mismatch preserves normalized CUR/imm r2, 2                    ; checked mismatch preserves normalized CUR/' \
  "$T/control-check.alpha" > "$T/expect-wrong-mismatch-cursor.alpha"
"$ASM" < "$T/expect-wrong-mismatch-cursor.alpha" > "$T/expect-wrong-mismatch-cursor.tape"
stamp_seed "$T/expect-wrong-mismatch-cursor.tape" "$SEED" "$T/expect-wrong-mismatch-cursor" >/dev/null
sed 's/imm r2, 2                    ; checked match consumes exactly one byte/imm r2, 1                    ; checked match consumes exactly one byte/' \
  "$T/control-check.alpha" > "$T/expect-wrong-match-cursor.alpha"
"$ASM" < "$T/expect-wrong-match-cursor.alpha" > "$T/expect-wrong-match-cursor.tape"
stamp_seed "$T/expect-wrong-match-cursor.tape" "$SEED" "$T/expect-wrong-match-cursor" >/dev/null
sed 's/imm r2, 1                    ; checked nonzero match entails CUR<LEN/imm r2, 0                    ; checked nonzero match entails CUR<LEN/' \
  "$T/control-check.alpha" > "$T/expect-drop-match-range.alpha"
"$ASM" < "$T/expect-drop-match-range.alpha" > "$T/expect-drop-match-range.tape"
stamp_seed "$T/expect-drop-match-range.tape" "$SEED" "$T/expect-drop-match-range" >/dev/null

}

expect_reject_teeth() {
for expect_tooth in expect-wrong-skip-continuation expect-wrong-cbyte-continuation expect-wrong-adv-continuation expect-wrong-slot expect-wrong-comparison expect-event-undercount expect-primitive-undercount expect-drop-delimiter-premise expect-wrong-mismatch-cursor expect-wrong-match-cursor expect-drop-match-range; do
  set +e
  "$T/$expect_tooth" < "$T/control.bundle" > "$T/stdout"
  expect_tooth_status=$?
  set -e
  if [ "$expect_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $expect_tooth was not rejected" >&2
    exit 1
  fi
done
}
