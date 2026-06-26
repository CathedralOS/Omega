use crate::InstructionSelectionInput;
use omega_checked_trees::expression::{
    BinaryOperator, Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
    TableBinaryExpression,
};
use omega_checked_trees::name::Identifier;
use omega_checked_trees::statement::TransitionGuard;
use omega_core::arena::Arena;
use omega_runtime_branching::{RuntimeLeafBranchExpansion, RuntimeStraightLineBranchExpansion};
use omega_state_guards::{StateGuardKind, StateGuardOperator};

use super::super::storage_places::{
    clamp_runtime_case_comparison_operands, clamp_runtime_case_comparison_operands_in_table,
    enum_variant_value, enum_variant_value_in_table,
    resolve_runtime_frame_base_indexed_target_in_table,
    resolve_runtime_frame_fixed_indexed_target_in_table,
    resolve_runtime_frame_indexed_is_fat_slice_in_table,
    resolve_runtime_frame_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_is_signed_in_table,
    resolve_runtime_frame_indexed_primitive_type_in_table, resolve_runtime_storage_place,
    resolve_runtime_storage_place_in_table,
    resolve_runtime_storage_place_is_bounded_byte_buffer_in_table,
    resolve_runtime_storage_place_is_fat_slice_in_table,
    resolve_runtime_storage_primitive_type_in_table,
    resolve_runtime_transition_guard_call_result_place, static_elided_local_value_in_table,
    static_fixed_array_len_in_table,
};
use omega_checked_trees::types::PrimitiveType;
use omega_abstract_operations::{
    RuntimeValueOperand, RuntimeValueOperandHandle, SelectedInstructionKind, TargetDataObjectHandle,
};
use omega_runtime_text::places::{
    expression_place_eq_across_tables, expression_place_eq_table_tree,
};
use super::writes::resolve_runtime_text_equals_operand_in_table;
use std::sync::Arc;

struct RuntimeTextLiteralGuard {
    buffer: TargetDataObjectHandle,
    literal: Arc<str>,
}

#[derive(Clone, Copy)]
struct RuntimeTextInputBufferData {
    buffer: TargetDataObjectHandle,
}

pub(super) fn select_runtime_leaf_branch_guards(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Vec<SelectedInstructionKind> {
    if expansion.guard_kind == StateGuardKind::Always || !expansion.resolved_guard.is_valid() {
        return Vec::new();
    }

    select_runtime_branch_guard_conjuncts_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        expansion.statement_index,
        &input.runtime_branching_calls.expressions,
        expansion.resolved_guard,
        runtime_value_operands,
    )
}

pub(super) fn select_runtime_straight_line_branch_guards(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Vec<SelectedInstructionKind> {
    if expansion.guard_kind == StateGuardKind::Always || !expansion.resolved_guard.is_valid() {
        return Vec::new();
    }

    select_runtime_branch_guard_conjuncts_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        expansion.statement_index,
        &input.runtime_branching_calls.expressions,
        expansion.resolved_guard,
        runtime_value_operands,
    )
}

fn select_runtime_branch_guard_conjuncts_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    guard: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Vec<SelectedInstructionKind> {
    let mut guards = Vec::new();
    collect_runtime_branch_guard_conjuncts_in_table(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        guard,
        runtime_value_operands,
        &mut guards,
    );
    guards
}

pub(super) fn select_runtime_dispatch_expression_guard_conjuncts_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    guard: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Vec<SelectedInstructionKind> {
    select_runtime_branch_guard_conjuncts_in_table(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        guard,
        runtime_value_operands,
    )
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct StaticGuardConjunctSummary {
    pub(super) has_true: bool,
    pub(super) has_false: bool,
}

pub(super) fn static_guard_conjunct_summary_in_table(
    input: &InstructionSelectionInput<'_>,
    expressions: &ExpressionTable,
    guard: ExpressionHandle,
) -> StaticGuardConjunctSummary {
    let mut summary = StaticGuardConjunctSummary::default();
    collect_static_guard_conjunct_summary_in_table(input, expressions, guard, &mut summary);
    summary
}

fn collect_static_guard_conjunct_summary_in_table(
    input: &InstructionSelectionInput<'_>,
    expressions: &ExpressionTable,
    guard: ExpressionHandle,
    summary: &mut StaticGuardConjunctSummary,
) {
    if !guard.is_valid() {
        return;
    }

    if let ExpressionNode::Binary(binary) = expressions.expression(guard)
        && binary.operator == BinaryOperator::And
    {
        collect_static_guard_conjunct_summary_in_table(input, expressions, binary.left, summary);
        collect_static_guard_conjunct_summary_in_table(input, expressions, binary.right, summary);
        return;
    }

    match static_guard_truth_in_table(input, expressions, guard) {
        Some(true) => summary.has_true = true,
        Some(false) => summary.has_false = true,
        None => {}
    }
}

fn collect_runtime_branch_guard_conjuncts_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    guard: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    guards: &mut Vec<SelectedInstructionKind>,
) {
    if !guard.is_valid() {
        return;
    }

    if let ExpressionNode::Binary(binary) = expressions.expression(guard)
        && binary.operator == BinaryOperator::And
    {
        collect_runtime_branch_guard_conjuncts_in_table(
            input,
            dispatch_index,
            source_key,
            statement_index,
            expressions,
            binary.left,
            runtime_value_operands,
            guards,
        );
        collect_runtime_branch_guard_conjuncts_in_table(
            input,
            dispatch_index,
            source_key,
            statement_index,
            expressions,
            binary.right,
            runtime_value_operands,
            guards,
        );
        return;
    }

    if let Some(guard) = select_runtime_dispatch_expression_guard_in_table(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        guard,
        runtime_value_operands,
    ) {
        guards.push(guard);
    }
}

fn static_guard_truth_in_table(
    input: &InstructionSelectionInput<'_>,
    expressions: &ExpressionTable,
    guard: ExpressionHandle,
) -> Option<bool> {
    let ExpressionNode::Binary(binary) = expressions.expression(guard) else {
        let value = enum_variant_value_in_table(&input.layouts, expressions, guard)
            .or_else(|| static_guard_value_in_table(expressions, guard))?;
        return Some(value != 0);
    };

    let operator = match binary.operator {
        BinaryOperator::Equal => StateGuardOperator::Equal,
        BinaryOperator::NotEqual => StateGuardOperator::NotEqual,
        _ => return None,
    };
    let left = enum_variant_value_in_table(&input.layouts, expressions, binary.left)
        .or_else(|| static_guard_value_in_table(expressions, binary.left))?;
    let right = enum_variant_value_in_table(&input.layouts, expressions, binary.right)
        .or_else(|| static_guard_value_in_table(expressions, binary.right))?;

    Some(match operator {
        StateGuardOperator::Equal => left == right,
        StateGuardOperator::NotEqual => left != right,
        _ => return None,
    })
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

    if let Some(guard) = runtime_text_equals_literal_guard(
        input,
        dispatch_index,
        source_key,
        &normalized_guard,
        runtime_value_operands,
    ) {
        return Some(guard);
    }

    if let Some(guard) = runtime_text_equals_place_guard(
        input,
        dispatch_index,
        source_key,
        &normalized_guard,
        runtime_value_operands,
    ) {
        return Some(guard);
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

    select_runtime_dispatch_expression_guard_in_table_once(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        guard,
        runtime_value_operands,
    )
    .or_else(|| {
        let (normalized_expressions, normalized_guard) =
            normalized_boolean_wrapped_guard_in_table(expressions, guard)?;
        select_runtime_dispatch_expression_guard_in_table_once(
            input,
            dispatch_index,
            source_key,
            statement_index,
            &normalized_expressions,
            normalized_guard,
            runtime_value_operands,
        )
    })
}

fn select_runtime_dispatch_expression_guard_in_table_once(
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

    if let Some(selected) = runtime_text_equals_literal_guard_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        guard,
        runtime_value_operands,
    ) {
        return Some(selected);
    }

    if let Some(selected) = runtime_text_equals_place_guard_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        guard,
        runtime_value_operands,
    ) {
        return Some(selected);
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

fn normalized_boolean_wrapped_guard_in_table(
    expressions: &ExpressionTable,
    guard: ExpressionHandle,
) -> Option<(ExpressionTable, ExpressionHandle)> {
    let ExpressionNode::Binary(binary) = expressions.expression(guard) else {
        return None;
    };

    let (inner, expected_true) =
        if let ExpressionNode::Boolean(value) = expressions.expression(binary.left) {
            (binary.right, *value)
        } else if let ExpressionNode::Boolean(value) = expressions.expression(binary.right) {
            (binary.left, *value)
        } else {
            return None;
        };

    let expected_true = match binary.operator {
        BinaryOperator::Equal => expected_true,
        BinaryOperator::NotEqual => !expected_true,
        _ => return None,
    };

    let mut normalized = expressions.clone();
    if expected_true {
        return Some((normalized, inner));
    }

    let ExpressionNode::Binary(inner_binary) = normalized.expression(inner) else {
        return None;
    };
    let inner_binary = *inner_binary;
    let inverted = match inner_binary.operator {
        BinaryOperator::Equal => BinaryOperator::NotEqual,
        BinaryOperator::NotEqual => BinaryOperator::Equal,
        BinaryOperator::Greater => BinaryOperator::LessOrEqual,
        BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
        BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
        BinaryOperator::LessOrEqual => BinaryOperator::Greater,
        _ => return None,
    };

    let normalized_guard = normalized.insert(ExpressionNode::Binary(TableBinaryExpression {
        left: inner_binary.left,
        operator: inverted,
        right: inner_binary.right,
    }));
    Some((normalized, normalized_guard))
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
    if !matches!(byte_size, 1 | 2 | 4 | 8) {
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
    if !guard.is_valid() {
        return None;
    }

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
    if !matches!(byte_size, 1 | 2 | 4 | 8) {
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

/// `String place ==/!= "literal"` in guard position, lowered as the inline
/// `TextEqualsLiteral` content compare (bool 0/1) against 1. This is the
/// selection for text guards whose String side has NO runtime text buffer --
/// plain machine/frame String fields and slice-element String fields
/// (`transition items[i].name == "expected"`). Before it existed these guards
/// selected NOTHING: the dispatch edge silently emitted no compare and was
/// always taken, so the guard evaluated TRUE even against an EMPTY field.
fn runtime_text_equals_literal_guard(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    guard: &TransitionGuard,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    let TransitionGuard::When(expression) = guard else {
        return None;
    };
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    runtime_text_equals_literal_guard_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        delegated_expression,
        runtime_value_operands,
    )
}

fn runtime_text_equals_literal_guard_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    expressions: &ExpressionTable,
    guard: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    let ExpressionNode::Binary(binary) = expressions.expression(guard) else {
        return None;
    };
    let operator = match binary.operator {
        BinaryOperator::Equal => StateGuardOperator::Equal,
        BinaryOperator::NotEqual => StateGuardOperator::NotEqual,
        _ => return None,
    };
    let (place_expression, literal) = if let Some(literal) =
        expressions.string_literal_value(binary.left)
    {
        (binary.right, literal)
    } else if let Some(literal) = expressions.string_literal_value(binary.right) {
        (binary.left, literal)
    } else {
        return None;
    };
    // An owned `[u8; N]` carrier is excluded from the String/slice `place_is_string`
    // gate by design (its `{len, bytes}` layout is not a `{ptr, len}` descriptor),
    // so resolve its storage place to a `Storage` ADDRESS operand directly and flag
    // carrier addressing for the encoder; otherwise take the descriptor place path.
    let (place, place_is_bounded_buffer) =
        if resolve_runtime_storage_place_is_bounded_byte_buffer_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            place_expression,
        ) {
            let storage = resolve_runtime_storage_place_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                place_expression,
            )?;
            let operand = runtime_value_operands.insert(RuntimeValueOperand::Storage {
                region: storage.region,
                byte_offset: storage.byte_offset,
                byte_size: storage.byte_count,
            });
            (operand, true)
        } else {
            let operand = resolve_runtime_text_descriptor_place_operand_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                place_expression,
                runtime_value_operands,
            )?;
            (operand, false)
        };
    let text_equals = runtime_value_operands.insert(RuntimeValueOperand::TextEqualsLiteral {
        place,
        literal: literal.to_string(),
        place_is_bounded_buffer,
    });
    let expected_true = runtime_value_operands.insert(RuntimeValueOperand::Immediate(1));
    // `==` holds when the content-equality bool is 1; `!=` when it is not.
    Some(SelectedInstructionKind::CompareRuntimeValues {
        left: text_equals,
        right: expected_true,
        byte_size: 1,
        operator,
    })
}

/// `String place ==/!= String place` in guard position (the String clause of
/// synthesized Equatable structural equality, or a direct field-vs-field
/// compare), lowered through the SAME `TextEquals` content-compare leaf the
/// value position uses (length compare + bounded byte loop), checked against
/// 1. Without this the guard fell through to the raw storage compare, whose
/// 16-byte descriptor load the encoder rejects loudly.
fn runtime_text_equals_place_guard(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    guard: &TransitionGuard,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    let TransitionGuard::When(expression) = guard else {
        return None;
    };
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    runtime_text_equals_place_guard_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        delegated_expression,
        runtime_value_operands,
    )
}

fn runtime_text_equals_place_guard_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    expressions: &ExpressionTable,
    guard: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    let ExpressionNode::Binary(binary) = expressions.expression(guard) else {
        return None;
    };
    let operator = match binary.operator {
        BinaryOperator::Equal => StateGuardOperator::Equal,
        BinaryOperator::NotEqual => StateGuardOperator::NotEqual,
        _ => return None,
    };
    // Resolve the POSITIVE leaf (`Equal`) regardless of the guard's own
    // polarity: `!=` is expressed by the compare operator below, never by the
    // resolver's negated-leaf wrapping (which would double-negate).
    let text_equals = resolve_runtime_text_equals_operand_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        BinaryOperator::Equal,
        binary.left,
        binary.right,
        runtime_value_operands,
    )?;
    let expected_true = runtime_value_operands.insert(RuntimeValueOperand::Immediate(1));
    // `==` holds when the content-equality bool is 1; `!=` when it is not.
    Some(SelectedInstructionKind::CompareRuntimeValues {
        left: text_equals,
        right: expected_true,
        byte_size: 1,
        operator,
    })
}

/// The String side of a text-vs-literal guard as a PLACE operand naming its
/// 16-byte `{ptr, len}` text descriptor. The indirect resolutions
/// (frame-indexed, frame-base-indexed, frame-fixed-indexed, pointee) are all
/// tried BEFORE the static storage fallback: an indexed or pointer-rooted
/// expression must never fall back to a static storage resolution of the
/// descriptor slot itself (the descriptor-as-value trap -- for a pointee
/// place the storage resolver "sees through" the reference and hands back
/// the POINTER slot's raw bytes as if they were the descriptor).
/// Only String-typed places qualify -- this operand is a CONTENT compare and
/// must never see a scalar that happens to be 16 bytes.
fn resolve_runtime_text_descriptor_place_operand_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<RuntimeValueOperandHandle> {
    // The leaf-descriptor resolver covers name paths (`self.name`, pointee
    // `room.name`, and constant member-index paths); the indexed resolver
    // covers indexed element fields (`items[i].name`, fixed `items[0].name`,
    // and inline-array elements alike), whose Index node the name-path walk
    // cannot see through.
    // A `&[u8] in Utf8` text view shares the IDENTICAL 16-byte `{ptr, len}`
    // descriptor with `String`, so it is content-comparable through the SAME
    // text leaves. Recognize such a slice-descriptor place (not
    // `PrimitiveType::String`) too -- otherwise a `text == "literal"` guard over
    // a `&[u8] in Utf8` value falls through to the raw scalar compare, which
    // compares the descriptor's POINTER words and silently mismatches the
    // interpreter's content equality.
    let place_is_string = matches!(
        resolve_runtime_storage_primitive_type_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            expression,
        ),
        Some(PrimitiveType::String)
    ) || matches!(
        resolve_runtime_frame_indexed_primitive_type_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            expression,
        ),
        Some(PrimitiveType::String)
    ) || resolve_runtime_storage_place_is_fat_slice_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) || resolve_runtime_frame_indexed_is_fat_slice_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    );
    if !place_is_string {
        return None;
    }
    let string_descriptor_size = input.runtime_abi.string_descriptor_size();

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) && indexed_target.byte_count == string_descriptor_size
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

    // Inline frame fixed arrays (`let rooms: [Room; N]` indexed at runtime):
    // the element address is frame base + base offset + index*element.
    if let Some(indexed_target) = resolve_runtime_frame_base_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) && indexed_target.byte_count == string_descriptor_size
    {
        return Some(
            runtime_value_operands.insert(RuntimeValueOperand::FrameBaseIndexed {
                base_byte_offset: indexed_target.base_byte_offset,
                index_offset: indexed_target.index_offset,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                byte_size: indexed_target.byte_count,
            }),
        );
    }

    // Slice elements at a CONSTANT index (`items[0].name`): descriptor-based
    // access with the scaled index folded into a constant offset.
    if let Some(indexed_target) = resolve_runtime_frame_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) && indexed_target.byte_count == string_descriptor_size
    {
        return Some(
            runtime_value_operands.insert(RuntimeValueOperand::FrameFixedIndexed {
                descriptor_offset: indexed_target.descriptor_offset,
                element_index: indexed_target.element_index,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                byte_size: indexed_target.byte_count,
            }),
        );
    }

    // Pointer-rooted fields (`room.name` where `room` is a `&mut Room` frame
    // slot): deref the pointer slot, descriptor at pointee + field offset.
    // MUST precede the storage fallback, which would otherwise resolve the
    // same path to the pointer slot's own bytes (always-unequal compare).
    if let Some(pointee_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) && pointee_target.pointee_byte_size == string_descriptor_size
    {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Pointee {
            pointer_byte_offset: pointee_target.pointer_byte_offset,
            field_byte_offset: pointee_target.field_byte_offset,
            byte_size: pointee_target.pointee_byte_size,
        }));
    }

    if let Some(place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) && place.byte_count == string_descriptor_size
    {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Storage {
            region: place.region,
            byte_offset: place.byte_offset,
            byte_size: place.byte_count,
        }));
    }

    None
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
    let string_descriptor_size = input.runtime_abi.string_descriptor_size();

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
    let string_descriptor_size = input.runtime_abi.string_descriptor_size();

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
            is_float: false,
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
            is_float: false,
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
    // A case-name equality (the lowered form of `in`) compares the TAG only;
    // the place operand must not read payload bytes.
    clamp_runtime_case_comparison_operands(
        &input.layouts,
        binary.operator,
        &binary.left,
        &binary.right,
        left,
        right,
        runtime_value_operands,
    );
    let byte_size = runtime_value_compare_byte_size(runtime_value_operands, left, right);
    if !matches!(byte_size, 1 | 2 | 4 | 8) {
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
    let mut operator = runtime_compare_operator(binary.operator)?;
    // An ordered comparison on an unsigned operand (e.g. a `u32` value-transition
    // arm guard `x <= 2`) must branch unsigned; `runtime_compare_operator` only
    // produces the signed form. The dispatch-edge path post-processes this; the
    // leaf value-transition path (this function) did not, so a u32 > INT_MAX
    // compared as a negative number and took the wrong arm.
    if comparison_operands_unsigned_in_table(
        input,
        dispatch_index,
        source_key,
        operator,
        expressions,
        binary.left,
        binary.right,
    ) {
        operator = unsigned_comparison_operator(operator);
    }
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
    // A case-name equality (the lowered form of `in`) compares the TAG only;
    // the place operand must not read payload bytes.
    clamp_runtime_case_comparison_operands_in_table(
        &input.layouts,
        expressions,
        binary.operator,
        binary.left,
        binary.right,
        left,
        right,
        runtime_value_operands,
    );
    let byte_size = runtime_value_compare_byte_size(runtime_value_operands, left, right);
    if !matches!(byte_size, 1 | 2 | 4 | 8) {
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
        // Transparent through an inlined-argument `mut <literal>` wrapper.
        Expression::Mutable(inner) => static_guard_value(inner),
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
        ExpressionNode::Mutable(inner) => static_guard_value_in_table(expressions, *inner),
        // A field read on a STRUCT LITERAL receiver projects to the literal
        // field value: a call argument `Card { power: 3 }` substituted into
        // an inline arm guard `card.power > 0` folds to `3 > 0`.
        ExpressionNode::Member(member) => {
            let mut receiver = member.receiver;
            while let ExpressionNode::Mutable(inner) = expressions.expression(receiver) {
                receiver = *inner;
            }
            let ExpressionNode::StructLiteral(struct_literal) = expressions.expression(receiver)
            else {
                return None;
            };
            expressions
                .struct_fields(struct_literal.fields)
                .iter()
                .find(|field| field.name == member.member)
                .and_then(|field| static_guard_value_in_table(expressions, field.value))
        }
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_runtime_value_operand(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    _source_machine: &str,
    _source_state: &str,
    expression: &Expression,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<RuntimeValueOperandHandle> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_value_operand_in_table(
        input,
        dispatch_index,
        source_key,
        statement_index,
        &delegated_expressions,
        delegated_expression,
        runtime_value_operands,
    )
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
            // Guard comparison operands; float comparisons lower via ucomisd
            // elsewhere, so the integer value-operand path stays as-is here.
            is_float: false,
            // Integer arm derives its own width; default 8 matches prior behavior.
            byte_width: 8,
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

    if let Some(indexed_target) = resolve_runtime_frame_base_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(
            runtime_value_operands.insert(RuntimeValueOperand::FrameBaseIndexed {
                base_byte_offset: indexed_target.base_byte_offset,
                index_offset: indexed_target.index_offset,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                byte_size: indexed_target.byte_count,
            }),
        );
    }

    if let Some(indexed_target) = resolve_runtime_frame_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(
            runtime_value_operands.insert(RuntimeValueOperand::FrameFixedIndexed {
                descriptor_offset: indexed_target.descriptor_offset,
                element_index: indexed_target.element_index,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                byte_size: indexed_target.byte_count,
            }),
        );
    }

    if let Some(place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Storage {
            region: place.region,
            byte_offset: place.byte_offset,
            byte_size: place.byte_count,
        }));
    }

    // `.len` of (an alias of) a FIXED ARRAY has no storage place -- the length
    // is a layout constant -- so it folds to an immediate. This covers an
    // inline-leaf arm guard `s.len > 0` whose `s` substitutes to a caller
    // local `let s = self.arr.as_slice()` that storage elided (no frame slot,
    // hence no slice descriptor with a runtime len to read).
    if let Some(length) = static_fixed_array_len_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Immediate(length)));
    }

    // An ELIDED constant local (`let flag: bool = true` used only as a call
    // argument) likewise has no storage place; fold it to its initializer.
    if let Some(value) = static_elided_local_value_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Immediate(value)));
    }

    None
}

fn runtime_value_operand_byte_size(
    runtime_value_operands: &Arena<RuntimeValueOperand>,
    operand: RuntimeValueOperandHandle,
) -> usize {
    match runtime_value_operands.get(operand) {
        RuntimeValueOperand::Immediate(_) => 8,
        RuntimeValueOperand::Storage { byte_size, .. } => *byte_size,
        RuntimeValueOperand::Pointee { byte_size, .. } => *byte_size,
        RuntimeValueOperand::FrameIndexed { byte_size, .. }
        | RuntimeValueOperand::FrameBaseIndexed { byte_size, .. } => *byte_size,
        RuntimeValueOperand::FrameFixedIndexed { byte_size, .. } => *byte_size,
        RuntimeValueOperand::Binary { left, right, .. } => {
            runtime_value_operand_byte_size(runtime_value_operands, *left).max(
                runtime_value_operand_byte_size(runtime_value_operands, *right),
            )
        }
        RuntimeValueOperand::Convert {
            target_byte_size, ..
        } => *target_byte_size,
        // Text content equality evaluates to a bool.
        RuntimeValueOperand::TextEquals { .. } | RuntimeValueOperand::TextEqualsLiteral { .. } => 1,
    }
}

fn runtime_value_compare_byte_size(
    runtime_value_operands: &Arena<RuntimeValueOperand>,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
) -> usize {
    match (
        runtime_value_operands.get(left),
        runtime_value_operands.get(right),
    ) {
        (RuntimeValueOperand::Immediate(_), RuntimeValueOperand::Immediate(_)) => 8,
        (RuntimeValueOperand::Immediate(_), _) => {
            runtime_value_operand_byte_size(runtime_value_operands, right)
        }
        (_, RuntimeValueOperand::Immediate(_)) => {
            runtime_value_operand_byte_size(runtime_value_operands, left)
        }
        _ => runtime_value_operand_byte_size(runtime_value_operands, left).max(
            runtime_value_operand_byte_size(runtime_value_operands, right),
        ),
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

/// Swap an ordered comparison operator to its unsigned form. Equal/NotEqual and
/// already-unsigned/arithmetic operators are returned unchanged. Mirrors the
/// dispatch-edge path's `unsigned_comparison_operator`.
fn unsigned_comparison_operator(operator: StateGuardOperator) -> StateGuardOperator {
    match operator {
        StateGuardOperator::Greater => StateGuardOperator::GreaterUnsigned,
        StateGuardOperator::GreaterOrEqual => StateGuardOperator::GreaterOrEqualUnsigned,
        StateGuardOperator::Less => StateGuardOperator::LessUnsigned,
        StateGuardOperator::LessOrEqual => StateGuardOperator::LessOrEqualUnsigned,
        other => other,
    }
}

/// True when an ordered comparison's operands are an unsigned integer type, so the
/// branch must use unsigned conditions. The operand type is read from whichever
/// side resolves to a storage place (the literal side does not). Only the four
/// ordered operators are affected; Equal/NotEqual are signedness-agnostic and pass
/// through unchanged via `unsigned_comparison_operator`. Mirrors the dispatch-edge
/// path's `guard_comparison_operands_unsigned`, which the leaf value-transition
/// guard path does not run through.
fn comparison_operands_unsigned_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    operator: StateGuardOperator,
    expressions: &ExpressionTable,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    if !matches!(
        operator,
        StateGuardOperator::Greater
            | StateGuardOperator::GreaterOrEqual
            | StateGuardOperator::Less
            | StateGuardOperator::LessOrEqual
    ) {
        return false;
    }
    let signed =
        resolve_runtime_storage_is_signed_in_table(input, dispatch_index, source_key, expressions, left)
            .or_else(|| {
                resolve_runtime_storage_is_signed_in_table(
                    input,
                    dispatch_index,
                    source_key,
                    expressions,
                    right,
                )
            });
    signed == Some(false)
}

fn runtime_arithmetic_operator(operator: BinaryOperator) -> Option<StateGuardOperator> {
    match operator {
        BinaryOperator::Add => Some(StateGuardOperator::Add),
        BinaryOperator::And => Some(StateGuardOperator::And),
        BinaryOperator::Modulo => Some(StateGuardOperator::Modulo),
        BinaryOperator::Multiply => Some(StateGuardOperator::Multiply),
        BinaryOperator::Subtract => Some(StateGuardOperator::Subtract),
        BinaryOperator::Divide
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
    let buffer = input
        .runtime_text
        .buffers
        .iter()
        .find_map(|(_, buffer)| {
            (buffer.source_key == source_key
                && runtime_text_buffer_matches_tree_expression(input, buffer, expression))
            .then_some(buffer)
        })
        .or_else(|| {
            input.runtime_text.buffers.iter().find_map(|(_, buffer)| {
                runtime_text_buffer_matches_tree_expression(input, buffer, expression)
                    .then_some(buffer)
            })
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
    // The LAST matching buffer in this state, not the first: a place built up by
    // several chained text writes (`x = x + a; x = x + b;`) gets one buffer per
    // write, and a guard reads the place after all statements have run, so the
    // live value is in the most recently materialized buffer.
    let buffer = input
        .runtime_text
        .buffers
        .iter()
        .filter(|(_, buffer)| {
            buffer.source_key == source_key
                && runtime_text_buffer_matches_table_expression(
                    input,
                    buffer,
                    expressions,
                    expression,
                )
        })
        .map(|(_, buffer)| buffer)
        .last()
        .or_else(|| {
            input
                .runtime_text
                .buffers
                .iter()
                .filter(|(_, buffer)| {
                    runtime_text_buffer_matches_table_expression(
                        input, buffer, expressions, expression,
                    )
                })
                .map(|(_, buffer)| buffer)
                .last()
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

fn runtime_text_buffer_matches_tree_expression(
    input: &InstructionSelectionInput<'_>,
    buffer: &omega_runtime_text::RuntimeTextBuffer,
    expression: &Expression,
) -> bool {
    expression_place_eq_table_tree(
        &input.runtime_text.expressions,
        buffer.text_place,
        expression,
    ) || expression_place_eq_table_tree(&input.runtime_text.expressions, buffer.target, expression)
}

fn runtime_text_buffer_matches_table_expression(
    input: &InstructionSelectionInput<'_>,
    buffer: &omega_runtime_text::RuntimeTextBuffer,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    expression_place_eq_across_tables(
        &input.runtime_text.expressions,
        buffer.text_place,
        expressions,
        expression,
    ) || expression_place_eq_across_tables(
        &input.runtime_text.expressions,
        buffer.target,
        expressions,
        expression,
    )
}

fn source_machine_name(
    input: &InstructionSelectionInput<'_>,
    key: omega_control_flow::StateKey,
) -> Identifier {
    input.control_flow.state_machine_name_by_key_cloned(key)
}

fn source_state_name(
    input: &InstructionSelectionInput<'_>,
    key: omega_control_flow::StateKey,
) -> Identifier {
    input.control_flow.state_name_by_key_cloned(key)
}
