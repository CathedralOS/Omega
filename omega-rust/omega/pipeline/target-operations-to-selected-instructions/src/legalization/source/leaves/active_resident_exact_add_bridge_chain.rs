//! Exact bridge-chain projection retaining one inner operand across the first resident use.

use super::super::shared::*;
use super::active_resident_exact_add_chain::derive_exact_add;
use super::immediate::derive_operand;
use super::{DerivedValue, LeafContext};

pub(super) fn derive<'a>(
    context: &LeafContext<'a>,
    expression: &TargetIntegerExpression,
) -> Result<DerivedValue<'a>, LegalizationError> {
    if context.nodes.len() != 8 {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    }
    let Shape {
        resident,
        left,
        right,
        inner,
        middle,
        bridge,
        result,
    } = shape(expression).ok_or(Error::UnsupportedSourceShape {
        function: context.function,
    })?;
    let resident = derive_operand(
        context.function,
        context.arm_edge,
        resident,
        &context.nodes[0],
        context.u64_type,
    )?;
    let left = derive_operand(
        context.function,
        context.arm_edge,
        left,
        &context.nodes[1],
        context.u64_type,
    )?;
    let right = derive_operand(
        context.function,
        context.arm_edge,
        right,
        &context.nodes[2],
        context.u64_type,
    )?;
    let inner = derive_exact_add(
        context,
        &context.nodes[3],
        inner.0,
        inner.1,
        left.source_value,
        right.source_value,
    )?;
    let middle = derive_exact_add(
        context,
        &context.nodes[4],
        middle.0,
        middle.1,
        resident.source_value,
        inner.source_value,
    )?;
    let bridge = derive_exact_add(
        context,
        &context.nodes[5],
        bridge.0,
        bridge.1,
        right.source_value,
        middle.source_value,
    )?;
    let result = derive_exact_add(
        context,
        &context.nodes[6],
        result.0,
        result.1,
        resident.source_value,
        bridge.source_value,
    )?;
    if result.source_value != context.source_value {
        return Err(Error::UnsupportedSourceShape {
            function: context.function,
        });
    }
    Ok((
        &context.nodes[7],
        SourceLeafValue::ActiveResidentExactAddBridgeChain(Box::new(
            SourceActiveResidentExactAddBridgeChain {
                resident,
                left,
                right,
                inner,
                middle,
                bridge,
                result,
            },
        )),
    ))
}

pub(in crate::legalization::source) fn is_active_resident_exact_add_bridge_chain(
    expression: &TargetIntegerExpression,
) -> bool {
    shape(expression).is_some()
}

struct Shape<'a> {
    resident: &'a TargetIntegerExpression,
    left: &'a TargetIntegerExpression,
    right: &'a TargetIntegerExpression,
    inner: (OperationId, semantic_vocabulary::ObligationId),
    middle: (OperationId, semantic_vocabulary::ObligationId),
    bridge: (OperationId, semantic_vocabulary::ObligationId),
    result: (OperationId, semantic_vocabulary::ObligationId),
}

fn shape(expression: &TargetIntegerExpression) -> Option<Shape<'_>> {
    let TargetIntegerExpression::ExactAdd {
        psi_operation: result_op,
        obligation: result_ob,
        left: result_left,
        right: result_right,
    } = expression
    else {
        return None;
    };
    let TargetIntegerExpression::Immediate {
        source_value: resident_value,
        ..
    } = result_left.as_ref()
    else {
        return None;
    };
    let TargetIntegerExpression::ExactAdd {
        psi_operation: bridge_op,
        obligation: bridge_ob,
        left: bridge_left,
        right: bridge_right,
    } = result_right.as_ref()
    else {
        return None;
    };
    let TargetIntegerExpression::Immediate {
        source_value: bridge_value,
        ..
    } = bridge_left.as_ref()
    else {
        return None;
    };
    let TargetIntegerExpression::ExactAdd {
        psi_operation: middle_op,
        obligation: middle_ob,
        left: middle_left,
        right: middle_right,
    } = bridge_right.as_ref()
    else {
        return None;
    };
    let TargetIntegerExpression::Immediate {
        source_value: middle_resident,
        ..
    } = middle_left.as_ref()
    else {
        return None;
    };
    let TargetIntegerExpression::ExactAdd {
        psi_operation: inner_op,
        obligation: inner_ob,
        left: inner_left,
        right: inner_right,
    } = middle_right.as_ref()
    else {
        return None;
    };
    let TargetIntegerExpression::Immediate { .. } = inner_left.as_ref() else {
        return None;
    };
    let TargetIntegerExpression::Immediate {
        source_value: inner_right_value,
        ..
    } = inner_right.as_ref()
    else {
        return None;
    };
    (*resident_value == *middle_resident && *bridge_value == *inner_right_value).then_some(Shape {
        resident: result_left,
        left: inner_left,
        right: inner_right,
        inner: (*inner_op, *inner_ob),
        middle: (*middle_op, *middle_ob),
        bridge: (*bridge_op, *bridge_ob),
        result: (*result_op, *result_ob),
    })
}
