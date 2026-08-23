#!/usr/bin/env sh
# Phase-isolated canaries for procedure59 cmp_op shape and meaning.

cmp_op_build_tooth() { # name sed-expression
  cmp_op_tooth_name=$1
  cmp_op_tooth_sed=$2
  sed "$cmp_op_tooth_sed" "$T/control-check.alpha" > "$T/$cmp_op_tooth_name.alpha"
  "$ASM" < "$T/$cmp_op_tooth_name.alpha" > "$T/$cmp_op_tooth_name.tape"
  stamp_seed "$T/$cmp_op_tooth_name.tape" "$SEED" "$T/$cmp_op_tooth_name" >/dev/null
}

cmp_op_build_teeth() {
  cmp_op_build_tooth cmp-op-wrong-block \
    '/cmp_op_shape_blocks:/,/call cmp_op_require_transitions/{s/imm r21, 40384/imm r21, 40385/;}'
  cmp_op_build_tooth cmp-op-wrong-lt-guard \
    's/imm r24, 39461                ; first byte/imm r24, 39462                ; first byte/'
  cmp_op_build_tooth cmp-op-wrong-bang-continuation \
    's/imm r26, 40153              ; unchecked bang tail continuation/imm r26, 40154              ; unchecked bang tail continuation/'
  cmp_op_build_tooth cmp-op-wrong-restore-local \
    's/imm r24, 40564              ; restored entry CUR/imm r24, 40565              ; restored entry CUR/'
  cmp_op_build_tooth cmp-op-wrong-restore-store \
    's/imm r24, 40599              ; restore CUR on single/imm r24, 40600              ; restore CUR on single/'
  cmp_op_build_tooth cmp-op-wrong-frame-slots \
    's/imm r22, 2                    ; locals c and saved i/imm r22, 1                    ; locals c and saved i/'
  cmp_op_build_tooth cmp-op-local-undercount \
    's/imm r23, 178                 ; exclusive local row/imm r23, 177                 ; exclusive local row/'
  cmp_op_build_tooth cmp-op-memory-undercount \
    's/imm r25, 57                  ; exclusive raw-memory row/imm r25, 56                  ; exclusive raw-memory row/'
  cmp_op_build_tooth cmp-op-transition-undercount \
    's/imm r27, 225                 ; exclusive transition row/imm r27, 224                 ; exclusive transition row/'
  cmp_op_build_tooth cmp-op-event-undercount \
    's/imm r29, 463                 ; exclusive event row/imm r29, 462                 ; exclusive event row/'
  cmp_op_build_tooth cmp-op-primitive-undercount \
    's/imm r23, 656                 ; exclusive primitive row/imm r23, 655                 ; exclusive primitive row/'
  cmp_op_build_tooth cmp-op-binary-push-undercount \
    's/imm r25, 194                 ; exclusive binary-push row/imm r25, 193                 ; exclusive binary-push row/'
  cmp_op_build_tooth cmp-op-wrong-empty-argument-boundary \
    's/imm r26, 339                 ; exact empty argument-push start/imm r26, 338                 ; exact empty argument-push start/'
  cmp_op_build_tooth cmp-op-store-push-undercount \
    's/imm r29, 388                 ; exact restore-CUR store push/imm r29, 387                 ; exact restore-CUR store push/'
  cmp_op_build_tooth cmp-op-call-undercount \
    's/imm r22, 12                  ; adv\/cbyte calls/imm r22, 11                  ; adv\/cbyte calls/'
  cmp_op_build_tooth cmp-op-ret-undercount \
    's/imm r23, 9                   ; eight explicit plus synthetic return/imm r23, 8                   ; eight explicit plus synthetic return/'
  cmp_op_build_tooth cmp-op-store-undercount \
    's/imm r24, 14                  ; prologue\/local\/push\/raw stores/imm r24, 13                  ; prologue\/local\/push\/raw stores/'

  cmp_op_build_tooth cmp-op-wrong-first-lt \
    's/imm r1, 60                   ; source/imm r1, 59                   ; source/'
  cmp_op_build_tooth cmp-op-wrong-lookahead-eq \
    's/imm r1, 61                   ; source lookahead/imm r1, 60                   ; source lookahead/'
  cmp_op_build_tooth cmp-op-wrong-first-complement \
    '/cmp_op_first_count_other:/,/cmp_op_first_publish:/{s/imm r1, 252/imm r1, 251/;}'
  cmp_op_build_tooth cmp-op-wrong-lookahead-complement \
    '/cmp_op_lookahead_count_no:/,/cmp_op_lookahead_publish:/{s/imm r1, 255/imm r1, 254/;}'
  cmp_op_build_tooth cmp-op-wrong-lt-result \
    '/cmp_op_path_lt:/,/cmp_op_path_le:/{s/imm r20, 0/imm r20, 1/;}'
  cmp_op_build_tooth cmp-op-wrong-le-result \
    '/cmp_op_path_le:/,/cmp_op_path_gt:/{s/imm r20, 4/imm r20, 3/;}'
  cmp_op_build_tooth cmp-op-wrong-gt-result \
    '/cmp_op_path_gt:/,/cmp_op_path_ge:/{s/imm r20, 1/imm r20, 0/;}'
  cmp_op_build_tooth cmp-op-wrong-ge-result \
    '/cmp_op_path_ge:/,/cmp_op_path_bang:/{s/imm r20, 5/imm r20, 4/;}'
  cmp_op_build_tooth cmp-op-wrong-eqeq-result \
    '/cmp_op_path_eqeq:/,/cmp_op_path_other:/{s/imm r20, 2/imm r20, 3/;}'
  cmp_op_build_tooth cmp-op-wrong-single-eq-result \
    '/cmp_op_path_single_eq:/,/cmp_op_path_eqeq:/{s/imm r20, 530280/imm r20, 530288/;}'
  cmp_op_build_tooth cmp-op-drop-single-eq-restore \
    's/imm r2, 1                    ; saved entry CUR restored/imm r2, 0                    ; saved entry CUR restored/'
  cmp_op_build_tooth cmp-op-bang-uses-adve \
    's/imm r21, 1094994008           ; consumed unconditional tail ADVX/imm r21, 1094992965           ; consumed unconditional tail ADVX/'
  cmp_op_build_tooth cmp-op-add-bang-lookahead \
    's/imm r21, 1                    ; consumed absence of second-byte check/imm r21, 0                    ; consumed absence of second-byte check/'
  cmp_op_build_tooth cmp-op-wrong-bang-delta \
    's/imm r2, 2                    ; exact bang CUR delta/imm r2, 1                    ; exact bang CUR delta/'
  cmp_op_build_tooth cmp-op-wrong-other-delta \
    's/imm r2, 0                    ; consumes nothing/imm r2, 1                    ; consumes nothing/'
  cmp_op_build_tooth cmp-op-collapse-first-zero-provenance \
    's/mov r21, r25                  ; consumed hit-vs-boundary provenance/imm r21, 1                    ; consumed hit-vs-boundary provenance/'
  cmp_op_build_tooth cmp-op-wrong-first-zero-value \
    's/imm r21, 0                    ; consumed exact zero value/imm r21, 1                    ; consumed exact zero value/'
  cmp_op_build_tooth cmp-op-collapse-second-zero-provenance \
    's/mov r21, r26                  ; consumed second hit\/boundary join/imm r21, 1                    ; consumed second hit\/boundary join/'
  cmp_op_build_tooth cmp-op-wrong-second-zero-value \
    's/imm r21, 0                    ; consumed exact zero lookahead/imm r21, 1                    ; consumed exact zero lookahead/'
  cmp_op_build_tooth cmp-op-drop-trailing-bang-gap \
    's/imm r21, 1                    ; consumed trailing-bang entry gap/imm r21, 0                    ; consumed trailing-bang entry gap/'
  cmp_op_build_tooth cmp-op-drop-restore-flow \
    's/imm r21, 1                    ; consumed load\/local\/push\/store chain/imm r21, 0                    ; consumed load\/local\/push\/store chain/'
  cmp_op_build_tooth cmp-op-drop-recognized-first-hit \
    's/imm r21, 1                    ; recognized first byte consumes CBLE hit/imm r21, 0                    ; recognized first byte consumes CBLE hit/'
  cmp_op_build_tooth cmp-op-drop-equal-lookahead-hit \
    "s/imm r21, 1                    ; '=' lookahead consumes CBLE hit/imm r21, 0                    ; '=' lookahead consumes CBLE hit/"
  cmp_op_build_tooth cmp-op-wrong-allones-probe \
    's/imm r3, 1                    ; independent allones-plus-one probe/imm r3, 2                    ; independent allones-plus-one probe/'
}

cmp_op_reject_teeth() {
  for cmp_op_tooth_name in \
    cmp-op-wrong-block cmp-op-wrong-lt-guard \
    cmp-op-wrong-bang-continuation cmp-op-wrong-restore-local \
    cmp-op-wrong-restore-store cmp-op-wrong-frame-slots \
    cmp-op-local-undercount cmp-op-memory-undercount \
    cmp-op-transition-undercount cmp-op-event-undercount \
    cmp-op-primitive-undercount cmp-op-binary-push-undercount \
    cmp-op-wrong-empty-argument-boundary cmp-op-store-push-undercount \
    cmp-op-call-undercount cmp-op-ret-undercount cmp-op-store-undercount \
    cmp-op-wrong-first-lt cmp-op-wrong-lookahead-eq \
    cmp-op-wrong-first-complement cmp-op-wrong-lookahead-complement \
    cmp-op-wrong-lt-result cmp-op-wrong-le-result cmp-op-wrong-gt-result \
    cmp-op-wrong-ge-result cmp-op-wrong-eqeq-result \
    cmp-op-wrong-single-eq-result cmp-op-drop-single-eq-restore \
    cmp-op-bang-uses-adve cmp-op-add-bang-lookahead cmp-op-wrong-bang-delta \
    cmp-op-wrong-other-delta cmp-op-collapse-first-zero-provenance \
    cmp-op-wrong-first-zero-value cmp-op-collapse-second-zero-provenance \
    cmp-op-wrong-second-zero-value cmp-op-drop-trailing-bang-gap \
    cmp-op-drop-restore-flow cmp-op-drop-recognized-first-hit \
    cmp-op-drop-equal-lookahead-hit cmp-op-wrong-allones-probe
  do
    set +e
    "$T/$cmp_op_tooth_name" < "$T/control.bundle" > "$T/stdout"
    cmp_op_tooth_status=$?
    set -e
    if [ "$cmp_op_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $cmp_op_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
