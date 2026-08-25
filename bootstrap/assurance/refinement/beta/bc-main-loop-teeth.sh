#!/usr/bin/env sh
# Extracted Checker-A historical canaries for main-loop.

main_loop_build_teeth() {
# Phase-isolated main.loop-entry teeth retain the complete root prefix and
# cbyte theorem while breaking the call, expression, guarded split, terminal
# payload, body cutpoint, or exhaustive row inventory only in this phase.
sed 's/imm r26, 51271               ; checked loop cbyte continuation/imm r26, 51272               ; checked loop cbyte continuation/' \
  "$T/control-check.alpha" > "$T/main-loop-wrong-continuation.alpha"
"$ASM" < "$T/main-loop-wrong-continuation.alpha" > "$T/main-loop-wrong-continuation.tape"
stamp_seed "$T/main-loop-wrong-continuation.tape" "$SEED" "$T/main-loop-wrong-continuation" >/dev/null
sed 's/imm r23, 11                  ; !=/imm r23, 10                  ; !=/' \
  "$T/control-check.alpha" > "$T/main-loop-wrong-comparison.alpha"
"$ASM" < "$T/main-loop-wrong-comparison.alpha" > "$T/main-loop-wrong-comparison.tape"
stamp_seed "$T/main-loop-wrong-comparison.tape" "$SEED" "$T/main-loop-wrong-comparison" >/dev/null
sed 's/imm r21, 51375               ; checked zero continuation/imm r21, 51376               ; checked zero continuation/' \
  "$T/control-check.alpha" > "$T/main-loop-wrong-zero-target.alpha"
"$ASM" < "$T/main-loop-wrong-zero-target.alpha" > "$T/main-loop-wrong-zero-target.tape"
stamp_seed "$T/main-loop-wrong-zero-target.tape" "$SEED" "$T/main-loop-wrong-zero-target" >/dev/null
sed 's/imm r2, 2                    ; logical-end relation CUR=LEN/imm r2, 1                    ; logical-end relation CUR=LEN/' \
  "$T/control-check.alpha" > "$T/main-loop-wrong-end-clause.alpha"
"$ASM" < "$T/main-loop-wrong-end-clause.alpha" > "$T/main-loop-wrong-end-clause.tape"
stamp_seed "$T/main-loop-wrong-end-clause.tape" "$SEED" "$T/main-loop-wrong-end-clause" >/dev/null
sed 's/imm r2, 1                    ; nonzero hit relation CUR<LEN/imm r2, 2                    ; nonzero hit relation CUR<LEN/' \
  "$T/control-check.alpha" > "$T/main-loop-wrong-nonzero-clause.alpha"
"$ASM" < "$T/main-loop-wrong-nonzero-clause.alpha" > "$T/main-loop-wrong-nonzero-clause.tape"
stamp_seed "$T/main-loop-wrong-nonzero-clause.tape" "$SEED" "$T/main-loop-wrong-nonzero-clause" >/dev/null
sed 's/imm r2, 0                    ; checked zero != zero result/imm r2, 1                    ; checked zero != zero result/' \
  "$T/control-check.alpha" > "$T/main-loop-wrong-zero-result.alpha"
"$ASM" < "$T/main-loop-wrong-zero-result.alpha" > "$T/main-loop-wrong-zero-result.tape"
stamp_seed "$T/main-loop-wrong-zero-result.tape" "$SEED" "$T/main-loop-wrong-zero-result" >/dev/null
sed 's/imm r2, 0                    ; checked concrete halt payload/imm r2, 1                    ; checked concrete halt payload/' \
  "$T/control-check.alpha" > "$T/main-loop-wrong-halt.alpha"
"$ASM" < "$T/main-loop-wrong-halt.alpha" > "$T/main-loop-wrong-halt.tape"
stamp_seed "$T/main-loop-wrong-halt.tape" "$SEED" "$T/main-loop-wrong-halt" >/dev/null
sed 's/imm r2, 51405                 ; checked main.body cutpoint/imm r2, 51406                 ; checked main.body cutpoint/' \
  "$T/control-check.alpha" > "$T/main-loop-wrong-body.alpha"
"$ASM" < "$T/main-loop-wrong-body.alpha" > "$T/main-loop-wrong-body.tape"
stamp_seed "$T/main-loop-wrong-body.tape" "$SEED" "$T/main-loop-wrong-body" >/dev/null
sed 's/imm r23, 812                 ; checked exclusive loop primitive row/imm r23, 811                 ; checked exclusive loop primitive row/' \
  "$T/control-check.alpha" > "$T/main-loop-primitive-undercount.alpha"
"$ASM" < "$T/main-loop-primitive-undercount.alpha" > "$T/main-loop-primitive-undercount.tape"
stamp_seed "$T/main-loop-primitive-undercount.tape" "$SEED" "$T/main-loop-primitive-undercount" >/dev/null
sed 's/imm r29, 610                 ; checked exclusive loop event row/imm r29, 609                 ; checked exclusive loop event row/' \
  "$T/control-check.alpha" > "$T/main-loop-event-undercount.alpha"
"$ASM" < "$T/main-loop-event-undercount.alpha" > "$T/main-loop-event-undercount.tape"
stamp_seed "$T/main-loop-event-undercount.tape" "$SEED" "$T/main-loop-event-undercount" >/dev/null
sed 's/imm r21, 1347636301            ; required conditional MLSP/imm r21, 1297238352            ; required conditional MLSP/' \
  "$T/control-check.alpha" > "$T/main-loop-wrong-generic.alpha"
"$ASM" < "$T/main-loop-wrong-generic.alpha" > "$T/main-loop-wrong-generic.tape"
stamp_seed "$T/main-loop-wrong-generic.tape" "$SEED" "$T/main-loop-wrong-generic" >/dev/null
sed 's/imm r27, 1                     ; checked root source bridge token/imm r27, 2                     ; checked root source bridge token/' \
  "$T/control-check.alpha" > "$T/main-loop-wrong-source-bridge.alpha"
"$ASM" < "$T/main-loop-wrong-source-bridge.alpha" > "$T/main-loop-wrong-source-bridge.tape"
stamp_seed "$T/main-loop-wrong-source-bridge.tape" "$SEED" "$T/main-loop-wrong-source-bridge" >/dev/null

}

main_loop_reject_teeth() {
for main_loop_tooth in main-loop-wrong-continuation main-loop-wrong-comparison main-loop-wrong-zero-target main-loop-wrong-end-clause main-loop-wrong-nonzero-clause main-loop-wrong-zero-result main-loop-wrong-halt main-loop-wrong-body main-loop-primitive-undercount main-loop-event-undercount main-loop-wrong-generic main-loop-wrong-source-bridge; do
  set +e
  "$T/$main_loop_tooth" < "$T/control.bundle" > "$T/stdout"
  main_loop_tooth_status=$?
  set -e
  if [ "$main_loop_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $main_loop_tooth was not rejected" >&2
    exit 1
  fi
done
}
