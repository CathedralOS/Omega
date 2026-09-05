//! Independent replay of the exact fork/join graph used by guarded-original allocation.

use legalized_operations::LegalizedActiveResidentExactAddOriginalVictimChain;
use semantic_vocabulary::{EdgeId, OperationId, ScalarType};
use target_operations::TargetIntegerExpression;

use crate::{LegalizationError, LegalizationError as Error};

use super::{replay_exact_add_node, replay_immediate};

pub(in crate::legalization::replay) fn is_shape(expression: &TargetIntegerExpression) -> bool {
    let TargetIntegerExpression::ExactAdd {
        left: result_left,
        right: result_right,
        ..
    } = expression
    else {
        return false;
    };
    let TargetIntegerExpression::Immediate {
        source_value: resident,
        ..
    } = result_left.as_ref()
    else {
        return false;
    };
    let TargetIntegerExpression::ExactAdd {
        left: join_left,
        right: join_right,
        ..
    } = result_right.as_ref()
    else {
        return false;
    };
    let TargetIntegerExpression::ExactAdd {
        left: middle_left,
        right: middle_right,
        ..
    } = join_left.as_ref()
    else {
        return false;
    };
    let TargetIntegerExpression::Immediate {
        source_value: middle_resident,
        ..
    } = middle_left.as_ref()
    else {
        return false;
    };
    let TargetIntegerExpression::ExactAdd {
        left: inner_left,
        right: inner_right,
        ..
    } = middle_right.as_ref()
    else {
        return false;
    };
    let TargetIntegerExpression::Immediate { .. } = inner_left.as_ref() else {
        return false;
    };
    let TargetIntegerExpression::Immediate {
        source_value: right,
        ..
    } = inner_right.as_ref()
    else {
        return false;
    };
    let TargetIntegerExpression::ExactAdd {
        left: bridge_left,
        right: bridge_right,
        ..
    } = join_right.as_ref()
    else {
        return false;
    };
    matches!(
        (bridge_left.as_ref(), bridge_right.as_ref()),
        (
            TargetIntegerExpression::Immediate {
                source_value: bridge_right_value,
                ..
            },
            TargetIntegerExpression::Immediate {
                source_value: bridge_resident,
                ..
            },
        ) if resident == middle_resident
            && resident == bridge_resident
            && right == bridge_right_value
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn replay<'a>(
    function: usize,
    arm_edge: EdgeId,
    expression: &TargetIntegerExpression,
    source_value: &semantic_vocabulary::ValueId,
    nodes: &'a [optimization_unit::OptimizationNode],
    optimized: &optimization_unit::PsiOptimizationFunction,
    accepted_facts: &[optimization_unit::AcceptedObligationFact],
    chain: &LegalizedActiveResidentExactAddOriginalVictimChain,
    u64_type: ScalarType,
) -> Result<(&'a optimization_unit::OptimizationNode, Vec<OperationId>), LegalizationError> {
    if nodes.len() != 9 || !is_shape(expression) {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let TargetIntegerExpression::ExactAdd {
        psi_operation: result_op,
        obligation: result_ob,
        left: result_left,
        right: result_right,
    } = expression
    else {
        unreachable!("shape admitted result add")
    };
    let TargetIntegerExpression::ExactAdd {
        psi_operation: join_op,
        obligation: join_ob,
        left: join_left,
        right: join_right,
    } = result_right.as_ref()
    else {
        unreachable!("shape admitted join add")
    };
    let TargetIntegerExpression::ExactAdd {
        psi_operation: middle_op,
        obligation: middle_ob,
        left: middle_left,
        right: middle_right,
    } = join_left.as_ref()
    else {
        unreachable!("shape admitted middle add")
    };
    let TargetIntegerExpression::ExactAdd {
        psi_operation: inner_op,
        obligation: inner_ob,
        left: inner_left,
        right: inner_right,
    } = middle_right.as_ref()
    else {
        unreachable!("shape admitted inner add")
    };
    let TargetIntegerExpression::ExactAdd {
        psi_operation: bridge_op,
        obligation: bridge_ob,
        left: bridge_left,
        right: bridge_right,
    } = join_right.as_ref()
    else {
        unreachable!("shape admitted bridge add")
    };
    for (target, node, proposed) in [
        (result_left.as_ref(), &nodes[0], &chain.resident),
        (middle_left.as_ref(), &nodes[0], &chain.resident),
        (bridge_right.as_ref(), &nodes[0], &chain.resident),
        (inner_left.as_ref(), &nodes[1], &chain.left),
        (inner_right.as_ref(), &nodes[2], &chain.right),
        (bridge_left.as_ref(), &nodes[2], &chain.right),
    ] {
        replay_immediate(function, arm_edge, target, node, proposed, u64_type)?;
    }
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
            chain.resident.source_value,
            &chain.bridge,
        ),
        (
            &nodes[6],
            *join_op,
            *join_ob,
            chain.middle.source_value,
            chain.bridge.source_value,
            &chain.join,
        ),
        (
            &nodes[7],
            *result_op,
            *result_ob,
            chain.resident.source_value,
            chain.join.source_value,
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
        &nodes[8],
        vec![
            chain.resident.constant_operation,
            chain.left.constant_operation,
            chain.right.constant_operation,
            chain.inner.operation,
            chain.middle.operation,
            chain.bridge.operation,
            chain.join.operation,
            chain.result.operation,
        ],
    ))
}
