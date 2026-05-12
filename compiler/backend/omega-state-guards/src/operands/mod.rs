mod classify;
mod layout;
mod values;

use crate::StateGuardOperand;
use classify::classify_guard_operand;
use layout::resolve_guard_operand_layout;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_layout::LayoutPlan;
use omega_runtime_storage::RuntimeStoragePlan;
use omega_control_flow::StateKey;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use values::resolved_guard_operand_value;

pub(crate) struct GuardOperands {
    pub left: StateGuardOperand,
    pub right: StateGuardOperand,
}

impl GuardOperands {
    pub(crate) fn insert_into(
        self,
        arena: &mut Arena<StateGuardOperand>,
    ) -> HandleSpan<StateGuardOperand> {
        arena.insert_many([self.left, self.right])
    }
}

pub(crate) fn guard_operands(
    source_expressions: &ExpressionTable,
    guard_expressions: &mut ExpressionTable,
    layouts: &LayoutPlan,
    runtime_storage: &RuntimeStoragePlan,
    entry_machine: SymbolHandle,
    source_key: StateKey,
    source_machine: SymbolHandle,
    source_dispatch_index: u32,
    guard: Option<ExpressionHandle>,
) -> Option<GuardOperands> {
    let ExpressionNode::Binary(binary) = source_expressions.expression(guard?) else {
        return None;
    };

    Some(GuardOperands {
        left: guard_operand(
            source_expressions,
            guard_expressions,
            layouts,
            runtime_storage,
            entry_machine,
            source_key,
            source_machine,
            source_dispatch_index,
            binary.left,
        ),
        right: guard_operand(
            source_expressions,
            guard_expressions,
            layouts,
            runtime_storage,
            entry_machine,
            source_key,
            source_machine,
            source_dispatch_index,
            binary.right,
        ),
    })
}

fn guard_operand(
    source_expressions: &ExpressionTable,
    guard_expressions: &mut ExpressionTable,
    layouts: &LayoutPlan,
    runtime_storage: &RuntimeStoragePlan,
    entry_machine: SymbolHandle,
    source_key: StateKey,
    source_machine: SymbolHandle,
    source_dispatch_index: u32,
    expression: ExpressionHandle,
) -> StateGuardOperand {
    let resolved_value = resolved_guard_operand_value(layouts, source_expressions, expression);
    let operand_layout = resolve_guard_operand_layout(
        layouts,
        runtime_storage,
        entry_machine,
        source_key,
        source_machine,
        source_dispatch_index,
        source_expressions,
        expression,
    );
    StateGuardOperand {
        expression: guard_expressions.copy_from(source_expressions, expression),
        kind: classify_guard_operand(source_expressions, expression, resolved_value.is_some()),
        storage: operand_layout
            .as_ref()
            .map(|layout| layout.storage)
            .unwrap_or_default(),
        byte_offset: operand_layout
            .as_ref()
            .map(|layout| layout.byte_offset)
            .unwrap_or(0),
        byte_size: operand_layout
            .as_ref()
            .map(|layout| layout.layout.size)
            .unwrap_or(0),
        resolved_value: resolved_value.unwrap_or(0),
        has_resolved_value: resolved_value.is_some(),
    }
}
