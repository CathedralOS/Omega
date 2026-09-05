//! Optimizer module role: executable entrance. Fail-closed target-lowering admission for projected qualification custody.

mod structural_call_return;

use abstract_operations::{AbstractOperation, AbstractOperationPlan};

use crate::LoweringError;

pub(super) fn reject_unsupported(plan: &AbstractOperationPlan) -> Result<(), LoweringError> {
    if !has_any_projected_qualifications(plan) {
        return Ok(());
    }
    if external_signatures_have_projected_qualifications(plan)
        || !structural_call_return::admits_complete_roster(plan)
    {
        return Err(LoweringError::UnsupportedProjectedStructuralQualifications);
    }
    Ok(())
}

fn has_any_projected_qualifications(plan: &AbstractOperationPlan) -> bool {
    plan.functions.iter().any(|function| {
        function
            .structural_parameters
            .iter()
            .any(|parameter| !parameter.projected_qualifications.is_empty())
            || function
                .result
                .structural()
                .is_some_and(|result| !result.projected_qualifications.is_empty())
            || function.operations.iter().any(|operation| match operation {
                AbstractOperation::EstablishPayloadlessCase { result, .. }
                | AbstractOperation::CallStructural { result, .. } => {
                    !result.projected_qualifications.is_empty()
                }
                _ => false,
            })
    }) || external_signatures_have_projected_qualifications(plan)
}

fn external_signatures_have_projected_qualifications(plan: &AbstractOperationPlan) -> bool {
    plan.boundary_machines.iter().any(|boundary| {
        boundary
            .structural_parameters
            .iter()
            .any(|parameter| !parameter.projected_qualifications.is_empty())
    }) || plan.provider_candidates.iter().any(|candidate| {
        candidate
            .signature
            .parameters
            .iter()
            .any(|parameter| !parameter.projected_qualifications.is_empty())
    })
}
