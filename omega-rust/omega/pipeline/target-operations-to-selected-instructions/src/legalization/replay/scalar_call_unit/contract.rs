use super::super::shared::*;
use super::super::validators::validate_scalar_call_unit_form;
use super::callee::replay_callee;
use super::grammar::{call_parts, constant_parts, home, immediate, is_u64, replay_call_sources};
use super::operations::{replay_call, replay_constant, replay_return};

#[allow(clippy::too_many_arguments)]
pub(super) fn replay(
    function: usize,
    target_function: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed_plan: &LegalizedOperationPlan,
    proposed: &LegalizedScalarCallUnitFunction,
) -> Result<(), LegalizationError> {
    if !validate_scalar_call_unit_form(target_function, proposed.recipe) {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    let TargetOperation::UnitBody(body) = &target_function.operation else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let Some((target_return, target_operations)) = body.operations.split_last() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let Some((abstract_return, abstract_operations)) = abstracted.operations.split_last() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [entry] = abstracted.block_entries.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [block] = optimized.blocks.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let Some((node_return, nodes)) = block.nodes.split_last() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if target_operations.len() != abstract_operations.len()
        || nodes.len() != target_operations.len()
        || proposed.operations.len() != target_operations.len()
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    let Some(attachment) = target_function.attachment else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let TargetUnitOperation::Return {
        psi_edge,
        cleanup_actions,
    } = target_return
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let empty_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target.target),
        &CallSignature {
            parameters: Vec::new(),
            result: None,
        },
    )
    .map_err(|_| Error::UnsupportedSourceShape { function })?;
    if target_function.machine != abstracted.machine
        || target_function.machine != optimized.machine
        || abstracted.attachment != Some(attachment)
        || optimized.attachment != Some(attachment)
        || target_function.fixed_integer_scalar_abi.is_some()
        || abstracted.result != abstract_operations::AbstractFunctionResult::Unit
        || optimized.result != abstracted.result
        || abstracted.entry != entry.block
        || optimized.entry != entry.block
        || block.id != entry.block
        || entry.operation_offset != 0
        || !entry.parameters.is_empty()
        || !block.parameters.is_empty()
        || !abstracted.parameters.is_empty()
        || !optimized.parameters.is_empty()
        || !abstracted.structural_parameters.is_empty()
        || !optimized.structural_parameters.is_empty()
        || !optimized.structural_places.is_empty()
        || !abstracted.entry_claims.is_empty()
        || !optimized.entry_claim_declarations.is_empty()
        || !optimized.content_entry_claims.is_empty()
        || !optimized.entry_claims.is_empty()
        || !abstracted.published_service_ceiling.is_empty()
        || !optimized.published_service_ceiling.is_empty()
        || !optimized.declared_places.is_empty()
        || body.structural_types != abstract_plan.structural_types
        || body.structural_types != unit.structural_types
        || !body.parameters.is_empty()
        || !body.scalar_parameters.is_empty()
        || body.call_plan != empty_call_plan
        || !cleanup_actions.is_empty()
        || abstracted.operations
            != block
                .nodes
                .iter()
                .map(|node| node.operation.clone())
                .collect::<Vec<_>>()
        || !matches!(abstract_return, AbstractOperation::ReturnUnit { psi_edge: edge, cleanup_actions } if edge == psi_edge && cleanup_actions.is_empty())
    {
        return Err(Error::UnsupportedSourceShape { function });
    }

    let mut operation_ids = Vec::new();
    let mut result_ids = Vec::new();
    let mut call_count = 0;
    for (index, proposed_operation) in proposed.operations.iter().enumerate() {
        let native = &target_operations[index];
        let abstract_operation = &abstract_operations[index];
        let node = &nodes[index];
        let ordinal = u32::try_from(index).map_err(|_| Error::NonCanonicalLegalizedPlan)?;
        let (operation, result) = match proposed_operation {
            LegalizedScalarCallUnitOperation::Constant(constant) => {
                let (operation, result, integer, _) =
                    constant_parts(native, abstract_operation, function)?;
                if !is_u64(integer) {
                    return Err(Error::NonCanonicalLegalizedPlan);
                }
                replay_constant(
                    function, block.id, ordinal, native, node, operation, result, constant,
                )?;
                (operation, result)
            }
            LegalizedScalarCallUnitOperation::Call(call) => {
                let (operation, result, callee, arguments) =
                    call_parts(native, abstract_operation, function)?;
                let [left, right] = arguments else {
                    return Err(Error::NonCanonicalLegalizedPlan);
                };
                let source_for =
                    |value: &ValueId| -> Result<TargetUnitScalarArgumentSource, LegalizationError> {
                        let mut found =
                            target_operations[..index]
                                .iter()
                                .filter(|prior| match prior {
                                    TargetUnitOperation::IntegerConstant { result, .. } => {
                                        result == value
                                    }
                                    TargetUnitOperation::ScalarCall { result_home, .. } => {
                                        result_home.source_value == *value
                                    }
                                    _ => false,
                                });
                        let prior = found.next().ok_or(Error::NonCanonicalLegalizedPlan)?;
                        if found.next().is_some() {
                            return Err(Error::NonCanonicalLegalizedPlan);
                        }
                        match prior {
                            TargetUnitOperation::IntegerConstant { .. } => immediate(prior),
                            TargetUnitOperation::ScalarCall { .. } => home(prior),
                            _ => Err(Error::NonCanonicalLegalizedPlan),
                        }
                    };
                replay_call_sources(native, [source_for(left)?, source_for(right)?], function)?;
                replay_call(
                    function, block.id, ordinal, native, node, operation, result, call,
                )?;
                replay_callee(
                    function,
                    callee,
                    &call.call_plan,
                    target,
                    abstract_plan,
                    unit,
                    proposed_plan,
                )?;
                call_count += 1;
                (operation, result)
            }
        };
        if operation_ids.contains(&operation) || result_ids.contains(&result) {
            return Err(Error::NonCanonicalLegalizedPlan);
        }
        operation_ids.push(operation);
        result_ids.push(result);
    }
    if call_count == 0
        || target_function.provenance
            != (TerminalPsiProvenance {
                operations: operation_ids,
                edges: vec![*psi_edge],
            })
        || proposed.machine != target_function.machine
        || proposed.attachment != attachment
        || proposed.provenance != target_function.provenance
        || proposed.entry_block != block.id
        || proposed.return_edge != *psi_edge
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    replay_return(function, node_return, *psi_edge, proposed)?;
    Ok(())
}
