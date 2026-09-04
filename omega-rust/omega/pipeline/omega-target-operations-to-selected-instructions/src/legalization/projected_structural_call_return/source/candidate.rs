//! Exhaustive projected-roster notice for producer-side fail-closed routing.

use omega_abstract_operations::{AbstractOperation, AbstractOperationPlan};
use omega_target_operations::{TargetOperation, TargetOperationPlan};

pub(super) fn is_candidate(target: &TargetOperationPlan, source: &AbstractOperationPlan) -> bool {
    source.functions.iter().any(|function| {
        function
            .structural_parameters
            .iter()
            .any(|parameter| !parameter.projected_qualifications.is_empty())
            || function.result.structural().is_some_and(|result| {
                !result.projected_qualifications.is_empty()
            })
            || function.operations.iter().any(|operation| {
                matches!(operation, AbstractOperation::CallStructural { result, .. } if !result.projected_qualifications.is_empty())
            })
    }) || target.functions.iter().any(|function| has_target_roster(&function.operation))
}

fn has_target_roster(operation: &TargetOperation) -> bool {
    match operation {
        TargetOperation::UnitBody(body) => body
            .parameters
            .iter()
            .any(|parameter| !parameter.projected_qualifications.is_empty()),
        TargetOperation::ReturnStructuralCall {
            operation_result,
            result,
            structural_parameters,
            ..
        } => {
            !operation_result.projected_qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || structural_parameters
                    .iter()
                    .any(|parameter| !parameter.projected_qualifications.is_empty())
        }
        TargetOperation::ReturnStructuralParameter {
            parameters,
            source,
            result,
            ..
        } => {
            parameters
                .iter()
                .any(|parameter| !parameter.projected_qualifications.is_empty())
                || !source.projected_qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
        }
        TargetOperation::ReturnStructuralScalarCall {
            structural_parameters,
            ..
        }
        | TargetOperation::ScalarReturnWithCleanup {
            structural_parameters,
            ..
        }
        | TargetOperation::ReturnBoundaryPortReadU8 {
            structural_parameters,
            ..
        }
        | TargetOperation::BooleanControlWithCleanup {
            structural_parameters,
            ..
        } => structural_parameters
            .iter()
            .any(|parameter| !parameter.projected_qualifications.is_empty()),
        _ => false,
    }
}
