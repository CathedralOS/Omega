#!/usr/bin/env sh
# Extracted Checker-A historical canaries for declare.

declare_build_teeth() {
# Phase-isolated declare teeth break exact guard/value/table closure or one
# branch of the conditional insertion/resource relation.
sed 's/imm r24, 24313               ; checked declare room guard/imm r24, 24314               ; checked declare room guard/' \
  "$T/control-check.alpha" > "$T/declare-wrong-guard.alpha"
"$ASM" < "$T/declare-wrong-guard.alpha" > "$T/declare-wrong-guard.tape"
stamp_seed "$T/declare-wrong-guard.tape" "$SEED" "$T/declare-wrong-guard" >/dev/null
sed 's/imm r24, 24190               ; checked NLOC snapshot into s/imm r24, 24191               ; checked NLOC snapshot into s/' \
  "$T/control-check.alpha" > "$T/declare-wrong-snapshot.alpha"
"$ASM" < "$T/declare-wrong-snapshot.alpha" > "$T/declare-wrong-snapshot.tape"
stamp_seed "$T/declare-wrong-snapshot.tape" "$SEED" "$T/declare-wrong-snapshot" >/dev/null
sed 's/imm r23, 1024                ; checked declare capacity/imm r23, 1023                ; checked declare capacity/' \
  "$T/control-check.alpha" > "$T/declare-wrong-capacity.alpha"
"$ASM" < "$T/declare-wrong-capacity.alpha" > "$T/declare-wrong-capacity.tape"
stamp_seed "$T/declare-wrong-capacity.tape" "$SEED" "$T/declare-wrong-capacity" >/dev/null
sed 's/imm r23, 252                 ; checked declare exhaustion status/imm r23, 253                 ; checked declare exhaustion status/' \
  "$T/control-check.alpha" > "$T/declare-wrong-status.alpha"
"$ASM" < "$T/declare-wrong-status.alpha" > "$T/declare-wrong-status.tape"
stamp_seed "$T/declare-wrong-status.tape" "$SEED" "$T/declare-wrong-status" >/dev/null
sed 's/imm r23, 2097120             ; IDOFF payload/imm r23, 2097128             ; IDOFF payload/' \
  "$T/control-check.alpha" > "$T/declare-wrong-idoff.alpha"
"$ASM" < "$T/declare-wrong-idoff.alpha" > "$T/declare-wrong-idoff.tape"
stamp_seed "$T/declare-wrong-idoff.tape" "$SEED" "$T/declare-wrong-idoff" >/dev/null
sed 's/imm r23, 2097112             ; IDLEN payload/imm r23, 2097120             ; IDLEN payload/' \
  "$T/control-check.alpha" > "$T/declare-wrong-idlen.alpha"
"$ASM" < "$T/declare-wrong-idlen.alpha" > "$T/declare-wrong-idlen.tape"
stamp_seed "$T/declare-wrong-idlen.tape" "$SEED" "$T/declare-wrong-idlen" >/dev/null
sed 's/imm r25, 44                  ; checked exclusive declare memory row/imm r25, 43                  ; checked exclusive declare memory row/' \
  "$T/control-check.alpha" > "$T/declare-memory-undercount.alpha"
"$ASM" < "$T/declare-memory-undercount.alpha" > "$T/declare-memory-undercount.tape"
stamp_seed "$T/declare-memory-undercount.tape" "$SEED" "$T/declare-memory-undercount" >/dev/null
sed 's/imm r23, 451                 ; checked exclusive declare primitive row/imm r23, 450                 ; checked exclusive declare primitive row/' \
  "$T/control-check.alpha" > "$T/declare-primitive-undercount.alpha"
"$ASM" < "$T/declare-primitive-undercount.alpha" > "$T/declare-primitive-undercount.tape"
stamp_seed "$T/declare-primitive-undercount.tape" "$SEED" "$T/declare-primitive-undercount" >/dev/null
sed 's/imm r2, 1                    ; checked full-table return zero/imm r2, 2                    ; checked full-table return zero/' \
  "$T/control-check.alpha" > "$T/declare-wrong-full-return.alpha"
"$ASM" < "$T/declare-wrong-full-return.alpha" > "$T/declare-wrong-full-return.tape"
stamp_seed "$T/declare-wrong-full-return.tape" "$SEED" "$T/declare-wrong-full-return" >/dev/null
sed 's/imm r2, 1                    ; checked 0<=s<=1023 table index/imm r2, 0                    ; checked 0<=s<=1023 table index/' \
  "$T/control-check.alpha" > "$T/declare-drop-table-bound.alpha"
"$ASM" < "$T/declare-drop-table-bound.alpha" > "$T/declare-drop-table-bound.tape"
stamp_seed "$T/declare-drop-table-bound.tape" "$SEED" "$T/declare-drop-table-bound" >/dev/null
sed 's/imm r2, 2                    ; checked NLOC=s+1 in \[1,1024\]/imm r2, 1                    ; checked NLOC=s+1 in [1,1024]/' \
  "$T/control-check.alpha" > "$T/declare-wrong-nloc-update.alpha"
"$ASM" < "$T/declare-wrong-nloc-update.alpha" > "$T/declare-wrong-nloc-update.tape"
stamp_seed "$T/declare-wrong-nloc-update.tape" "$SEED" "$T/declare-wrong-nloc-update" >/dev/null
sed 's/imm r2, 2                    ; checked successful return is s/imm r2, 1                    ; checked successful return is s/' \
  "$T/control-check.alpha" > "$T/declare-wrong-room-return.alpha"
"$ASM" < "$T/declare-wrong-room-return.alpha" > "$T/declare-wrong-room-return.tape"
stamp_seed "$T/declare-wrong-room-return.tape" "$SEED" "$T/declare-wrong-room-return" >/dev/null

}

declare_reject_teeth() {
for declare_tooth in declare-wrong-guard declare-wrong-snapshot declare-wrong-capacity declare-wrong-status declare-wrong-idoff declare-wrong-idlen declare-memory-undercount declare-primitive-undercount declare-wrong-full-return declare-drop-table-bound declare-wrong-nloc-update declare-wrong-room-return; do
  set +e
  "$T/$declare_tooth" < "$T/control.bundle" > "$T/stdout"
  declare_tooth_status=$?
  set -e
  if [ "$declare_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $declare_tooth was not rejected" >&2
    exit 1
  fi
done
}
