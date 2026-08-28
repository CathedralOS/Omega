use crate::builder::{classify_transition_guard_expression, guard_lowering, guard_operator};
use crate::clauses::{StateGuardClause, StateGuardClauses};
use crate::operands::guard_operands;
use crate::{
    StateGuardKind, StateGuardLowering, StateGuardOperandKind, StateGuardOperandStorage,
    StateGuardOperator, StateGuardPlan,
};
use omega_control_flow::StateKey;
use omega_layout::LayoutPlan;
use omega_runtime_storage::RuntimeStoragePlan;
use psi_checked_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable, TableBinaryExpression,
};
use psi_symbols::SymbolHandle;

pub fn lower_guard_conjunction(
    plan: &StateGuardPlan,
    layouts: &LayoutPlan,
    runtime_storage: &RuntimeStoragePlan,
    receiver_bases: &[Option<usize>],
    state_contexts: &[u32],
    entry_machine: SymbolHandle,
    source_key: StateKey,
    source_machine: SymbolHandle,
    source_dispatch_index: u32,
    statement_order: usize,
) -> StateGuardClauses {
    let Some(guard) = plan.guard_for_dispatch(source_dispatch_index, statement_order) else {
        return StateGuardClauses::new();
    };
    let expression = guard.expression;
    if !expression.is_valid() {
        return StateGuardClauses::new();
    }

    let clause_capacity =
        guard_conjunction_clause_count(&plan.expressions, expression).unwrap_or(0);
    let mut clauses = StateGuardClauses::with_capacity(clause_capacity);
    let mut scratch_expressions =
        ExpressionTable::with_expression_capacity(clause_capacity.saturating_mul(2));
    if lower_guard_conjunction_expression(
        plan,
        layouts,
        runtime_storage,
        receiver_bases,
        state_contexts,
        entry_machine,
        source_key,
        source_machine,
        source_dispatch_index,
        guard.statement_index,
        expression,
        &mut scratch_expressions,
        &mut clauses,
    )
    .is_none()
    {
        return StateGuardClauses::new();
    }

    clauses
}

fn guard_conjunction_clause_count(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<usize> {
    match expressions.expression(expression) {
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
            let left = guard_conjunction_clause_count(expressions, binary.left)?;
            let right = guard_conjunction_clause_count(expressions, binary.right)?;
            Some(left.saturating_add(right))
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
            Some(1)
        }
        _ if expressions.expression_is_direct_place_path(expression) => Some(1),
        _ => None,
    }
}

fn lower_guard_conjunction_expression(
    plan: &StateGuardPlan,
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
    scratch_expressions: &mut ExpressionTable,
    clauses: &mut StateGuardClauses,
) -> Option<()> {
    match plan.expressions.expression(expression) {
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
            lower_guard_conjunction_expression(
                plan,
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
                scratch_expressions,
                clauses,
            )?;
            lower_guard_conjunction_expression(
                plan,
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
                scratch_expressions,
                clauses,
            )
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
            lower_guard_leaf(
                plan,
                layouts,
                runtime_storage,
                receiver_bases,
                state_contexts,
                entry_machine,
                source_key,
                source_machine,
                source_dispatch_index,
                statement_index,
                expression,
                scratch_expressions,
                clauses,
            )
        }
        _ if plan.expressions.expression_is_direct_place_path(expression) => lower_guard_leaf(
            plan,
            layouts,
            runtime_storage,
            receiver_bases,
            state_contexts,
            entry_machine,
            source_key,
            source_machine,
            source_dispatch_index,
            statement_index,
            expression,
            scratch_expressions,
            clauses,
        ),
        _ => None,
    }
}

fn lower_guard_leaf(
    plan: &StateGuardPlan,
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
    scratch_expressions: &mut ExpressionTable,
    clauses: &mut StateGuardClauses,
) -> Option<()> {
    let mut operand_expressions = ExpressionTable::with_expression_capacity(2);
    let (source_expressions, normalized_expression, kind, operator) =
        if plan.expressions.expression_is_direct_place_path(expression) {
            scratch_expressions.clear();
            let left = scratch_expressions.copy_from(&plan.expressions, expression);
            let right = scratch_expressions.insert(ExpressionNode::Boolean(true));
            let normalized_expression =
                scratch_expressions.insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: BinaryOperator::Equal,
                    right,
                }));
            (
                &*scratch_expressions,
                normalized_expression,
                StateGuardKind::RuntimeEquality,
                StateGuardOperator::Equal,
            )
        } else {
            (
                &plan.expressions,
                expression,
                classify_transition_guard_expression(&plan.expressions, expression),
                guard_operator(&plan.expressions, expression),
            )
        };
    let operands = guard_operands(
        source_expressions,
        &mut operand_expressions,
        layouts,
        runtime_storage,
        receiver_bases,
        state_contexts,
        entry_machine,
        source_key,
        source_machine,
        source_dispatch_index,
        statement_index,
        normalized_expression,
    )?;
    let lowering = guard_lowering(kind, operator, Some(&operands));
    if matches!(
        lowering,
        StateGuardLowering::NeedsRuntimeExpression | StateGuardLowering::NoOp
    ) {
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

    // FLOAT-kinded conjunct: one side is a constant float expression --
    // the emission's FCMP path narrows the f64 expectation bits by the
    // place's byte_size (f32-wide compares included). Found via
    // `self.f > 3.14 && self.f < 3.15` on `f: f32` comparing raw f32
    // pattern bytes against f64 literal bits on the INTEGER compare path
    // (the conjunction splitter hardcoded is_float: false).
    let is_float = match source_expressions.expression(normalized_expression) {
        ExpressionNode::Binary(binary) => {
            crate::operands::values::guard_operand_is_float_constant(
                source_expressions,
                binary.left,
            ) || crate::operands::values::guard_operand_is_float_constant(
                source_expressions,
                binary.right,
            )
        }
        _ => false,
    };
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
        is_float,
    });

    Some(())
}
