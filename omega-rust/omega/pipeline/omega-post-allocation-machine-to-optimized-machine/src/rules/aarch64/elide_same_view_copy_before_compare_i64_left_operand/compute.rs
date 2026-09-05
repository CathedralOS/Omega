use omega_optimization_core::OptimizationWorkBudget;
use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions_to_register_homes::{ValidatedLiveness, ValidatedSelectedAnalysis};

use crate::{Aarch64SameViewCopyElisionError, Aarch64SameViewCopyElisionPlan, SameViewCopyInputs};
use omega_register_homes_to_post_allocation_machine::ValidatedPostAllocationMachinePlan;

pub(super) fn compute<S: ValidatedSelectedAnalysis>(
    selected: &S,
    liveness: &ValidatedLiveness,
    source: &ValidatedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<Aarch64SameViewCopyElisionPlan, Aarch64SameViewCopyElisionError> {
    compute_from_inputs(
        SameViewCopyInputs {
            selected: selected.selected_plan(),
            selected_identity: selected.selected_identity(),
            liveness: liveness.plan(),
            liveness_identity: liveness.receipt().identity(),
            source: source.plan(),
            source_identity: source.receipt().identity(),
            physical,
        },
        budget,
    )
}

pub(crate) fn compute_from_inputs(
    inputs: SameViewCopyInputs<'_>,
    budget: OptimizationWorkBudget,
) -> Result<Aarch64SameViewCopyElisionPlan, Aarch64SameViewCopyElisionError> {
    super::super::same_view_copy_before_compare::propose(
        inputs,
        budget,
        super::contract::CONTRACT,
        &super::pattern::AARCH64_SAME_VIEW_COPY_BEFORE_COMPARE_I64_LEFT_OPERAND_V1,
    )
}
