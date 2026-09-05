//! Optimizer module role: executable entrance. Independent projected-roster replay for one structural call/return closure.

mod layout;
mod local;
mod model;
mod source;
mod target;

pub use model::{
    StructuralCallReturnCallerTranslationReceipt,
    StructuralCallReturnProjectedQualificationReceipt,
    StructuralCallReturnProjectedQualificationValidationError, StructuralCallReturnRosterLocation,
    StructuralParameterReturnCalleeTranslationReceipt,
};

pub(crate) use local::{
    is_callee_candidate, is_caller_candidate, validate_callee, validate_caller,
};

use abstract_operations::{AbstractOperation, AbstractOperationPlan};
use target_operations::TargetOperationPlan;

pub(crate) fn is_candidate(source: &AbstractOperationPlan) -> bool {
    source.functions.iter().any(|function| {
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
    }) || source.boundary_machines.iter().any(|boundary| {
        boundary
            .structural_parameters
            .iter()
            .any(|parameter| !parameter.projected_qualifications.is_empty())
    }) || source.provider_candidates.iter().any(|candidate| {
        candidate
            .signature
            .parameters
            .iter()
            .any(|parameter| !parameter.projected_qualifications.is_empty())
    })
}

pub(crate) fn validate(
    source: &AbstractOperationPlan,
    target: &TargetOperationPlan,
) -> Result<
    StructuralCallReturnProjectedQualificationReceipt,
    StructuralCallReturnProjectedQualificationValidationError,
> {
    let closure = source::reconstruct(source)?;
    target::replay(&closure, target)?;
    Ok(StructuralCallReturnProjectedQualificationReceipt::new(
        closure.caller,
        closure.callee,
        closure.roster,
    ))
}
