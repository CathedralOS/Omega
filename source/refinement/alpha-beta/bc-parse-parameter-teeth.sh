#!/usr/bin/env sh
# Phase-isolated negative canaries for the parse_proc parameter-loop proof.
# Sourced by bc-block-control.sh after T/ASM/SEED are established.

parse_parameter_build_tooth() { # name sed-expression
  parse_parameter_tooth_name=$1
  parse_parameter_tooth_sed=$2
  sed "$parse_parameter_tooth_sed" "$T/control-check.alpha" \
    > "$T/$parse_parameter_tooth_name.alpha"
  "$ASM" < "$T/$parse_parameter_tooth_name.alpha" \
    > "$T/$parse_parameter_tooth_name.tape"
  stamp_seed "$T/$parse_parameter_tooth_name.tape" "$SEED" \
    "$T/$parse_parameter_tooth_name" >/dev/null
}

parse_parameter_build_teeth() {
  # Exact procedure-68 parameter-loop shape and exhaustive row closure.
  parse_parameter_build_tooth parse-parameter-wrong-room-guard \
    's/imm r24, 49588              ; checked NLOC<4 guard/imm r24, 49589              ; checked NLOC<4 guard/'
  parse_parameter_build_tooth parse-parameter-wrong-declare-continuation \
    's/imm r26, 49710              ; checked declare continuation/imm r26, 49711              ; checked declare continuation/'
  parse_parameter_build_tooth parse-parameter-wrong-limit \
    's/imm r23, 4                   ; checked parameter limit/imm r23, 5                   ; checked parameter limit/'
  parse_parameter_build_tooth parse-parameter-wrong-comma-byte \
    's/imm r23, 44                  ; checked comma byte/imm r23, 45                  ; checked comma byte/'
  parse_parameter_build_tooth parse-parameter-event-undercount \
    's/imm r29, 596                 ; checked exclusive params event row/imm r29, 595                 ; checked exclusive params event row/'

  # Conditional meaning: room declaration, exact successor state, separator,
  # rank decrease/renaming, quiet state, and both terminal outcomes.
  parse_parameter_build_tooth parse-parameter-wrong-declare-token \
    's/imm r21, 1397506884           ; DCLS room theorem/imm r21, 1397506885           ; DCLS room theorem/'
  parse_parameter_build_tooth parse-parameter-drop-source-segment \
    's/imm r21, 1                    ; checked successful slurp source segment/imm r21, 0                    ; checked successful slurp source segment/'
  parse_parameter_build_tooth parse-parameter-wrong-successor \
    's/imm r2, 2                    ; checked parameter-count successor/imm r2, 1                    ; checked parameter-count successor/'
  parse_parameter_build_tooth parse-parameter-wrong-rank \
    's/store r1, r2                  ; checked rank decrease by one/store r1, r1                  ; checked rank decrease by one/'
  parse_parameter_build_tooth parse-parameter-wrong-comma-range \
    's/imm r2, 5                    ; comma is nonzero, hence CUR<LEN/imm r2, 4                    ; comma is nonzero, hence CUR<LEN/'
  parse_parameter_build_tooth parse-parameter-wrong-comma-normalization \
    's/imm r2, 2                    ; checked comma consumed \/ normalized/imm r2, 1                    ; checked comma consumed \/ normalized/'
  parse_parameter_build_tooth parse-parameter-wrong-noncomma-normalization \
    's/imm r2, 2                    ; checked direct normalized cursor/imm r2, 1                    ; checked direct normalized cursor/'
  parse_parameter_build_tooth parse-parameter-wrong-close-result \
    's/imm r2, 1                    ; successful loop exit at close/imm r2, 2                    ; successful loop exit at close/'
  parse_parameter_build_tooth parse-parameter-wrong-close-cursor \
    's/imm r21, 2                    ; close cursor\/result preserved/imm r21, 1                    ; close cursor\/result preserved/'
  parse_parameter_build_tooth parse-parameter-wrong-full-status \
    's/imm r2, 2                    ; RESOURCE_FAIL becomes numeric 252/imm r2, 1                    ; RESOURCE_FAIL becomes numeric 252/'
  parse_parameter_build_tooth parse-parameter-drop-full-table \
    's/imm r21, 1                    ; full path retains parameter prefix/imm r21, 2                    ; full path retains parameter prefix/'
  parse_parameter_build_tooth parse-parameter-drop-table-rename \
    's/store r1, r2                  ; parameter prefix renamed/store r1, r1                  ; parameter prefix renamed/'
  parse_parameter_build_tooth parse-parameter-drop-id-rename \
    's/store r1, r2                  ; current identifier state renamed/store r1, r1                  ; current identifier state renamed/'
}

parse_parameter_reject_teeth() {
  for parse_parameter_tooth_name in \
    parse-parameter-wrong-room-guard \
    parse-parameter-wrong-declare-continuation \
    parse-parameter-wrong-limit \
    parse-parameter-wrong-comma-byte \
    parse-parameter-event-undercount \
    parse-parameter-wrong-declare-token \
    parse-parameter-drop-source-segment \
    parse-parameter-wrong-successor \
    parse-parameter-wrong-rank \
    parse-parameter-wrong-comma-range \
    parse-parameter-wrong-comma-normalization \
    parse-parameter-wrong-noncomma-normalization \
    parse-parameter-wrong-close-result \
    parse-parameter-wrong-close-cursor \
    parse-parameter-wrong-full-status \
    parse-parameter-drop-full-table \
    parse-parameter-drop-table-rename \
    parse-parameter-drop-id-rename
  do
    set +e
    "$T/$parse_parameter_tooth_name" < "$T/control.bundle" > "$T/stdout"
    parse_parameter_tooth_status=$?
    set -e
    if [ "$parse_parameter_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $parse_parameter_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
