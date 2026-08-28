#!/usr/bin/env sh
# Phase-isolated ranged-store transfer teeth.

ranged_transfer_build_teeth() {
  # Misjoin n's address use to c's PC, underreport selected procedures' real
  # relative frame depth, or change the exact transferred value class.
  sed '/; slurp locals:/,/; declare snapshot/{s/imm r24, 446/imm r24, 503/;}' \
    "$T/control-check.alpha" > "$T/transfer-wrong-local.alpha"
  "$ASM" < "$T/transfer-wrong-local.alpha" > "$T/transfer-wrong-local.tape"
  stamp_seed "$T/transfer-wrong-local.tape" "$SEED" "$T/transfer-wrong-local" >/dev/null
  sed '/transfer_frame_push:/,/jmp transfer_frame_next3/{s/imm r1, 33/imm r1, 25/;}' \
    "$T/control-check.alpha" > "$T/transfer-shallow-frame.alpha"
  "$ASM" < "$T/transfer-shallow-frame.alpha" > "$T/transfer-shallow-frame.tape"
  stamp_seed "$T/transfer-shallow-frame.tape" "$SEED" "$T/transfer-shallow-frame" >/dev/null
  sed '/transfer_value_add_src_now:/,/call transfer_value_set/{s/imm r20, 5/imm r20, 10/;}' \
    "$T/control-check.alpha" > "$T/transfer-wrong-value-tag.alpha"
  "$ASM" < "$T/transfer-wrong-value-tag.alpha" > "$T/transfer-wrong-value-tag.tape"
  stamp_seed "$T/transfer-wrong-value-tag.tape" "$SEED" "$T/transfer-wrong-value-tag" >/dev/null
}

ranged_transfer_reject_teeth() {
  set +e
  "$T/transfer-wrong-local" < "$T/control.bundle" > "$T/stdout"
  transfer_wrong_local_status=$?
  set -e
  if [ "$transfer_wrong_local_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — wrong ranged-store local transfer was not rejected" >&2
    exit 1
  fi
  set +e
  "$T/transfer-shallow-frame" < "$T/control.bundle" > "$T/stdout"
  transfer_shallow_frame_status=$?
  set -e
  if [ "$transfer_shallow_frame_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — underreported selected frame depth was not rejected" >&2
    exit 1
  fi
  set +e
  "$T/transfer-wrong-value-tag" < "$T/control.bundle" > "$T/stdout"
  transfer_wrong_value_tag_status=$?
  set -e
  if [ "$transfer_wrong_value_tag_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — wrong ranged-store value tag was not rejected" >&2
    exit 1
  fi
}
