//! Structural calls with scalar, mixed or structural results: arguments,
//! claim transfers, result custody and crash continuations validate against
//! the callee.

use crate::validation::structural_operations::claim_transfers::{
    validate_service_reach, validate_unit_call_claim_transfers,
};
use crate::validation::structural_operations::contract_places::validate_unit_call_contract_places;
use crate::validation::structural_operations::crash_continuations::validate_unit_call_crash_continuations;
use crate::validation::structural_operations::payloadless_calls::is_exact_payloadless_structural_call;
use crate::validation::structural_operations::structural_arguments::{
    StructuralArgumentSourcePolicy, structural_paths_may_overlap, validate_structural_arguments,
};
use crate::validation::{
    BTreeMap, BTreeSet, MachineId, ModuleError, OperationKind, StructuralMultiplicity,
    StructuralPlaceKind, TerminalMachine, TerminalModule, Terminator, resolve_structural_path,
};

pub(super) fn validate_call_structural_scalar(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    let OperationKind::CallStructuralScalar {
        callee,
        arguments,
        erased_arguments,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = &operation.kind
    else {
        unreachable!("dispatched validate_call_structural_scalar")
    };
    let callee = machines
        .get(callee)
        .copied()
        .ok_or(ModuleError::UnknownCallTarget {
            operation: operation.id,
            callee: *callee,
        })?;
    let expected = callee.result.scalar().map(|result| result.scalar_type);
    let actual = operation.result.scalar().map(|result| result.scalar_type);
    if expected.is_none() || actual != expected {
        return Err(ModuleError::StructuralScalarCallTargetMismatch {
            operation: operation.id,
            callee: callee.id,
            expected,
            actual,
        });
    }
    // Exact completed-record receiver custody is independent of scalar return shape.
    validate_structural_arguments(
        module,
        machine,
        structural_arguments,
        &callee.structural_parameters,
        operation.id,
        true,
        StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults,
    )?;
    validate_unit_call_contract_places(callee, operation.id)?;
    validate_service_reach(
        operation.id,
        &machine.published_service_ceiling,
        &callee.published_service_ceiling,
    )?;
    if requirement_obligations.len() != callee.contract.requires.len() {
        return Err(ModuleError::CallRequirementArityMismatch {
            operation: operation.id,
            expected: callee.contract.requires.len(),
            actual: requirement_obligations.len(),
        });
    }
    if erased_arguments.len() != callee.contract.erased_scalar_formals.len() {
        return Err(ModuleError::ErasedCallArgumentArityMismatch {
            operation: operation.id,
            expected: callee.contract.erased_scalar_formals.len(),
            actual: erased_arguments.len(),
        });
    }
    crate::validation::validate_erased_argument_terms(machine, operation.id, erased_arguments)?;

    validate_unit_call_claim_transfers(
        module,
        machine,
        callee,
        structural_arguments,
        claim_transfers,
        operation.id,
    )?;
    validate_unit_call_crash_continuations(
        module,
        machine,
        callee,
        arguments,
        structural_arguments,
        crash_continuations,
        operation.id,
    )?;
    Ok(())
}

pub(super) fn validate_call_structural(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
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
    let erased_arguments = match &operation.kind {
        OperationKind::CallStructuralWithScalarArguments {
            erased_arguments, ..
        } => erased_arguments.as_slice(),
        _ => &[],
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
    let callee = machines
        .get(callee)
        .copied()
        .ok_or(ModuleError::UnknownCallTarget {
            operation: operation.id,
            callee: *callee,
        })?;
    let Some(callee_result) = callee.result.structural() else {
        return Err(ModuleError::StructuralCallTargetMismatch {
            operation: operation.id,
            callee: callee.id,
        });
    };
    let Some(result) = operation.result.structural() else {
        return Err(ModuleError::StructuralCallResultMismatch(operation.id));
    };
    if is_exact_payloadless_structural_call(module, operation, machines) {
        let result_place = machine
            .structural_places
            .iter()
            .find(|place| place.id == result.place);
        if !matches!(
            result_place.map(|place| place.kind),
            Some(StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            }) if producer == operation.id && structural_type == result.structural_type
        ) {
            return Err(ModuleError::StructuralCallResultPlaceMismatch(operation.id));
        }
        validate_service_reach(
            operation.id,
            &machine.published_service_ceiling,
            &callee.published_service_ceiling,
        )?;
        if !selected_evidence.is_empty() && callee.contract.outcome_specific_ensures.is_empty() {
            return Err(ModuleError::InvalidOutcomeSpecificCallEvidence {
                caller: machine.id,
                operation: operation.id,
            });
        }
        return Ok(());
    }
    if callee.parameters.len() != arguments.len()
        || structural_arguments.len() != 1
        || !structural_arguments[0].path.is_empty()
        || callee.structural_parameters.len() != 1
        || result.structural_type != callee_result.structural_type
        || result.multiplicity != callee_result.multiplicity
        || !crate::validation::structural_result_contracts::call_result_matches(
            result,
            callee_result,
        )
        || result.multiplicity != StructuralMultiplicity::Linear
    {
        return Err(ModuleError::StructuralCallTargetMismatch {
            operation: operation.id,
            callee: callee.id,
        });
    }
    let result_place = machine
        .structural_places
        .iter()
        .find(|place| place.id == result.place);
    if !matches!(
        result_place.map(|place| place.kind),
        Some(StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        }) if producer == operation.id && structural_type == result.structural_type
    ) {
        return Err(ModuleError::StructuralCallResultPlaceMismatch(operation.id));
    }
    if result
        .qualifications
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
        || result.claims.windows(2).any(|pair| pair[0] >= pair[1])
        || result.claims.is_empty()
        || claim_transfers.is_empty()
        || claim_transfers
            .iter()
            .any(|transfer| transfer.argument_index != 0)
        || returned_claim_transfers.is_empty()
        || returned_claim_transfers
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || result.claims.iter().any(|binding| {
            resolve_structural_path(module, result.structural_type, &binding.path).is_none()
        })
        || result.claims.iter().enumerate().any(|(index, binding)| {
            result.claims[index + 1..]
                .iter()
                .any(|other| structural_paths_may_overlap(&binding.path, &other.path))
        })
    {
        return Err(ModuleError::NonCanonicalStructuralOperationResult(
            operation.id,
        ));
    }
    let callee_claims = callee
        .entry_claims
        .iter()
        .map(|claim| (claim.claim, claim.path.as_slice()))
        .collect::<BTreeMap<_, _>>();
    let result_claims = result
        .claims
        .iter()
        .map(|binding| (binding.claim, binding.path.as_slice()))
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
        || result_claims.len() != result.claims.len()
        || returned_callee_claims.len() != returned_claim_transfers.len()
        || returned_caller_claims.len() != returned_claim_transfers.len()
        || returned_callee_claims != callee_claims.keys().copied().collect()
        || returned_caller_claims != result_claims.keys().copied().collect()
        || transferred_caller_claims != result_claims.keys().copied().collect()
        || returned_claim_transfers.iter().any(|transfer| {
            callee_claims.get(&transfer.callee_claim) != result_claims.get(&transfer.caller_claim)
        })
    {
        return Err(ModuleError::StructuralCallClaimInterfaceMismatch(
            operation.id,
        ));
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
        return Err(ModuleError::StructuralCallClaimInterfaceMismatch(
            operation.id,
        ));
    }
    validate_structural_arguments(
        module,
        machine,
        structural_arguments,
        &callee.structural_parameters,
        operation.id,
        true,
        StructuralArgumentSourcePolicy::ParametersOrLinearCallResults,
    )?;
    validate_unit_call_contract_places(callee, operation.id)?;
    validate_service_reach(
        operation.id,
        &machine.published_service_ceiling,
        &callee.published_service_ceiling,
    )?;
    if requirement_obligations.len() != callee.contract.requires.len() {
        return Err(ModuleError::CallRequirementArityMismatch {
            operation: operation.id,
            expected: callee.contract.requires.len(),
            actual: requirement_obligations.len(),
        });
    }
    if erased_arguments.len() != callee.contract.erased_scalar_formals.len() {
        return Err(ModuleError::ErasedCallArgumentArityMismatch {
            operation: operation.id,
            expected: callee.contract.erased_scalar_formals.len(),
            actual: erased_arguments.len(),
        });
    }
    crate::validation::validate_erased_argument_terms(machine, operation.id, erased_arguments)?;

    validate_unit_call_claim_transfers(
        module,
        machine,
        callee,
        structural_arguments,
        claim_transfers,
        operation.id,
    )?;
    validate_unit_call_crash_continuations(
        module,
        machine,
        callee,
        arguments,
        structural_arguments,
        crash_continuations,
        operation.id,
    )?;
    Ok(())
}
