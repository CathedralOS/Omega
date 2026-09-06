use super::super::shared::*;
use super::callee::validate_callee;
use super::nodes::{validate_return_node, validate_value_node};
use super::operations::{
    call_parts, constant_parts, home, immediate, is_u64, validate_call_sources,
};

pub(super) struct MatchedSequence<'a> {
    pub target: &'a target_operations::TargetFunction,
    pub attachment: semantic_vocabulary::StructuralTypeId,
    pub block: &'a optimization_unit::OptimizationBlock,
    pub operations: &'a [TargetUnitOperation],
    pub nodes: &'a [optimization_unit::OptimizationNode],
    pub return_node: &'a optimization_unit::OptimizationNode,
    pub return_edge: EdgeId,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn match_sequence<'a>(
    function: usize,
    target_function: &'a target_operations::TargetFunction,
    abstracted: &'a abstract_operations::AbstractFunction,
    optimized: &'a optimization_unit::PsiOptimizationFunction,
    target: &'a TargetOperationPlan,
    abstract_plan: &'a AbstractOperationPlan,
    unit: &'a PsiOptimizationUnit,
) -> Result<MatchedSequence<'a>, LegalizationError> {
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
        || !target_operations
            .iter()
            .any(|operation| matches!(operation, TargetUnitOperation::ScalarCall { .. }))
    {
        return Err(Error::UnsupportedSourceShape { function });
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

    let mut definitions = Vec::new();
    let mut operations = Vec::new();
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    for (index, ((target_operation, abstract_operation), node)) in target_operations
        .iter()
        .zip(abstract_operations)
        .zip(nodes)
        .enumerate()
    {
        let (operation, result, source) = match target_operation {
            TargetUnitOperation::IntegerConstant { .. } => {
                let (operation, result, scalar_type, _) =
                    constant_parts(target_operation, abstract_operation, function)?;
                if !is_u64(scalar_type) {
                    return Err(Error::UnsupportedSourceShape { function });
                }
                (operation, result, immediate(target_operation)?)
            }
            TargetUnitOperation::ScalarCall { call_plan, .. } => {
                let (operation, result, callee, arguments) =
                    call_parts(target_operation, abstract_operation, function)?;
                let [left, right] = arguments else {
                    return Err(Error::UnsupportedSourceShape { function });
                };
                let resolve = |value: &ValueId| {
                    definitions
                        .iter()
                        .find(|(defined, _)| defined == value)
                        .map(|(_, source)| *source)
                        .ok_or(Error::SourceCustodyMismatch)
                };
                validate_call_sources(
                    target_operation,
                    [resolve(left)?, resolve(right)?],
                    function,
                )?;
                validate_callee(function, callee, call_plan, target, abstract_plan, unit)?;
                (operation, result, home(target_operation)?)
            }
            _ => return Err(Error::UnsupportedSourceShape { function }),
        };
        if operations.contains(&operation) || definitions.iter().any(|(value, _)| *value == result)
        {
            return Err(Error::SourceCustodyMismatch);
        }
        validate_value_node(
            function,
            block.id,
            u32::try_from(index).map_err(|_| Error::SourceCustodyMismatch)?,
            node,
            operation,
            result,
            integer,
        )?;
        definitions.push((result, source));
        operations.push(operation);
    }
    validate_return_node(function, node_return, *psi_edge)?;
    if target_function.provenance
        != (TerminalPsiProvenance {
            operations,
            edges: vec![*psi_edge],
        })
    {
        return Err(Error::SourceCustodyMismatch);
    }
    Ok(MatchedSequence {
        target: target_function,
        attachment,
        block,
        operations: target_operations,
        nodes,
        return_node: node_return,
        return_edge: *psi_edge,
    })
}
