//! Validation of primitive structural calls.

use crate::validation::structural::operations::claim_transfers::{
    is_record_loan, validate_service_reach, validate_unit_call_claim_transfers,
};
use crate::validation::structural::operations::crash_continuations::validate_unit_call_crash_continuations;
use crate::validation::structural::operations::structural_arguments::{
    StructuralArgumentSourcePolicy, is_unrestricted_mutable_subloan,
    is_unrestricted_shared_subloan, validate_structural_arguments,
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
    // The callee establishes owned bytes and their predicates; this call
    // transfers that exact normal result, not a descriptor or new evidence.
    let owned_bytes = result.multiplicity == StructuralMultiplicity::Unrestricted
        && module.structural_types.iter().any(|declaration| {
            declaration.id == result.structural_type
                && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
                )
        });
    // A completed copy record may contain bounded byte fields with retained
    // predicates. Transport uses its exact result signature; it does not
    // extend the separate record-constructor operand vocabulary.
    let owned_record = result.multiplicity == StructuralMultiplicity::Unrestricted
        && module.structural_types.iter().any(|declaration| {
            declaration.id == result.structural_type
                && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::Record { .. }
                )
        })
        && crate::validation::structural::result_contracts::has_plain_owned_shape(
            module,
            result.structural_type,
        );
    let primitive_payload = owned_bytes
        || owned_record
        || crate::validation::scalar::case::plain_type(module, result.structural_type)
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
            erased_proof_arguments: _,
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
        || !crate::validation::structural::result_contracts::call_result_matches(result, signature)
        || (!result.qualifications.is_empty() && !owned_bytes) || !result.projected_qualifications.is_empty()
        || (owned_bytes && (!signature.reference_sources.is_empty() || !result.qualification_establishments.is_empty()))
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
    // A plain payload result holds no reference and no claim, so a loan
    // presented to this call ends with it. A completed local record lending
    // the exact subtree the formal names is one such loan, as at a Unit call:
    // the local keeps its value, and source, liveness and overlap checks
    // still run per argument below.
    let borrowed_projections = structural_arguments
        .iter()
        .zip(&callee.structural_parameters)
        .all(|(argument, expected)| {
            argument.path.is_empty()
                || is_unrestricted_mutable_subloan(module, machine, expected, argument)
                || is_unrestricted_shared_subloan(module, machine, expected, argument)
                || is_record_loan(module, machine, expected, argument)
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
