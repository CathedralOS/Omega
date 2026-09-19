//! Validation of primitive structural calls.

use crate::validation::structural_operations::claim_transfers::{
    validate_service_reach, validate_unit_call_claim_transfers,
};
use crate::validation::structural_operations::crash_continuations::validate_unit_call_crash_continuations;
use crate::validation::structural_operations::structural_arguments::{
    StructuralArgumentSourcePolicy, is_unrestricted_mutable_subloan, validate_structural_arguments,
};
use crate::validation::{
    BTreeMap, MachineId, ModuleError, OperationKind, StructuralMultiplicity, StructuralPlaceKind,
    TerminalMachine, TerminalModule,
};

/// Scalar cases and unrestricted primitive arrays share argument, requirement,
/// crash, and service checks. Selecting an executable payload shape supplies no
/// facts or permission to skip the callee body, contract, or availability checks.
pub(crate) fn validate_primitive_structural_call(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    operation: &terminal_psi::Operation,
) -> Result<bool, ModuleError> {
    let Some(result) = operation.result.structural() else {
        return Ok(false);
    };
    let primitive_payload =
        crate::validation::scalar_case::plain_type(module, result.structural_type)
            || crate::validation::record::plain_type(module, result.structural_type)
            || (result.multiplicity == StructuralMultiplicity::Unrestricted
                && terminal_semantics::scalar_array_leaf_shape(
                    module.structural_types.iter(),
                    result.structural_type,
                )
                .is_some());
    if !primitive_payload
        || !matches!(
            result.multiplicity,
            StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
        )
        || !result.claims.is_empty()
    {
        return Ok(false);
    }
    let (
        callee_id,
        arguments,
        structural_arguments,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations,
        crash_continuations,
    ) = match &operation.kind {
        OperationKind::CallStructuralWithScalarArguments {
            callee,
            arguments,
            erased_arguments: _,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => (
            *callee,
            arguments.as_slice(),
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
        ),
        OperationKind::CallStructural {
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
            selected_evidence,
        } if selected_evidence.is_empty() => (
            *callee,
            &[][..],
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
        ),
        _ => return Ok(false),
    };
    let callee = machines
        .get(&callee_id)
        .copied()
        .ok_or(ModuleError::UnknownCallTarget {
            operation: operation.id,
            callee: callee_id,
        })?;
    if !callee.contract.outcome_specific_ensures.is_empty() {
        return Ok(false);
    }
    let failure = || ModuleError::StructuralCallTargetMismatch {
        operation: operation.id,
        callee: callee_id,
    };
    let signature = callee.result.structural().ok_or_else(failure)?;
    if !matches!(result.multiplicity, StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted)
        || !crate::validation::structural_result_contracts::call_result_matches(result, signature)
        || !result.qualifications.is_empty() || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty() || !claim_transfers.is_empty() || !returned_claim_transfers.is_empty()
        || !callee.entry_claims.is_empty() || !callee.content_entry_claims.is_empty()
        || !callee.content_identity_reshuffles.is_empty() || !callee.content_partition_compositions.is_empty()
        || module.evidence_contract_lanes.iter().any(|lane| lane.machine == callee.id)
        || requirement_obligations.len() != callee.contract.requires.len()
        || !machine.structural_places.iter().any(|place| place.id == result.place
            && matches!(place.kind, StructuralPlaceKind::OperationResult { producer, structural_type }
                if producer == operation.id && structural_type == result.structural_type))
    {
        return Err(failure());
    }
    if arguments.len() != callee.parameters.len() {
        return Err(ModuleError::CallArgumentArityMismatch {
            operation: operation.id,
            expected: callee.parameters.len(),
            actual: arguments.len(),
        });
    }
    let borrowed_projections = structural_arguments
        .iter()
        .zip(&callee.structural_parameters)
        .all(|(argument, expected)| {
            argument.path.is_empty() || is_unrestricted_mutable_subloan(machine, expected, argument)
        });
    validate_structural_arguments(
        module,
        machine,
        structural_arguments,
        &callee.structural_parameters,
        operation.id,
        borrowed_projections,
        StructuralArgumentSourcePolicy::ParametersOrAffineLocalsAndCallResults,
    )?;
    validate_unit_call_claim_transfers(
        module,
        machine,
        callee,
        structural_arguments,
        claim_transfers,
        operation.id,
    )?;
    validate_service_reach(
        operation.id,
        &machine.published_service_ceiling,
        &callee.published_service_ceiling,
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
    Ok(true)
}
