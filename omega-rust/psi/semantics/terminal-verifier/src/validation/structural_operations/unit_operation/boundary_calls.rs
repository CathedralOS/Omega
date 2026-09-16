//! Boundary calls and port writes: requirements, completion receipts and
//! service reach validate against the boundary machine or service.

use crate::validation::structural_operations::boundary_requirements::{
    validate_boundary_completion_receipts, validate_boundary_requirements,
};
use crate::validation::structural_operations::claim_transfers::validate_service_reach;
use crate::validation::structural_operations::structural_arguments::{
    StructuralArgumentSourcePolicy, validate_structural_arguments,
};
use crate::validation::{
    BoundaryContentGuarantee, ModuleError, OperationKind, TerminalMachine, TerminalModule,
};

pub(super) fn validate_boundary_call(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    let OperationKind::BoundaryCall {
        boundary,
        arguments: _,
        structural_arguments,
        completion_receipts,
    } = &operation.kind
    else {
        unreachable!("dispatched validate_boundary_call")
    };
    let boundary = module
        .boundary_machines
        .iter()
        .find(|candidate| candidate.id == *boundary)
        .ok_or(ModuleError::UnknownBoundaryCallTarget {
            operation: operation.id,
            boundary: *boundary,
        })?;
    if boundary
        .content_guarantees
        .iter()
        .any(|guarantee| matches!(guarantee, BoundaryContentGuarantee::RetainedBorrow(_)))
    {
        return Err(ModuleError::RetainedBorrowBoundaryIsNotExecutable {
            operation: operation.id,
            boundary: boundary.id,
        });
    }
    validate_structural_arguments(
        module,
        machine,
        structural_arguments,
        &boundary.structural_parameters,
        operation.id,
        true,
        StructuralArgumentSourcePolicy::ParametersOrBoundaryActuals,
    )?;
    validate_service_reach(
        operation.id,
        &machine.published_service_ceiling,
        &boundary.published_service_ceiling,
    )?;
    validate_boundary_requirements(machine, boundary, structural_arguments, operation.id)?;
    validate_boundary_completion_receipts(
        machine,
        structural_arguments,
        completion_receipts,
        operation.id,
    )?;
    Ok(())
}

pub(super) fn validate_port_write(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    let OperationKind::PortWrite { service, .. } = &operation.kind else {
        unreachable!("dispatched validate_port_write")
    };
    if !module
        .services
        .iter()
        .any(|candidate| candidate.id == *service)
    {
        return Err(ModuleError::UnknownOperationService {
            operation: operation.id,
            service: *service,
        });
    }
    if !machine.published_service_ceiling.contains(service) {
        return Err(ModuleError::OperationServiceOutsidePublishedCeiling {
            operation: operation.id,
            service: *service,
        });
    }
    Ok(())
}
