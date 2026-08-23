#!/usr/bin/env sh
# Phase-isolated canaries for procedure56 parse_char shape and meaning.

parse_char_build_tooth() { # name sed-expression
  parse_char_tooth_name=$1
  parse_char_tooth_sed=$2
  sed "$parse_char_tooth_sed" "$T/control-check.alpha" \
    > "$T/$parse_char_tooth_name.alpha"
  "$ASM" < "$T/$parse_char_tooth_name.alpha" \
    > "$T/$parse_char_tooth_name.tape"
  stamp_seed "$T/$parse_char_tooth_name.tape" "$SEED" \
    "$T/$parse_char_tooth_name" >/dev/null
}

parse_char_build_teeth() {
  parse_char_build_tooth parse-char-wrong-backslash-guard \
    's/imm r24, 37701              ; first byte is backslash/imm r24, 37702              ; first byte is backslash/'
  parse_char_build_tooth parse-char-wrong-zero-guard \
    's/imm r24, 38380              ; escaped zero/imm r24, 38381              ; escaped zero/'
  parse_char_build_tooth parse-char-wrong-tail-continuation \
    's/imm r26, 38464              ; checked second tail continuation/imm r26, 38465              ; checked second tail continuation/'
  parse_char_build_tooth parse-char-wrong-local-store \
    's/imm r24, 38418              ; zero escape -> 0/imm r24, 38419              ; zero escape -> 0/'
  parse_char_build_tooth parse-char-transition-undercount \
    's/imm r27, 211                 ; exclusive transition row/imm r27, 210                 ; exclusive transition row/'
  parse_char_build_tooth parse-char-event-undercount \
    's/imm r29, 421                 ; exclusive event row/imm r29, 420                 ; exclusive event row/'
  parse_char_build_tooth parse-char-primitive-undercount \
    's/imm r23, 622                 ; exclusive primitive row/imm r23, 621                 ; exclusive primitive row/'
  parse_char_build_tooth parse-char-wrong-empty-argument-boundary \
    's/imm r26, 335                 ; exact empty argument-push start/imm r26, 334                 ; exact empty argument-push start/'
  parse_char_build_tooth parse-char-call-undercount \
    's/imm r22, 6                   ; four adv plus two cbyte calls/imm r22, 5                   ; four adv plus two cbyte calls/'
  parse_char_build_tooth parse-char-ret-undercount \
    's/imm r23, 2                   ; parse_char explicit and synthetic return/imm r23, 1                   ; parse_char explicit and synthetic return/'
  parse_char_build_tooth parse-char-store-undercount \
    's/imm r24, 12                  ; prologue\/local\/binary stores/imm r24, 11                  ; prologue\/local\/binary stores/'

  parse_char_build_tooth parse-char-wrong-backslash-byte \
    's/imm r1, 92                   ; exact backslash discriminator/imm r1, 91                   ; exact backslash discriminator/'
  parse_char_build_tooth parse-char-wrong-n-mapping \
    's/imm r2, 10                   ; checked n mapping/imm r2, 11                   ; checked n mapping/'
  parse_char_build_tooth parse-char-wrong-t-mapping \
    's/imm r2, 9                    ; checked t mapping/imm r2, 10                    ; checked t mapping/'
  parse_char_build_tooth parse-char-wrong-r-mapping \
    's/imm r2, 13                   ; checked r mapping/imm r2, 12                   ; checked r mapping/'
  parse_char_build_tooth parse-char-wrong-zero-mapping \
    's/imm r2, 0                    ; checked 0 mapping/imm r2, 1                    ; checked 0 mapping/'
  parse_char_build_tooth parse-char-drop-default-preservation \
    's/imm r2, 256                  ; checked default preserves selected byte/imm r2, 255                  ; checked default preserves selected byte/'
  parse_char_build_tooth parse-char-tail-uses-adve \
    's/imm r21, 1094994008           ; both fin calls use ADVX, never ADVE/imm r21, 1094992965           ; both fin calls use ADVX, never ADVE/'
  parse_char_build_tooth parse-char-add-closing-validation \
    's/imm r2, 1                    ; no closing-quote premise or observation/imm r2, 0                    ; no closing-quote premise or observation/'
  parse_char_build_tooth parse-char-wrong-ordinary-delta \
    's/imm r2, 3                    ; exact ordinary final CUR=i+3/imm r2, 2                    ; exact ordinary final CUR=i+3/'
  parse_char_build_tooth parse-char-wrong-escape-delta \
    's/imm r2, 4                    ; exact escape final CUR=i+4/imm r2, 3                    ; exact escape final CUR=i+4/'
  parse_char_build_tooth parse-char-drop-ordinary-observation-join \
    's/imm r21, 1                   ; consumed ordinary first-observation join/imm r21, 2                   ; consumed ordinary first-observation join/'
  parse_char_build_tooth parse-char-drop-ordinary-branch-join \
    's/imm r21, 1                   ; consumed ordinary non-backslash join/imm r21, 2                   ; consumed ordinary non-backslash join/'
  parse_char_build_tooth parse-char-drop-ordinary-fin-join \
    's/imm r21, 1                   ; consumed ordinary fin-CUR join/imm r21, 2                   ; consumed ordinary fin-CUR join/'
  parse_char_build_tooth parse-char-drop-escape-observation-join \
    's/imm r21, 1                   ; consumed escape first-observation join/imm r21, 2                   ; consumed escape first-observation join/'
  parse_char_build_tooth parse-char-drop-escape-branch-join \
    's/imm r21, 2                   ; consumed escape backslash join/imm r21, 1                   ; consumed escape backslash join/'
  parse_char_build_tooth parse-char-drop-escape-fin-join \
    's/imm r21, 2                   ; consumed escape fin-CUR join/imm r21, 1                   ; consumed escape fin-CUR join/'
  parse_char_build_tooth parse-char-drop-escape-value-join \
    's/imm r21, 92                  ; consumed exact backslash value join/imm r21, 91                  ; consumed exact backslash value join/'
  parse_char_build_tooth parse-char-drop-ordinary-end \
    '/parse_char_prove_malformed_tails:/,/^        ret$/{s/imm r20, 1/imm r20, 0/;}'
  parse_char_build_tooth parse-char-drop-ordinary-no-close \
    '/parse_char_prove_malformed_tails:/,/^        ret$/{s/imm r20, 2/imm r20, 0/;}'
  parse_char_build_tooth parse-char-drop-escape-end \
    '/parse_char_prove_malformed_tails:/,/^        ret$/{s/imm r20, 3/imm r20, 0/;}'
  parse_char_build_tooth parse-char-drop-escape-no-close \
    '/parse_char_prove_malformed_tails:/,/^        ret$/{s/imm r20, 4/imm r20, 0/;}'
  parse_char_build_tooth parse-char-drop-first-boundary-zero \
    's/mov r21, r26                  ; consumed first hit\/boundary join/mov r21, r27                  ; consumed first hit\/boundary join/'
  parse_char_build_tooth parse-char-drop-first-inrange-nul \
    's/mov r21, r24                  ; consumed malformed first-byte join/imm r21, 1                    ; consumed malformed first-byte join/'
  parse_char_build_tooth parse-char-drop-escape-boundary-zero \
    's/mov r21, r23                  ; consumed second hit\/boundary join/imm r21, 1                    ; consumed second hit\/boundary join/'
  parse_char_build_tooth parse-char-drop-escape-inrange-nul \
    's/imm r21, 0                    ; consumed escaped-NUL value join/imm r21, 1                    ; consumed escaped-NUL value join/'
  parse_char_build_tooth parse-char-drop-malformed-result-path \
    's/mov r20, r29                 ; consumed malformed result-path cell/mov r20, r23                 ; consumed malformed result-path cell/'
  parse_char_build_tooth parse-char-drop-malformed-terminal-bound \
    's/mov r20, r30                 ; consumed malformed terminal-bound cell/mov r20, r23                 ; consumed malformed terminal-bound cell/'
}

parse_char_reject_teeth() {
  for parse_char_tooth_name in \
    parse-char-wrong-backslash-guard \
    parse-char-wrong-zero-guard \
    parse-char-wrong-tail-continuation \
    parse-char-wrong-local-store \
    parse-char-transition-undercount \
    parse-char-event-undercount \
    parse-char-primitive-undercount \
    parse-char-wrong-empty-argument-boundary \
    parse-char-call-undercount \
    parse-char-ret-undercount \
    parse-char-store-undercount \
    parse-char-wrong-backslash-byte \
    parse-char-wrong-n-mapping \
    parse-char-wrong-t-mapping \
    parse-char-wrong-r-mapping \
    parse-char-wrong-zero-mapping \
    parse-char-drop-default-preservation \
    parse-char-tail-uses-adve \
    parse-char-add-closing-validation \
    parse-char-wrong-ordinary-delta \
    parse-char-wrong-escape-delta \
    parse-char-drop-ordinary-observation-join \
    parse-char-drop-ordinary-branch-join \
    parse-char-drop-ordinary-fin-join \
    parse-char-drop-escape-observation-join \
    parse-char-drop-escape-branch-join \
    parse-char-drop-escape-fin-join \
    parse-char-drop-escape-value-join \
    parse-char-drop-ordinary-end \
    parse-char-drop-ordinary-no-close \
    parse-char-drop-escape-end \
    parse-char-drop-escape-no-close \
    parse-char-drop-first-boundary-zero \
    parse-char-drop-first-inrange-nul \
    parse-char-drop-escape-boundary-zero \
    parse-char-drop-escape-inrange-nul \
    parse-char-drop-malformed-result-path \
    parse-char-drop-malformed-terminal-bound
  do
    set +e
    "$T/$parse_char_tooth_name" < "$T/control.bundle" > "$T/stdout"
    parse_char_tooth_status=$?
    set -e
    if [ "$parse_char_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $parse_char_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
