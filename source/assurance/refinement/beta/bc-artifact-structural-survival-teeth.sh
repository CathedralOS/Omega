#!/usr/bin/env sh
# Checker-A artifact mutations must retain generic Alpha structural validity.

artifact_structural_survival_build_teeth() { :; }

artifact_structural_survival_check_one() { # mutation failure-description
  set +e
  "$T/structure-check" < "$T/$1.tape" > "$T/stdout"
  artifact_structural_survival_status=$?
  set -e
  if [ "$artifact_structural_survival_status" != 0 ] || [ -s "$T/stdout" ]; then
    echo "bc block control FAIL — $2" >&2
    exit 1
  fi
}

artifact_structural_survival_check_teeth() {
  # Preserve the historical special-case diagnostic before the common list.
  artifact_structural_survival_check_one retarget \
    "retarget was not a structurally valid boundary mutation"

  for artifact_structural_survival_mutation in \
    call-retarget read-register write-register helper-write emit-byte \
    emit-length emit-pointer emit-helper orphan-io frame-size saved-fp \
    frame-base param-offset param-register call-pop-order call-pop-step \
    local-load-slot local-store-slot local-base local-load-opcode \
    local-store-opcode memory-load-width memory-store-width \
    memory-load-register memory-store-register memory-pop-step literal-value \
    literal-register arithmetic-opcode arithmetic-pop-step arithmetic-register \
    comparison-opcode comparison-operand comparison-branch-target \
    comparison-result comparison-pop-step push-step push-stack-register \
    push-value-register push-opcode
  do
    artifact_structural_survival_check_one \
      "$artifact_structural_survival_mutation" \
      "$artifact_structural_survival_mutation was not a structurally valid mutation"
  done
}

# The phased shard harness accepts either a reject or check suffix. Keep the
# alias explicit so wiring can treat all Checker-A modules uniformly.
artifact_structural_survival_reject_teeth() {
  artifact_structural_survival_check_teeth
}
