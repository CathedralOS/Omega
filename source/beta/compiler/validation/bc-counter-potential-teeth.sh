#!/usr/bin/env sh
# Phase-isolated counter-context and stack-potential teeth.

counter_potential_build_teeth() {
  # Break the checked BCS9-row/live-counter relation, undercount one protected
  # writer, and underreport only the final exact root instantiation.
  sed '/counter_context_build_row:/,/store r1, r21/{s/imm r21, 64/imm r21, 63/;}' \
    "$T/control-check.alpha" > "$T/counter-wrong-context.alpha"
  "$ASM" < "$T/counter-wrong-context.alpha" > "$T/counter-wrong-context.tape"
  stamp_seed "$T/counter-wrong-context.tape" "$SEED" "$T/counter-wrong-context" >/dev/null
  sed '/counter_writer_resource_count:/,/counter_writer_counts_ok:/{s/imm r1, 7/imm r1, 6/;}' \
    "$T/control-check.alpha" > "$T/counter-missing-writer.alpha"
  "$ASM" < "$T/counter-missing-writer.alpha" > "$T/counter-missing-writer.tape"
  stamp_seed "$T/counter-missing-writer.tape" "$SEED" "$T/counter-missing-writer" >/dev/null
  sed '/stack_lift_instantiate_main:/,/stack_lift_main_hidden:/{s/imm r1, 12720/imm r1, 12712/;}' \
    "$T/control-check.alpha" > "$T/stack-underreported-root.alpha"
  "$ASM" < "$T/stack-underreported-root.alpha" > "$T/stack-underreported-root.tape"
  stamp_seed "$T/stack-underreported-root.tape" "$SEED" "$T/stack-underreported-root" >/dev/null
}

counter_potential_reject_teeth() {
  set +e
  "$T/counter-wrong-context" < "$T/control.bundle" > "$T/stdout"
  counter_wrong_context_status=$?
  set -e
  if [ "$counter_wrong_context_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — wrong counter/potential context relation was not rejected" >&2
    exit 1
  fi
  set +e
  "$T/counter-missing-writer" < "$T/control.bundle" > "$T/stdout"
  counter_missing_writer_status=$?
  set -e
  if [ "$counter_missing_writer_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — undercounted protected writer was not rejected" >&2
    exit 1
  fi
  set +e
  "$T/stack-underreported-root" < "$T/control.bundle" > "$T/stdout"
  stack_underreported_root_status=$?
  set -e
  if [ "$stack_underreported_root_status" != 1 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — underreported absolute stack root was not rejected" >&2
    exit 1
  fi
}
