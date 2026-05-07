use crate::ir::expression::Expression;
use crate::ir::statement::TransitionGuard;
use crate::native::runtime_flow::RuntimeTransitionTarget;
use crate::native::state_dispatch::{DispatchEdge, StateDispatchPlan};
use omega_core::arena::Arena;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGuardPlan {
    pub guards: Arena<StateGuard>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateGuard {
    pub source_machine: String,
    pub source_state: String,
    pub source_dispatch_index: u32,
    pub target: RuntimeTransitionTarget,
    pub target_dispatch_index: u32,
    pub continuation: RuntimeTransitionTarget,
    pub continuation_dispatch_index: u32,
    pub statement_order: usize,
    pub kind: StateGuardKind,
    pub expression: Expression,
    pub has_expression: bool,
    pub forms_cycle: bool,
}

impl Default for StateGuard {
    fn default() -> Self {
        Self {
            source_machine: String::new(),
            source_state: String::new(),
            source_dispatch_index: 0,
            target: RuntimeTransitionTarget::None,
            target_dispatch_index: 0,
            continuation: RuntimeTransitionTarget::None,
            continuation_dispatch_index: 0,
            statement_order: 0,
            kind: StateGuardKind::Always,
            expression: Expression::Boolean(true),
            has_expression: false,
            forms_cycle: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateGuardKind {
    #[default]
    Always,
    RuntimeBinary,
    RuntimeExpression,
}

pub fn build_state_guard_plan(state_dispatch: &StateDispatchPlan) -> StateGuardPlan {
    let mut plan = StateGuardPlan::default();

    for (_, state) in state_dispatch.states.iter() {
        let Some(edges) = state_dispatch.edges.span(state.edges) else {
            continue;
        };

        for (statement_order, edge) in edges.iter().enumerate() {
            plan.guards.insert(build_state_guard(
                &state.machine,
                &state.state,
                state.dispatch_index,
                statement_order,
                edge,
            ));
        }
    }

    plan
}

fn build_state_guard(
    source_machine: &str,
    source_state: &str,
    source_dispatch_index: u32,
    statement_order: usize,
    edge: &DispatchEdge,
) -> StateGuard {
    let (kind, expression, has_expression) = guard_data(&edge.guard);

    StateGuard {
        source_machine: source_machine.to_owned(),
        source_state: source_state.to_owned(),
        source_dispatch_index,
        target: edge.target.clone(),
        target_dispatch_index: edge.target_dispatch_index,
        continuation: edge.continuation.clone(),
        continuation_dispatch_index: edge.continuation_dispatch_index,
        statement_order,
        kind,
        expression,
        has_expression,
        forms_cycle: edge.forms_cycle,
    }
}

fn guard_data(guard: &TransitionGuard) -> (StateGuardKind, Expression, bool) {
    match guard {
        TransitionGuard::Always => (StateGuardKind::Always, Expression::Boolean(true), false),
        TransitionGuard::When(expression) => (
            match expression {
                Expression::Binary(_) => StateGuardKind::RuntimeBinary,
                _ => StateGuardKind::RuntimeExpression,
            },
            expression.clone(),
            true,
        ),
    }
}
