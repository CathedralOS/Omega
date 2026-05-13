mod model;
mod normalize;
mod operands;

pub use model::{
    StateGuard, StateGuardKind, StateGuardOperand, StateGuardOperandKind, StateGuardOperandStorage,
    StateGuardPlan,
};
use omega_checked_trees::expression::{
    BinaryOperator, Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use omega_checked_trees::machine::Machine;
use omega_checked_trees::statement::TransitionGuard;
use omega_checked_trees::Program;
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_core::arena::Arena;
use omega_core::symbols::SymbolHandle;
use omega_layout::LayoutPlan;
use omega_runtime_storage::RuntimeStoragePlan;
use omega_state_dispatch::{DispatchEdge, StateDispatchPlan};
use omega_state_values::simplify_expression;
pub use omega_target_operations::{StateGuardLowering, StateGuardOperator};
use normalize::normalize_guard_expression;
use operands::{GuardOperands, guard_operands};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StateGuardClause {
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

pub fn build_state_guard_plan(
    program: &Program,
    state_dispatch: &StateDispatchPlan,
    control_flow: &ControlFlowPlan,
    layouts: &LayoutPlan,
    runtime_storage: &RuntimeStoragePlan,
    entry_machine: SymbolHandle,
) -> StateGuardPlan {
    let mut plan = StateGuardPlan::default();

    for (_, state) in state_dispatch.states.iter() {
        let Some(machine) = machine_by_symbol(program, state.key.machine) else {
            continue;
        };
        let Some(edges) = state_dispatch.edges.span(state.edges) else {
            continue;
        };

        for (statement_order, edge) in edges.iter().enumerate() {
            if control_flow.state_by_key(state.key).is_none() {
                continue;
            }
            plan.guards.insert(build_state_guard(
                program,
                &control_flow.expressions,
                &mut plan.expressions,
                &mut plan.operands,
                layouts,
                runtime_storage,
                entry_machine,
                machine,
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
                BinaryOperator::Add
                | BinaryOperator::And
                | BinaryOperator::Divide
                | BinaryOperator::Modulo
                | BinaryOperator::Multiply
                | BinaryOperator::Or
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::Subtract => {
                    StateGuardKind::RuntimeExpression
                }
            },
            _ => StateGuardKind::RuntimeExpression,
        },
    }
}

pub fn classify_transition_guard_expression(
    table: &ExpressionTable,
    guard: Option<ExpressionHandle>,
) -> StateGuardKind {
    let Some(guard) = guard else {
        return StateGuardKind::Always;
    };

    match table.expression(guard) {
        ExpressionNode::Binary(binary) => match binary.operator {
            BinaryOperator::Equal => StateGuardKind::RuntimeEquality,
            BinaryOperator::NotEqual => StateGuardKind::RuntimeInequality,
            BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual => StateGuardKind::RuntimeOrdering,
            BinaryOperator::Add
            | BinaryOperator::And
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::Multiply
            | BinaryOperator::Or
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
            | BinaryOperator::Subtract => {
                StateGuardKind::RuntimeExpression
            }
        },
        _ => StateGuardKind::RuntimeExpression,
    }
}

fn build_state_guard(
    program: &Program,
    source_expressions: &ExpressionTable,
    guard_expressions: &mut ExpressionTable,
    operand_arena: &mut Arena<StateGuardOperand>,
    layouts: &LayoutPlan,
    runtime_storage: &RuntimeStoragePlan,
    entry_machine: SymbolHandle,
    source_machine: &Machine,
    source: StateKey,
    source_dispatch_index: u32,
    statement_order: usize,
    edge: &DispatchEdge,
) -> StateGuard {
    let source_guard = edge.expressions.guard;
    let simplified_guard = source_guard.map(|guard| {
        simplify_expression(
            program,
            source_machine,
            &source_expressions.to_tree(guard),
        )
    });
    let mut normalized_expressions = ExpressionTable::new();
    let normalized_guard = normalize_guard_expression(
        source_expressions,
        simplified_guard.as_ref(),
        source_guard,
    )
        .map(|guard| normalized_expressions.insert_tree(&guard));
    let kind = classify_transition_guard_expression(&normalized_expressions, normalized_guard);
    let operator = normalized_guard
        .map(|guard| guard_operator(&normalized_expressions, guard))
        .unwrap_or(StateGuardOperator::None);
    let expression = normalized_guard
        .map(|guard| guard_expressions.copy_from(&normalized_expressions, guard))
        .unwrap_or_else(ExpressionHandle::invalid);
    let has_expression = normalized_guard.is_some();
    let guard_operands = guard_operands(
        &normalized_expressions,
        guard_expressions,
        layouts,
        runtime_storage,
        entry_machine,
        source,
        source_machine.symbol,
        source_dispatch_index,
        statement_order,
        normalized_guard,
    );
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

fn machine_by_symbol<'program>(
    program: &'program Program,
    machine_symbol: SymbolHandle,
) -> Option<&'program Machine> {
    program
        .machines
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
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
        StateGuardOperator::Equal
            | StateGuardOperator::NotEqual
            | StateGuardOperator::Greater
            | StateGuardOperator::GreaterOrEqual
            | StateGuardOperator::Less
            | StateGuardOperator::LessOrEqual
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

fn guard_operator(table: &ExpressionTable, expression: ExpressionHandle) -> StateGuardOperator {
    let ExpressionNode::Binary(binary) = table.expression(expression) else {
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
        BinaryOperator::Divide
        | BinaryOperator::Modulo
        | BinaryOperator::Multiply
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight
        | BinaryOperator::Subtract => StateGuardOperator::None,
    }
}

pub fn lower_guard_conjunction(
    plan: &StateGuardPlan,
    layouts: &LayoutPlan,
    runtime_storage: &RuntimeStoragePlan,
    entry_machine: SymbolHandle,
    source_key: StateKey,
    source_machine: SymbolHandle,
    source_dispatch_index: u32,
    statement_order: usize,
) -> Option<Vec<StateGuardClause>> {
    let guard = plan.guard_for_dispatch(source_dispatch_index, statement_order)?;
    let expression = guard.expression;
    if !expression.is_valid() {
        return None;
    }

    let mut leaves = Vec::new();
    flatten_guard_conjunction(&plan.expressions, expression, &mut leaves)?;

    let mut clauses = Vec::with_capacity(leaves.len());
    for leaf in leaves {
        let kind = classify_transition_guard_expression(&plan.expressions, Some(leaf));
        let operator = guard_operator(&plan.expressions, leaf);
        let mut scratch = ExpressionTable::new();
        let operands = guard_operands(
            &plan.expressions,
            &mut scratch,
            layouts,
            runtime_storage,
            entry_machine,
            source_key,
            source_machine,
            source_dispatch_index,
            statement_order,
            Some(leaf),
        )?;
        let lowering = guard_lowering(kind, operator, Some(&operands));
        if matches!(lowering, StateGuardLowering::NeedsRuntimeExpression | StateGuardLowering::NoOp)
        {
            return None;
        }

        let mut place_operands = [&operands.left, &operands.right]
            .into_iter()
            .filter(|operand| {
                operand.kind == StateGuardOperandKind::Place
                    && operand.storage != StateGuardOperandStorage::Unknown
            });
        let place_operand = place_operands.next()?;
        let right_place_operand = place_operands.next();
        let expected_value = [&operands.left, &operands.right]
            .into_iter()
            .find(|operand| operand.has_resolved_value)
            .map(|operand| operand.resolved_value)
            .unwrap_or(0);

        clauses.push(StateGuardClause {
            lowering,
            operator,
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
        });
    }

    Some(clauses)
}

fn flatten_guard_conjunction(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    leaves: &mut Vec<ExpressionHandle>,
) -> Option<()> {
    match table.expression(expression) {
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
            flatten_guard_conjunction(table, binary.left, leaves)?;
            flatten_guard_conjunction(table, binary.right, leaves)?;
            Some(())
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
            ) =>
        {
            leaves.push(expression);
            Some(())
        }
        _ => None,
    }
}
