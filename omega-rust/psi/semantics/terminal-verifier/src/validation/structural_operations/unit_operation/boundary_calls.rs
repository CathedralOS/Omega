//! Boundary calls and port writes: requirements, completion receipts and
//! service reach validate against the boundary machine or service.

use crate::validation::structural_operations::boundary_requirements::{
    validate_boundary_completion_receipts, validate_boundary_requirements,
};
use crate::validation::structural_operations::claim_transfers::validate_service_reach;
use crate::validation::structural_operations::structural_arguments::{
    StructuralArgumentSourcePolicy, linear_call_result, validate_structural_arguments,
};
use crate::validation::{
    BoundaryContentGuarantee, BoundaryMachineDeclaration, ModuleError, OperationKind,
    OperationResult, StructuralAccess, StructuralArgument, TerminalMachine, TerminalModule,
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
    for guarantee in &boundary.content_guarantees {
        let BoundaryContentGuarantee::RetainedBorrow(custody) = guarantee else {
            continue;
        };
        validate_retained_borrow_call(
            module,
            machine,
            operation,
            boundary,
            custody,
            structural_arguments,
        )?;
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

/// An invoked retained-borrow call admits only when the caller loans the
/// custody's exact source parameter place under a shared borrow and the
/// operation's result is the row's exact retained occurrence: the loan's
/// claim frontier is bound whole beneath the result place, so the loan stays
/// live for the result's lifetime and settles when that occurrence is
/// consumed.
fn validate_retained_borrow_call(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    boundary: &BoundaryMachineDeclaration,
    custody: &terminal_psi::RetainedBorrowCustody,
    arguments: &[StructuralArgument],
) -> Result<(), ModuleError> {
    let reject = || ModuleError::InvalidRetainedBorrowBoundaryCall {
        operation: operation.id,
        boundary: boundary.id,
    };
    let terminal_psi::RetainedBorrowPlace {
        version: semantic_vocabulary::ContentPlaceVersion::Entry,
        root:
            terminal_psi::RetainedBorrowPlaceRoot::Parameter {
                position,
                is_self: false,
                ..
            },
        segments,
    } = &custody.source
    else {
        return Err(reject());
    };
    if !segments.is_empty() || !boundary.has_valid_parameter_order() {
        return Err(reject());
    }
    let Some(lane) = boundary
        .parameter_positions(terminal_psi::BoundaryParameterKind::Structural)
        .position(|formal| *position == u32::try_from(formal).unwrap_or(u32::MAX))
    else {
        return Err(reject());
    };
    let Some(argument) = arguments.get(lane) else {
        return Err(reject());
    };
    if argument.access != StructuralAccess::SharedBorrow || !argument.path.is_empty() {
        return Err(reject());
    }
    if boundary
        .structural_parameters
        .get(lane)
        .is_none_or(|expected| expected.access != custody.access)
    {
        return Err(reject());
    }
    let OperationResult::Structural(result) = &operation.result else {
        return Err(reject());
    };
    let terminal_psi::BoundaryMachineResult::Structural(declared) = &boundary.result else {
        return Err(reject());
    };
    if result.structural_type != declared.structural_type
        || result.multiplicity != custody.result_multiplicity
        || result.multiplicity != declared.multiplicity
        || !result.qualifications.iter().any(|domain| {
            module.structural_domains.iter().any(|declaration| {
                declaration.id == *domain
                    && declaration.semantic_domain == custody.retained_semantic_domain
            })
        })
    {
        return Err(reject());
    }
    let mut loan = machine
        .entry_claims
        .iter()
        .filter(|claim| claim.input == argument.place)
        .map(|claim| claim.claim)
        .collect::<Vec<_>>();
    loan.extend(
        machine
            .content_entry_claims
            .iter()
            .filter(|claim| claim.input.root == argument.place)
            .map(|claim| claim.claim),
    );
    loan.extend(
        linear_call_result(machine, argument.place)
            .into_iter()
            .flat_map(|(_, produced)| produced.claims.iter().map(|binding| binding.claim)),
    );
    loan.sort();
    let mut bound = result
        .claims
        .iter()
        .map(|binding| binding.claim)
        .collect::<Vec<_>>();
    bound.sort();
    if loan.is_empty()
        || bound != loan
        || result.claims.iter().any(|binding| !binding.path.is_empty())
    {
        return Err(reject());
    }
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
