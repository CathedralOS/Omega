//! Operands of calls and boundary calls: each scalar argument must carry
//! its parameter's type and be defined.

use super::super::{
    BTreeMap, BTreeSet, BoundaryMachineDeclaration, MachineId, ModuleError, OperationKind,
    ScalarType, TerminalMachine, ValueId,
};
use super::{ScalarCallKind, validate_call_arguments};

pub(super) fn validate_call(
    operation: &terminal_psi::Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::Call {
        callee, arguments, ..
    } = &operation.kind
    else {
        unreachable!("dispatched validate_call")
    };
    let callee = machines
        .get(callee)
        .copied()
        .expect("call target was validated during operation registration");
    validate_call_arguments(
        operation.id,
        arguments,
        &callee
            .parameters
            .iter()
            .map(|parameter| parameter.scalar_type)
            .collect::<Vec<_>>(),
        value_types,
        defined,
        ScalarCallKind::Ordinary,
    )?;
    Ok(())
}

pub(super) fn validate_call_unit(
    operation: &terminal_psi::Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::CallUnit {
        callee, arguments, ..
    } = &operation.kind
    else {
        unreachable!("dispatched validate_call_unit")
    };
    let callee = machines
        .get(callee)
        .copied()
        .expect("Unit call target was validated during operation registration");
    validate_call_arguments(
        operation.id,
        arguments,
        &callee
            .parameters
            .iter()
            .map(|parameter| parameter.scalar_type)
            .collect::<Vec<_>>(),
        value_types,
        defined,
        ScalarCallKind::Ordinary,
    )?;
    Ok(())
}

pub(super) fn validate_call_structural_scalar(
    operation: &terminal_psi::Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::CallStructuralScalar {
        callee, arguments, ..
    } = &operation.kind
    else {
        unreachable!("dispatched validate_call_structural_scalar")
    };
    let callee = machines
        .get(callee)
        .copied()
        .expect("structural scalar call target was validated during operation registration");
    validate_call_arguments(
        operation.id,
        arguments,
        &callee
            .parameters
            .iter()
            .map(|parameter| parameter.scalar_type)
            .collect::<Vec<_>>(),
        value_types,
        defined,
        ScalarCallKind::Ordinary,
    )?;
    Ok(())
}

pub(super) fn validate_call_structural_with_scalar_arguments(
    operation: &terminal_psi::Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::CallStructuralWithScalarArguments {
        callee, arguments, ..
    } = &operation.kind
    else {
        unreachable!("dispatched validate_call_structural_with_scalar_arguments")
    };
    let callee = machines
        .get(callee)
        .copied()
        .expect("mixed structural call target was validated during operation registration");
    validate_call_arguments(
        operation.id,
        arguments,
        &callee
            .parameters
            .iter()
            .map(|parameter| parameter.scalar_type)
            .collect::<Vec<_>>(),
        value_types,
        defined,
        ScalarCallKind::Ordinary,
    )?;
    Ok(())
}

pub(super) fn validate_boundary_call(
    _machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    boundary_machines: &[BoundaryMachineDeclaration],
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::BoundaryCall {
        boundary,
        arguments,
        ..
    } = &operation.kind
    else {
        unreachable!("dispatched validate_boundary_call")
    };
    let boundary = boundary_machines
        .iter()
        .find(|candidate| candidate.id == *boundary)
        .expect("boundary target was validated during operation registration");
    validate_call_arguments(
        operation.id,
        arguments,
        &boundary.scalar_parameters,
        value_types,
        defined,
        ScalarCallKind::Boundary,
    )?;
    // The invocation's crash continuations are reconstructed — and covered by
    // certificate evidence — at verification, not searched during validation.
    // The formal telescope must still form so the reconstruction can bind it.
    if !boundary.crash_routes.is_empty() {
        boundary
            .scalar_contract_parameters()
            .ok_or(ModuleError::InvalidBoundaryCrashParameters(boundary.id))?;
    }
    Ok(())
}
