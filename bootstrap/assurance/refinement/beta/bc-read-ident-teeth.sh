#!/usr/bin/env sh
# Extracted Checker-A historical canaries for read-ident.

read_ident_build_teeth() {
# Phase-isolated read_ident teeth break exact calls/argument flow, fixed-global
# addresses, subtraction, table closure, or the terminating scan relation.
sed 's/imm r26, 5303               ; checked read_ident cbyte continuation/imm r26, 5304               ; checked read_ident cbyte continuation/' \
  "$T/control-check.alpha" > "$T/read-ident-wrong-cbyte-continuation.alpha"
"$ASM" < "$T/read-ident-wrong-cbyte-continuation.alpha" > "$T/read-ident-wrong-cbyte-continuation.tape"
stamp_seed "$T/read-ident-wrong-cbyte-continuation.tape" "$SEED" "$T/read-ident-wrong-cbyte-continuation" >/dev/null
sed 's/imm r26, 5344               ; checked read_ident alnum continuation/imm r26, 5345               ; checked read_ident alnum continuation/' \
  "$T/control-check.alpha" > "$T/read-ident-wrong-alnum-continuation.alpha"
"$ASM" < "$T/read-ident-wrong-alnum-continuation.alpha" > "$T/read-ident-wrong-alnum-continuation.tape"
stamp_seed "$T/read-ident-wrong-alnum-continuation.tape" "$SEED" "$T/read-ident-wrong-alnum-continuation" >/dev/null
sed 's/imm r23, 5303               ; checked cbyte-to-alnum argument/imm r23, 5304               ; checked cbyte-to-alnum argument/' \
  "$T/control-check.alpha" > "$T/read-ident-wrong-argument.alpha"
"$ASM" < "$T/read-ident-wrong-argument.alpha" > "$T/read-ident-wrong-argument.tape"
stamp_seed "$T/read-ident-wrong-argument.tape" "$SEED" "$T/read-ident-wrong-argument" >/dev/null
sed 's/imm r23, 2097120             ; checked IDOFF address/imm r23, 2097121             ; checked IDOFF address/' \
  "$T/control-check.alpha" > "$T/read-ident-wrong-idoff.alpha"
"$ASM" < "$T/read-ident-wrong-idoff.alpha" > "$T/read-ident-wrong-idoff.tape"
stamp_seed "$T/read-ident-wrong-idoff.tape" "$SEED" "$T/read-ident-wrong-idoff" >/dev/null
sed 's/imm r23, 2097112             ; checked IDLEN address/imm r23, 2097113             ; checked IDLEN address/' \
  "$T/control-check.alpha" > "$T/read-ident-wrong-idlen.alpha"
"$ASM" < "$T/read-ident-wrong-idlen.alpha" > "$T/read-ident-wrong-idlen.tape"
stamp_seed "$T/read-ident-wrong-idlen.tape" "$SEED" "$T/read-ident-wrong-idlen" >/dev/null
sed 's/imm r23, 4                   ; checked CUR-IDOFF subtraction/imm r23, 3                   ; checked CUR-IDOFF subtraction/' \
  "$T/control-check.alpha" > "$T/read-ident-wrong-subtraction.alpha"
"$ASM" < "$T/read-ident-wrong-subtraction.alpha" > "$T/read-ident-wrong-subtraction.tape"
stamp_seed "$T/read-ident-wrong-subtraction.tape" "$SEED" "$T/read-ident-wrong-subtraction" >/dev/null
sed 's/imm r25, 21                  ; checked exclusive read_ident memory row/imm r25, 20                  ; checked exclusive read_ident memory row/' \
  "$T/control-check.alpha" > "$T/read-ident-memory-undercount.alpha"
"$ASM" < "$T/read-ident-memory-undercount.alpha" > "$T/read-ident-memory-undercount.tape"
stamp_seed "$T/read-ident-memory-undercount.tape" "$SEED" "$T/read-ident-memory-undercount" >/dev/null
sed 's/imm r29, 37                  ; checked exclusive read_ident event row/imm r29, 36                  ; checked exclusive read_ident event row/' \
  "$T/control-check.alpha" > "$T/read-ident-event-undercount.alpha"
"$ASM" < "$T/read-ident-event-undercount.alpha" > "$T/read-ident-event-undercount.tape"
stamp_seed "$T/read-ident-event-undercount.tape" "$SEED" "$T/read-ident-event-undercount" >/dev/null
sed 's/imm r23, 95                  ; checked exclusive read_ident primitive row/imm r23, 94                  ; checked exclusive read_ident primitive row/' \
  "$T/control-check.alpha" > "$T/read-ident-primitive-undercount.alpha"
"$ASM" < "$T/read-ident-primitive-undercount.alpha" > "$T/read-ident-primitive-undercount.tape"
stamp_seed "$T/read-ident-primitive-undercount.tape" "$SEED" "$T/read-ident-primitive-undercount" >/dev/null
sed 's/imm r2, 1                    ; checked read_ident rank decrease/imm r2, 0                    ; checked read_ident rank decrease/' \
  "$T/control-check.alpha" > "$T/read-ident-zero-rank.alpha"
"$ASM" < "$T/read-ident-zero-rank.alpha" > "$T/read-ident-zero-rank.tape"
stamp_seed "$T/read-ident-zero-rank.tape" "$SEED" "$T/read-ident-zero-rank" >/dev/null
sed 's/imm r2, 2                    ; checked read_ident successor renaming/imm r2, 1                    ; checked read_ident successor renaming/' \
  "$T/control-check.alpha" > "$T/read-ident-wrong-rename.alpha"
"$ASM" < "$T/read-ident-wrong-rename.alpha" > "$T/read-ident-wrong-rename.tape"
stamp_seed "$T/read-ident-wrong-rename.tape" "$SEED" "$T/read-ident-wrong-rename" >/dev/null
sed 's/imm r2, 1                    ; checked first non-alnum\/end\/NUL stop/imm r2, 2                    ; checked first non-alnum\/end\/NUL stop/' \
  "$T/control-check.alpha" > "$T/read-ident-wrong-stop.alpha"
"$ASM" < "$T/read-ident-wrong-stop.alpha" > "$T/read-ident-wrong-stop.tape"
stamp_seed "$T/read-ident-wrong-stop.tape" "$SEED" "$T/read-ident-wrong-stop" >/dev/null

}

read_ident_reject_teeth() {
for read_ident_tooth in read-ident-wrong-cbyte-continuation read-ident-wrong-alnum-continuation read-ident-wrong-argument read-ident-wrong-idoff read-ident-wrong-idlen read-ident-wrong-subtraction read-ident-memory-undercount read-ident-event-undercount read-ident-primitive-undercount read-ident-zero-rank read-ident-wrong-rename read-ident-wrong-stop; do
  set +e
  "$T/$read_ident_tooth" < "$T/control.bundle" > "$T/stdout"
  read_ident_tooth_status=$?
  set -e
  if [ "$read_ident_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $read_ident_tooth was not rejected" >&2
    exit 1
  fi
done
}
