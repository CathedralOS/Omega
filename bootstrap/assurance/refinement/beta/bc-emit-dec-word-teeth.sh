#!/usr/bin/env sh
# Phase-isolated canaries for Checker D's full-Word emit_dec theorem.

emit_dec_word_build_tooth() { # name exact-old exact-new
  emit_dec_word_tooth_name=$1
  emit_dec_word_tooth_old=$2
  emit_dec_word_tooth_new=$3
  emit_dec_word_tooth_count=$(grep -F -c -- "$emit_dec_word_tooth_old" \
    "$T/emit-dec-word-check.alpha" || true)
  if [ "$emit_dec_word_tooth_count" != 1 ]; then
    echo "bc block control FAIL — $emit_dec_word_tooth_name anchor count $emit_dec_word_tooth_count" >&2
    exit 1
  fi
  awk -v old="$emit_dec_word_tooth_old" -v new="$emit_dec_word_tooth_new" '
    {
      at = index($0, old)
      if (at != 0) {
        $0 = substr($0, 1, at - 1) new substr($0, at + length(old))
      }
      print
    }
  ' "$T/emit-dec-word-check.alpha" > "$T/$emit_dec_word_tooth_name.alpha"
  "$ASM" < "$T/$emit_dec_word_tooth_name.alpha" \
    > "$T/$emit_dec_word_tooth_name.tape"
  stamp_seed "$T/$emit_dec_word_tooth_name.tape" "$SEED" \
    "$T/$emit_dec_word_tooth_name" >/dev/null
}

emit_dec_word_build_teeth() {
  emit_dec_word_build_tooth emit-dec-word-wrong-stack-bridge \
    'imm r2, 1464094803             ; SPDW' \
    'imm r2, 1464094802             ; SPDW'
  emit_dec_word_build_tooth emit-dec-word-wrong-guard \
    'imm r24, 29307              ; checked n>=10 recursive guard' \
    'imm r24, 29308              ; checked n>=10 recursive guard'
  emit_dec_word_build_tooth emit-dec-word-wrong-continuation \
    'imm r26, 29443               ; checked child continuation' \
    'imm r26, 29444               ; checked child continuation'
  emit_dec_word_build_tooth emit-dec-word-wrong-write \
    'imm r23, 29567              ; checked direct digit write' \
    'imm r23, 29568              ; checked direct digit write'
  emit_dec_word_build_tooth emit-dec-word-wrong-division \
    'imm r23, 6                   ; n/10' \
    'imm r23, 7                   ; n/10'
  emit_dec_word_build_tooth emit-dec-word-wrong-remainder \
    'imm r23, 7                   ; n%10' \
    'imm r23, 6                   ; n%10'
  emit_dec_word_build_tooth emit-dec-word-wrong-digit-constant \
    'imm r23, 48                  ; decimal digit zero' \
    'imm r23, 49                  ; decimal digit zero'
  emit_dec_word_build_tooth emit-dec-word-wrong-argument-push \
    'imm r23, 29402              ; recursive q argument' \
    'imm r23, 29403              ; recursive q argument'
  emit_dec_word_build_tooth emit-dec-word-primitive-undercount \
    'imm r23, 532                 ; exclusive primitive row' \
    'imm r23, 531                 ; exclusive primitive row'
  emit_dec_word_build_tooth emit-dec-word-argument-undercount \
    'imm r27, 316                 ; exclusive argument-push row' \
    'imm r27, 315                 ; exclusive argument-push row'
  emit_dec_word_build_tooth emit-dec-word-drop-division-count \
    'imm r25, 1                   ; division count' \
    'imm r25, 0                   ; division count'
  emit_dec_word_build_tooth emit-dec-word-store-undercount \
    'imm r1, 7                    ; checked exact target store count' \
    'imm r1, 6                    ; checked exact target store count'

  emit_dec_word_build_tooth emit-dec-word-wrong-prior-interval-high \
    'mov r26, r12                  ; preserve interval high across helpers' \
    'mov r26, r11                  ; preserve interval high across helpers'
  emit_dec_word_build_tooth emit-dec-word-wrong-negative-branch-metadata \
    'imm r17, 1                    ; negative false branch' \
    'imm r17, 0                    ; negative false branch'
  emit_dec_word_build_tooth emit-dec-word-wrong-positive-branch-metadata \
    'imm r17, 2                    ; regular positive true branch' \
    'imm r17, 1                    ; regular positive true branch'
  emit_dec_word_build_tooth emit-dec-word-wrong-int-min-byte \
    'imm r2, 40                    ; INT64_MIN emits byte 40' \
    'imm r2, 41                    ; INT64_MIN emits byte 40'
  emit_dec_word_build_tooth emit-dec-word-drop-signed-partition \
    'imm r21, 1                    ; complete signed Word partition' \
    'imm r21, 0                    ; complete signed Word partition'
  emit_dec_word_build_tooth emit-dec-word-drop-trap-exclusion \
    'store r1, r21                 ; fixed +10 excludes both trap cases' \
    'store r1, r1                  ; fixed +10 excludes both trap cases'
  emit_dec_word_build_tooth emit-dec-word-wrong-negative-minimum \
    'imm r21, 9223372036854775808  ; INT64_MIN' \
    'imm r21, 9223372036854775809  ; INT64_MIN'
  emit_dec_word_build_tooth emit-dec-word-wrong-negative-remainder-bound \
    'sub r21, r1                  ; signed -9 (avoid large-decimal encoding)' \
    'add r21, r1                  ; signed -9 (avoid large-decimal encoding)'
  emit_dec_word_build_tooth emit-dec-word-wrong-negative-length \
    'imm r21, 1                    ; negative output is exactly one byte' \
    'imm r21, 2                    ; negative output is exactly one byte'
  emit_dec_word_build_tooth emit-dec-word-drop-negative-no-child \
    'store r1, r2                  ; negative path executes no recursive call' \
    'store r1, r1                  ; negative path executes no recursive call'
  emit_dec_word_build_tooth emit-dec-word-drop-negative-no-division \
    'store r1, r2                  ; negative path executes no division' \
    'store r1, r1                  ; negative path executes no division'
  emit_dec_word_build_tooth emit-dec-word-invent-minus-prefix \
    'store r1, r2                  ; exact 48+srem(n,10), no minus prefix' \
    'store r1, r1                  ; exact 48+srem(n,10), no minus prefix'
  emit_dec_word_build_tooth emit-dec-word-wrong-small-trace \
    'imm r21, 1                    ; exact one-byte base trace' \
    'imm r21, 2                    ; exact one-byte base trace'
  emit_dec_word_build_tooth emit-dec-word-drop-q-handoff \
    'store r1, r2                  ; exact q passed by push315/event308' \
    'store r1, r1                  ; exact q passed by push315/event308'
  emit_dec_word_build_tooth emit-dec-word-drop-child-q \
    'store r1, r2                  ; same q consumed by child theorem' \
    'store r1, r1                  ; same q consumed by child theorem'
  emit_dec_word_build_tooth emit-dec-word-reverse-trace-order \
    'store r1, r2                  ; child trace precedes current digit' \
    'store r1, r1                  ; child trace precedes current digit'
  emit_dec_word_build_tooth emit-dec-word-drop-reconstruction \
    'store r1, r2                  ; selected n=10*q+r reconstruction' \
    'store r1, r1                  ; selected n=10*q+r reconstruction'
  emit_dec_word_build_tooth emit-dec-word-drop-remainder-relation \
    'store r1, r2                  ; selected signed remainder bound' \
    'store r1, r1                  ; selected signed remainder bound'
  emit_dec_word_build_tooth emit-dec-word-drop-quiet \
    'store r1, r21                 ; caller-visible state quiet for this row' \
    'store r1, r1                  ; caller-visible state quiet for this row'
  emit_dec_word_build_tooth emit-dec-word-drop-frame \
    'store r1, r21                 ; caller r15/r14 restored for this row' \
    'store r1, r1                  ; caller r15/r14 restored for this row'
  emit_dec_word_build_tooth emit-dec-word-drop-result-zero \
    'store r1, r21                 ; explicit source result is exact zero' \
    'store r1, r1                  ; explicit source result is exact zero'
  emit_dec_word_build_tooth emit-dec-word-wrong-maximum-rank \
    'store r1, r2                  ; maximum positive activation/output rank' \
    'store r1, r1                  ; maximum positive activation/output rank'
  emit_dec_word_build_tooth emit-dec-word-drop-three-way-join \
    'store r1, r2                  ; exhaustive signed three-way join' \
    'store r1, r1                  ; exhaustive signed three-way join'
  emit_dec_word_build_tooth emit-dec-word-wrong-publication \
    'imm r2, 1464026436            ; DECW' \
    'imm r2, 1464026435            ; DECW'
}

emit_dec_word_reject_teeth() {
  for emit_dec_word_tooth_name in \
    emit-dec-word-wrong-stack-bridge \
    emit-dec-word-wrong-guard \
    emit-dec-word-wrong-continuation \
    emit-dec-word-wrong-write \
    emit-dec-word-wrong-division \
    emit-dec-word-wrong-remainder \
    emit-dec-word-wrong-digit-constant \
    emit-dec-word-wrong-argument-push \
    emit-dec-word-primitive-undercount \
    emit-dec-word-argument-undercount \
    emit-dec-word-drop-division-count \
    emit-dec-word-store-undercount \
    emit-dec-word-wrong-prior-interval-high \
    emit-dec-word-wrong-negative-branch-metadata \
    emit-dec-word-wrong-positive-branch-metadata \
    emit-dec-word-wrong-int-min-byte \
    emit-dec-word-drop-signed-partition \
    emit-dec-word-drop-trap-exclusion \
    emit-dec-word-wrong-negative-minimum \
    emit-dec-word-wrong-negative-remainder-bound \
    emit-dec-word-wrong-negative-length \
    emit-dec-word-drop-negative-no-child \
    emit-dec-word-drop-negative-no-division \
    emit-dec-word-invent-minus-prefix \
    emit-dec-word-wrong-small-trace \
    emit-dec-word-drop-q-handoff \
    emit-dec-word-drop-child-q \
    emit-dec-word-reverse-trace-order \
    emit-dec-word-drop-reconstruction \
    emit-dec-word-drop-remainder-relation \
    emit-dec-word-drop-quiet \
    emit-dec-word-drop-frame \
    emit-dec-word-drop-result-zero \
    emit-dec-word-wrong-maximum-rank \
    emit-dec-word-drop-three-way-join \
    emit-dec-word-wrong-publication
  do
    set +e
    "$T/$emit_dec_word_tooth_name" < "$T/control.bundle" > "$T/stdout"
    emit_dec_word_tooth_status=$?
    set -e
    if [ "$emit_dec_word_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $emit_dec_word_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
