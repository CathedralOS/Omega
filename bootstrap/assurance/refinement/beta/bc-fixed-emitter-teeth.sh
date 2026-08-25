#!/usr/bin/env sh
# Extracted Checker-A historical canaries for fixed-emitter.

fixed_emitter_build_teeth() {
# Phase-isolated fixed-emitter teeth preserve every per-event helper clause but
# break source-order selection, the inter-event call continuation, or one exact
# procedure total in only the new concatenation phase.
sed 's/imm r20, 311                  ; checked first prelude event/imm r20, 312                  ; checked first prelude event/' \
  "$T/control-check.alpha" > "$T/fixed-emit-wrong-row.alpha"
"$ASM" < "$T/fixed-emit-wrong-row.alpha" > "$T/fixed-emit-wrong-row.tape"
stamp_seed "$T/fixed-emit-wrong-row.tape" "$SEED" "$T/fixed-emit-wrong-row" >/dev/null
sed 's/imm r3, 9                    ; checked call continuation width/imm r3, 8                    ; checked call continuation width/' \
  "$T/control-check.alpha" > "$T/fixed-emit-wrong-continuation.alpha"
"$ASM" < "$T/fixed-emit-wrong-continuation.alpha" > "$T/fixed-emit-wrong-continuation.tape"
stamp_seed "$T/fixed-emit-wrong-continuation.tape" "$SEED" "$T/fixed-emit-wrong-continuation" >/dev/null
sed 's/imm r22, 55                   ; checked prelude byte total/imm r22, 54                   ; checked prelude byte total/' \
  "$T/control-check.alpha" > "$T/fixed-emit-wrong-total.alpha"
"$ASM" < "$T/fixed-emit-wrong-total.alpha" > "$T/fixed-emit-wrong-total.tape"
stamp_seed "$T/fixed-emit-wrong-total.tape" "$SEED" "$T/fixed-emit-wrong-total" >/dev/null
sed 's/imm r23, 21226                ; checked exclusive end/imm r23, 21225                ; checked exclusive end/' \
  "$T/control-check.alpha" > "$T/fixed-emit-wrong-end.alpha"
"$ASM" < "$T/fixed-emit-wrong-end.alpha" > "$T/fixed-emit-wrong-end.tape"
stamp_seed "$T/fixed-emit-wrong-end.tape" "$SEED" "$T/fixed-emit-wrong-end" >/dev/null

}

fixed_emitter_reject_teeth() {
for fixed_emit_tooth in fixed-emit-wrong-row fixed-emit-wrong-continuation fixed-emit-wrong-total fixed-emit-wrong-end; do
  set +e
  "$T/$fixed_emit_tooth" < "$T/control.bundle" > "$T/stdout"
  fixed_emit_tooth_status=$?
  set -e
  if [ "$fixed_emit_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $fixed_emit_tooth was not rejected" >&2
    exit 1
  fi
done
}
