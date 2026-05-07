use crate::ir::expression::{BinaryOperator, Expression};
use crate::ir::statement::TransitionGuard;
use crate::native::runtime_dispatch::states::{DispatchEdge, StateDispatchPlan};
use crate::native::runtime_flow::RuntimeTransitionTarget;
use omega_core::arena::{Arena, HandleSpan};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGuardPlan {
    pub guards: Arena<StateGuard>,
    pub operands: Arena<StateGuardOperand>,
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
    pub operator: StateGuardOperator,
    pub expression: Expression,
    pub operands: HandleSpan<StateGuardOperand>,
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
            operator: StateGuardOperator::None,
            expression: Expression::Boolean(true),
            operands: HandleSpan::empty(),
            has_expression: false,
            forms_cycle: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateGuardKind {
    #[default]
    Always,
    RuntimeEquality,
    RuntimeInequality,
    RuntimeOrdering,
    RuntimeExpression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateGuardOperator {
    #[default]
    None,
    Equal,
    NotEqual,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
    Add,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateGuardOperand {
    pub expression: Expression,
    pub kind: StateGuardOperandKind,
}

impl Default for StateGuardOperand {
    fn default() -> Self {
        Self {
            expression: Expression::Boolean(true),
            kind: StateGuardOperandKind::OtherExpression,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateGuardOperandKind {
    Place,
    StaticSymbol,
    Literal,
    #[default]
    OtherExpression,
}

pub fn build_state_guard_plan(state_dispatch: &StateDispatchPlan) -> StateGuardPlan {
    let mut plan = StateGuardPlan::default();

    for (_, state) in state_dispatch.states.iter() {
        let Some(edges) = state_dispatch.edges.span(state.edges) else {
            continue;
        };

        for (statement_order, edge) in edges.iter().enumerate() {
            plan.guards.insert(build_state_guard(
                &mut plan.operands,
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
    operands: &mut Arena<StateGuardOperand>,
    source_machine: &str,
    source_state: &str,
    source_dispatch_index: u32,
    statement_order: usize,
    edge: &DispatchEdge,
) -> StateGuard {
    let (kind, operator, expression, has_expression) = guard_data(&edge.guard);
    let operands = operands.insert_many(guard_operands(&edge.guard));

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
        operator,
        expression,
        operands,
        has_expression,
        forms_cycle: edge.forms_cycle,
    }
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

fn guard_operands(guard: &TransitionGuard) -> Vec<StateGuardOperand> {
    let TransitionGuard::When(Expression::Binary(binary)) = guard else {
        return Vec::new();
    };

    [binary.left.clone(), binary.right.clone()]
        .into_iter()
        .map(|expression| StateGuardOperand {
            kind: classify_guard_operand(&expression),
            expression,
        })
        .collect()
}

fn classify_guard_operand(expression: &Expression) -> StateGuardOperandKind {
    match expression {
        Expression::Name(path) if is_static_symbol_path(path) => {
            StateGuardOperandKind::StaticSymbol
        }
        Expression::Name(_) | Expression::Indexed(_) => StateGuardOperandKind::Place,
        Expression::Boolean(_)
        | Expression::Float(_)
        | Expression::Integer(_)
        | Expression::String(_) => StateGuardOperandKind::Literal,
        Expression::ArrayLiteral(_)
        | Expression::Binary(_)
        | Expression::Mutable(_)
        | Expression::StructLiteral(_) => StateGuardOperandKind::OtherExpression,
    }
}

fn is_static_symbol_path(path: &[String]) -> bool {
    path.first()
        .and_then(|segment| segment.chars().next())
        .is_some_and(char::is_uppercase)
}
