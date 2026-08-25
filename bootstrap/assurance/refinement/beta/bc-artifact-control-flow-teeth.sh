#!/usr/bin/env sh
# Checker-A control-flow custody negatives. Common setup owns every bundle.

artifact_control_flow_build_teeth() { :; }

artifact_control_flow_reject_teeth() {
  case_run "valid-boundary transition retarget" 1 "$T/retarget.bundle"
  case_run "block pc into opcode-looking operand" 1 "$T/operand.bundle"
  case_run "duplicate block location" 1 "$T/duplicate.bundle"
  case_run "missing transition location" 1 "$T/missing.bundle"
  case_run "noncanonical transition order" 1 "$T/noncanonical.bundle"
}
