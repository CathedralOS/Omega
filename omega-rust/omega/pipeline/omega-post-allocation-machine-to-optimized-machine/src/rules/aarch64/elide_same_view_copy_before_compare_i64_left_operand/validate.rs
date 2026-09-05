//! Optimizer module role: executable entrance. Independent non-terminal copy-elision replay.

mod replay;

use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions_to_register_homes::{ValidatedLiveness, ValidatedSelectedAnalysis};

use crate::*;
use omega_register_homes_to_post_allocation_machine::ValidatedPostAllocationMachinePlan;

pub fn validate_aarch64_same_view_copy_i64_before_compare_i64_left_operand<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    liveness: &ValidatedLiveness,
    source: &ValidatedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    plan: Aarch64SameViewCopyElisionPlan,
) -> Result<ValidatedAarch64SameViewCopyElision, Aarch64SameViewCopyElisionError> {
    validate(selected, liveness, source, physical, plan)
}

pub(super) fn validate<S: ValidatedSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedLiveness,
    source: &ValidatedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    plan: Aarch64SameViewCopyElisionPlan,
) -> Result<ValidatedAarch64SameViewCopyElision, Aarch64SameViewCopyElisionError> {
    let inputs = SameViewCopyInputs {
        selected: selected.selected_plan(),
        selected_identity: selected.selected_identity(),
        liveness: liveness.plan(),
        liveness_identity: liveness.receipt().identity(),
        source: source.plan(),
        source_identity: source.receipt().identity(),
        physical,
    };
    validate_from_inputs(inputs, plan)
}

pub(crate) fn validate_from_inputs(
    inputs: SameViewCopyInputs<'_>,
    plan: Aarch64SameViewCopyElisionPlan,
) -> Result<ValidatedAarch64SameViewCopyElision, Aarch64SameViewCopyElisionError> {
    if plan.policy
        != Aarch64SameViewCopyElisionPolicy::Aarch64ElideSameViewCopyI64BeforeCompareI64LeftOperandV1
    {
        return Err(Aarch64SameViewCopyElisionError::ArtifactMismatch);
    }
    let expected = replay::replay(&inputs, plan.budget)?;
    if plan != expected {
        return Err(Aarch64SameViewCopyElisionError::ArtifactMismatch);
    }
    let receipt = same_view_copy_elision_receipt(&plan);
    Ok(ValidatedAarch64SameViewCopyElision::new(plan, receipt))
}
