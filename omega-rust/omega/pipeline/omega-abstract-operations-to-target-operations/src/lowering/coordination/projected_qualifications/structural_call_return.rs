//! Exact producer-side projected-roster closure; independent validation lives under `validation/`.

use omega_abstract_operations::{AbstractOperation, AbstractOperationPlan};

pub(super) fn admits_complete_roster(plan: &AbstractOperationPlan) -> bool {
    if plan.functions.len() != 2 {
        return false;
    }
    let Some(caller) = plan
        .functions
        .iter()
        .find(|function| function.machine == plan.entry)
    else {
        return false;
    };
    let ([caller_parameter], Some(caller_result)) = (
        caller.structural_parameters.as_slice(),
        caller.result.structural(),
    ) else {
        return false;
    };
    let [
        AbstractOperation::CallStructural {
            result: operation_result,
            callee,
            structural_arguments,
            ..
        },
        AbstractOperation::ReturnStructural { source, .. },
    ] = caller.operations.as_slice()
    else {
        return false;
    };
    let [argument] = structural_arguments.as_slice() else {
        return false;
    };
    let Some(callee) = plan
        .functions
        .iter()
        .find(|function| function.machine == *callee)
    else {
        return false;
    };
    let ([callee_parameter], Some(callee_result)) = (
        callee.structural_parameters.as_slice(),
        callee.result.structural(),
    ) else {
        return false;
    };
    let [
        AbstractOperation::ReturnStructural {
            source: callee_source,
            ..
        },
    ] = callee.operations.as_slice()
    else {
        return false;
    };
    let rows = &caller_parameter.projected_qualifications;
    !rows.is_empty()
        && caller_result.projected_qualifications == *rows
        && operation_result.projected_qualifications == *rows
        && callee_parameter.projected_qualifications == *rows
        && callee_result.projected_qualifications == *rows
        && argument.place == caller_parameter.place
        && argument.path.is_empty()
        && operation_result.place == *source
        && *callee_source == callee_parameter.place
}
