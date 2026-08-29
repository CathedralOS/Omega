use super::super::shared::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_callee_alpha_match(
    function: usize,
    callee: psi_core::MachineId,
    proposed: &omega_legalized_operations::LegalizedCallUnit,
    caller_claims: &[psi_terminal::EntryClaim],
    target_plan: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let target_callees = target_plan
        .functions
        .iter()
        .filter(|candidate| candidate.machine == callee)
        .collect::<Vec<_>>();
    let abstract_callees = abstract_plan
        .functions
        .iter()
        .filter(|candidate| candidate.machine == callee)
        .collect::<Vec<_>>();
    let optimized_callees = unit
        .functions
        .iter()
        .filter(|candidate| candidate.machine == callee)
        .collect::<Vec<_>>();
    let ([target_callee], [abstract_callee], [optimized_callee]) = (
        target_callees.as_slice(),
        abstract_callees.as_slice(),
        optimized_callees.as_slice(),
    ) else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let TargetOperation::UnitBody(callee_body) = &target_callee.operation else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let expected_callee_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target_plan.target),
        &CallSignature {
            parameters: callee_body
                .parameters
                .iter()
                .map(|parameter| parameter.shape)
                .collect(),
            result: None,
        },
    )
    .map_err(|_| Error::UnsupportedSourceShape { function })?;
    if abstract_callee.result != omega_abstract_operations::AbstractFunctionResult::Unit
        || optimized_callee.result != abstract_callee.result
        || !abstract_callee.parameters.is_empty()
        || !optimized_callee.parameters.is_empty()
        || abstract_callee.structural_parameters != optimized_callee.structural_parameters
        || abstract_callee.entry_claims != optimized_callee.entry_claim_declarations
        || callee_body.call_plan != expected_callee_plan
        || callee_body.parameters.len() != abstract_callee.structural_parameters.len()
        || proposed.arguments.len() != abstract_callee.structural_parameters.len()
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    for ((argument, semantic_parameter), target_parameter) in proposed
        .arguments
        .iter()
        .zip(&abstract_callee.structural_parameters)
        .zip(&callee_body.parameters)
    {
        if argument.semantic.access != semantic_parameter.access
            || argument.target.structural_type != semantic_parameter.structural_type
            || argument.target.shape != target_parameter.shape
            || argument.target.destination != target_parameter.placement
            || semantic_parameter.place != target_parameter.place
            || semantic_parameter.structural_type != target_parameter.structural_type
            || semantic_parameter.multiplicity != target_parameter.multiplicity
            || semantic_parameter.access != target_parameter.access
        {
            return Err(Error::NonCanonicalLegalizedPlan);
        }
    }
    if proposed.claim_transfers.len() != abstract_callee.entry_claims.len() {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    for (transfer, callee_claim) in proposed
        .claim_transfers
        .iter()
        .zip(&abstract_callee.entry_claims)
    {
        let argument_index = usize::try_from(transfer.argument_index)
            .map_err(|_| Error::NonCanonicalLegalizedPlan)?;
        let Some(argument) = proposed.arguments.get(argument_index) else {
            return Err(Error::NonCanonicalLegalizedPlan);
        };
        let Some(callee_parameter) = abstract_callee.structural_parameters.get(argument_index)
        else {
            return Err(Error::NonCanonicalLegalizedPlan);
        };
        let matching_caller_claims = caller_claims
            .iter()
            .filter(|claim| claim.claim == transfer.claim)
            .collect::<Vec<_>>();
        let [caller_claim] = matching_caller_claims.as_slice() else {
            return Err(Error::NonCanonicalLegalizedPlan);
        };
        if caller_claim.input != argument.semantic.place
            || !caller_claim.path.is_empty()
            || callee_claim.input != callee_parameter.place
            || !callee_claim.path.is_empty()
        {
            return Err(Error::NonCanonicalLegalizedPlan);
        }
    }
    Ok(())
}
