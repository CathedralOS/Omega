use super::context::RuntimeDispatchLoopContext;
use omega_state_guards::{
    StateGuardLowering, StateGuardOperandKind, StateGuardOperandStorage, StateGuardOperator,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct DispatchGuardComparison {
    pub lowering: StateGuardLowering,
    pub operator: StateGuardOperator,
    pub byte_offset: usize,
    pub byte_size: usize,
    pub expected_value: i64,
    pub has_storage: bool,
}

pub(super) fn dispatch_guard_comparison(
    context: &RuntimeDispatchLoopContext,
    source_dispatch_index: u32,
    statement_order: usize,
) -> DispatchGuardComparison {
    let Some(guard) = context
        .state_guards
        .guards
        .iter()
        .find(|(_, guard)| {
            guard.source_dispatch_index == source_dispatch_index
                && guard.statement_order == statement_order
        })
        .map(|(_, guard)| guard)
    else {
        return DispatchGuardComparison {
            lowering: StateGuardLowering::NeedsRuntimeExpression,
            ..DispatchGuardComparison::default()
        };
    };

    let Some(operands) = context.state_guards.operands.span(guard.operands) else {
        return DispatchGuardComparison {
            lowering: guard.lowering,
            operator: guard.operator,
            ..DispatchGuardComparison::default()
        };
    };
    let Some(place_operand) = operands.iter().find(|operand| {
        operand.kind == StateGuardOperandKind::Place
            && operand.storage == StateGuardOperandStorage::MachineOwned
    }) else {
        return DispatchGuardComparison {
            lowering: guard.lowering,
            operator: guard.operator,
            ..DispatchGuardComparison::default()
        };
    };
    let expected_value = operands
        .iter()
        .find(|operand| operand.has_resolved_value)
        .map(|operand| operand.resolved_value)
        .unwrap_or(0);

    DispatchGuardComparison {
        lowering: guard.lowering,
        operator: guard.operator,
        byte_offset: place_operand.byte_offset,
        byte_size: place_operand.byte_size,
        expected_value,
        has_storage: true,
    }
}
