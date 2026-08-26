#!/usr/bin/env sh
# Phase-isolated frame-summary teeth.

frame_summary_build_teeth() {
  # Omit one saved-fp owner while adjusting only the derived subtotals, so the
  # independent store scan must reject it; then underreport every procedure's
  # checked local peak.
  sed \
    -e '/fs_store_saved_entry:/,/fs_store_saved_next:/{s/call fs_store_mark/call fs_store_skip_first_saved/;}' \
    -e '/fs_store_counts:/,/fs_store_saved_count:/{s/imm r3, 607/imm r3, 606/;}' \
    -e '/fs_store_saved_count:/,/fs_store_push_count:/{s/imm r3, 70/imm r3, 69/;}' \
    "$T/control-check.alpha" > "$T/frame-missing-store-owner.alpha"
  "$ASM" < "$T/frame-missing-store-owner.alpha" > "$T/frame-missing-store-owner.tape"
  stamp_seed "$T/frame-missing-store-owner.tape" "$SEED" "$T/frame-missing-store-owner" >/dev/null
  sed '/fs_proc_expected_peak:/,/store r1, r4/{s/imm r1, 8/imm r1, 0/;}' \
    "$T/control-check.alpha" > "$T/frame-underreported-peak.alpha"
  "$ASM" < "$T/frame-underreported-peak.alpha" > "$T/frame-underreported-peak.tape"
  stamp_seed "$T/frame-underreported-peak.tape" "$SEED" "$T/frame-underreported-peak" >/dev/null
}

frame_summary_reject_teeth() {
  set +e
  "$T/frame-missing-store-owner" < "$T/control.bundle" > "$T/stdout"
  frame_missing_store_owner_status=$?
  set -e
  if [ "$frame_missing_store_owner_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — missing frame-summary store owner was not rejected" >&2
    exit 1
  fi
  set +e
  "$T/frame-underreported-peak" < "$T/control.bundle" > "$T/stdout"
  frame_underreported_peak_status=$?
  set -e
  if [ "$frame_underreported_peak_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — underreported procedure-local frame peak was not rejected" >&2
    exit 1
  fi
}
