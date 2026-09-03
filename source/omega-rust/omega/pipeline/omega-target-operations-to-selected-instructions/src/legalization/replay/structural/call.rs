use super::super::shared::*;
use super::callee_contract::replay_callee_alpha_match;

#[allow(clippy::too_many_arguments)]
pub(crate) fn replay_structural_call(
    function: usize,
    target_call: &TargetUnitOperation,
    abstract_call: &AbstractOperation,
    optimized_call: &omega_optimization_unit::OptimizationNode,
    proposed: &omega_legalized_operations::LegalizedCallUnit,
    caller_parameters: &[omega_legalized_operations::LegalizedCallUnitParameter],
    caller_claims: &[psi_terminal::EntryClaim],
    target_plan: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let (
        psi_operation,
        callee,
        structural_arguments,
        target_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
        expected_source,
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
                result: omega_abstract_operations::AbstractBoundaryResult::Unit,
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
        || proposed.source != expected_source
        || proposed.operation != psi_operation
        || proposed.callee != callee
        || proposed.claim_transfers != *claim_transfers
        || proposed.requirement_obligations != requirement_obligations
        || proposed.crash_continuations != crash_continuations
        || proposed.fuel != optimized_call.fuel
        || proposed.effect != optimized_call.effect
        || proposed.ownership != optimized_call.ownership
        || proposed.arguments.len() != structural_arguments.len()
        || proposed.arguments.len() != target_arguments.len()
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    proposed
        .validate_source()
        .map_err(|_| Error::NonCanonicalLegalizedPlan)?;
    for (((proposed_argument, semantic), target_argument), source) in proposed
        .arguments
        .iter()
        .zip(structural_arguments)
        .zip(target_arguments)
        .map(|triple| {
            let source = caller_parameters
                .iter()
                .find(|parameter| parameter.semantic.place == triple.0.1.place);
            (triple, source)
        })
    {
        let Some(source) = source else {
            return Err(Error::NonCanonicalLegalizedPlan);
        };
        if proposed_argument.semantic != *semantic
            || proposed_argument.target != *target_argument
            || semantic.place != target_argument.place
            || semantic.access != target_argument.access
            || !semantic.path.is_empty()
            || !target_argument.path.is_empty()
            || target_argument.root_structural_type != source.semantic.structural_type
            || target_argument.structural_type != source.semantic.structural_type
            || target_argument.shape != source.target.shape
            || target_argument.source_byte_offset != 0
            || target_argument.fixed_array_length.is_some()
            || target_argument.element_stride.is_some()
            || target_argument.source != source.target.placement
        {
            return Err(Error::NonCanonicalLegalizedPlan);
        }
    }

    replay_callee_alpha_match(
        function,
        callee,
        proposed,
        caller_claims,
        target_plan,
        abstract_plan,
        unit,
    )
}
