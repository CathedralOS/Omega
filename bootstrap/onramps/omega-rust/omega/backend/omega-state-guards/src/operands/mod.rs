mod classify;
mod layout;
pub(crate) mod values;

use crate::StateGuardOperand;
use classify::classify_guard_operand;
use layout::resolve_guard_operand_layout;
use omega_control_flow::StateKey;
use omega_layout::LayoutPlan;
use omega_runtime_storage::RuntimeStoragePlan;
use psi_arena::{Arena, HandleSpan};
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use psi_symbols::SymbolHandle;
use values::{enum_variant_tag_value, resolved_guard_operand_value};

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
    receiver_bases: &[Option<usize>],
    state_contexts: &[u32],
    entry_machine: SymbolHandle,
    source_key: StateKey,
    source_machine: SymbolHandle,
    source_dispatch_index: u32,
    statement_index: usize,
    guard: ExpressionHandle,
) -> Option<GuardOperands> {
    if !guard.is_valid() {
        return None;
    }
    let ExpressionNode::Binary(binary) = source_expressions.expression(guard) else {
        return None;
    };

    let mut operands = GuardOperands {
        left: guard_operand(
            source_expressions,
            guard_expressions,
            layouts,
            runtime_storage,
            receiver_bases,
            state_contexts,
            entry_machine,
            source_key,
            source_machine,
            source_dispatch_index,
            statement_index,
            binary.left,
        ),
        right: guard_operand(
            source_expressions,
            guard_expressions,
            layouts,
            runtime_storage,
            receiver_bases,
            state_contexts,
            entry_machine,
            source_key,
            source_machine,
            source_dispatch_index,
            statement_index,
            binary.right,
        ),
    };

    // TAG-ONLY case comparison: when one side names a CASE of an enum-shaped
    // data (`self.cmd == Command::Move`), the storage side must read only the
    // 4-byte tag prefix, never the full tag-plus-payload value (a payload-
    // carrying enum is wider than its tag, and stale payload bytes must not
    // affect the compare). Payload-less enums are exactly tag-sized already.
    let left_is_case = enum_variant_tag_value(layouts, source_expressions, binary.left).is_some();
    let right_is_case = enum_variant_tag_value(layouts, source_expressions, binary.right).is_some();
    if left_is_case || right_is_case {
        for operand in [&mut operands.left, &mut operands.right] {
            if operand.byte_size > omega_layout::ENUM_TAG_BYTES {
                operand.byte_size = omega_layout::ENUM_TAG_BYTES;
            }
        }
    }

    Some(operands)
}

fn guard_operand(
    source_expressions: &ExpressionTable,
    guard_expressions: &mut ExpressionTable,
    layouts: &LayoutPlan,
    runtime_storage: &RuntimeStoragePlan,
    receiver_bases: &[Option<usize>],
    state_contexts: &[u32],
    entry_machine: SymbolHandle,
    source_key: StateKey,
    source_machine: SymbolHandle,
    source_dispatch_index: u32,
    statement_index: usize,
    expression: ExpressionHandle,
) -> StateGuardOperand {
    let resolved_value = resolved_guard_operand_value(layouts, source_expressions, expression);
    let operand_layout = resolve_guard_operand_layout(
        layouts,
        runtime_storage,
        receiver_bases,
        state_contexts,
        entry_machine,
        source_key,
        source_machine,
        source_dispatch_index,
        statement_index,
        source_expressions,
        expression,
    );
    let has_storage_layout = operand_layout.is_some();
    StateGuardOperand {
        expression: guard_expressions.copy_from(source_expressions, expression),
        kind: classify_guard_operand(
            source_expressions,
            expression,
            resolved_value.is_some(),
            has_storage_layout,
        ),
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
