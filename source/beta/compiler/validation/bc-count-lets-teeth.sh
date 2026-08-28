#!/usr/bin/env sh
# Phase-isolated negative canaries for the count_lets proof modules.
# Sourced by bc-block-control.sh after T/ASM/SEED are established.

count_lets_build_tooth() { # name sed-expression
  count_lets_tooth_name=$1
  count_lets_tooth_sed=$2
  sed "$count_lets_tooth_sed" "$T/control-check.alpha" \
    > "$T/$count_lets_tooth_name.alpha"
  "$ASM" < "$T/$count_lets_tooth_name.alpha" \
    > "$T/$count_lets_tooth_name.tape"
  stamp_seed "$T/$count_lets_tooth_name.tape" "$SEED" \
    "$T/$count_lets_tooth_name" >/dev/null
}

count_lets_build_teeth() {
  # Exact procedure-39 control, calls, row closure, and data constants.
  count_lets_build_tooth count-lets-wrong-depth-guard \
    's/imm r24, 27659              ; checked positive-depth guard/imm r24, 27660              ; checked positive-depth guard/'
  count_lets_build_tooth count-lets-wrong-skip-continuation \
    's/imm r26, 27696              ; checked skip_ws continuation/imm r26, 27697              ; checked skip_ws continuation/'
  count_lets_build_tooth count-lets-wrong-char-continuation \
    's/imm r26, 28568              ; checked character skipper continuation/imm r26, 28569              ; checked character skipper continuation/'
  count_lets_build_tooth count-lets-wrong-islet-continuation \
    's/imm r26, 28821              ; checked is_let continuation/imm r26, 28822              ; checked is_let continuation/'
  count_lets_build_tooth count-lets-local-undercount \
    's/imm r23, 118                 ; checked exclusive count local row/imm r23, 117                 ; checked exclusive count local row/'
  count_lets_build_tooth count-lets-event-undercount \
    's/imm r29, 308                 ; checked exclusive count event row/imm r29, 307                 ; checked exclusive count event row/'
  count_lets_build_tooth count-lets-primitive-undercount \
    's/imm r23, 523                 ; checked exclusive count primitive row/imm r23, 522                 ; checked exclusive count primitive row/'
  count_lets_build_tooth count-lets-wrong-char-byte \
    's/imm r23, 39                  ; checked character quote byte/imm r23, 40                  ; checked character quote byte/'
  count_lets_build_tooth count-lets-wrong-restore-push \
    's/imm r23, 29039              ; checked CUR restore address push/imm r23, 29040              ; checked CUR restore address push/'

  # Out-of-range normalization and the exhaustive one-iteration relations.
  count_lets_build_tooth count-lets-drop-swsx-bound \
    's/imm r2, 1                    ; checked LEN<CUR<=LEN+2 premise/imm r2, 0                    ; checked LEN<CUR<=LEN+2 premise/'
  count_lets_build_tooth count-lets-wrong-swsx-preserve \
    's/store r1, r2                  ; CUR preserved; quiet\/restored frame/store r1, r1                  ; CUR preserved; quiet\/restored frame/'
  count_lets_build_tooth count-lets-drop-body-bound \
    's/imm r2, 1                    ; checked depth>0, CUR<=LEN+2, safe bounds/imm r2, 0                    ; checked depth>0, CUR<=LEN+2, safe bounds/'
  count_lets_build_tooth count-lets-wrong-outside-zero \
    's/imm r2, 1                    ; checked out-of-range cbyte zero/imm r2, 2                    ; checked out-of-range cbyte zero/'
  count_lets_build_tooth count-lets-wrong-open-depth \
    's/imm r2, 2                    ; checked depth+1, no overflow/imm r2, 1                    ; checked depth+1, no overflow/'
  count_lets_build_tooth count-lets-wrong-close-depth \
    's/imm r2, 3                    ; checked positive depth-1, possibly zero/imm r2, 2                    ; checked positive depth-1, possibly zero/'

  # Fixed-point rank, exact count, identifier carry, and done restoration.
  count_lets_build_tooth count-lets-wrong-nonlet-count \
    's/imm r2, 1                    ; exact count unchanged/imm r2, 2                    ; exact count unchanged/'
  count_lets_build_tooth count-lets-wrong-let-count \
    's/imm r2, 2                    ; checked exact count+1 only for ILET one/imm r2, 1                    ; checked exact count+1 only for ILET one/'
  count_lets_build_tooth count-lets-zero-rank \
    's/store r1, r2                  ; checked zero-exit rank witness/store r1, r1                  ; checked zero-exit rank witness/'
  count_lets_build_tooth count-lets-wrong-close-zero-rank \
    's/imm r2, 2                    ; checked close-zero rank decrease by two/imm r2, 1                    ; checked close-zero rank decrease by two/'
  count_lets_build_tooth count-lets-wrong-progress-rank \
    's/imm r2, 2                    ; checked composite-rank strict decrease/imm r2, 1                    ; checked composite-rank strict decrease/'
  count_lets_build_tooth count-lets-wrong-close-split \
    's/imm r2, 3                    ; checked old depth>1, new depth>0/imm r2, 4                    ; checked old depth>1, new depth>0/'
  count_lets_build_tooth count-lets-drop-id-rename \
    's/store r1, r2                  ; checked current identifier state renamed/store r1, r1                  ; checked current identifier state renamed/'
  count_lets_build_tooth count-lets-wrong-restoration \
    's/imm r2, 1                    ; checked CUR restored to entry start/imm r2, 2                    ; checked CUR restored to entry start/'
  count_lets_build_tooth count-lets-wrong-result \
    's/store r1, r2                  ; checked return exact let count/store r1, r1                  ; checked return exact let count/'
}

count_lets_reject_teeth() {
  for count_lets_tooth_name in \
    count-lets-wrong-depth-guard \
    count-lets-wrong-skip-continuation \
    count-lets-wrong-char-continuation \
    count-lets-wrong-islet-continuation \
    count-lets-local-undercount \
    count-lets-event-undercount \
    count-lets-primitive-undercount \
    count-lets-wrong-char-byte \
    count-lets-wrong-restore-push \
    count-lets-drop-swsx-bound \
    count-lets-wrong-swsx-preserve \
    count-lets-drop-body-bound \
    count-lets-wrong-outside-zero \
    count-lets-wrong-open-depth \
    count-lets-wrong-close-depth \
    count-lets-wrong-nonlet-count \
    count-lets-wrong-let-count \
    count-lets-zero-rank \
    count-lets-wrong-close-zero-rank \
    count-lets-wrong-progress-rank \
    count-lets-wrong-close-split \
    count-lets-drop-id-rename \
    count-lets-wrong-restoration \
    count-lets-wrong-result
  do
    set +e
    "$T/$count_lets_tooth_name" < "$T/control.bundle" > "$T/stdout"
    count_lets_tooth_status=$?
    set -e
    if [ "$count_lets_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $count_lets_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
