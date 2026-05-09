mod model;
mod operands;

pub use model::{
    StateGuard, StateGuardKind, StateGuardOperand, StateGuardOperandKind, StateGuardOperandStorage,
    StateGuardPlan,
};
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_core::arena::Arena;
use omega_core::symbols::SymbolHandle;
use omega_layout::LayoutPlan;
use omega_state_dispatch::{DispatchEdge, StateDispatchPlan};
pub use omega_target_program::{StateGuardLowering, StateGuardOperator};
use omega_typed_program::expression::{BinaryOperator, Expression};
use omega_typed_program::statement::TransitionGuard;
use operands::{GuardOperands, guard_operands};

pub fn build_state_guard_plan(
    state_dispatch: &StateDispatchPlan,
    control_flow: &ControlFlowPlan,
    layouts: &LayoutPlan,
    entry_machine: SymbolHandle,
) -> StateGuardPlan {
    let mut plan = StateGuardPlan::default();

    for (_, state) in state_dispatch.states.iter() {
        let Some(edges) = state_dispatch.edges.span(state.edges) else {
            continue;
        };

        for (statement_order, edge) in edges.iter().enumerate() {
            if control_flow.state_by_key(state.key).is_none() {
                continue;
            }
            plan.guards.insert(build_state_guard(
                &mut plan.operands,
                layouts,
                entry_machine,
                state.key,
                state.dispatch_index,
                statement_order,
                edge,
            ));
        }
    }

    plan
}

pub fn classify_transition_guard(guard: &TransitionGuard) -> StateGuardKind {
    match guard {
        TransitionGuard::Always => StateGuardKind::Always,
        TransitionGuard::When(expression) => match expression {
            Expression::Binary(binary) => match binary.operator {
                BinaryOperator::Equal => StateGuardKind::RuntimeEquality,
                BinaryOperator::NotEqual => StateGuardKind::RuntimeInequality,
                BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual => StateGuardKind::RuntimeOrdering,
                BinaryOperator::Add | BinaryOperator::And | BinaryOperator::Or => {
                    StateGuardKind::RuntimeExpression
                }
            },
            _ => StateGuardKind::RuntimeExpression,
        },
    }
}

fn build_state_guard(
    operand_arena: &mut Arena<StateGuardOperand>,
    layouts: &LayoutPlan,
    entry_machine: SymbolHandle,
    source: StateKey,
    source_dispatch_index: u32,
    statement_order: usize,
    edge: &DispatchEdge,
) -> StateGuard {
    let (kind, operator, expression, has_expression) = guard_data(&edge.guard);
    let guard_operands = guard_operands(layouts, entry_machine, source.machine, &edge.guard);
    let lowering = guard_lowering(kind, operator, guard_operands.as_ref());
    let operands = guard_operands
        .map(|operands| operands.insert_into(operand_arena))
        .unwrap_or_default();

    StateGuard {
        source,
        source_dispatch_index,
        target: edge.target.clone(),
        target_dispatch_index: edge.target_dispatch_index,
        continuation: edge.continuation.clone(),
        continuation_dispatch_index: edge.continuation_dispatch_index,
        statement_order,
        kind,
        operator,
        lowering,
        expression,
        operands,
        has_expression,
        forms_cycle: edge.forms_cycle,
    }
}

fn guard_lowering(
    kind: StateGuardKind,
    operator: StateGuardOperator,
    operands: Option<&GuardOperands>,
) -> StateGuardLowering {
    if kind == StateGuardKind::Always {
        return StateGuardLowering::NoOp;
    }

    if !matches!(
        operator,
        StateGuardOperator::Equal | StateGuardOperator::NotEqual
    ) {
        return StateGuardLowering::NeedsRuntimeExpression;
    }

    let Some(operands) = operands else {
        return StateGuardLowering::NeedsRuntimeExpression;
    };
    let left = &operands.left;
    let right = &operands.right;

    if left.kind == StateGuardOperandKind::Place && right.has_resolved_value {
        return StateGuardLowering::CompareStaticValue;
    }

    if left.kind == StateGuardOperandKind::Place && right.kind == StateGuardOperandKind::Place {
        return StateGuardLowering::CompareRuntimeValue;
    }

    StateGuardLowering::NeedsRuntimeExpression
}

fn guard_data(guard: &TransitionGuard) -> (StateGuardKind, StateGuardOperator, Expression, bool) {
    match guard {
        TransitionGuard::Always => (
            StateGuardKind::Always,
            StateGuardOperator::None,
            Expression::Boolean(true),
            false,
        ),
        TransitionGuard::When(expression) => (
            classify_transition_guard(guard),
            guard_operator(expression),
            expression.clone(),
            true,
        ),
    }
}

fn guard_operator(expression: &Expression) -> StateGuardOperator {
    let Expression::Binary(binary) = expression else {
        return StateGuardOperator::None;
    };

    match binary.operator {
        BinaryOperator::Equal => StateGuardOperator::Equal,
        BinaryOperator::NotEqual => StateGuardOperator::NotEqual,
        BinaryOperator::Greater => StateGuardOperator::Greater,
        BinaryOperator::GreaterOrEqual => StateGuardOperator::GreaterOrEqual,
        BinaryOperator::Less => StateGuardOperator::Less,
        BinaryOperator::LessOrEqual => StateGuardOperator::LessOrEqual,
        BinaryOperator::Add => StateGuardOperator::Add,
        BinaryOperator::And => StateGuardOperator::And,
        BinaryOperator::Or => StateGuardOperator::Or,
    }
}
