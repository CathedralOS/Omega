//! Call operations: Unit, structural and scalar calls with their argument
//! and result shapes, boundary calls, port writes and dynamic Unit calls.

use super::{
    StructuralArgumentPresentation, callee_exact_payloadless_return, has_service,
    has_structural_type, is_claim_free_structural_call, validate_claim_indices,
    validate_structural_arguments, validate_structural_path,
};
use crate::codec_error::{CodecError, malformed};
use semantic_vocabulary::StructuralPlaceKind;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    BoundaryMachineResult, BoundaryStructuralResultDeclaration, Operation, OperationKind,
    OperationResult, StructuralMultiplicity, StructuralPathSegment, StructuralPlaceDeclaration,
    TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
};

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
    let Some(callee) = module
        .machines
        .iter()
        .find(|candidate| candidate.id == *callee)
    else {
        return malformed("unit call references an unknown callee");
    };
    if callee.result != TerminalMachineResult::Unit
        || structural_arguments.len() != callee.structural_parameters.len()
    {
        return malformed("unit call has the wrong callee result or structural arity");
    }
    validate_structural_arguments(
        module,
        machine,
        structural_arguments,
        &callee.structural_parameters,
        StructuralArgumentPresentation::Ordinary,
    )?;
    validate_claim_indices(
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
    let Some(callee) = module
        .machines
        .iter()
        .find(|candidate| candidate.id == *callee)
    else {
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
    validate_structural_arguments(
        module,
        machine,
        structural_arguments,
        &callee.structural_parameters,
        StructuralArgumentPresentation::Ordinary,
    )?;
    validate_claim_indices(
        machine,
        structural_arguments,
        claim_transfers
            .iter()
            .map(|transfer| (transfer.claim, transfer.argument_index)),
    )?;
    Ok(())
}

pub(super) fn validate_call_structural(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let (OperationKind::CallStructural {
        callee,
        structural_arguments,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations,
        crash_continuations,
        ..
    }
    | OperationKind::CallStructuralWithScalarArguments {
        callee,
        structural_arguments,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations,
        crash_continuations,
        ..
    }) = &operation.kind
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
    let Some(callee) = module
        .machines
        .iter()
        .find(|candidate| candidate.id == *callee)
    else {
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
        && returned_claim_transfers.is_empty()
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
        && returned_claim_transfers.is_empty()
        && requirement_obligations.len() == callee.contract.requires.len();
    if arguments.len() != callee.parameters.len()
        || structural_arguments.len() != callee.structural_parameters.len()
        || (!selected_evidence.is_empty() && !exact_payloadless)
        || (!exact_payloadless
            && !claim_free_result
            && (structural_arguments.len() != 1 || callee.structural_parameters.len() != 1))
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
    }) = machine
        .structural_places
        .iter()
        .find(|place| place.id == actual_result.place)
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
        if !module
            .structural_domains
            .iter()
            .any(|domain| domain.id == *qualification)
        {
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
    if actual_result.claims.is_empty()
        || claim_transfers.is_empty()
        || claim_transfers
            .iter()
            .any(|transfer| transfer.argument_index != 0)
        || returned_claim_transfers.is_empty()
    {
        return malformed("structural call requires a nonempty whole-root claim map");
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
        .map(|binding| (binding.claim, binding.path.as_slice()))
        .collect::<BTreeMap<_, _>>();
    let callee_claims = callee
        .entry_claims
        .iter()
        .map(|claim| (claim.claim, claim.path.as_slice()))
        .collect::<BTreeMap<_, _>>();
    let transferred_caller_claims = claim_transfers
        .iter()
        .map(|transfer| transfer.claim)
        .collect::<BTreeSet<_>>();
    let returned_callee_claims = returned_claim_transfers
        .iter()
        .map(|transfer| transfer.callee_claim)
        .collect::<BTreeSet<_>>();
    let returned_caller_claims = returned_claim_transfers
        .iter()
        .map(|transfer| transfer.caller_claim)
        .collect::<BTreeSet<_>>();
    if callee_claims.is_empty()
        || callee_claims.len() != callee.entry_claims.len()
        || caller_result_claims.len() != actual_result.claims.len()
        || transferred_caller_claims.len() != claim_transfers.len()
        || returned_callee_claims.len() != returned_claim_transfers.len()
        || returned_caller_claims.len() != returned_claim_transfers.len()
        || returned_callee_claims != callee_claims.keys().copied().collect()
        || returned_caller_claims != caller_result_claims.keys().copied().collect()
        || transferred_caller_claims != caller_result_claims.keys().copied().collect()
        || returned_claim_transfers.iter().any(|transfer| {
            callee_claims.get(&transfer.callee_claim)
                != caller_result_claims.get(&transfer.caller_claim)
        })
    {
        return malformed("structural call returned claims disagree with its result bindings");
    }
    let expected_callee_returns = callee
        .entry_claims
        .iter()
        .map(|claim| claim.claim)
        .collect::<Vec<_>>();
    if callee.blocks.iter().any(|block| {
        matches!(
            &block.terminator,
            Terminator::ReturnStructural {
                returned_claims,
                ..
            } if returned_claims != &expected_callee_returns
        )
    }) {
        return malformed("structural callee return does not preserve its exact entry claim map");
    }
    validate_structural_arguments(
        module,
        machine,
        structural_arguments,
        &callee.structural_parameters,
        StructuralArgumentPresentation::Ordinary,
    )?;
    validate_claim_indices(
        machine,
        structural_arguments,
        claim_transfers
            .iter()
            .map(|transfer| (transfer.claim, transfer.argument_index)),
    )?;
    Ok(())
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
        ..
    } = &operation.kind
    else {
        unreachable!("dispatched validate_boundary_call")
    };
    let Some(boundary) = module
        .boundary_machines
        .iter()
        .find(|candidate| candidate.id == *boundary)
    else {
        return malformed("boundary call references an unknown boundary");
    };
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
    validate_structural_arguments(
        module,
        machine,
        structural_arguments,
        &boundary.structural_parameters,
        StructuralArgumentPresentation::Boundary,
    )?;
    validate_claim_indices(
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
