use super::super::shared::*;
use super::callee_contract::validate_callee_alpha_match;

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_structural_call(
    function: usize,
    target_call: &TargetUnitOperation,
    abstract_call: &AbstractOperation,
    optimized_call: &omega_optimization_unit::OptimizationNode,
    caller_parameters: &[LegalizedCallUnitParameter],
    caller_claims: &[psi_terminal::EntryClaim],
    target_plan: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<LegalizedCallUnit, LegalizationError> {
    let (
        psi_operation,
        callee,
        structural_arguments,
        target_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
        source,
        expected_ownership,
    ) = match (target_call, abstract_call) {
        (
            TargetUnitOperation::Call {
                psi_operation: target_operation,
                callee: target_callee,
                arguments: target_arguments,
                claim_transfers: target_transfers,
                requirement_obligations: target_requirements,
                crash_continuations: target_crash_continuations,
            },
            AbstractOperation::CallUnit {
                psi_operation,
                callee,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            },
        ) if target_operation == psi_operation
            && target_callee == callee
            && target_transfers == claim_transfers
            && target_requirements == requirement_obligations
            && target_crash_continuations == crash_continuations =>
        {
            (
                *psi_operation,
                *callee,
                structural_arguments,
                target_arguments,
                claim_transfers,
                requirement_obligations.as_slice(),
                crash_continuations.as_slice(),
                omega_legalized_operations::LegalizedCallUnitSource::AuthoredCallUnit,
                OwnershipEvent::ClaimTransfer(
                    claim_transfers
                        .iter()
                        .map(|transfer| transfer.claim)
                        .collect(),
                ),
            )
        }
        (
            TargetUnitOperation::InstalledProviderCall {
                psi_operation: target_operation,
                boundary: target_boundary,
                provider,
                call_plan: _,
                scalar_arguments: target_scalar_arguments,
                source_arguments,
                arguments: target_arguments,
                claim_transfers: target_transfers,
                completion_claim_sources: target_sources,
                completion_receipts: target_receipts,
            },
            AbstractOperation::BoundaryCall {
                psi_operation,
                result: None,
                boundary,
                arguments,
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
            },
        ) if target_operation == psi_operation
            && target_boundary == boundary
            && arguments.is_empty()
            && target_scalar_arguments.is_empty()
            && source_arguments == structural_arguments
            && target_sources == completion_claim_sources
            && target_receipts == completion_receipts
            && target_transfers
                == &completion_receipts
                    .iter()
                    .map(|receipt| psi_terminal::ClaimTransfer {
                        claim: receipt.claim,
                        argument_index: receipt.argument_index,
                    })
                    .collect::<Vec<_>>()
            && provider.boundary == *boundary
            && abstract_plan
                .provider_candidates
                .iter()
                .any(|candidate| candidate == provider) =>
        {
            (
                *psi_operation,
                provider.candidate,
                structural_arguments,
                target_arguments,
                target_transfers,
                &[] as &[psi_core::ObligationId],
                &[] as &[psi_terminal::CrashRouteBucket],
                omega_legalized_operations::LegalizedCallUnitSource::InstalledProvider {
                    boundary: *boundary,
                    provider: provider.clone(),
                    completion_claim_sources: completion_claim_sources.clone(),
                    completion_receipts: completion_receipts.clone(),
                },
                OwnershipEvent::ClaimCompletion(
                    completion_receipts
                        .iter()
                        .map(|receipt| receipt.claim)
                        .collect(),
                ),
            )
        }
        _ => return Err(Error::UnsupportedSourceShape { function }),
    };
    if optimized_call.operation != *abstract_call
        || optimized_call.provenance != [PsiProvenance::Operation(psi_operation)]
        || optimized_call.ownership != [expected_ownership]
        || optimized_call.effect.input != 0
        || optimized_call.effect.output != 1
        || !optimized_call.definitions.is_empty()
        || !optimized_call.uses.is_empty()
        || !optimized_call.successors.is_empty()
        || structural_arguments.len() != target_arguments.len()
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let arguments = structural_arguments
        .iter()
        .zip(target_arguments)
        .map(|(semantic, target)| {
            let source = caller_parameters
                .iter()
                .find(|parameter| parameter.semantic.place == semantic.place)
                .ok_or(Error::UnsupportedSourceShape { function })?;
            (semantic.access == target.access
                && semantic.place == target.place
                && semantic.path.is_empty()
                && target.path.is_empty()
                && target.root_structural_type == source.semantic.structural_type
                && target.structural_type == source.semantic.structural_type
                && target.shape == source.target.shape
                && target.source_byte_offset == 0
                && target.fixed_array_length.is_none()
                && target.element_stride.is_none()
                && target.source == source.target.placement)
                .then(|| LegalizedCallUnitArgument {
                    semantic: semantic.clone(),
                    target: target.clone(),
                })
                .ok_or(Error::UnsupportedSourceShape { function })
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_callee_alpha_match(
        function,
        callee,
        &arguments,
        claim_transfers,
        caller_claims,
        target_plan,
        abstract_plan,
        unit,
    )?;
    let call = LegalizedCallUnit {
        source,
        operation: psi_operation,
        callee,
        arguments,
        claim_transfers: claim_transfers.to_vec(),
        requirement_obligations: requirement_obligations.to_vec(),
        crash_continuations: crash_continuations.to_vec(),
        fuel: optimized_call.fuel.clone(),
        effect: optimized_call.effect,
        ownership: optimized_call.ownership.clone(),
    };
    call.validate_source()
        .map_err(|_| Error::UnsupportedSourceShape { function })?;
    Ok(call)
}
