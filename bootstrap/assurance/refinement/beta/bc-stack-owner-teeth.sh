#!/usr/bin/env sh
# Phase-isolated stack-owner custody tooth.

stack_owner_build_teeth() {
  # Leave the exact source, tape, witness, and every prior checker phase
  # unchanged, but underreport the prelude fp owner. Adjust only the derived-map
  # subtotal so rejection must come from the exhaustive equality scan.
  sed \
    -e '/imm r0, 10/{n;s/call stack_owner_mark/call stack_owner_skip_mark/;}' \
    -e '/stack_owner_count:/,/stack_scan_init/{s/imm r3, 2630/imm r3, 2629/;}' \
    "$T/control-check.alpha" > "$T/stack-missing-owner.alpha"
  "$ASM" < "$T/stack-missing-owner.alpha" > "$T/stack-missing-owner.tape"
  stamp_seed "$T/stack-missing-owner.tape" "$SEED" "$T/stack-missing-owner" >/dev/null
}

stack_owner_reject_teeth() {
  set +e
  "$T/stack-missing-owner" < "$T/control.bundle" > "$T/stdout"
  missing_stack_owner_status=$?
  set -e
  if [ "$missing_stack_owner_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — underreported stack owner: expected 1/empty, got $missing_stack_owner_status/$(wc -c < "$T/stdout" | tr -d ' ') bytes" >&2
    exit 1
  fi
}
