use omega_optimization_core::OptimizationWorkBudget;

use crate::{Aarch64SameViewCopyElisionError, Aarch64SameViewCopyElisionPlan, SameViewCopyInputs};

pub(super) fn replay(
    inputs: &SameViewCopyInputs<'_>,
    budget: OptimizationWorkBudget,
) -> Result<Aarch64SameViewCopyElisionPlan, Aarch64SameViewCopyElisionError> {
    super::super::super::same_view_copy_before_compare::replay(
        inputs,
        budget,
        super::super::contract::CONTRACT,
    )
}
