#!/usr/bin/env sh
# Phase-isolated negative canaries for bounded emit_dec shape and meaning.
# Sourced by bc-block-control.sh after T/ASM/SEED are established.

emit_dec_build_tooth() { # name sed-expression
  emit_dec_tooth_name=$1
  emit_dec_tooth_sed=$2
  sed "$emit_dec_tooth_sed" "$T/control-check.alpha" \
    > "$T/$emit_dec_tooth_name.alpha"
  "$ASM" < "$T/$emit_dec_tooth_name.alpha" \
    > "$T/$emit_dec_tooth_name.tape"
  stamp_seed "$T/$emit_dec_tooth_name.tape" "$SEED" \
    "$T/$emit_dec_tooth_name" >/dev/null
}

emit_dec_build_induction_tooth() { # name sed-expression
  emit_dec_tooth_name=$1
  emit_dec_tooth_sed=$2
  sed "$emit_dec_tooth_sed" "$T/control-check.alpha" \
    > "$T/$emit_dec_tooth_name.raw.alpha"
  sed 's/call emit_dec_run_arithmetic_sweep/call emit_dec_seed_arithmetic_sweep/' \
    "$T/$emit_dec_tooth_name.raw.alpha" > "$T/$emit_dec_tooth_name.alpha"
  "$ASM" < "$T/$emit_dec_tooth_name.alpha" \
    > "$T/$emit_dec_tooth_name.tape"
  stamp_seed "$T/$emit_dec_tooth_name.tape" "$SEED" \
    "$T/$emit_dec_tooth_name" >/dev/null
}

emit_dec_build_teeth() {
  # Exact procedure-40 control, arithmetic, output, and decoded inventory.
  emit_dec_build_tooth emit-dec-wrong-guard \
    's/imm r24, 29307              ; checked n>=10 recursive guard/imm r24, 29308              ; checked n>=10 recursive guard/'
  emit_dec_build_tooth emit-dec-wrong-continuation \
    's/imm r26, 29443               ; checked child continuation/imm r26, 29444               ; checked child continuation/'
  emit_dec_build_tooth emit-dec-wrong-write \
    's/imm r23, 29567              ; checked direct digit write/imm r23, 29568              ; checked direct digit write/'
  emit_dec_build_tooth emit-dec-wrong-division \
    's/imm r23, 6                   ; n\/10/imm r23, 7                   ; n\/10/'
  emit_dec_build_tooth emit-dec-wrong-remainder \
    's/imm r23, 7                   ; n%10/imm r23, 6                   ; n%10/'
  emit_dec_build_tooth emit-dec-wrong-digit-constant \
    's/imm r23, 48                  ; decimal digit zero/imm r23, 49                  ; decimal digit zero/'
  emit_dec_build_tooth emit-dec-wrong-argument-push \
    's/imm r23, 29402              ; recursive q argument/imm r23, 29403              ; recursive q argument/'
  emit_dec_build_tooth emit-dec-primitive-undercount \
    's/imm r23, 532                 ; exclusive primitive row/imm r23, 531                 ; exclusive primitive row/'
  emit_dec_build_tooth emit-dec-argument-undercount \
    's/imm r27, 316                 ; exclusive argument-push row/imm r27, 315                 ; exclusive argument-push row/'
  emit_dec_build_tooth emit-dec-drop-division-count \
    's/imm r25, 1                   ; division count/imm r25, 0                   ; division count/'
  emit_dec_build_tooth emit-dec-store-undercount \
    's/imm r1, 7                    ; checked exact target store count/imm r1, 6                    ; checked exact target store count/'

  # Exhaustive bounded arithmetic and the four-phase output induction.
  emit_dec_build_tooth emit-dec-omit-8192 \
    's/imm r1, 8193                 ; checked sweep includes n=8192/imm r1, 8192                 ; checked sweep includes n=8192/'
  emit_dec_build_tooth emit-dec-tight-remainder \
    's/imm r1, 10                   ; checked remainder upper bound/imm r1, 9                    ; checked remainder upper bound/'
  emit_dec_build_tooth emit-dec-wrong-reconstruction \
    's/imm r1, 10                   ; checked reconstruction multiplier/imm r1, 11                   ; checked reconstruction multiplier/'
  emit_dec_build_tooth emit-dec-break-q-rank \
    's/jlt r11, r10, emit_dec_sweep_child_phase ; checked q<n/jlt r10, r11, emit_dec_sweep_child_phase ; checked q<n/'
  emit_dec_build_tooth emit-dec-break-child-phase \
    's/add r0, r1                   ; checked child phase plus one/sub r0, r1                   ; checked child phase plus one/'
  # Phase-isolated induction teeth seed the already exercised arithmetic
  # certificate, avoiding seven redundant 8,193-row sweeps.
  emit_dec_build_induction_tooth emit-dec-drop-base-branch \
    's/imm r2, 1                    ; checked selected false n>=10 edge/imm r2, 0                    ; checked selected false n>=10 edge/'
  emit_dec_build_induction_tooth emit-dec-drop-recursive-branch \
    's/imm r2, 2                    ; checked selected true n>=10 edge/imm r2, 0                    ; checked selected true n>=10 edge/'
  emit_dec_build_induction_tooth emit-dec-drop-call-value \
    's/imm r2, 1                    ; checked push315 passes exact q to child/imm r2, 0                    ; checked push315 passes exact q to child/'
  emit_dec_build_induction_tooth emit-dec-reverse-output-order \
    's/store r1, r2                  ; checked child output precedes digit/store r1, r1                  ; checked child output precedes digit/'
  emit_dec_build_induction_tooth emit-dec-wrong-recursive-output \
    's/imm r2, 2                    ; exact trace || dec(q) || digit(r)/imm r2, 1                    ; exact trace || dec(q) || digit(r)/'
  emit_dec_build_induction_tooth emit-dec-wrong-child-join \
    's/imm r1, 2                    ; checked immediately prior phase offset/imm r1, 1                    ; checked immediately prior phase offset/'
  emit_dec_build_induction_tooth emit-dec-drop-return-frame \
    's/store r1, r2                  ; decimal return zero \/ restored caller frame/store r1, r1                  ; decimal return zero \/ restored caller frame/'
}

emit_dec_reject_teeth() {
  for emit_dec_tooth_name in \
    emit-dec-wrong-guard \
    emit-dec-wrong-continuation \
    emit-dec-wrong-write \
    emit-dec-wrong-division \
    emit-dec-wrong-remainder \
    emit-dec-wrong-digit-constant \
    emit-dec-wrong-argument-push \
    emit-dec-primitive-undercount \
    emit-dec-argument-undercount \
    emit-dec-drop-division-count \
    emit-dec-store-undercount \
    emit-dec-omit-8192 \
    emit-dec-tight-remainder \
    emit-dec-wrong-reconstruction \
    emit-dec-break-q-rank \
    emit-dec-break-child-phase \
    emit-dec-drop-base-branch \
    emit-dec-drop-recursive-branch \
    emit-dec-drop-call-value \
    emit-dec-reverse-output-order \
    emit-dec-wrong-recursive-output \
    emit-dec-wrong-child-join \
    emit-dec-drop-return-frame
  do
    set +e
    "$T/$emit_dec_tooth_name" < "$T/control.bundle" > "$T/stdout"
    emit_dec_tooth_status=$?
    set -e
    if [ "$emit_dec_tooth_status" != 1 ] || [ -s "$T/stdout" ]; then
      echo "bc block control FAIL — $emit_dec_tooth_name was not rejected" >&2
      exit 1
    fi
  done
}
