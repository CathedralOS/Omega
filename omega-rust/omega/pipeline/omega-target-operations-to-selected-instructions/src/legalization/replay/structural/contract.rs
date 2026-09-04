use super::super::shared::*;
use super::super::validators::ValidatedStructuralUnitForm;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_replayed_contract(
    function: usize,
    target: &omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    proposed: &LegalizedStructuralUnitFunction,
    target_plan: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    validated: &ValidatedStructuralUnitForm<'_>,
) -> Result<(), LegalizationError> {
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
    } = validated.target_return
    else {
        unreachable!()
    };
    let expected_provenance = TerminalPsiProvenance {
        operations: if let Some((_, abstract_settlements, _)) = validated.settlement_rows {
            abstract_settlements
                .iter()
                .filter_map(|operation| match operation {
                    AbstractOperation::BoundaryCall { psi_operation, .. } => Some(*psi_operation),
                    _ => None,
                })
                .collect()
        } else {
            validated
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
    let expected_return_effect_input = validated.settlement_rows.map_or_else(
        || u64::from(validated.abstract_call.is_some()),
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
        || abstracted.result != omega_abstract_operations::AbstractFunctionResult::Unit
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
        || validated.abstract_return != &validated.optimized_return.operation
        || !matches!(validated.abstract_return, AbstractOperation::ReturnUnit { psi_edge: edge, cleanup_actions } if edge == psi_edge && cleanup_actions.is_empty())
        || validated.optimized_return.provenance != [PsiProvenance::Edge(*psi_edge)]
        || validated.optimized_return.effect.input != expected_return_effect_input
        || validated.optimized_return.effect.output != expected_return_effect_input + 1
        || !validated.optimized_return.definitions.is_empty()
        || !validated.optimized_return.uses.is_empty()
        || !validated.optimized_return.successors.is_empty()
        || validated.optimized_return.ownership != [OwnershipEvent::Cleanup(Vec::new())]
        || body.parameters.len() != abstracted.structural_parameters.len()
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
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
        || proposed.machine != target.machine
        || proposed.attachment != target.attachment
        || proposed.provenance != target.provenance
        || proposed.structural_types != body.structural_types
        || proposed.call_plan != body.call_plan
        || proposed.entry_claims != abstracted.entry_claims
        || proposed.published_service_ceiling != abstracted.published_service_ceiling
        || proposed.entry_block != optimized_block.id
        || proposed.boundary_settlements.len()
            != validated
                .settlement_rows
                .map_or(0, |(rows, _, _)| rows.len())
        || proposed.return_edge != *psi_edge
        || proposed.return_fuel != validated.optimized_return.fuel
        || proposed.return_effect != validated.optimized_return.effect
        || proposed.return_ownership != validated.optimized_return.ownership
        || proposed.parameters.len() != body.parameters.len()
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    for ((proposed_parameter, semantic), target_parameter) in proposed
        .parameters
        .iter()
        .zip(&abstracted.structural_parameters)
        .zip(&body.parameters)
    {
        if proposed_parameter.semantic != *semantic
            || proposed_parameter.target != *target_parameter
            || semantic.place != target_parameter.place
            || semantic.structural_type != target_parameter.structural_type
            || semantic.multiplicity != target_parameter.multiplicity
            || semantic.access != target_parameter.access
        {
            return Err(Error::NonCanonicalLegalizedPlan);
        }
    }
    let expected_structural_places = abstracted
        .structural_parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: parameter.is_self,
            },
        })
        .collect::<Vec<_>>();
    if proposed.structural_places != expected_structural_places {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(())
}
