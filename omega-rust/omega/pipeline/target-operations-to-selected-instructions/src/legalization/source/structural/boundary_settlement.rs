use super::super::shared::*;

pub(super) fn validate_input(
    function: usize,
    index: usize,
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    optimized: &optimization_unit::OptimizationNode,
    caller_parameters: &[super::input::Parameter<'_>],
    caller_claims: &[terminal_psi::EntryClaim],
    abstract_plan: &AbstractOperationPlan,
) -> Result<(), LegalizationError> {
    let TargetUnitOperation::BoundarySettlement {
        psi_operation: target_operation,
        boundary: target_boundary,
        result: target_operations::TargetBoundaryResult::Unit,
        execution,
        realization: target_operations::BoundaryRealization::ClaimCompletionOnly(_),
        scalar_arguments,
        runtime_scalar_arguments,
        arguments: target_arguments,
        byte_sequence_arguments,
        completion_claim_sources: target_sources,
        completion_receipts: target_receipts,
    } = target
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let target_operations::BoundaryExecutionBinding::AdmittedProvider(_) = execution else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let AbstractOperation::BoundaryCall {
        psi_operation,
        result: abstract_operations::AbstractBoundaryResult::Unit,
        boundary,
        arguments,
        structural_arguments,
        completion_claim_sources,
        completion_receipts,
    } = abstracted
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let boundary_declarations = abstract_plan
        .boundary_machines
        .iter()
        .filter(|declaration| declaration.id == *boundary)
        .collect::<Vec<_>>();
    let [declaration] = boundary_declarations.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let expected_evidence = structural_arguments
        .iter()
        .enumerate()
        .map(|(argument_index, argument)| {
            let matching_claims = caller_claims
                .iter()
                .filter(|claim| claim.input == argument.place && claim.path.is_empty())
                .collect::<Vec<_>>();
            let [claim] = matching_claims.as_slice() else {
                return Err(Error::UnsupportedSourceShape { function });
            };
            Ok((
                abstract_operations::CompletionClaimSource {
                    claim: claim.claim,
                    entry: Some((*claim).clone()),
                    content: None,
                },
                terminal_psi::CompletionReceipt {
                    claim: claim.claim,
                    argument_index: argument_index as u32,
                },
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let expected_receipts = expected_evidence
        .iter()
        .map(|(_, receipt)| *receipt)
        .collect::<Vec<_>>();
    let expected_sources = caller_claims
        .iter()
        .cloned()
        .map(|entry| abstract_operations::CompletionClaimSource {
            claim: entry.claim,
            entry: Some(entry),
            content: None,
        })
        .collect::<Vec<_>>();
    let completed_claims = expected_receipts
        .iter()
        .map(|receipt| receipt.claim)
        .collect::<Vec<_>>();
    if target_operation != psi_operation
        || target_boundary != boundary
        || !scalar_arguments.is_empty()
        || !runtime_scalar_arguments.is_empty()
        || !arguments.is_empty()
        || !byte_sequence_arguments.is_empty()
        || target_arguments != structural_arguments
        || target_sources != completion_claim_sources
        || target_receipts != completion_receipts
        || structural_arguments.is_empty()
        || !declaration.scalar_parameters.is_empty()
        || !declaration.result.is_unit()
        || !declaration.program_local_root_introductions.is_empty()
        || !declaration.content_guarantees.is_empty()
        || !declaration.published_service_ceiling.is_empty()
        || declaration.structural_parameters.len() != structural_arguments.len()
        || declaration.requires.iter().any(|requirement| {
            requirement.argument_index as usize >= declaration.structural_parameters.len()
        })
        || completion_receipts != &expected_receipts
        || completion_claim_sources != &expected_sources
        || optimized.operation != *abstracted
        || optimized.provenance != [PsiProvenance::Operation(*psi_operation)]
        || optimized.effect.input != index as u64
        || optimized.effect.output != index as u64 + 1
        || !optimized.definitions.is_empty()
        || !optimized.uses.is_empty()
        || !optimized.successors.is_empty()
        || optimized.ownership != [OwnershipEvent::ClaimCompletion(completed_claims)]
        || structural_arguments
            .iter()
            .enumerate()
            .any(|(index, argument)| {
                let caller_matches = caller_parameters
                    .iter()
                    .filter(|parameter| parameter.semantic.place == argument.place)
                    .collect::<Vec<_>>();
                let [caller] = caller_matches.as_slice() else {
                    return true;
                };
                let boundary_parameter = &declaration.structural_parameters[index];
                let mut expected_qualifications = boundary_parameter.qualifications.clone();
                expected_qualifications.extend(
                    declaration
                        .requires
                        .iter()
                        .filter(|requirement| requirement.argument_index as usize == index)
                        .map(|requirement| requirement.domain),
                );
                expected_qualifications.sort_unstable();
                expected_qualifications.dedup();
                !argument.path.is_empty()
                    || argument.access != terminal_psi::StructuralAccess::Owned
                    || caller.semantic.multiplicity != terminal_psi::StructuralMultiplicity::Linear
                    || caller.semantic.access != terminal_psi::StructuralAccess::Owned
                    || boundary_parameter.position != index as u32
                    || boundary_parameter.structural_type != caller.semantic.structural_type
                    || boundary_parameter.multiplicity != caller.semantic.multiplicity
                    || boundary_parameter.access != caller.semantic.access
                    || expected_qualifications != caller.semantic.qualifications
            })
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    Ok(())
}

pub(crate) fn derive_boundary_settlement(
    function: usize,
    index: usize,
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    optimized: &optimization_unit::OptimizationNode,
    caller_parameters: &[LegalizedCallUnitParameter],
    caller_claims: &[terminal_psi::EntryClaim],
    abstract_plan: &AbstractOperationPlan,
) -> Result<LegalizedBoundarySettlement, LegalizationError> {
    let parameters = caller_parameters
        .iter()
        .map(|parameter| super::input::Parameter {
            semantic: &parameter.semantic,
            target: &parameter.target,
        })
        .collect::<Vec<_>>();
    validate_input(
        function,
        index,
        target,
        abstracted,
        optimized,
        &parameters,
        caller_claims,
        abstract_plan,
    )?;
    let TargetUnitOperation::BoundarySettlement {
        execution: target_operations::BoundaryExecutionBinding::AdmittedProvider(provider_execution),
        realization: target_operations::BoundaryRealization::ClaimCompletionOnly(realization),
        ..
    } = target
    else {
        unreachable!()
    };
    let AbstractOperation::BoundaryCall {
        psi_operation,
        boundary,
        structural_arguments,
        completion_claim_sources,
        completion_receipts,
        ..
    } = abstracted
    else {
        unreachable!()
    };
    Ok(LegalizedBoundarySettlement {
        operation: *psi_operation,
        boundary: *boundary,
        provider_execution: *provider_execution,
        realization: *realization,
        arguments: structural_arguments.clone(),
        completion_claim_sources: completion_claim_sources.clone(),
        completion_receipts: completion_receipts.clone(),
        fuel: optimized.fuel.clone(),
        effect: optimized.effect,
        ownership: optimized.ownership.clone(),
    })
}
