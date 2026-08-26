//! Emission adapter for the bounded structural-call/scalar-return carrier.

use omega_target::NativeTarget;
use omega_terminal_assigned_target_operations::{
    TerminalAssignedFunction, TerminalAssignedOperation, TerminalAssignedUnitBody,
    TerminalAssignedUnitOperation,
};

use super::{EmissionError, unit::UnitEmission, unit::emit_unit_body};

pub(super) fn emit(
    operation: &TerminalAssignedOperation,
    target: NativeTarget,
    functions: &[TerminalAssignedFunction],
) -> Result<UnitEmission, EmissionError> {
    let TerminalAssignedOperation::ReturnStructuralScalarCall {
        psi_edge,
        psi_operation,
        scalar_type,
        callee,
        structural_types,
        call_plan,
        structural_parameters,
        copies,
        claim_transfers,
        ..
    } = operation
    else {
        unreachable!("structural scalar emission receives its exact assigned carrier")
    };
    emit_unit_body(
        &TerminalAssignedUnitBody {
            structural_types: structural_types.clone(),
            call_plan: call_plan.clone(),
            parameters: structural_parameters.clone(),
            operations: vec![
                TerminalAssignedUnitOperation::Call {
                    psi_operation: *psi_operation,
                    callee: *callee,
                    result: Some(*scalar_type),
                    copies: copies.clone(),
                    claim_transfers: claim_transfers.clone(),
                },
                TerminalAssignedUnitOperation::Return {
                    psi_edge: *psi_edge,
                    cleanup_actions: Vec::new(),
                },
            ],
        },
        target,
        functions,
    )
}
