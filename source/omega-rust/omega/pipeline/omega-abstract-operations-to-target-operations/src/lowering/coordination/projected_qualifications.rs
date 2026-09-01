//! Optimizer module role: semantic leaf. Fail-closed target-lowering fence for path qualification custody.

use omega_abstract_operations::{AbstractOperation, AbstractOperationPlan};

use crate::LoweringError;

pub(super) fn reject_unsupported(plan: &AbstractOperationPlan) -> Result<(), LoweringError> {
    let functions = plan.functions.iter().flat_map(|function| {
        function
            .structural_parameters
            .iter()
            .map(|parameter| &parameter.projected_qualifications)
    });
    let function_results = plan.functions.iter().filter_map(|function| {
        function
            .result
            .structural()
            .map(|result| &result.projected_qualifications)
    });
    let operation_results = plan
        .functions
        .iter()
        .flat_map(|function| &function.operations)
        .filter_map(|operation| match operation {
            AbstractOperation::EstablishPayloadlessCase { result, .. }
            | AbstractOperation::CallStructural { result, .. } => {
                Some(&result.projected_qualifications)
            }
            _ => None,
        });
    let boundaries = plan.boundary_machines.iter().flat_map(|boundary| {
        boundary
            .structural_parameters
            .iter()
            .map(|parameter| &parameter.projected_qualifications)
    });
    let providers = plan.provider_candidates.iter().flat_map(|candidate| {
        candidate
            .signature
            .parameters
            .iter()
            .map(|parameter| &parameter.projected_qualifications)
    });
    if functions
        .chain(boundaries)
        .chain(providers)
        .chain(function_results)
        .chain(operation_results)
        .any(|rows| !rows.is_empty())
    {
        Err(LoweringError::UnsupportedProjectedStructuralQualifications)
    } else {
        Ok(())
    }
}
