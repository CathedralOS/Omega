use super::super::shared::*;
use super::super::validators::validate_scalar_call_unit_form;
use super::callee::replay_callee;
use super::grammar::{call_parts, constant_parts, home, immediate, is_u64, replay_call_sources};
use super::operations::{replay_call, replay_constant, replay_return};

#[allow(clippy::too_many_arguments)]
pub(super) fn replay(
    function: usize,
    target_function: &omega_target_operations::TargetFunction,
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
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

    let (a_op, a_result, a_type, a_value) = constant_parts(target_a, abstract_a, function)?;
    let (b_op, b_result, b_type, b_value) = constant_parts(target_b, abstract_b, function)?;
    if !is_u64(a_type)
        || a_type != b_type
        || a_value == b_value
        || a_result == b_result
        || a_op == b_op
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let (call1_op, call1_result, callee, args1) =
        call_parts(target_call1, abstract_call1, function)?;
    let (call2_op, call2_result, callee2, args2) =
        call_parts(target_call2, abstract_call2, function)?;
    let (call3_op, call3_result, callee3, args3) =
        call_parts(target_call3, abstract_call3, function)?;
    if callee != callee2
        || callee != callee3
        || args1 != [a_result, b_result]
        || args2 != [a_result, b_result]
        || args3 != [call1_result, call2_result]
        || [a_result, b_result, call1_result, call2_result, call3_result]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != 5
        || [a_op, b_op, call1_op, call2_op, call3_op]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != 5
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    replay_call_sources(
        target_call1,
        [immediate(target_a)?, immediate(target_b)?],
        function,
    )?;
    replay_call_sources(
        target_call2,
        [immediate(target_a)?, immediate(target_b)?],
        function,
    )?;
    replay_call_sources(
        target_call3,
        [home(target_call1)?, home(target_call2)?],
        function,
    )?;

    let expected_provenance = TerminalPsiProvenance {
        operations: vec![a_op, b_op, call1_op, call2_op, call3_op],
        edges: vec![*psi_edge],
    };
    if target_function.provenance != expected_provenance
        || proposed.machine != target_function.machine
        || proposed.attachment != attachment
        || proposed.provenance != target_function.provenance
        || proposed.entry_block != block.id
        || proposed.return_edge != *psi_edge
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    for (index, (((target_constant, node), operation), result)) in [target_a, target_b]
        .into_iter()
        .zip([node_a, node_b])
        .zip([a_op, b_op])
        .zip([a_result, b_result])
        .enumerate()
    {
        replay_constant(
            function,
            block.id,
            index as u32,
            target_constant,
            node,
            operation,
            result,
            &proposed.constants[index],
        )?;
    }
    for (index, (((target_call, node), operation), result)) in
        [target_call1, target_call2, target_call3]
            .into_iter()
            .zip([node_call1, node_call2, node_call3])
            .zip([call1_op, call2_op, call3_op])
            .zip([call1_result, call2_result, call3_result])
            .enumerate()
    {
        replay_call(
            function,
            block.id,
            (index + 2) as u32,
            target_call,
            node,
            operation,
            result,
            &proposed.calls[index],
        )?;
    }
    replay_return(function, node_return, *psi_edge, proposed)?;
    replay_callee(function, callee, target, abstract_plan, unit, proposed_plan)?;
    Ok(())
}
