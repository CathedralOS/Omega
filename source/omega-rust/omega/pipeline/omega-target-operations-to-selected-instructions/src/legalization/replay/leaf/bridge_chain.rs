//! Independent replay of the pressure-bearing resident bridge chain.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_active_resident_bridge_chain<'a>(
    function: usize,
    arm_edge: EdgeId,
    expression: &TargetIntegerExpression,
    source_value: &psi_core::ValueId,
    nodes: &'a [omega_optimization_unit::OptimizationNode],
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_facts: &[omega_optimization_unit::AcceptedObligationFact],
    chain: &omega_legalized_operations::LegalizedActiveResidentExactAddBridgeChain,
    u64_type: ScalarType,
) -> Result<
    (
        &'a omega_optimization_unit::OptimizationNode,
        Vec<OperationId>,
    ),
    LegalizationError,
> {
    if nodes.len() != 8 {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let TargetIntegerExpression::ExactAdd {
        psi_operation: result_op,
        obligation: result_ob,
        left: result_left,
        right: result_right,
    } = expression
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let TargetIntegerExpression::ExactAdd {
        psi_operation: bridge_op,
        obligation: bridge_ob,
        left: bridge_left,
        right: bridge_right,
    } = result_right.as_ref()
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let TargetIntegerExpression::ExactAdd {
        psi_operation: middle_op,
        obligation: middle_ob,
        left: middle_left,
        right: middle_right,
    } = bridge_right.as_ref()
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let TargetIntegerExpression::ExactAdd {
        psi_operation: inner_op,
        obligation: inner_ob,
        left: inner_left,
        right: inner_right,
    } = middle_right.as_ref()
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    replay_immediate(
        function,
        arm_edge,
        result_left,
        &nodes[0],
        &chain.resident,
        u64_type,
    )?;
    replay_immediate(
        function,
        arm_edge,
        middle_left,
        &nodes[0],
        &chain.resident,
        u64_type,
    )?;
    replay_immediate(
        function,
        arm_edge,
        inner_left,
        &nodes[1],
        &chain.left,
        u64_type,
    )?;
    replay_immediate(
        function,
        arm_edge,
        inner_right,
        &nodes[2],
        &chain.right,
        u64_type,
    )?;
    replay_immediate(
        function,
        arm_edge,
        bridge_left,
        &nodes[2],
        &chain.right,
        u64_type,
    )?;
    for (node, operation, obligation, left, right, proposed) in [
        (
            &nodes[3],
            *inner_op,
            *inner_ob,
            chain.left.source_value,
            chain.right.source_value,
            &chain.inner,
        ),
        (
            &nodes[4],
            *middle_op,
            *middle_ob,
            chain.resident.source_value,
            chain.inner.source_value,
            &chain.middle,
        ),
        (
            &nodes[5],
            *bridge_op,
            *bridge_ob,
            chain.right.source_value,
            chain.middle.source_value,
            &chain.bridge,
        ),
        (
            &nodes[6],
            *result_op,
            *result_ob,
            chain.resident.source_value,
            chain.bridge.source_value,
            &chain.result,
        ),
    ] {
        replay_exact_add_node(
            function,
            optimized,
            accepted_facts,
            node,
            operation,
            obligation,
            left,
            right,
            proposed,
        )?;
    }
    if chain.result.source_value != *source_value {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok((
        &nodes[7],
        vec![
            chain.resident.constant_operation,
            chain.left.constant_operation,
            chain.right.constant_operation,
            chain.inner.operation,
            chain.middle.operation,
            chain.bridge.operation,
            chain.result.operation,
        ],
    ))
}
