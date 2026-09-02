use super::super::shared::*;
use super::callee::validate_callee;
use super::nodes::{validate_return_node, validate_value_node};
use super::operations::{
    call_parts, constant_parts, home, immediate, is_u64, validate_call_sources,
};

pub(super) struct MatchedChain<'a> {
    pub target: &'a omega_target_operations::TargetFunction,
    pub attachment: psi_core::StructuralTypeId,
    pub block: &'a omega_optimization_unit::OptimizationBlock,
    pub target_constants: [&'a TargetUnitOperation; 2],
    pub target_calls: [&'a TargetUnitOperation; 3],
    pub nodes: [&'a omega_optimization_unit::OptimizationNode; 6],
    pub return_edge: EdgeId,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn match_exact_chain<'a>(
    function: usize,
    target_function: &'a omega_target_operations::TargetFunction,
    abstracted: &'a omega_abstract_operations::AbstractFunction,
    optimized: &'a omega_optimization_unit::PsiOptimizationFunction,
    target: &'a TargetOperationPlan,
    abstract_plan: &'a AbstractOperationPlan,
    unit: &'a PsiOptimizationUnit,
) -> Result<MatchedChain<'a>, LegalizationError> {
    let TargetOperation::UnitBody(body) = &target_function.operation else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [
        target_a,
        target_b,
        target_call1,
        target_call2,
        target_call3,
        target_return,
    ] = body.operations.as_slice()
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [
        abstract_a,
        abstract_b,
        abstract_call1,
        abstract_call2,
        abstract_call3,
        abstract_return,
    ] = abstracted.operations.as_slice()
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [entry] = abstracted.block_entries.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [block] = optimized.blocks.as_slice() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let [
        node_a,
        node_b,
        node_call1,
        node_call2,
        node_call3,
        node_return,
    ] = block.nodes.as_slice()
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
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
        || abstracted.result != omega_abstract_operations::AbstractFunctionResult::Unit
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

    let (a_operation, a_result, a_type, a_value) = constant_parts(target_a, abstract_a, function)?;
    let (b_operation, b_result, b_type, b_value) = constant_parts(target_b, abstract_b, function)?;
    if !is_u64(a_type)
        || a_type != b_type
        || a_value == b_value
        || a_result == b_result
        || a_operation == b_operation
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let (call1_operation, call1_result, callee, call1_arguments) =
        call_parts(target_call1, abstract_call1, function)?;
    let (call2_operation, call2_result, call2_callee, call2_arguments) =
        call_parts(target_call2, abstract_call2, function)?;
    let (call3_operation, call3_result, call3_callee, call3_arguments) =
        call_parts(target_call3, abstract_call3, function)?;
    if callee != call2_callee
        || callee != call3_callee
        || call1_arguments != [a_result, b_result]
        || call2_arguments != [a_result, b_result]
        || call3_arguments != [call1_result, call2_result]
        || [a_result, b_result, call1_result, call2_result, call3_result]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != 5
        || [
            a_operation,
            b_operation,
            call1_operation,
            call2_operation,
            call3_operation,
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
            != 5
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    validate_call_sources(
        target_call1,
        [immediate(target_a)?, immediate(target_b)?],
        function,
    )?;
    validate_call_sources(
        target_call2,
        [immediate(target_a)?, immediate(target_b)?],
        function,
    )?;
    validate_call_sources(
        target_call3,
        [home(target_call1)?, home(target_call2)?],
        function,
    )?;
    for (index, (node, operation, result)) in [
        (node_a, a_operation, a_result),
        (node_b, b_operation, b_result),
        (node_call1, call1_operation, call1_result),
        (node_call2, call2_operation, call2_result),
        (node_call3, call3_operation, call3_result),
    ]
    .into_iter()
    .enumerate()
    {
        validate_value_node(
            function,
            block.id,
            index as u32,
            node,
            operation,
            result,
            a_type,
        )?;
    }
    validate_return_node(function, node_return, *psi_edge)?;
    validate_callee(function, callee, target, abstract_plan, unit)?;
    let expected_provenance = TerminalPsiProvenance {
        operations: vec![
            a_operation,
            b_operation,
            call1_operation,
            call2_operation,
            call3_operation,
        ],
        edges: vec![*psi_edge],
    };
    if target_function.provenance != expected_provenance {
        return Err(Error::UnsupportedSourceShape { function });
    }
    Ok(MatchedChain {
        target: target_function,
        attachment,
        block,
        target_constants: [target_a, target_b],
        target_calls: [target_call1, target_call2, target_call3],
        nodes: [
            node_a,
            node_b,
            node_call1,
            node_call2,
            node_call3,
            node_return,
        ],
        return_edge: *psi_edge,
    })
}
