#!/usr/bin/env sh
# Phase-isolated static ranged-store teeth.

ranged_static_build_teeth() {
  # One mutant drops the sole source-range class; the other underreports the
  # loop invariant while leaving every prior phase and exact input unchanged.
  sed '/composition_store_source_range:/,/jmp composition_store_ranged_count/{s/imm r6, 2/imm r6, 0/;}' \
    "$T/control-check.alpha" > "$T/ranged-missing-class.alpha"
  "$ASM" < "$T/ranged-missing-class.alpha" > "$T/ranged-missing-class.tape"
  stamp_seed "$T/ranged-missing-class.tape" "$SEED" "$T/ranged-missing-class" >/dev/null
  sed '/ranged_interval_loop_candidate:/,/call ranged_interval_store/{s/imm r22, 1048576/imm r22, 1048575/;}' \
    "$T/control-check.alpha" > "$T/ranged-underreported-loop.alpha"
  "$ASM" < "$T/ranged-underreported-loop.alpha" > "$T/ranged-underreported-loop.tape"
  stamp_seed "$T/ranged-underreported-loop.tape" "$SEED" "$T/ranged-underreported-loop" >/dev/null
}

ranged_static_reject_teeth() {
  set +e
  "$T/ranged-missing-class" < "$T/control.bundle" > "$T/stdout"
  ranged_missing_class_status=$?
  set -e
  if [ "$ranged_missing_class_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — missing ranged-store class was not rejected" >&2
    exit 1
  fi
  set +e
  "$T/ranged-underreported-loop" < "$T/control.bundle" > "$T/stdout"
  ranged_underreported_loop_status=$?
  set -e
  if [ "$ranged_underreported_loop_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — underreported ranged loop invariant was not rejected" >&2
    exit 1
  fi
}
