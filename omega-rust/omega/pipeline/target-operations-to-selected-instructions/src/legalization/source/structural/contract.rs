use super::super::matchers::MatchedStructuralUnitForm;
use super::super::shared::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_and_derive_parameters(
    function: usize,
    target: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    target_plan: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    matched: &MatchedStructuralUnitForm<'_>,
) -> Result<Vec<LegalizedCallUnitParameter>, LegalizationError> {
    let TargetOperation::UnitBody(body) = &target.operation else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [abstract_entry] = abstracted.block_entries.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [optimized_block] = optimized.blocks.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let TargetUnitOperation::Return {
        psi_edge,
        cleanup_actions,
    } = matched.target_return
    else {
        unreachable!()
    };
    let expected_provenance = TerminalPsiProvenance {
        operations: if let Some((_, abstract_settlements, _)) = matched.settlement_rows {
            abstract_settlements
                .iter()
                .filter_map(|operation| match operation {
                    AbstractOperation::BoundaryCall { psi_operation, .. } => Some(*psi_operation),
                    _ => None,
                })
                .collect()
        } else {
            matched
                .abstract_call
                .and_then(|operation| match operation {
                    AbstractOperation::CallUnit { psi_operation, .. }
                    | AbstractOperation::BoundaryCall { psi_operation, .. } => Some(*psi_operation),
                    _ => None,
                })
                .into_iter()
                .collect()
        },
        edges: vec![*psi_edge],
    };
    let expected_return_effect_input = matched.settlement_rows.map_or_else(
        || u64::from(matched.abstract_call.is_some()),
        |(rows, _, _)| rows.len() as u64,
    );
    let expected_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target_plan.target),
        &CallSignature {
            parameters: body
                .parameters
                .iter()
                .map(|parameter| parameter.shape)
                .collect(),
            result: None,
        },
    )
    .map_err(|_| Error::UnsupportedSourceShape { function })?;
    if target.machine != abstracted.machine
        || target.machine != optimized.machine
        || target.attachment != abstracted.attachment
        || target.attachment != optimized.attachment
        || target.provenance != expected_provenance
        || abstracted.result != abstract_operations::AbstractFunctionResult::Unit
        || optimized.result != abstracted.result
        || !abstracted.parameters.is_empty()
        || !optimized.parameters.is_empty()
        || body.structural_types != abstract_plan.structural_types
        || body.structural_types != unit.structural_types
        || body.call_plan != expected_call_plan
        || abstracted.structural_parameters != optimized.structural_parameters
        || abstracted.entry_claims != optimized.entry_claim_declarations
        || abstracted.published_service_ceiling != optimized.published_service_ceiling
        || abstracted.entry != abstract_entry.block
        || optimized.entry != abstract_entry.block
        || optimized_block.id != abstract_entry.block
        || abstract_entry.operation_offset != 0
        || !abstract_entry.parameters.is_empty()
        || !optimized_block.parameters.is_empty()
        || !cleanup_actions.is_empty()
        || matched.abstract_return != &matched.optimized_return.operation
        || !matches!(matched.abstract_return, AbstractOperation::ReturnUnit { psi_edge: edge, cleanup_actions } if edge == psi_edge && cleanup_actions.is_empty())
        || matched.optimized_return.provenance != [PsiProvenance::Edge(*psi_edge)]
        || matched.optimized_return.effect.input != expected_return_effect_input
        || matched.optimized_return.effect.output != expected_return_effect_input + 1
        || !matched.optimized_return.definitions.is_empty()
        || !matched.optimized_return.uses.is_empty()
        || !matched.optimized_return.successors.is_empty()
        || matched.optimized_return.ownership != [OwnershipEvent::Cleanup(Vec::new())]
        || body.parameters.len() != abstracted.structural_parameters.len()
    {
        return Err(Error::UnsupportedSourceShape { function });
    }

    let parameters = abstracted
        .structural_parameters
        .iter()
        .zip(&body.parameters)
        .map(|(semantic, target)| {
            (semantic.place == target.place
                && semantic.structural_type == target.structural_type
                && semantic.multiplicity == target.multiplicity
                && semantic.access == target.access)
                .then(|| LegalizedCallUnitParameter {
                    semantic: semantic.clone(),
                    target: target.clone(),
                })
                .ok_or(Error::UnsupportedSourceShape { function })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let expected_places = abstracted
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect::<std::collections::BTreeSet<_>>();
    let expected_claim_ids = abstracted
        .entry_claims
        .iter()
        .map(|claim| claim.claim)
        .collect::<std::collections::BTreeSet<_>>();
    if optimized.declared_places != expected_places
        || optimized.entry_claims != expected_claim_ids
        || abstracted
            .entry_claims
            .iter()
            .any(|claim| !claim.path.is_empty() || !expected_places.contains(&claim.input))
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    Ok(parameters)
}
