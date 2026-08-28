#!/usr/bin/env sh
# Phase-isolated negative canaries for emit_ident_at shape and meaning.
# Sourced by bc-block-control.sh after T/ASM/SEED are established.

emit_ident_build_tooth() { # name sed-expression
  emit_ident_tooth_name=$1
  emit_ident_tooth_sed=$2
  sed "$emit_ident_tooth_sed" "$T/control-check.alpha" \
    > "$T/$emit_ident_tooth_name.alpha"
  "$ASM" < "$T/$emit_ident_tooth_name.alpha" \
    > "$T/$emit_ident_tooth_name.tape"
  stamp_seed "$T/$emit_ident_tooth_name.tape" "$SEED" \
    "$T/$emit_ident_tooth_name" >/dev/null
}

emit_ident_build_teeth() {
  # Exact procedure 45 artifact joins.
  emit_ident_build_tooth emit-ident-wrong-guard \
    's/imm r24, 31677              ; checked k<len guard/imm r24, 31678              ; checked k<len guard/'
  emit_ident_build_tooth emit-ident-wrong-write \
    's/imm r23, 31853              ; checked direct write_byte lowering/imm r23, 31854              ; checked direct write_byte lowering/'
  emit_ident_build_tooth emit-ident-wrong-source-load \
    's/imm r24, 31850              ; byte\[SRC+off+k\]/imm r24, 31851              ; byte[SRC+off+k]/'
  emit_ident_build_tooth emit-ident-wrong-source-base \
    's/imm r23, 2097152             ; source arena base/imm r23, 2097153             ; source arena base/'
  emit_ident_build_tooth emit-ident-local-undercount \
    's/imm r23, 133                 ; exclusive local row/imm r23, 132                 ; exclusive local row/'
  emit_ident_build_tooth emit-ident-primitive-undercount \
    's/imm r23, 550                 ; exclusive primitive row/imm r23, 549                 ; exclusive primitive row/'
  emit_ident_build_tooth emit-ident-drop-direct-write-count \
    's/imm r24, 1                   ; one direct output/imm r24, 0                   ; one direct output/'

  # Slice premise, guard partition, one-byte step, rank, renaming, and terminal trace.
  emit_ident_build_tooth emit-ident-drop-slice-bound \
    's/imm r2, 1                    ; checked 0<=off<=off+len<=LEN/imm r2, 0                    ; checked 0<=off<=off+len<=LEN/'
  emit_ident_build_tooth emit-ident-drop-positive-guard \
    's/imm r2, 1                    ; checked selected positive guard k<len/imm r2, 0                    ; checked selected positive guard k<len/'
  emit_ident_build_tooth emit-ident-drop-false-guard \
    's/imm r2, 2                    ; checked selected false guard len<=k/imm r2, 0                    ; checked selected false guard len<=k/'
  emit_ident_build_tooth emit-ident-wrong-byte \
    's/imm r2, 1                    ; checked address and exact source byte/imm r2, 2                    ; checked address and exact source byte/'
  emit_ident_build_tooth emit-ident-drop-output-extension \
    's/imm r2, 2                    ; exact output prefix extended one byte/imm r2, 0                    ; exact output prefix extended one byte/'
  emit_ident_build_tooth emit-ident-zero-rank \
    's/store r1, r2                  ; checked len-k rank decreases by one/store r1, r1                  ; checked len-k rank decreases by one/'
  emit_ident_build_tooth emit-ident-wrong-successor \
    's/store r1, r2                  ; exact successor k+1<=len/store r1, r1                  ; exact successor k+1<=len/'
  emit_ident_build_tooth emit-ident-drop-output-rename \
    's/store r1, r2                  ; checked extended output renamed/store r1, r1                  ; checked extended output renamed/'
  emit_ident_build_tooth emit-ident-wrong-stop \
    's/imm r2, 1                    ; checked stop equality k=len/imm r2, 2                    ; checked stop equality k=len/'
  emit_ident_build_tooth emit-ident-wrong-result \
    's/imm r2, 1                    ; return zero \/ restored caller frame/imm r2, 2                    ; return zero \/ restored caller frame/'
}

emit_ident_reject_teeth() {
  for emit_ident_tooth_name in \
    emit-ident-wrong-guard \
    emit-ident-wrong-write \
    emit-ident-wrong-source-load \
    emit-ident-wrong-source-base \
    emit-ident-local-undercount \
    emit-ident-primitive-undercount \
    emit-ident-drop-direct-write-count \
    emit-ident-drop-slice-bound \
    emit-ident-drop-positive-guard \
    emit-ident-drop-false-guard \
    emit-ident-wrong-byte \
    emit-ident-drop-output-extension \
    emit-ident-zero-rank \
    emit-ident-wrong-successor \
    emit-ident-drop-output-rename \
    emit-ident-wrong-stop \
    emit-ident-wrong-result
  do
    set +e
    "$T/$emit_ident_tooth_name" < "$T/control.bundle" > "$T/stdout"
    emit_ident_tooth_status=$?
    set -e
    if [ "$emit_ident_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $emit_ident_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
