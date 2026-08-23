#!/usr/bin/env sh
# Phase-isolated canaries for procedure62's immediate boundary theorem.

gen_stmts_boundary_build_tooth() { # name sed-expression
  gen_stmts_boundary_tooth_name=$1
  gen_stmts_boundary_tooth_sed=$2
  sed "$gen_stmts_boundary_tooth_sed" "$T/control-check.alpha" \
    > "$T/$gen_stmts_boundary_tooth_name.alpha"
  "$ASM" < "$T/$gen_stmts_boundary_tooth_name.alpha" \
    > "$T/$gen_stmts_boundary_tooth_name.tape"
  stamp_seed "$T/$gen_stmts_boundary_tooth_name.tape" "$SEED" \
    "$T/$gen_stmts_boundary_tooth_name" >/dev/null
}

gen_stmts_boundary_build_teeth() {
  gen_stmts_boundary_build_tooth gen-stmts-wrong-depth-guard \
    's/imm r24, 44234              ; depth<64 guard/imm r24, 44235              ; depth<64 guard/'
  gen_stmts_boundary_build_tooth gen-stmts-wrong-resource-guard \
    's/imm r24, 44566              ; RESOURCE!=0 guard/imm r24, 44567              ; RESOURCE!=0 guard/'
  gen_stmts_boundary_build_tooth gen-stmts-wrong-body-guard \
    "s/imm r24, 44679              ; cbyte!='}' guard/imm r24, 44680              ; cbyte!='}' guard/"
  gen_stmts_boundary_build_tooth gen-stmts-wrong-eof-guard \
    's/imm r24, 44937              ; cbyte==0 guard/imm r24, 44938              ; cbyte==0 guard/'
  gen_stmts_boundary_build_tooth gen-stmts-wrong-child-continuation \
    's/imm r26, 44965              ; checked but not executed by summary/imm r26, 44966              ; checked but not executed by summary/'
  gen_stmts_boundary_build_tooth gen-stmts-wrong-resource-store \
    's/imm r24, 44305              ; exact depth-exhaustion resource store/imm r24, 44306              ; exact depth-exhaustion resource store/'
  gen_stmts_boundary_build_tooth gen-stmts-wrong-depth-increment \
    's/imm r24, 44447              ; exact admitted BLOCKDEPTH store/imm r24, 44448              ; exact admitted BLOCKDEPTH store/'
  gen_stmts_boundary_build_tooth gen-stmts-wrong-exhausted-decrement \
    's/imm r24, 45222              ; exact exhausted-path decrement store/imm r24, 45223              ; exact exhausted-path decrement store/'
  gen_stmts_boundary_build_tooth gen-stmts-event-undercount \
    's/imm r29, 508/imm r29, 507/'
  gen_stmts_boundary_build_tooth gen-stmts-primitive-undercount \
    's/imm r23, 725                 ; exclusive primitive row/imm r23, 724                 ; exclusive primitive row/'
  gen_stmts_boundary_build_tooth gen-stmts-ret-undercount \
    's/imm r23, 5                   ; four source plus synthetic ret/imm r23, 4                   ; four source plus synthetic ret/'
  gen_stmts_boundary_build_tooth gen-stmts-store-undercount \
    's/imm r24, 20                  ; prologue\/local\/push\/raw stores/imm r24, 19                  ; prologue\/local\/push\/raw stores/'

  gen_stmts_boundary_build_tooth gen-stmts-omit-depth64 \
    's/imm r1, 65                   ; checked depth sweep includes 64/imm r1, 64                   ; checked depth sweep includes 64/'
  gen_stmts_boundary_build_tooth gen-stmts-wrong-context-formula \
    's/imm r13, 63                  ; admitted remaining = 63-D/imm r13, 62                  ; admitted remaining = 63-D/'
  gen_stmts_boundary_build_tooth gen-stmts-drop-depth64-preservation \
    's/store r1, r2                 ; BLOCKDEPTH remains exact D=64/store r1, r1                 ; BLOCKDEPTH remains exact D=64/'
  gen_stmts_boundary_build_tooth gen-stmts-drop-admitted-depth \
    's/store r1, r2                 ; exact BLOCKDEPTH=D+1 in \[1,64\]/store r1, r1                 ; exact BLOCKDEPTH=D+1 in [1,64]/'
  gen_stmts_boundary_build_tooth gen-stmts-drop-skip \
    's/store r1, r2                 ; terminating skip_ws, no output/store r1, r1                 ; terminating skip_ws, no output/'
  gen_stmts_boundary_build_tooth gen-stmts-drop-resource-decrement \
    's/store r1, r2                 ; exact resource decrement restores D/store r1, r1                 ; exact resource decrement restores D/'
  gen_stmts_boundary_build_tooth gen-stmts-drop-close-advance \
    's/store r1, r2                 ; exact one-byte ADVE consumption/store r1, r1                 ; exact one-byte ADVE consumption/'
  gen_stmts_boundary_build_tooth gen-stmts-drop-eof-unconsumed \
    's/store r1, r2                 ; exact zero byte remains unconsumed/store r1, r1                 ; exact zero byte remains unconsumed/'
  gen_stmts_boundary_build_tooth gen-stmts-drop-child-nonzero \
    "s/imm r2, 3                    ; selected byte !=0 and !='}'/imm r2, 2                    ; selected byte !=0 and !='}'/"
  gen_stmts_boundary_build_tooth gen-stmts-drop-child-cutpoint \
    's/imm r2, 44956/imm r2, 44957/'
  gen_stmts_boundary_build_tooth gen-stmts-claim-child-outcome \
    's/imm r2, 1                    ; gen_stmt outcome deliberately unexecuted/imm r2, 0                    ; gen_stmt outcome deliberately unexecuted/'
  gen_stmts_boundary_build_tooth gen-stmts-drop-active-frame \
    's/imm r2, 1                    ; active-frame\/depth\/cursor cutpoint/imm r2, 0                    ; active-frame\/depth\/cursor cutpoint/'
}

gen_stmts_boundary_reject_teeth() {
  for gen_stmts_boundary_tooth_name in \
    gen-stmts-wrong-depth-guard \
    gen-stmts-wrong-resource-guard \
    gen-stmts-wrong-body-guard \
    gen-stmts-wrong-eof-guard \
    gen-stmts-wrong-child-continuation \
    gen-stmts-wrong-resource-store \
    gen-stmts-wrong-depth-increment \
    gen-stmts-wrong-exhausted-decrement \
    gen-stmts-event-undercount \
    gen-stmts-primitive-undercount \
    gen-stmts-ret-undercount \
    gen-stmts-store-undercount \
    gen-stmts-omit-depth64 \
    gen-stmts-wrong-context-formula \
    gen-stmts-drop-depth64-preservation \
    gen-stmts-drop-admitted-depth \
    gen-stmts-drop-skip \
    gen-stmts-drop-resource-decrement \
    gen-stmts-drop-close-advance \
    gen-stmts-drop-eof-unconsumed \
    gen-stmts-drop-child-nonzero \
    gen-stmts-drop-child-cutpoint \
    gen-stmts-claim-child-outcome \
    gen-stmts-drop-active-frame
  do
    set +e
    "$T/$gen_stmts_boundary_tooth_name" < "$T/control.bundle" > "$T/stdout"
    gen_stmts_boundary_tooth_status=$?
    set -e
    if [ "$gen_stmts_boundary_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $gen_stmts_boundary_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
