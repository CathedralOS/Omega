use crate::InstructionSelectionInput;
use omega_checked_trees::expression::{
    BinaryOperator, Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use omega_checked_trees::name::ProgramName;
use omega_checked_trees::statement::TransitionGuard;
use omega_core::arena::Arena;
use omega_runtime_branching::{RuntimeLeafBranchExpansion, RuntimeStraightLineBranchExpansion};
use omega_state_guards::StateGuardOperator;

use super::super::storage_places::{
    enum_variant_value, enum_variant_value_in_table, resolve_runtime_frame_indexed_target,
    resolve_runtime_frame_indexed_target_in_table, resolve_runtime_pointee_slot_offset,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_place,
    resolve_runtime_storage_place_in_table, resolve_runtime_transition_guard_call_result_place,
};
use omega_runtime_text::places::{
    expression_place_eq_across_tables, expression_place_eq_table_tree,
};
use omega_target_operations::{
    RuntimeValueOperand, RuntimeValueOperandHandle, SelectedInstructionKind, TargetDataObjectHandle,
};
use std::sync::Arc;

struct RuntimeTextLiteralGuard {
    buffer: TargetDataObjectHandle,
    literal: Arc<str>,
}

#[derive(Clone, Copy)]
struct RuntimeTextInputBufferData {
    buffer: TargetDataObjectHandle,
}

pub(super) fn select_runtime_leaf_branch_guard(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    if let Some(guard) = runtime_storage_guard_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        &input.runtime_branching_calls.expressions,
        expansion.resolved_guard,
    ) {
        return Some(guard);
    }
    if let Some(guard) = runtime_boolean_condition_guard_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        expansion.statement_index,
        &input.runtime_branching_calls.expressions,
        expansion.resolved_guard,
        runtime_value_operands,
    ) {
        return Some(guard);
    }
    if let Some(literal_guard) = runtime_text_literal_guard_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        &input.runtime_branching_calls.expressions,
        expansion.resolved_guard,
    ) {
        return Some(SelectedInstructionKind::CompareRuntimeTextLiteral {
            buffer: literal_guard.buffer,
            literal: literal_guard.literal,
        });
    }
    if let Some(guard) = runtime_text_storage_guard_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        &input.runtime_branching_calls.expressions,
        expansion.resolved_guard,
    ) {
        return Some(guard);
    }
    if let Some(guard) = runtime_value_guard_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        expansion.statement_index,
        &input.runtime_branching_calls.expressions,
        expansion.resolved_guard,
        runtime_value_operands,
    ) {
        return Some(guard);
    }

    let resolved_guard = runtime_branch_guard(input, expansion.resolved_guard);
    let normalized_guard =
        normalized_boolean_wrapped_guard(&resolved_guard).unwrap_or(resolved_guard);
    if let Some(guard) = runtime_boolean_condition_guard(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        expansion.statement_index,
        &normalized_guard,
        runtime_value_operands,
    ) {
        return Some(guard);
    }

    if let Some(literal_guard) = runtime_text_literal_guard(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        &normalized_guard,
    ) {
        return Some(SelectedInstructionKind::CompareRuntimeTextLiteral {
            buffer: literal_guard.buffer,
            literal: literal_guard.literal,
        });
    }

    runtime_text_storage_guard(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        &normalized_guard,
    )
    .or_else(|| {
        runtime_value_guard(
            input,
            expansion.dispatch_index,
            expansion.source_key,
            expansion.statement_index,
            &normalized_guard,
            runtime_value_operands,
        )
    })
    .or_else(|| {
        runtime_storage_guard(
            input,
            expansion.dispatch_index,
            expansion.source_key,
            &normalized_guard,
        )
    })
}

pub(super) fn select_runtime_straight_line_branch_guard(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    if let Some(guard) = runtime_storage_guard_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        &input.runtime_branching_calls.expressions,
        expansion.resolved_guard,
    ) {
        return Some(guard);
    }
    if let Some(guard) = runtime_boolean_condition_guard_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        expansion.statement_index,
        &input.runtime_branching_calls.expressions,
        expansion.resolved_guard,
        runtime_value_operands,
    ) {
        return Some(guard);
    }
    if let Some(literal_guard) = runtime_text_literal_guard_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        &input.runtime_branching_calls.expressions,
        expansion.resolved_guard,
    ) {
        return Some(SelectedInstructionKind::CompareRuntimeTextLiteral {
            buffer: literal_guard.buffer,
            literal: literal_guard.literal,
        });
    }
    if let Some(guard) = runtime_text_storage_guard_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        &input.runtime_branching_calls.expressions,
        expansion.resolved_guard,
    ) {
        return Some(guard);
    }
    if let Some(guard) = runtime_value_guard_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        expansion.statement_index,
        &input.runtime_branching_calls.expressions,
        expansion.resolved_guard,
        runtime_value_operands,
    ) {
        return Some(guard);
    }

    let resolved_guard = runtime_branch_guard(input, expansion.resolved_guard);
    let normalized_guard =
        normalized_boolean_wrapped_guard(&resolved_guard).unwrap_or(resolved_guard);
    if let Some(guard) = runtime_boolean_condition_guard(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        expansion.statement_index,
        &normalized_guard,
        runtime_value_operands,
    ) {
        return Some(guard);
    }

    if let Some(literal_guard) = runtime_text_literal_guard(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        &normalized_guard,
    ) {
        return Some(SelectedInstructionKind::CompareRuntimeTextLiteral {
            buffer: literal_guard.buffer,
            literal: literal_guard.literal,
        });
    }

    runtime_text_storage_guard(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        &normalized_guard,
    )
    .or_else(|| {
        runtime_value_guard(
            input,
            expansion.dispatch_index,
            expansion.source_key,
            expansion.statement_index,
            &normalized_guard,
            runtime_value_operands,
        )
    })
    .or_else(|| {
        runtime_storage_guard(
            input,
            expansion.dispatch_index,
            expansion.source_key,
            &normalized_guard,
        )
    })
}

fn runtime_branch_guard(
    input: &InstructionSelectionInput<'_>,
    guard: ExpressionHandle,
) -> TransitionGuard {
    if guard.is_valid() {
        TransitionGuard::When(input.runtime_branching_calls.expressions.to_tree(guard))
    } else {
        TransitionGuard::Always
    }
}

pub(super) fn select_runtime_dispatch_expression_guard(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    guard: &TransitionGuard,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    let normalized_guard = normalized_boolean_wrapped_guard(guard).unwrap_or_else(|| guard.clone());
    if let Some(guard) = runtime_boolean_condition_guard(
        input,
        dispatch_index,
        source_key,
        statement_index,
        &normalized_guard,
        runtime_value_operands,
    ) {
        return Some(guard);
    }

    if let Some(literal_guard) =
        runtime_text_literal_guard(input, dispatch_index, source_key, &normalized_guard)
    {
        return Some(SelectedInstructionKind::CompareRuntimeTextLiteral {
            buffer: literal_guard.buffer,
            literal: literal_guard.literal,
        });
    }

    runtime_text_storage_guard(input, dispatch_index, source_key, &normalized_guard)
        .or_else(|| {
            runtime_value_guard(
                input,
                dispatch_index,
                source_key,
                statement_index,
                &normalized_guard,
                runtime_value_operands,
            )
        })
        .or_else(|| runtime_storage_guard(input, dispatch_index, source_key, &normalized_guard))
}

pub(super) fn select_runtime_dispatch_expression_guard_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    guard: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    if !guard.is_valid() {
        return None;
    }

    if let Some(guard) = runtime_boolean_condition_guard_in_table(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        guard,
        runtime_value_operands,
    ) {
        return Some(guard);
    }

    if let Some(literal_guard) =
        runtime_text_literal_guard_in_table(input, dispatch_index, source_key, expressions, guard)
    {
        return Some(SelectedInstructionKind::CompareRuntimeTextLiteral {
            buffer: literal_guard.buffer,
            literal: literal_guard.literal,
        });
    }

    runtime_text_storage_guard_in_table(input, dispatch_index, source_key, expressions, guard)
        .or_else(|| {
            runtime_value_guard_in_table(
                input,
                dispatch_index,
                source_key,
                statement_index,
                expressions,
                guard,
                runtime_value_operands,
            )
        })
        .or_else(|| {
            runtime_storage_guard_in_table(input, dispatch_index, source_key, expressions, guard)
        })
}

fn normalized_boolean_wrapped_guard(guard: &TransitionGuard) -> Option<TransitionGuard> {
    let TransitionGuard::When(Expression::Binary(binary)) = guard else {
        return None;
    };

    let (inner, expected_true) = match (&binary.left, &binary.right) {
        (inner, Expression::Boolean(value)) => (inner, *value),
        (Expression::Boolean(value), inner) => (inner, *value),
        _ => return None,
    };

    let expected_true = match binary.operator {
        BinaryOperator::Equal => expected_true,
        BinaryOperator::NotEqual => !expected_true,
        _ => return None,
    };

    if expected_true {
        return Some(TransitionGuard::When(inner.clone()));
    }

    let Expression::Binary(inner_binary) = inner else {
        return None;
    };
    let inverted = match inner_binary.operator {
        BinaryOperator::Equal => BinaryOperator::NotEqual,
        BinaryOperator::NotEqual => BinaryOperator::Equal,
        BinaryOperator::Greater => BinaryOperator::LessOrEqual,
        BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
        BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
        BinaryOperator::LessOrEqual => BinaryOperator::Greater,
        _ => return None,
    };

    Some(TransitionGuard::When(Expression::Binary(Box::new(
        omega_checked_trees::expression::BinaryExpression {
            left: inner_binary.left.clone(),
            operator: inverted,
            right: inner_binary.right.clone(),
        },
    ))))
}

fn runtime_boolean_condition_guard(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    guard: &TransitionGuard,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    let TransitionGuard::When(expression) = guard else {
        return None;
    };
    let (expression, expected_true) = boolean_condition_expression(expression)?;
    if matches!(expression, Expression::Binary(_)) {
        return None;
    }

    let source_machine = source_machine_name(input, source_key);
    let source_state = source_state_name(input, source_key);
    let operand = resolve_runtime_value_operand(
        input,
        dispatch_index,
        source_key,
        statement_index,
        &source_machine,
        &source_state,
        expression,
        runtime_value_operands,
    )?;
    let byte_size = runtime_value_operand_byte_size(runtime_value_operands, operand);
    if !matches!(byte_size, 1 | 4 | 8) {
        return None;
    }
    let expected =
        runtime_value_operands.insert(RuntimeValueOperand::Immediate(i64::from(expected_true)));

    Some(SelectedInstructionKind::CompareRuntimeValues {
        left: operand,
        right: expected,
        byte_size,
        operator: StateGuardOperator::Equal,
    })
}

fn runtime_boolean_condition_guard_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    guard: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    let (expression, expected_true) = boolean_condition_expression_in_table(expressions, guard)?;
    if matches!(
        expressions.expression(expression),
        ExpressionNode::Binary(_)
    ) {
        return None;
    }

    let operand = resolve_runtime_value_operand_in_table(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        expression,
        runtime_value_operands,
    )?;
    let byte_size = runtime_value_operand_byte_size(runtime_value_operands, operand);
    if !matches!(byte_size, 1 | 4 | 8) {
        return None;
    }
    let expected =
        runtime_value_operands.insert(RuntimeValueOperand::Immediate(i64::from(expected_true)));

    Some(SelectedInstructionKind::CompareRuntimeValues {
        left: operand,
        right: expected,
        byte_size,
        operator: StateGuardOperator::Equal,
    })
}

fn boolean_condition_expression(expression: &Expression) -> Option<(&Expression, bool)> {
    let Expression::Binary(binary) = expression else {
        return Some((expression, true));
    };

    let (inner, expected_true) = match (&binary.left, &binary.right) {
        (inner, Expression::Boolean(value)) => (inner, *value),
        (Expression::Boolean(value), inner) => (inner, *value),
        _ => return Some((expression, true)),
    };

    let expected_true = match binary.operator {
        BinaryOperator::Equal => expected_true,
        BinaryOperator::NotEqual => !expected_true,
        _ => return Some((expression, true)),
    };

    Some((inner, expected_true))
}

fn boolean_condition_expression_in_table(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<(ExpressionHandle, bool)> {
    let ExpressionNode::Binary(binary) = expressions.expression(expression) else {
        return Some((expression, true));
    };

    let (inner, expected_true) =
        if let ExpressionNode::Boolean(value) = expressions.expression(binary.left) {
            (binary.right, *value)
        } else if let ExpressionNode::Boolean(value) = expressions.expression(binary.right) {
            (binary.left, *value)
        } else {
            return Some((expression, true));
        };

    let expected_true = match binary.operator {
        BinaryOperator::Equal => expected_true,
        BinaryOperator::NotEqual => !expected_true,
        _ => return Some((expression, true)),
    };

    Some((inner, expected_true))
}

fn runtime_text_literal_guard(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    guard: &TransitionGuard,
) -> Option<RuntimeTextLiteralGuard> {
    let TransitionGuard::When(Expression::Binary(binary)) = guard else {
        return None;
    };
    if binary.operator != BinaryOperator::Equal {
        return None;
    }

    let (text_place, literal) = match (&binary.left, &binary.right) {
        (text_place, Expression::String(literal)) => (text_place, literal),
        (Expression::String(literal), text_place) => (text_place, literal),
        _ => return None,
    };

    let buffer = runtime_text_input_buffer_data_for_text_place_in_state(
        input,
        dispatch_index,
        source_key,
        text_place,
    );
    if !buffer.buffer.is_valid() {
        return None;
    }
    Some(RuntimeTextLiteralGuard {
        buffer: buffer.buffer,
        literal: literal.clone(),
    })
}

fn runtime_text_literal_guard_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    expressions: &ExpressionTable,
    guard: ExpressionHandle,
) -> Option<RuntimeTextLiteralGuard> {
    let ExpressionNode::Binary(binary) = expressions.expression(guard) else {
        return None;
    };
    if binary.operator != BinaryOperator::Equal {
        return None;
    }

    let (text_place, literal) = if let Some(literal) = expressions.string_literal_value(binary.left)
    {
        (binary.right, literal)
    } else if let Some(literal) = expressions.string_literal_value(binary.right) {
        (binary.left, literal)
    } else {
        return None;
    };

    let buffer = runtime_text_input_buffer_data_for_text_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        text_place,
    );
    if !buffer.buffer.is_valid() {
        return None;
    }
    Some(RuntimeTextLiteralGuard {
        buffer: buffer.buffer,
        literal,
    })
}

fn runtime_text_storage_guard(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    guard: &TransitionGuard,
) -> Option<SelectedInstructionKind> {
    let TransitionGuard::When(Expression::Binary(binary)) = guard else {
        return None;
    };
    if binary.operator != BinaryOperator::Equal {
        return None;
    }
    let operator = StateGuardOperator::Equal;
    let source_machine = source_machine_name(input, source_key);
    let source_state = source_state_name(input, source_key);

    let left_place = resolve_runtime_storage_place(
        input,
        dispatch_index,
        source_key,
        &source_machine,
        &source_state,
        &binary.left,
    );
    let right_place = resolve_runtime_storage_place(
        input,
        dispatch_index,
        source_key,
        &source_machine,
        &source_state,
        &binary.right,
    );
    let left_buffer = runtime_text_input_buffer_data_for_text_place_in_state(
        input,
        dispatch_index,
        source_key,
        &binary.left,
    );
    let right_buffer = runtime_text_input_buffer_data_for_text_place_in_state(
        input,
        dispatch_index,
        source_key,
        &binary.right,
    );
    let string_descriptor_size = input.target.pointer_size * 2;

    if let Some(source_place) = left_place.clone()
        && right_buffer.buffer.is_valid()
        && source_place.byte_count == string_descriptor_size
    {
        return Some(SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer: right_buffer.buffer,
            source_region: source_place.region,
            source_offset: source_place.byte_offset,
            operator,
        });
    }

    if left_buffer.buffer.is_valid()
        && let Some(source_place) = right_place
        && source_place.byte_count == string_descriptor_size
    {
        return Some(SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer: left_buffer.buffer,
            source_region: source_place.region,
            source_offset: source_place.byte_offset,
            operator,
        });
    }

    None
}

fn runtime_text_storage_guard_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    expressions: &ExpressionTable,
    guard: ExpressionHandle,
) -> Option<SelectedInstructionKind> {
    let ExpressionNode::Binary(binary) = expressions.expression(guard) else {
        return None;
    };
    if binary.operator != BinaryOperator::Equal {
        return None;
    }
    let operator = StateGuardOperator::Equal;

    let left_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        binary.left,
    );
    let right_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        binary.right,
    );
    let left_buffer = runtime_text_input_buffer_data_for_text_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        binary.left,
    );
    let right_buffer = runtime_text_input_buffer_data_for_text_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        binary.right,
    );
    let string_descriptor_size = input.target.pointer_size * 2;

    if let Some(source_place) = left_place.clone()
        && right_buffer.buffer.is_valid()
        && source_place.byte_count == string_descriptor_size
    {
        return Some(SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer: right_buffer.buffer,
            source_region: source_place.region,
            source_offset: source_place.byte_offset,
            operator,
        });
    }

    if left_buffer.buffer.is_valid()
        && let Some(source_place) = right_place
        && source_place.byte_count == string_descriptor_size
    {
        return Some(SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer: left_buffer.buffer,
            source_region: source_place.region,
            source_offset: source_place.byte_offset,
            operator,
        });
    }

    None
}

fn runtime_storage_guard(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    guard: &TransitionGuard,
) -> Option<SelectedInstructionKind> {
    let TransitionGuard::When(Expression::Binary(binary)) = guard else {
        return None;
    };
    let operator = match binary.operator {
        BinaryOperator::Equal => StateGuardOperator::Equal,
        BinaryOperator::NotEqual => StateGuardOperator::NotEqual,
        _ => return None,
    };
    let source_machine = source_machine_name(input, source_key);
    let source_state = source_state_name(input, source_key);
    let left = resolve_runtime_storage_place(
        input,
        dispatch_index,
        source_key,
        &source_machine,
        &source_state,
        &binary.left,
    );
    let right = resolve_runtime_storage_place(
        input,
        dispatch_index,
        source_key,
        &source_machine,
        &source_state,
        &binary.right,
    );

    if let (Some(left), Some(right)) = (left.clone(), right.clone()) {
        if left.byte_count != right.byte_count {
            return None;
        }

        return Some(SelectedInstructionKind::CompareRuntimeStorage {
            left_region: left.region,
            left_offset: left.byte_offset,
            right_region: right.region,
            right_offset: right.byte_offset,
            byte_size: left.byte_count,
            operator,
        });
    }

    if let Some(place) = left
        && let Some(expected_value) = enum_variant_value(&input.layouts, &binary.right)
            .or_else(|| static_guard_value(&binary.right))
    {
        return Some(SelectedInstructionKind::CompareRuntimeStorageValue {
            region: place.region,
            byte_offset: place.byte_offset,
            byte_size: place.byte_count,
            expected_value,
            operator,
        });
    }

    if let Some(place) = right
        && let Some(expected_value) = enum_variant_value(&input.layouts, &binary.left)
            .or_else(|| static_guard_value(&binary.left))
    {
        return Some(SelectedInstructionKind::CompareRuntimeStorageValue {
            region: place.region,
            byte_offset: place.byte_offset,
            byte_size: place.byte_count,
            expected_value,
            operator,
        });
    }

    None
}

pub(super) fn runtime_storage_guard_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    expressions: &ExpressionTable,
    guard: ExpressionHandle,
) -> Option<SelectedInstructionKind> {
    let ExpressionNode::Binary(binary) = expressions.expression(guard) else {
        return None;
    };
    let operator = match binary.operator {
        BinaryOperator::Equal => StateGuardOperator::Equal,
        BinaryOperator::NotEqual => StateGuardOperator::NotEqual,
        _ => return None,
    };

    let left = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        binary.left,
    );
    let right = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        binary.right,
    );

    if let (Some(left), Some(right)) = (left.clone(), right.clone()) {
        if left.byte_count != right.byte_count {
            return None;
        }

        return Some(SelectedInstructionKind::CompareRuntimeStorage {
            left_region: left.region,
            left_offset: left.byte_offset,
            right_region: right.region,
            right_offset: right.byte_offset,
            byte_size: left.byte_count,
            operator,
        });
    }

    if let Some(place) = left
        && let Some(expected_value) =
            enum_variant_value_in_table(&input.layouts, expressions, binary.right)
                .or_else(|| static_guard_value_in_table(expressions, binary.right))
    {
        return Some(SelectedInstructionKind::CompareRuntimeStorageValue {
            region: place.region,
            byte_offset: place.byte_offset,
            byte_size: place.byte_count,
            expected_value,
            operator,
        });
    }

    if let Some(place) = right
        && let Some(expected_value) =
            enum_variant_value_in_table(&input.layouts, expressions, binary.left)
                .or_else(|| static_guard_value_in_table(expressions, binary.left))
    {
        return Some(SelectedInstructionKind::CompareRuntimeStorageValue {
            region: place.region,
            byte_offset: place.byte_offset,
            byte_size: place.byte_count,
            expected_value,
            operator,
        });
    }

    None
}

fn runtime_value_guard(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    guard: &TransitionGuard,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    let TransitionGuard::When(Expression::Binary(binary)) = guard else {
        return None;
    };
    let operator = runtime_compare_operator(binary.operator)?;
    let source_machine = source_machine_name(input, source_key);
    let source_state = source_state_name(input, source_key);
    let left = resolve_runtime_value_operand(
        input,
        dispatch_index,
        source_key,
        statement_index,
        &source_machine,
        &source_state,
        &binary.left,
        runtime_value_operands,
    )?;
    let right = resolve_runtime_value_operand(
        input,
        dispatch_index,
        source_key,
        statement_index,
        &source_machine,
        &source_state,
        &binary.right,
        runtime_value_operands,
    )?;
    let byte_size = runtime_value_operand_byte_size(runtime_value_operands, left).max(
        runtime_value_operand_byte_size(runtime_value_operands, right),
    );
    if !matches!(byte_size, 1 | 4 | 8) {
        return None;
    }
    Some(SelectedInstructionKind::CompareRuntimeValues {
        left,
        right,
        byte_size,
        operator,
    })
}

fn runtime_value_guard_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    guard: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    let ExpressionNode::Binary(binary) = expressions.expression(guard) else {
        return None;
    };
    let operator = runtime_compare_operator(binary.operator)?;
    let left = resolve_runtime_value_operand_in_table(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        binary.left,
        runtime_value_operands,
    )?;
    let right = resolve_runtime_value_operand_in_table(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        binary.right,
        runtime_value_operands,
    )?;
    let byte_size = runtime_value_operand_byte_size(runtime_value_operands, left).max(
        runtime_value_operand_byte_size(runtime_value_operands, right),
    );
    if !matches!(byte_size, 1 | 4 | 8) {
        return None;
    }
    Some(SelectedInstructionKind::CompareRuntimeValues {
        left,
        right,
        byte_size,
        operator,
    })
}

fn static_guard_value(expression: &Expression) -> Option<i64> {
    match expression {
        Expression::Boolean(value) => Some(i64::from(*value)),
        Expression::Integer(value) => Some(*value),
        _ => None,
    }
}

fn static_guard_value_in_table(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<i64> {
    match expressions.expression(expression) {
        ExpressionNode::Boolean(value) => Some(i64::from(*value)),
        ExpressionNode::Integer(value) => Some(*value),
        _ => None,
    }
}

fn resolve_runtime_value_operand(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    source_machine: &str,
    source_state: &str,
    expression: &Expression,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<RuntimeValueOperandHandle> {
    if let Some(value) =
        enum_variant_value(&input.layouts, expression).or_else(|| static_guard_value(expression))
    {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Immediate(value)));
    }

    if let Expression::Binary(binary) = expression {
        let operator = runtime_arithmetic_operator(binary.operator)?;
        let left = resolve_runtime_value_operand(
            input,
            dispatch_index,
            source_key,
            statement_index,
            source_machine,
            source_state,
            &binary.left,
            runtime_value_operands,
        )?;
        let right = resolve_runtime_value_operand(
            input,
            dispatch_index,
            source_key,
            statement_index,
            source_machine,
            source_state,
            &binary.right,
            runtime_value_operands,
        )?;
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Binary {
            left,
            operator,
            right,
        }));
    }

    if matches!(expression, Expression::Call(_))
        && let Some(place) = resolve_runtime_transition_guard_call_result_place(
            input,
            dispatch_index,
            source_key,
            statement_index,
        )
    {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Storage {
            region: place.region,
            byte_offset: place.byte_offset,
            byte_size: place.byte_count,
        }));
    }

    if let Some(pointer_target) =
        resolve_runtime_pointee_slot_offset(input, dispatch_index, source_key, expression)
    {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Pointee {
            pointer_byte_offset: pointer_target.pointer_byte_offset,
            field_byte_offset: pointer_target.field_byte_offset,
            byte_size: pointer_target.pointee_byte_size,
        }));
    }

    if let Some(indexed_target) =
        resolve_runtime_frame_indexed_target(input, dispatch_index, source_key, expression)
    {
        return Some(
            runtime_value_operands.insert(RuntimeValueOperand::FrameIndexed {
                descriptor_offset: indexed_target.descriptor_offset,
                index_offset: indexed_target.index_offset,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                byte_size: indexed_target.byte_count,
            }),
        );
    }

    let place = resolve_runtime_storage_place(
        input,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        expression,
    )?;
    Some(runtime_value_operands.insert(RuntimeValueOperand::Storage {
        region: place.region,
        byte_offset: place.byte_offset,
        byte_size: place.byte_count,
    }))
}

fn resolve_runtime_value_operand_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<RuntimeValueOperandHandle> {
    if let Some(value) = enum_variant_value_in_table(&input.layouts, expressions, expression)
        .or_else(|| static_guard_value_in_table(expressions, expression))
    {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Immediate(value)));
    }

    if let ExpressionNode::Binary(binary) = expressions.expression(expression) {
        let operator = runtime_arithmetic_operator(binary.operator)?;
        let left = resolve_runtime_value_operand_in_table(
            input,
            dispatch_index,
            source_key,
            statement_index,
            expressions,
            binary.left,
            runtime_value_operands,
        )?;
        let right = resolve_runtime_value_operand_in_table(
            input,
            dispatch_index,
            source_key,
            statement_index,
            expressions,
            binary.right,
            runtime_value_operands,
        )?;
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Binary {
            left,
            operator,
            right,
        }));
    }

    if matches!(expressions.expression(expression), ExpressionNode::Call(_))
        && let Some(place) = resolve_runtime_transition_guard_call_result_place(
            input,
            dispatch_index,
            source_key,
            statement_index,
        )
    {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Storage {
            region: place.region,
            byte_offset: place.byte_offset,
            byte_size: place.byte_count,
        }));
    }

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Pointee {
            pointer_byte_offset: pointer_target.pointer_byte_offset,
            field_byte_offset: pointer_target.field_byte_offset,
            byte_size: pointer_target.pointee_byte_size,
        }));
    }

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(
            runtime_value_operands.insert(RuntimeValueOperand::FrameIndexed {
                descriptor_offset: indexed_target.descriptor_offset,
                index_offset: indexed_target.index_offset,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                byte_size: indexed_target.byte_count,
            }),
        );
    }

    let place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )?;
    Some(runtime_value_operands.insert(RuntimeValueOperand::Storage {
        region: place.region,
        byte_offset: place.byte_offset,
        byte_size: place.byte_count,
    }))
}

fn runtime_value_operand_byte_size(
    runtime_value_operands: &Arena<RuntimeValueOperand>,
    operand: RuntimeValueOperandHandle,
) -> usize {
    match runtime_value_operands.get(operand) {
        RuntimeValueOperand::Immediate(_) => 8,
        RuntimeValueOperand::Storage { byte_size, .. } => *byte_size,
        RuntimeValueOperand::Pointee { byte_size, .. } => *byte_size,
        RuntimeValueOperand::FrameIndexed { byte_size, .. } => *byte_size,
        RuntimeValueOperand::Binary { left, right, .. } => {
            runtime_value_operand_byte_size(runtime_value_operands, *left).max(
                runtime_value_operand_byte_size(runtime_value_operands, *right),
            )
        }
    }
}

fn runtime_compare_operator(operator: BinaryOperator) -> Option<StateGuardOperator> {
    match operator {
        BinaryOperator::Equal => Some(StateGuardOperator::Equal),
        BinaryOperator::NotEqual => Some(StateGuardOperator::NotEqual),
        BinaryOperator::Greater => Some(StateGuardOperator::Greater),
        BinaryOperator::GreaterOrEqual => Some(StateGuardOperator::GreaterOrEqual),
        BinaryOperator::Less => Some(StateGuardOperator::Less),
        BinaryOperator::LessOrEqual => Some(StateGuardOperator::LessOrEqual),
        BinaryOperator::Add
        | BinaryOperator::And
        | BinaryOperator::Divide
        | BinaryOperator::Modulo
        | BinaryOperator::Multiply
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight
        | BinaryOperator::Subtract => None,
    }
}

fn runtime_arithmetic_operator(operator: BinaryOperator) -> Option<StateGuardOperator> {
    match operator {
        BinaryOperator::Add => Some(StateGuardOperator::Add),
        BinaryOperator::Modulo => Some(StateGuardOperator::Modulo),
        BinaryOperator::Multiply => Some(StateGuardOperator::Multiply),
        BinaryOperator::Subtract => Some(StateGuardOperator::Subtract),
        BinaryOperator::And
        | BinaryOperator::Divide
        | BinaryOperator::Equal
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::NotEqual
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight => None,
    }
}

fn runtime_text_input_buffer_data_for_text_place_in_state(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    expression: &Expression,
) -> RuntimeTextInputBufferData {
    let buffer = input.runtime_text.buffers.iter().find_map(|(_, buffer)| {
        (buffer.source_key == source_key
            && expression_place_eq_table_tree(
                &input.runtime_text.expressions,
                buffer.text_place,
                expression,
            ))
        .then_some(buffer)
    });
    let Some(buffer) = buffer else {
        return invalid_runtime_text_input_buffer_data();
    };

    let data = input.data.objects.iter().find(|(_, data_object)| {
        data_object.source_key == buffer.source_key
            && data_object.source_statement == buffer.statement_index
    });
    let Some((data, _)) = data else {
        return invalid_runtime_text_input_buffer_data();
    };

    let _ = dispatch_index;

    RuntimeTextInputBufferData { buffer: data }
}

fn runtime_text_input_buffer_data_for_text_place_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> RuntimeTextInputBufferData {
    let buffer = input.runtime_text.buffers.iter().find_map(|(_, buffer)| {
        (buffer.source_key == source_key
            && expression_place_eq_across_tables(
                &input.runtime_text.expressions,
                buffer.text_place,
                expressions,
                expression,
            ))
        .then_some(buffer)
    });
    let Some(buffer) = buffer else {
        return invalid_runtime_text_input_buffer_data();
    };

    let data = input.data.objects.iter().find(|(_, data_object)| {
        data_object.source_key == buffer.source_key
            && data_object.source_statement == buffer.statement_index
    });
    let Some((data, _)) = data else {
        return invalid_runtime_text_input_buffer_data();
    };

    let _ = dispatch_index;

    RuntimeTextInputBufferData { buffer: data }
}

fn invalid_runtime_text_input_buffer_data() -> RuntimeTextInputBufferData {
    RuntimeTextInputBufferData {
        buffer: TargetDataObjectHandle::invalid(),
    }
}

fn source_machine_name(
    input: &InstructionSelectionInput<'_>,
    key: omega_control_flow::StateKey,
) -> ProgramName {
    input.control_flow.state_machine_name_by_key_cloned(key)
}

fn source_state_name(
    input: &InstructionSelectionInput<'_>,
    key: omega_control_flow::StateKey,
) -> ProgramName {
    input.control_flow.state_name_by_key_cloned(key)
}
