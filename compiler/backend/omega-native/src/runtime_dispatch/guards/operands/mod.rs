mod classify;
mod layout;
mod values;

use crate::runtime_dispatch::guards::StateGuardOperand;
use classify::classify_guard_operand;
use layout::resolve_guard_operand_layout;
use omega_core::arena::{Arena, HandleSpan};
use omega_typed_program::expression::Expression;
use omega_typed_program::statement::TransitionGuard;
use values::resolved_guard_operand_value;

pub(in crate::runtime_dispatch::guards) struct GuardOperands {
    pub left: StateGuardOperand,
    pub right: StateGuardOperand,
}

impl GuardOperands {
    pub(in crate::runtime_dispatch::guards) fn insert_into(
        self,
        arena: &mut Arena<StateGuardOperand>,
    ) -> HandleSpan<StateGuardOperand> {
        arena.insert_many([self.left, self.right])
    }
}

pub(in crate::runtime_dispatch::guards) fn guard_operands(
    layouts: &crate::layout::LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
    guard: &TransitionGuard,
) -> Option<GuardOperands> {
    let TransitionGuard::When(Expression::Binary(binary)) = guard else {
        return None;
    };

    Some(GuardOperands {
        left: guard_operand(layouts, entry_machine, source_machine, binary.left.clone()),
        right: guard_operand(layouts, entry_machine, source_machine, binary.right.clone()),
    })
}

fn guard_operand(
    layouts: &crate::layout::LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
    expression: Expression,
) -> StateGuardOperand {
    let resolved_value = resolved_guard_operand_value(layouts, &expression);
    let operand_layout =
        resolve_guard_operand_layout(layouts, entry_machine, source_machine, &expression);
    StateGuardOperand {
        kind: classify_guard_operand(&expression),
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
        expression,
        resolved_value: resolved_value.unwrap_or(0),
        has_resolved_value: resolved_value.is_some(),
    }
}
