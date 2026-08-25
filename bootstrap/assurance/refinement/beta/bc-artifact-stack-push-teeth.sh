#!/usr/bin/env sh
# Checker-A explicit data-stack push custody negatives.

artifact_stack_push_build_teeth() { :; }

artifact_stack_push_reject_teeth() {
  case_run "argument push stack step" 1 "$T/push-step.bundle"
  case_run "argument push stack register" 1 "$T/push-stack-register.bundle"
  case_run "argument push value register" 1 "$T/push-value-register.bundle"
  case_run "same-width argument push opcode" 1 "$T/push-opcode.bundle"
  case_run "duplicate stack-push location" 1 "$T/duplicate-push.bundle"
  case_run "cross-block stack-push location" 1 "$T/cross-block-push.bundle"
}
