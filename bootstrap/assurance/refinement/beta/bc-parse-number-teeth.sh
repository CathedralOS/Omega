#!/usr/bin/env sh
# Phase-isolated canaries for procedure33 parse_number shape and meaning.

parse_number_build_tooth() { # name sed-expression
  parse_number_tooth_name=$1
  parse_number_tooth_sed=$2
  sed "$parse_number_tooth_sed" "$T/control-check.alpha" \
    > "$T/$parse_number_tooth_name.alpha"
  "$ASM" < "$T/$parse_number_tooth_name.alpha" \
    > "$T/$parse_number_tooth_name.tape"
  stamp_seed "$T/$parse_number_tooth_name.tape" "$SEED" \
    "$T/$parse_number_tooth_name" >/dev/null
}

parse_number_build_teeth() {
  parse_number_build_tooth parse-number-wrong-digit-guard \
    's/imm r24, 23868              ; checked digit-true guard/imm r24, 23869              ; checked digit-true guard/'
  parse_number_build_tooth parse-number-wrong-backedge-pc \
    's/imm r24, 24116              ; checked body backedge/imm r24, 24117              ; checked body backedge/'
  parse_number_build_tooth parse-number-wrong-cbyte-ambient \
    's/imm r24, 1                  ; outer addition left is live/imm r24, 0                  ; outer addition left is live/'
  parse_number_build_tooth parse-number-wrong-local-store \
    's/imm r24, 24088              ; updated v/imm r24, 24089              ; updated v/'
  parse_number_build_tooth parse-number-event-undercount \
    's/imm r29, 273                 ; exclusive event row/imm r29, 272                 ; exclusive event row/'
  parse_number_build_tooth parse-number-primitive-undercount \
    's/imm r23, 432                 ; exclusive primitive row/imm r23, 431                 ; exclusive primitive row/'
  parse_number_build_tooth parse-number-store-undercount \
    's/imm r24, 8                   ; prologue\/local\/argument\/binary stores/imm r24, 7                   ; prologue\/local\/argument\/binary stores/'
  parse_number_build_tooth parse-number-call-undercount \
    's/imm r22, 4                   ; cbyte\/is_digit\/cbyte\/adv/imm r22, 3                   ; cbyte\/is_digit\/cbyte\/adv/'
  parse_number_build_tooth parse-number-ret-undercount \
    's/imm r23, 2                   ; parse_number explicit and synthetic return/imm r23, 1                   ; parse_number explicit and synthetic return/'
  parse_number_build_tooth parse-number-wrong-digit-upper \
    's/imm r1, 58                   ; checked exclusive digit upper bound/imm r1, 57                   ; checked exclusive digit upper bound/'
  parse_number_build_tooth parse-number-wrong-digit-offset \
    's/imm r1, 48                   ; checked exact digit offset/imm r1, 49                   ; checked exact digit offset/'
  parse_number_build_tooth parse-number-wrong-wrap-probe \
    's/imm r1, 6                    ; checked zero-wrap probe digit/imm r1, 5                    ; checked zero-wrap probe digit/'
  parse_number_build_tooth parse-number-wrong-wrap-count \
    's/imm r2, 2                    ; exact two word-wrap probes completed/imm r2, 1                    ; exact two word-wrap probes completed/'
  parse_number_build_tooth parse-number-drop-true-in-range \
    's/imm r2, 1                    ; IDIG true entails j<LEN and byte in 48..57/imm r2, 0                    ; IDIG true entails j<LEN and byte in 48..57/'
  parse_number_build_tooth parse-number-drop-same-cursor \
    's/imm r2, 1                    ; no cursor write between observations/imm r2, 0                    ; no cursor write between observations/'
  parse_number_build_tooth parse-number-drop-second-cbyte \
    's/store r1, r2                  ; second CBLE sees same j and byte/store r1, r1                  ; second CBLE sees same j and byte/'
  parse_number_build_tooth parse-number-drop-recurrence \
    "s/store r1, r2                  ; v'=(10\*v+d) mod 2\^64/store r1, r1                  ; v'=(10*v+d) mod 2^64/"
  parse_number_build_tooth parse-number-claim-nonwrapping \
    's/store r1, r2                  ; wrapping word semantics, no nonwrap claim/store r1, r1                  ; wrapping word semantics, no nonwrap claim/'
  parse_number_build_tooth parse-number-drop-successor \
    's/store r1, r2                  ; ADVE successor cursor j+1<=LEN/store r1, r1                  ; ADVE successor cursor j+1<=LEN/'
  parse_number_build_tooth parse-number-drop-rank \
    's/store r1, r2                  ; successor rank LEN-(j+1)/store r1, r1                  ; successor rank LEN-(j+1)/'
  parse_number_build_tooth parse-number-drop-return-value \
    's/store r1, r2                  ; return word equals current fold\/slot0/store r1, r1                  ; return word equals current fold\/slot0/'
  parse_number_build_tooth parse-number-consume-stop-cursor \
    's/imm r2, 1                    ; exit cursor is current j/imm r2, 0                    ; exit cursor is current j/'
  parse_number_build_tooth parse-number-drop-restored-frame \
    's/store r1, r2                  ; parse_number caller frame restored/store r1, r1                  ; parse_number caller frame restored/'
  parse_number_build_tooth parse-number-drop-backedge-rename \
    's/imm r2, 2                    ; checked successor-to-current rename/imm r2, 1                    ; checked successor-to-current rename/'
}

parse_number_reject_teeth() {
  for parse_number_tooth_name in \
    parse-number-wrong-digit-guard \
    parse-number-wrong-backedge-pc \
    parse-number-wrong-cbyte-ambient \
    parse-number-wrong-local-store \
    parse-number-event-undercount \
    parse-number-primitive-undercount \
    parse-number-store-undercount \
    parse-number-call-undercount \
    parse-number-ret-undercount \
    parse-number-wrong-digit-upper \
    parse-number-wrong-digit-offset \
    parse-number-wrong-wrap-probe \
    parse-number-wrong-wrap-count \
    parse-number-drop-true-in-range \
    parse-number-drop-same-cursor \
    parse-number-drop-second-cbyte \
    parse-number-drop-recurrence \
    parse-number-claim-nonwrapping \
    parse-number-drop-successor \
    parse-number-drop-rank \
    parse-number-drop-return-value \
    parse-number-consume-stop-cursor \
    parse-number-drop-restored-frame \
    parse-number-drop-backedge-rename
  do
    set +e
    "$T/$parse_number_tooth_name" < "$T/control.bundle" > "$T/stdout"
    parse_number_tooth_status=$?
    set -e
    if [ "$parse_number_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $parse_number_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
