//! Emission adapter for the bounded structural-call/scalar-return carrier.

use omega_assigned_target_operations::{
    AssignedFunction, AssignedOperation, AssignedUnitBody, AssignedUnitOperation,
};
use omega_target::NativeTarget;

use super::{EmissionError, unit::UnitEmission, unit::emit_unit_body};

pub(super) fn emit(
    operation: &AssignedOperation,
    target: NativeTarget,
    functions: &[AssignedFunction],
) -> Result<UnitEmission, EmissionError> {
    let AssignedOperation::ReturnStructuralScalarCall {
        psi_edge,
        psi_operation,
        scalar_type,
        callee,
        structural_types,
        call_plan,
        structural_parameters,
        copies,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
        ..
    } = operation
    else {
        unreachable!("structural scalar emission receives its exact assigned carrier")
    };
    emit_unit_body(
        &AssignedUnitBody {
            structural_types: structural_types.clone(),
            call_plan: call_plan.clone(),
            parameters: structural_parameters.clone(),
            operations: vec![
                AssignedUnitOperation::Call {
                    psi_operation: *psi_operation,
                    callee: *callee,
                    result: Some(*scalar_type),
                    copies: copies.clone(),
                    claim_transfers: claim_transfers.clone(),
                    requirement_obligations: requirement_obligations.clone(),
                    crash_continuations: crash_continuations.clone(),
                },
                AssignedUnitOperation::Return {
                    psi_edge: *psi_edge,
                    cleanup_actions: Vec::new(),
                },
            ],
        },
        target,
        functions,
    )
}
