#!/usr/bin/env sh
# Checker-A primitive and syntax-directed composition custody negatives.

artifact_primitive_composition_build_teeth() { :; }

artifact_primitive_composition_reject_teeth() {
  case_run "literal value" 1 "$T/literal-value.bundle"
  case_run "literal destination register" 1 "$T/literal-register.bundle"
  case_run "arithmetic opcode" 1 "$T/arithmetic-opcode.bundle"
  case_run "arithmetic pop step" 1 "$T/arithmetic-pop-step.bundle"
  case_run "arithmetic destination register" 1 "$T/arithmetic-register.bundle"
  case_run "duplicate expression primitive location" 1 "$T/duplicate-primitive.bundle"
  case_run "noncanonical expression primitive order" 1 "$T/noncanonical-primitive.bundle"
  case_run "same-valued synthetic literal location" 1 "$T/synthetic-literal.bundle"
  case_run "same-valued recursive expression order" 1 "$T/composition-order.bundle"
  case_run "ordinary-call argument composition order" 1 "$T/composition-argument-order.bundle"
  case_run "store address/value composition order" 1 "$T/composition-store-order.bundle"
}
