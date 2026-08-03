use super::context::RuntimeDispatchLoopContext;
use omega_state_guards::{
    StateGuardLowering, StateGuardOperandKind, StateGuardOperandStorage, StateGuardOperator,
};
use psi_checked_trees::expression::ExpressionHandle;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct DispatchGuardComparison {
    pub lowering: StateGuardLowering,
    pub operator: StateGuardOperator,
    pub storage: StateGuardOperandStorage,
    pub byte_offset: usize,
    pub right_storage: StateGuardOperandStorage,
    pub right_byte_offset: usize,
    pub byte_size: usize,
    pub expected_value: i64,
    pub has_storage: bool,
    pub has_right_storage: bool,
}

pub(super) fn dispatch_guard_comparison(
    context: &RuntimeDispatchLoopContext,
    source_dispatch_index: u32,
    statement_order: usize,
) -> DispatchGuardComparison {
    let Some(guard) = context
        .state_guards
        .guard_for_dispatch(source_dispatch_index, statement_order)
    else {
        return DispatchGuardComparison {
            lowering: StateGuardLowering::NeedsRuntimeExpression,
            ..DispatchGuardComparison::default()
        };
    };

    let Some(operands) = context.state_guards.operand_span(guard.operands) else {
        return DispatchGuardComparison {
            lowering: guard.lowering,
            operator: guard.operator,
            ..DispatchGuardComparison::default()
        };
    };
    let mut place_operands = operands.iter().filter(|operand| {
        operand.kind == StateGuardOperandKind::Place
            && operand.storage != StateGuardOperandStorage::Unknown
    });
    let Some(place_operand) = place_operands.next() else {
        return DispatchGuardComparison {
            lowering: guard.lowering,
            operator: guard.operator,
            ..DispatchGuardComparison::default()
        };
    };
    let right_place_operand = place_operands.next();
    let expected_value = operands
        .iter()
        .find(|operand| operand.has_resolved_value)
        .map(|operand| operand.resolved_value)
        .unwrap_or(0);

    DispatchGuardComparison {
        lowering: guard.lowering,
        operator: guard.operator,
        storage: place_operand.storage,
        byte_offset: place_operand.byte_offset,
        right_storage: right_place_operand
            .map(|operand| operand.storage)
            .unwrap_or(StateGuardOperandStorage::Unknown),
        right_byte_offset: right_place_operand
            .map(|operand| operand.byte_offset)
            .unwrap_or(0),
        byte_size: place_operand.byte_size,
        expected_value,
        has_storage: true,
        has_right_storage: right_place_operand.is_some(),
    }
}

pub(super) fn dispatch_guard_expression(
    context: &RuntimeDispatchLoopContext,
    source_dispatch_index: u32,
    statement_order: usize,
) -> ExpressionHandle {
    context
        .state_guards
        .guard_for_dispatch(source_dispatch_index, statement_order)
        .and_then(|guard| guard.has_expression.then_some(guard.expression))
        .unwrap_or_else(ExpressionHandle::invalid)
}
