//! Optimizer module role: executable entrance. Fail-closed target-lowering admission for projected qualification custody.

use abstract_operations::{AbstractOperation, AbstractOperationPlan, AbstractStructuralCallResult};

use crate::LoweringError;

pub(super) fn reject_unsupported(plan: &AbstractOperationPlan) -> Result<(), LoweringError> {
    if has_any_projected_qualifications(plan) {
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
            || function.block_entries.iter().any(|block| {
                block
                    .structural_parameters
                    .iter()
                    .any(|parameter| !parameter.projected_qualifications.is_empty())
            })
            || function
                .result
                .structural()
                .is_some_and(|result| !result.projected_qualifications.is_empty())
            || function.operations.iter().any(|operation| match operation {
                AbstractOperation::EstablishRecord { result, .. }
                | AbstractOperation::EstablishZeroedStructural { result, .. }
                | AbstractOperation::EstablishScalarArray { result, .. }
                | AbstractOperation::EstablishStructuralArray { result, .. }
                | AbstractOperation::EstablishByteSequenceConcatenation { result, .. }
                | AbstractOperation::EstablishCase { result, .. }
                | AbstractOperation::CallStructural {
                    result: AbstractStructuralCallResult::Structural { result, .. },
                    ..
                } => !result.projected_qualifications.is_empty(),
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
