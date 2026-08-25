#!/usr/bin/env sh
# Checker-A comparison-macro custody negatives.

artifact_comparison_build_teeth() { :; }

artifact_comparison_reject_teeth() {
  case_run "comparison branch opcode" 1 "$T/comparison-opcode.bundle"
  case_run "comparison branch operand order" 1 "$T/comparison-operand.bundle"
  case_run "comparison branch target" 1 "$T/comparison-branch-target.bundle"
  case_run "comparison materialized result" 1 "$T/comparison-result.bundle"
  case_run "comparison pop step" 1 "$T/comparison-pop-step.bundle"
}
