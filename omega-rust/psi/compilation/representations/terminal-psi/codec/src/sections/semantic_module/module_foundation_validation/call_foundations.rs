//! Call operations: Unit, structural and scalar calls with their argument
//! and result shapes, boundary calls and port writes.

use super::{
    StructuralArgumentPresentation, callee_exact_payloadless_return, has_service,
    has_structural_type, is_claim_free_structural_call, validate_claim_actions,
    validate_structural_arguments, validate_structural_path,
};
use crate::codec_error::{CodecError, malformed};
use semantic_vocabulary::StructuralPlaceKind;
use std::collections::BTreeSet;
use terminal_psi::{
    BoundaryMachineResult, BoundaryStructuralResultDeclaration, ClaimTransfer, Operation,
    OperationKind, OperationResult, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
};

/// The shared ordinary-call tail: structural operands present against the
/// callee's formals and every claim transfer lands on a structural argument.
fn validate_call_tail(
    module: &TerminalModule,
    machine: &TerminalMachine,
    structural_arguments: &[StructuralArgument],
    structural_parameters: &[StructuralParameterDeclaration],
    claim_transfers: &[ClaimTransfer],
) -> Result<(), CodecError> {
    validate_structural_arguments(
        module,
        machine,
        structural_arguments,
        structural_parameters,
        StructuralArgumentPresentation::Ordinary,
    )?;
    validate_claim_actions(
        machine,
        structural_arguments,
        claim_transfers
            .iter()
            .map(|transfer| (transfer.claim, transfer.argument_index)),
    )?;
    for transfer in claim_transfers {
        let parameter = structural_parameters
            .get(transfer.argument_index as usize)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "claim transfer has no formal parameter",
            ))?;
        validate_structural_path(module, parameter.structural_type, &transfer.path)?;
    }
    Ok(())
}

pub(super) fn validate_call_unit(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::CallUnit {
        callee,
        structural_arguments,
        claim_transfers,
        ..
    } = &operation.kind
    else {
        unreachable!("dispatched validate_call_unit")
    };
    if operation.result != OperationResult::Unit {
        return malformed("unit call declares a scalar result");
    }
    let Some(callee) = module.machine(*callee) else {
        return malformed("unit call references an unknown callee");
    };
    if callee.result != TerminalMachineResult::Unit
        || structural_arguments.len() != callee.structural_parameters.len()
    {
        return malformed("unit call has the wrong callee result or structural arity");
    }
    validate_call_tail(
        module,
        machine,
        structural_arguments,
        &callee.structural_parameters,
        claim_transfers,
    )
}

/// A selector call names one interface slot. Its result and operand arity
/// follow the slot's call shape; its structural operands present against
/// the formals of every row the module realizes, which the catalog requires
/// to match that shape exactly.
pub(super) fn validate_call_dynamic(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::CallDynamic {
        interface,
        slot,
        arguments,
        structural_arguments,
        claim_transfers,
        ..
    } = &operation.kind
    else {
        unreachable!("dispatched validate_call_dynamic")
    };
    let Some(shape) = module
        .dynamic_dispatch
        .interface(*interface)
        .and_then(|interface| interface.slots.get(usize::try_from(*slot).ok()?))
    else {
        return malformed("dynamic call names an unknown interface slot");
    };
    let result_matches = match (&operation.result, shape.result) {
        (OperationResult::Unit, None) => true,
        (OperationResult::Scalar(result), Some(expected)) => result.scalar_type == expected,
        _ => false,
    };
    if !result_matches
        || arguments.len() != shape.scalar_parameters.len()
        || structural_arguments.len() != shape.structural_parameters.len()
    {
        return malformed("dynamic call disagrees with its slot's call shape");
    }
    if shape
        .entry_claim_paths
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return malformed("dynamic slot claim paths are not canonical");
    }
    for (position, expected) in shape.entry_claim_paths.iter().enumerate() {
        let Some(formal) = shape
            .structural_parameters
            .get(expected.argument_index as usize)
        else {
            return malformed("dynamic slot claim has no structural formal");
        };
        if formal.access != terminal_psi::StructuralAccess::Owned
            || formal.multiplicity == StructuralMultiplicity::Unrestricted
            || (expected.path.is_empty() && formal.multiplicity != StructuralMultiplicity::Linear)
            || expected.path.iter().any(|segment| {
                !matches!(
                    segment,
                    StructuralPathSegment::Field(_)
                        | StructuralPathSegment::Case(_)
                        | StructuralPathSegment::FixedIndex(_)
                )
            })
            || shape.entry_claim_paths[..position].iter().any(|prior| {
                prior.argument_index == expected.argument_index
                    && expected.path.starts_with(&prior.path)
            })
        {
            return malformed("dynamic slot claim is not an independent owned path");
        }
        validate_structural_path(module, formal.structural_type, &expected.path)?;
    }
    if shape
        .structural_parameters
        .iter()
        .enumerate()
        .any(|(argument_index, formal)| {
            formal.access == terminal_psi::StructuralAccess::Owned
                && formal.multiplicity == StructuralMultiplicity::Linear
                && !shape
                    .entry_claim_paths
                    .iter()
                    .any(|claim| claim.argument_index as usize == argument_index)
        })
    {
        return malformed("dynamic slot linear formal has no entry claim path");
    }
    for transfer in claim_transfers {
        if !shape.entry_claim_paths.iter().any(|expected| {
            expected.argument_index == transfer.argument_index && expected.path == transfer.path
        }) {
            return malformed("dynamic call claim path disagrees with its slot");
        }
    }
    for row in module
        .dynamic_dispatch
        .tables
        .iter()
        .filter(|table| table.interface == *interface)
        .filter_map(|table| table.rows.get(*slot as usize).copied().flatten())
    {
        let Some(callee) = module.machine(row) else {
            return malformed("dynamic table row references an unknown machine");
        };
        let mut entry_claim_paths = callee
            .entry_claims
            .iter()
            .map(|claim| {
                Some(terminal_psi::TerminalDynamicClaimPath {
                    argument_index: u32::try_from(
                        callee
                            .structural_parameters
                            .iter()
                            .position(|parameter| parameter.place == claim.input)?,
                    )
                    .ok()?,
                    path: claim.path.clone(),
                })
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(CodecError::MalformedStructuralFoundation(
                "dynamic row entry claim has no parameter",
            ))?;
        entry_claim_paths.sort();
        if entry_claim_paths != shape.entry_claim_paths {
            return malformed("dynamic row claim frontier disagrees with its slot");
        }
        validate_structural_arguments(
            module,
            machine,
            structural_arguments,
            &callee.structural_parameters,
            StructuralArgumentPresentation::Ordinary,
        )?;
    }
    validate_claim_actions(
        machine,
        structural_arguments,
        claim_transfers
            .iter()
            .map(|transfer| (transfer.claim, transfer.argument_index)),
    )?;
    Ok(())
}

pub(super) fn validate_call_structural_scalar(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::CallStructuralScalar {
        callee,
        arguments,
        structural_arguments,
        claim_transfers,
        ..
    } = &operation.kind
    else {
        unreachable!("dispatched validate_call_structural_scalar")
    };
    let Some(callee) = module.machine(*callee) else {
        return malformed("structural scalar call references an unknown callee");
    };
    if arguments.len() != callee.parameters.len()
        || operation.result.scalar().map(|result| result.scalar_type)
            != callee.result.scalar().map(|result| result.scalar_type)
        || operation.result == OperationResult::Unit
        || structural_arguments.len() != callee.structural_parameters.len()
    {
        return malformed(
            "structural scalar call has the wrong callee signature or structural arity",
        );
    }
    validate_call_tail(
        module,
        machine,
        structural_arguments,
        &callee.structural_parameters,
        claim_transfers,
    )
}

pub(super) fn validate_call_structural(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let (terminal_psi::call_structural_pair_pattern!(
        callee,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
        ..
    )) = &operation.kind
    else {
        unreachable!("dispatched validate_call_structural")
    };
    let arguments = match &operation.kind {
        OperationKind::CallStructuralWithScalarArguments { arguments, .. } => arguments.as_slice(),
        _ => &[],
    };
    let selected_evidence = match &operation.kind {
        OperationKind::CallStructural {
            selected_evidence, ..
        } => selected_evidence.as_slice(),
        _ => &[],
    };
    let Some(callee) = module.machine(*callee) else {
        return malformed("structural call references an unknown callee");
    };
    let Some(expected_result) = callee.result.structural() else {
        return malformed("structural call references a non-structural-result callee");
    };
    let Some(actual_result) = operation.result.structural() else {
        return malformed("structural call has no structural operation result");
    };
    let exact_payloadless = callee.parameters.is_empty()
        && callee.structural_parameters.is_empty()
        && callee.entry_claims.is_empty()
        && callee.content_entry_claims.is_empty()
        && callee.contract.requires.is_empty()
        && callee.contract.ensures.is_empty()
        && callee.contract.crash_routes.is_empty()
        && module
            .evidence_contract_lanes
            .iter()
            .all(|lane| lane.machine != callee.id)
        && structural_arguments.is_empty()
        && claim_transfers.is_empty()
        && requirement_obligations.is_empty()
        && crash_continuations.is_empty()
        && actual_result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
        && expected_result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
        && actual_result.qualifications.is_empty()
        && expected_result.qualifications.is_empty()
        && actual_result.claims.is_empty()
        && callee_exact_payloadless_return(callee);
    let claim_free_result = is_claim_free_structural_call(actual_result, callee)
        && selected_evidence.is_empty()
        && claim_transfers.is_empty()
        && requirement_obligations.len() == callee.contract.requires.len();
    if arguments.len() != callee.parameters.len()
        || structural_arguments.len() != callee.structural_parameters.len()
        || (!selected_evidence.is_empty() && !exact_payloadless)
        || actual_result.structural_type != expected_result.structural_type
        || actual_result.multiplicity != expected_result.multiplicity
        || actual_result.qualifications != expected_result.qualifications
    {
        return malformed("structural call has the wrong callee signature or structural arity");
    }
    let Some(StructuralPlaceDeclaration {
        kind:
            StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            },
        ..
    }) = machine.structural_place(actual_result.place)
    else {
        return malformed("structural call result has no operation-result place declaration");
    };
    if *producer != operation.id || *structural_type != actual_result.structural_type {
        return malformed("structural call result place disagrees with its producer");
    }
    if !has_structural_type(module, actual_result.structural_type) {
        return malformed("structural call result has an unknown structural type");
    }
    for qualification in &actual_result.qualifications {
        if !module.structural_domain(*qualification).is_some() {
            return malformed("structural call result has an unknown structural domain");
        }
    }
    if exact_payloadless {
        return Ok(());
    }
    if claim_free_result {
        return validate_structural_arguments(
            module,
            machine,
            structural_arguments,
            &callee.structural_parameters,
            StructuralArgumentPresentation::Ordinary,
        );
    }
    // Input claims cross at their owned arguments; the result binds the
    // claims the callee's returning edge holds, one per result path. The
    // correspondence is by path, so a callee may return different inputs or
    // fresh establishments at one path on different exits, and the caller
    // never names a callee-local claim identity.
    if requirement_obligations.len() != callee.contract.requires.len() {
        return malformed("structural call obligations disagree with its callee contract");
    }
    let mut result_paths = Vec::with_capacity(actual_result.claims.len());
    for binding in &actual_result.claims {
        validate_structural_path(module, actual_result.structural_type, &binding.path)?;
        if result_paths
            .iter()
            .any(|previous: &Vec<StructuralPathSegment>| {
                previous.starts_with(&binding.path) || binding.path.starts_with(previous)
            })
        {
            return malformed("structural call result has overlapping claim paths");
        }
        result_paths.push(binding.path.clone());
    }
    let caller_result_claims = actual_result
        .claims
        .iter()
        .map(|binding| binding.claim)
        .collect::<BTreeSet<_>>();
    let whole_root_claim =
        matches!(actual_result.claims.as_slice(), [binding] if binding.path.is_empty());
    if caller_result_claims.len() != actual_result.claims.len()
        || match actual_result.multiplicity {
            StructuralMultiplicity::Linear => !whole_root_claim,
            StructuralMultiplicity::Affine => actual_result
                .claims
                .iter()
                .any(|binding| binding.path.is_empty()),
            StructuralMultiplicity::Unrestricted => !actual_result.claims.is_empty(),
        }
    {
        return malformed("structural call result claims disagree with its multiplicity");
    }
    if !terminal_semantics::result_claim_catalog_matches_returns(
        &actual_result.claims,
        callee
            .blocks
            .iter()
            .filter_map(|block| match &block.terminator {
                Terminator::ReturnStructural {
                    returned_claims, ..
                } => Some(returned_claims.as_slice()),
                _ => None,
            }),
    ) {
        return malformed("structural callee return disagrees with the caller's result claims");
    }
    validate_call_tail(
        module,
        machine,
        structural_arguments,
        &callee.structural_parameters,
        claim_transfers,
    )
}

/// Authenticate the terminal invocation's representation, not its runtime
/// authority. The instance must separately establish actual delegated
/// containing-process-domain authority for this exact boundary slot.
pub(super) fn validate_external_completion(
    module: &TerminalModule,
    machine: &TerminalMachine,
    block: &terminal_psi::Block,
) -> Result<(), CodecError> {
    let Terminator::ExternalCompletion {
        boundary,
        kind,
        arguments,
        structural_arguments,
        requirement_obligations,
        ..
    } = &block.terminator
    else {
        return Ok(());
    };
    let Some(declaration) =
        terminal_psi::find_boundary_machine(&module.boundary_machines, *boundary)
    else {
        return malformed("external completion references an unknown boundary");
    };
    if declaration.completion != terminal_psi::BoundaryCompletion::External(*kind)
        || declaration.result != BoundaryMachineResult::Unit
        || declaration.scalar_parameters.len() != 1
        || !matches!(declaration.scalar_parameters.first(),
            Some(semantic_vocabulary::ScalarType::Integer(integer))
                if integer.sign() == semantic_vocabulary::IntegerSign::Signed
                    && integer.bits() == 32 && !integer.is_address())
        || !declaration.structural_parameters.is_empty()
    {
        return malformed(
            "external completion disagrees with the canonical process-exit signature",
        );
    }
    if arguments.len() != declaration.scalar_parameters.len()
        || structural_arguments.len() != declaration.structural_parameters.len()
        || requirement_obligations.len() != declaration.scalar_requires.len()
    {
        return malformed("external completion has the wrong argument or obligation arity");
    }
    for (argument, expected) in arguments.iter().zip(&declaration.scalar_parameters) {
        let mut declarations = machine
            .parameters
            .iter()
            .copied()
            .chain(block.parameters.iter().copied())
            .chain(
                block
                    .operations
                    .iter()
                    .filter_map(|operation| operation.result.scalar()),
            )
            .filter(|candidate| candidate.id == *argument);
        if declarations
            .next()
            .is_none_or(|candidate| candidate.scalar_type != *expected)
            || declarations.next().is_some()
        {
            return malformed("external completion status has no exact runtime scalar declaration");
        }
    }
    validate_structural_arguments(
        module,
        machine,
        structural_arguments,
        &declaration.structural_parameters,
        StructuralArgumentPresentation::Boundary,
    )
}

pub(super) fn validate_boundary_call(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::BoundaryCall {
        boundary,
        arguments,
        structural_arguments,
        completion_receipts,
        requirement_obligations,
    } = &operation.kind
    else {
        unreachable!("dispatched validate_boundary_call")
    };
    let Some(boundary) = terminal_psi::find_boundary_machine(&module.boundary_machines, *boundary)
    else {
        return malformed("boundary call references an unknown boundary");
    };
    if boundary.completion != semantic_vocabulary::BoundaryCompletion::NormalReturn {
        return malformed("external completion requires a closed terminal invocation");
    }
    let actual_result = match &operation.result {
        OperationResult::Unit => Some(BoundaryMachineResult::Unit),
        OperationResult::Scalar(result) => Some(BoundaryMachineResult::Scalar(result.scalar_type)),
        OperationResult::Structural(result)
            if result.projected_qualifications.is_empty()
                && (result.claims.is_empty()
                    || result.multiplicity == StructuralMultiplicity::Linear) =>
        {
            Some(BoundaryMachineResult::Structural(
                BoundaryStructuralResultDeclaration {
                    structural_type: result.structural_type,
                    multiplicity: result.multiplicity,
                    qualifications: result.qualifications.clone(),
                },
            ))
        }
        OperationResult::Structural(_) => None,
    };
    if actual_result.as_ref() != Some(&boundary.result) {
        return malformed("boundary call result disagrees with its declaration");
    }
    if arguments.len() != boundary.scalar_parameters.len() {
        return malformed("boundary call has the wrong scalar arity");
    }
    if structural_arguments.len() != boundary.structural_parameters.len() {
        return malformed("boundary call has the wrong structural arity");
    }
    if requirement_obligations.len() != boundary.scalar_requires.len() {
        return malformed("boundary call has the wrong requirement obligation arity");
    }
    validate_structural_arguments(
        module,
        machine,
        structural_arguments,
        &boundary.structural_parameters,
        StructuralArgumentPresentation::Boundary,
    )?;
    validate_claim_actions(
        machine,
        structural_arguments,
        completion_receipts
            .iter()
            .map(|settlement| (settlement.claim, settlement.argument_index)),
    )?;
    Ok(())
}

pub(super) fn validate_port_write(
    module: &TerminalModule,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::PortWrite { service, .. } = &operation.kind else {
        unreachable!("dispatched validate_port_write")
    };
    if operation.result != OperationResult::Unit {
        return malformed("port write declares a scalar result");
    }
    if !has_service(module, *service) {
        return malformed("port write references an unknown service");
    }
    Ok(())
}
