#!/usr/bin/env sh
# Extracted Checker-A historical canaries for main-ready.

main_ready_build_teeth() {
# Phase-isolated main.ready teeth preserve the successful root/slurp bridge and
# all three imported callee summaries while breaking one exact cutpoint, call,
# trace, cursor, transition, or exhaustive-effect join in only this phase.
sed 's/imm r20, 526440               ; checked imported ready clause/imm r20, 526448               ; checked imported ready clause/' \
  "$T/control-check.alpha" > "$T/main-ready-wrong-clause.alpha"
"$ASM" < "$T/main-ready-wrong-clause.alpha" > "$T/main-ready-wrong-clause.tape"
stamp_seed "$T/main-ready-wrong-clause.tape" "$SEED" "$T/main-ready-wrong-clause" >/dev/null
sed 's/imm r22, 51235               ; checked first continuation/imm r22, 51236               ; checked first continuation/' \
  "$T/control-check.alpha" > "$T/main-ready-wrong-continuation.alpha"
"$ASM" < "$T/main-ready-wrong-continuation.alpha" > "$T/main-ready-wrong-continuation.tape"
stamp_seed "$T/main-ready-wrong-continuation.tape" "$SEED" "$T/main-ready-wrong-continuation" >/dev/null
sed 's/imm r20, 525952               ; checked second theorem import/imm r20, 525944               ; checked second theorem import/' \
  "$T/control-check.alpha" > "$T/main-ready-wrong-summary.alpha"
"$ASM" < "$T/main-ready-wrong-summary.alpha" > "$T/main-ready-wrong-summary.tape"
stamp_seed "$T/main-ready-wrong-summary.tape" "$SEED" "$T/main-ready-wrong-summary" >/dev/null
sed 's/imm r3, 187                   ; checked composed prefix length/imm r3, 186                   ; checked composed prefix length/' \
  "$T/control-check.alpha" > "$T/main-ready-wrong-length.alpha"
"$ASM" < "$T/main-ready-wrong-length.alpha" > "$T/main-ready-wrong-length.tape"
stamp_seed "$T/main-ready-wrong-length.tape" "$SEED" "$T/main-ready-wrong-length" >/dev/null
sed 's/imm r21, 2                     ; checked skip emits epsilon/imm r21, 1                     ; checked skip emits epsilon/' \
  "$T/control-check.alpha" > "$T/main-ready-wrong-order.alpha"
"$ASM" < "$T/main-ready-wrong-order.alpha" > "$T/main-ready-wrong-order.tape"
stamp_seed "$T/main-ready-wrong-order.tape" "$SEED" "$T/main-ready-wrong-order" >/dev/null
sed 's/imm r2, 51262                 ; checked ready->loop target/imm r2, 51263                 ; checked ready->loop target/' \
  "$T/control-check.alpha" > "$T/main-ready-wrong-target.alpha"
"$ASM" < "$T/main-ready-wrong-target.alpha" > "$T/main-ready-wrong-target.tape"
stamp_seed "$T/main-ready-wrong-target.tape" "$SEED" "$T/main-ready-wrong-target" >/dev/null
sed 's/imm r29, 608                 ; checked exclusive ready event row/imm r29, 607                 ; checked exclusive ready event row/' \
  "$T/control-check.alpha" > "$T/main-ready-event-undercount.alpha"
"$ASM" < "$T/main-ready-event-undercount.alpha" > "$T/main-ready-event-undercount.tape"
stamp_seed "$T/main-ready-event-undercount.tape" "$SEED" "$T/main-ready-event-undercount" >/dev/null

}

main_ready_reject_teeth() {
for main_ready_tooth in main-ready-wrong-clause main-ready-wrong-continuation main-ready-wrong-summary main-ready-wrong-length main-ready-wrong-order main-ready-wrong-target main-ready-event-undercount; do
  set +e
  "$T/$main_ready_tooth" < "$T/control.bundle" > "$T/stdout"
  main_ready_tooth_status=$?
  set -e
  if [ "$main_ready_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $main_ready_tooth was not rejected" >&2
    exit 1
  fi
done
}
