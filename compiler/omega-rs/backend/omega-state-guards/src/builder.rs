use crate::normalize::normalize_guard_expression_into_table;
use crate::operands::{GuardOperands, guard_operands};
use crate::{
    StateGuard, StateGuardKind, StateGuardLowering, StateGuardOperandKind,
    StateGuardOperandStorage, StateGuardOperator, StateGuardPlan,
};
use omega_checked_trees::CheckedTrees;
use omega_checked_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable, TableBinaryExpression,
};
use omega_checked_trees::machine::Machine;
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_core::arena::Arena;
use omega_core::symbols::SymbolHandle;
use omega_layout::LayoutPlan;
use omega_runtime_storage::RuntimeStoragePlan;
use omega_state_dispatch::{DispatchEdge, StateDispatchPlan};
use omega_state_values::simplify_expression;

pub fn build_state_guard_plan(
    program: &CheckedTrees,
    state_dispatch: &StateDispatchPlan,
    control_flow: &ControlFlowPlan,
    layouts: &LayoutPlan,
    runtime_storage: &RuntimeStoragePlan,
    entry_machine: SymbolHandle,
) -> StateGuardPlan {
    let guard_capacity = state_dispatch.edges.len();
    let mut plan = StateGuardPlan {
        expressions: ExpressionTable::with_expression_capacity(guard_capacity),
        guards: Arena::with_capacity(guard_capacity),
        operands: Arena::with_capacity(guard_capacity.saturating_mul(2)),
    };
    let mut normalized_expressions = ExpressionTable::with_expression_capacity(4);

    for (_, state) in state_dispatch.states.iter() {
        let Some(machine) = machine_by_symbol(program, state.key.machine) else {
            continue;
        };
        let Some(edges) = state_dispatch.edges.span(state.edges) else {
            continue;
        };

        // A dispatch state may be a SEGMENT (segment_index > 0) of a control-flow
        // state split at its dispatched-call boundaries; only the unsplit state
        // (segment 0) is in the control-flow plan, and a segment's guard operands
        // resolve against that state's places. Map to segment 0 for the lookups.
        let control_key = StateKey {
            segment_index: 0,
            ..state.key
        };

        for (statement_order, edge) in edges.iter().enumerate() {
            if control_flow.state_by_key(control_key).is_none() {
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
                &mut normalized_expressions,
                control_key,
                state.dispatch_index,
                statement_order,
                edge.statement_index,
                edge,
            ));
        }
    }

    plan
}

pub fn classify_transition_guard_expression(
    table: &ExpressionTable,
    guard: ExpressionHandle,
) -> StateGuardKind {
    if !guard.is_valid() {
        return StateGuardKind::Always;
    }

    match table.expression(guard) {
        ExpressionNode::Boolean(true) => StateGuardKind::Always,
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
            | BinaryOperator::Subtract => StateGuardKind::RuntimeExpression,
        },
        _ => StateGuardKind::RuntimeExpression,
    }
}

pub(crate) fn guard_lowering(
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
            | StateGuardOperator::GreaterUnsigned
            | StateGuardOperator::GreaterOrEqualUnsigned
            | StateGuardOperator::LessUnsigned
            | StateGuardOperator::LessOrEqualUnsigned
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

pub(crate) fn guard_operator(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> StateGuardOperator {
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

fn build_state_guard(
    program: &CheckedTrees,
    source_expressions: &ExpressionTable,
    guard_expressions: &mut ExpressionTable,
    operand_arena: &mut Arena<crate::StateGuardOperand>,
    layouts: &LayoutPlan,
    runtime_storage: &RuntimeStoragePlan,
    entry_machine: SymbolHandle,
    source_machine: &Machine,
    normalized_expressions: &mut ExpressionTable,
    source: StateKey,
    source_dispatch_index: u32,
    statement_order: usize,
    statement_index: usize,
    edge: &DispatchEdge,
) -> StateGuard {
    let source_guard = edge.expressions.guard;
    normalized_expressions.clear();
    let normalized_guard = normalized_guard_expression(
        program,
        source_machine,
        source_expressions,
        source_guard,
        normalized_expressions,
    );
    let kind = classify_transition_guard_expression(normalized_expressions, normalized_guard);
    let operator = normalized_guard
        .is_valid()
        .then(|| guard_operator(normalized_expressions, normalized_guard))
        .unwrap_or(StateGuardOperator::None);
    let expression = normalized_guard
        .is_valid()
        .then(|| guard_expressions.copy_from(normalized_expressions, normalized_guard))
        .unwrap_or_else(ExpressionHandle::invalid);
    let has_expression = normalized_guard.is_valid();
    let guard_operands = guard_operands(
        normalized_expressions,
        guard_expressions,
        layouts,
        runtime_storage,
        entry_machine,
        source,
        source_machine.symbol,
        source_dispatch_index,
        statement_index,
        normalized_guard,
    );
    let (kind, operator, lowering, operands) = if let Some(expected) = normalized_guard
        .is_valid()
        .then(|| normalized_guard)
        .and_then(|guard| match normalized_expressions.expression(guard) {
            ExpressionNode::Boolean(value) => Some(*value),
            _ => None,
        })
        && let Some(slot) = runtime_storage.transition_guard_result_slot(
            source_dispatch_index,
            source,
            statement_index,
        ) {
        let operands = GuardOperands {
            left: crate::StateGuardOperand {
                expression,
                kind: StateGuardOperandKind::Place,
                storage: StateGuardOperandStorage::RuntimeFrame,
                byte_offset: slot.byte_offset,
                byte_size: slot.byte_size,
                resolved_value: 0,
                has_resolved_value: false,
            },
            right: crate::StateGuardOperand {
                expression,
                kind: StateGuardOperandKind::Literal,
                storage: StateGuardOperandStorage::Unknown,
                byte_offset: 0,
                byte_size: slot.byte_size,
                resolved_value: i64::from(expected),
                has_resolved_value: true,
            },
        };
        (
            StateGuardKind::RuntimeEquality,
            StateGuardOperator::Equal,
            StateGuardLowering::CompareStaticValue,
            operands.insert_into(operand_arena),
        )
    } else {
        let lowering = guard_lowering(kind, operator, guard_operands.as_ref());
        let operands = guard_operands
            .map(|operands| operands.insert_into(operand_arena))
            .unwrap_or_default();
        (kind, operator, lowering, operands)
    };

    StateGuard {
        source,
        source_dispatch_index,
        target: edge.target,
        target_dispatch_index: edge.target_dispatch_index,
        continuation: edge.continuation,
        continuation_dispatch_index: edge.continuation_dispatch_index,
        statement_order,
        statement_index,
        kind,
        operator,
        lowering,
        expression,
        operands,
        has_expression,
        forms_cycle: edge.forms_cycle,
    }
}

fn normalized_guard_expression(
    program: &CheckedTrees,
    source_machine: &Machine,
    source_expressions: &ExpressionTable,
    source_guard: ExpressionHandle,
    normalized_expressions: &mut ExpressionTable,
) -> ExpressionHandle {
    if !source_guard.is_valid() {
        return ExpressionHandle::invalid();
    }

    if let ExpressionNode::Boolean(value) = source_expressions.expression(source_guard) {
        return normalized_expressions.insert(ExpressionNode::Boolean(*value));
    }

    if source_expressions.expression_is_direct_place_path(source_guard) {
        let left = normalized_expressions.copy_from(source_expressions, source_guard);
        let right = normalized_expressions.insert(ExpressionNode::Boolean(true));
        return normalized_expressions.insert(ExpressionNode::Binary(TableBinaryExpression {
            left,
            operator: BinaryOperator::Equal,
            right,
        }));
    }

    if let Some(normalized_guard) = normalized_direct_place_boolean_guard(
        source_expressions,
        source_guard,
        normalized_expressions,
    ) {
        return normalized_guard;
    }

    let simplified_guard = simplify_expression(
        program,
        source_machine,
        &source_expressions.to_tree(source_guard),
    );
    normalize_guard_expression_into_table(simplified_guard, normalized_expressions)
}

fn normalized_direct_place_boolean_guard(
    source_expressions: &ExpressionTable,
    source_guard: ExpressionHandle,
    normalized_expressions: &mut ExpressionTable,
) -> Option<ExpressionHandle> {
    let ExpressionNode::Binary(binary) = source_expressions.expression(source_guard) else {
        return None;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) {
        return None;
    }

    let (place, expected) = match (
        source_expressions.expression(binary.left),
        source_expressions.expression(binary.right),
    ) {
        (_, ExpressionNode::Boolean(value))
            if source_expressions.expression_is_direct_place_path(binary.left) =>
        {
            (binary.left, positive_branch(binary.operator, *value))
        }
        (ExpressionNode::Boolean(value), _)
            if source_expressions.expression_is_direct_place_path(binary.right) =>
        {
            (binary.right, positive_branch(binary.operator, *value))
        }
        _ => return None,
    };

    let left = normalized_expressions.copy_from(source_expressions, place);
    let right = normalized_expressions.insert(ExpressionNode::Boolean(expected));
    Some(
        normalized_expressions.insert(ExpressionNode::Binary(TableBinaryExpression {
            left,
            operator: BinaryOperator::Equal,
            right,
        })),
    )
}

fn positive_branch(operator: BinaryOperator, flag: bool) -> bool {
    match operator {
        BinaryOperator::Equal => flag,
        BinaryOperator::NotEqual => !flag,
        _ => true,
    }
}

fn machine_by_symbol<'program>(
    program: &'program CheckedTrees,
    machine_symbol: SymbolHandle,
) -> Option<&'program Machine> {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
}
