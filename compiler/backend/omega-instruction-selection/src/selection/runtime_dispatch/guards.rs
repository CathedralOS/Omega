use crate::InstructionSelectionInput;
use omega_checked_trees::expression::{BinaryOperator, Expression};
use omega_checked_trees::name::ProgramName;
use omega_checked_trees::statement::TransitionGuard;
use omega_runtime_branching::{RuntimeLeafBranchExpansion, RuntimeStraightLineBranchExpansion};
use omega_state_guards::StateGuardOperator;

use omega_runtime_text::places::expression_name_with_suffix_eq_tree;
use super::super::storage_places::{
    enum_variant_value, resolve_runtime_frame_indexed_target, resolve_runtime_storage_place,
    resolve_runtime_pointee_slot_offset, resolve_runtime_transition_guard_call_result_place,
};
use omega_target_operations::{RuntimeValueOperand, SelectedInstructionKind, TargetDataObjectHandle};

pub(super) fn select_runtime_leaf_branch_guard(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
) -> Option<SelectedInstructionKind> {
    if let Some((buffer, literal)) = runtime_text_literal_guard(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        &expansion.resolved_guard,
    ) {
        return Some(SelectedInstructionKind::CompareRuntimeTextLiteral { buffer, literal });
    }

    runtime_text_storage_guard(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        &expansion.resolved_guard,
    )
    .or_else(|| {
        runtime_value_guard(
            input,
            expansion.dispatch_index,
            expansion.source_key,
            expansion.statement_index,
            &expansion.resolved_guard,
        )
    })
    .or_else(|| {
        runtime_storage_guard(
            input,
            expansion.dispatch_index,
            expansion.source_key,
            &expansion.resolved_guard,
        )
    })
}

pub(super) fn select_runtime_straight_line_branch_guard(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
) -> Option<SelectedInstructionKind> {
    if let Some((buffer, literal)) = runtime_text_literal_guard(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        &expansion.resolved_guard,
    ) {
        return Some(SelectedInstructionKind::CompareRuntimeTextLiteral { buffer, literal });
    }

    runtime_text_storage_guard(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        &expansion.resolved_guard,
    )
    .or_else(|| {
        runtime_value_guard(
            input,
            expansion.dispatch_index,
            expansion.source_key,
            expansion.statement_index,
            &expansion.resolved_guard,
        )
    })
    .or_else(|| {
        runtime_storage_guard(
            input,
            expansion.dispatch_index,
            expansion.source_key,
            &expansion.resolved_guard,
        )
    })
}

pub(super) fn select_runtime_dispatch_expression_guard(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    guard: &TransitionGuard,
) -> Option<SelectedInstructionKind> {
    if let Some((buffer, literal)) =
        runtime_text_literal_guard(input, dispatch_index, source_key, guard)
    {
        return Some(SelectedInstructionKind::CompareRuntimeTextLiteral { buffer, literal });
    }

    runtime_text_storage_guard(input, dispatch_index, source_key, guard)
        .or_else(|| runtime_value_guard(input, dispatch_index, source_key, statement_index, guard))
        .or_else(|| runtime_storage_guard(input, dispatch_index, source_key, guard))
}

fn runtime_text_literal_guard(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    guard: &TransitionGuard,
) -> Option<(TargetDataObjectHandle, String)> {
    let TransitionGuard::When(Expression::Binary(binary)) = guard
    else {
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

    let (buffer, _) =
        runtime_text_input_buffer_data_for_text_place_in_state(input, dispatch_index, source_key, text_place)?;
    Some((buffer, literal.clone()))
}

fn runtime_text_storage_guard(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    guard: &TransitionGuard,
) -> Option<SelectedInstructionKind> {
    let TransitionGuard::When(Expression::Binary(binary)) = guard
    else {
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
    let left_buffer =
        runtime_text_input_buffer_data_for_text_place_in_state(input, dispatch_index, source_key, &binary.left);
    let right_buffer =
        runtime_text_input_buffer_data_for_text_place_in_state(input, dispatch_index, source_key, &binary.right);
    let string_descriptor_size = input.target.pointer_size * 2;

    if let (Some(source_place), Some((buffer, _))) = (left_place.clone(), right_buffer)
        && source_place.byte_count == string_descriptor_size
    {
        return Some(SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer,
            source_region: source_place.region,
            source_offset: source_place.byte_offset,
            operator,
        });
    }

    if let (Some((buffer, _)), Some(source_place)) = (left_buffer, right_place)
        && source_place.byte_count == string_descriptor_size
    {
        return Some(SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer,
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
    let TransitionGuard::When(Expression::Binary(binary)) = guard
    else {
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
        && let Some(expected_value) =
            enum_variant_value(&input.layouts, &binary.right).or_else(|| static_guard_value(&binary.right))
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
            enum_variant_value(&input.layouts, &binary.left).or_else(|| static_guard_value(&binary.left))
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
) -> Option<SelectedInstructionKind> {
    let TransitionGuard::When(Expression::Binary(binary)) = guard
    else {
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
    )?;
        let right = resolve_runtime_value_operand(
            input,
            dispatch_index,
            source_key,
            statement_index,
            &source_machine,
            &source_state,
            &binary.right,
    )?;
    let byte_size = runtime_value_operand_byte_size(&left)
        .max(runtime_value_operand_byte_size(&right));
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

fn resolve_runtime_value_operand(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    source_machine: &str,
    source_state: &str,
    expression: &Expression,
) -> Option<RuntimeValueOperand> {
    if let Some(value) = enum_variant_value(&input.layouts, expression)
        .or_else(|| static_guard_value(expression))
    {
        return Some(RuntimeValueOperand::Immediate(value));
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
        )?;
        let right = resolve_runtime_value_operand(
            input,
            dispatch_index,
            source_key,
            statement_index,
            source_machine,
            source_state,
            &binary.right,
        )?;
        return Some(RuntimeValueOperand::Binary {
            left: Box::new(left),
            operator,
            right: Box::new(right),
        });
    }

    if let Expression::Call(call) = expression
        && let Some(place) = resolve_runtime_transition_guard_call_result_place(
            input,
            dispatch_index,
            source_key,
            statement_index,
            call,
        )
    {
        return Some(RuntimeValueOperand::Storage {
            region: place.region,
            byte_offset: place.byte_offset,
            byte_size: place.byte_count,
        });
    }

    if let Some(pointer_target) =
        resolve_runtime_pointee_slot_offset(input, dispatch_index, source_key, expression)
    {
        return Some(RuntimeValueOperand::Pointee {
            pointer_byte_offset: pointer_target.pointer_byte_offset,
            field_byte_offset: pointer_target.field_byte_offset,
            byte_size: pointer_target.pointee_byte_size,
        });
    }

    if let Some(indexed_target) =
        resolve_runtime_frame_indexed_target(input, dispatch_index, source_key, expression)
    {
        return Some(RuntimeValueOperand::FrameIndexed {
            descriptor_offset: indexed_target.descriptor_offset,
            index_offset: indexed_target.index_offset,
            element_byte_size: indexed_target.element_byte_size,
            field_byte_offset: indexed_target.field_byte_offset,
            byte_size: indexed_target.byte_count,
        });
    }

    let place = resolve_runtime_storage_place(
        input,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        expression,
    )?;
    Some(RuntimeValueOperand::Storage {
        region: place.region,
        byte_offset: place.byte_offset,
        byte_size: place.byte_count,
    })
}

fn runtime_value_operand_byte_size(operand: &RuntimeValueOperand) -> usize {
    match operand {
        RuntimeValueOperand::Immediate(_) => 8,
        RuntimeValueOperand::Storage { byte_size, .. } => *byte_size,
        RuntimeValueOperand::Pointee { byte_size, .. } => *byte_size,
        RuntimeValueOperand::FrameIndexed { byte_size, .. } => *byte_size,
        RuntimeValueOperand::Binary { left, right, .. } => {
            runtime_value_operand_byte_size(left).max(runtime_value_operand_byte_size(right))
        }
    }
}

fn runtime_compare_operator(
    operator: BinaryOperator,
) -> Option<StateGuardOperator> {
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

fn runtime_arithmetic_operator(
    operator: BinaryOperator,
) -> Option<StateGuardOperator> {
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
) -> Option<(TargetDataObjectHandle, usize)> {
    let buffer = input.runtime_text.buffers.iter().find_map(|(_, buffer)| {
        (buffer.source_key == source_key
            && expression_name_with_suffix_eq_tree(
                &input.runtime_text.expressions,
                buffer.target,
                expression,
                "text",
            ))
        .then_some(buffer)
    })?;

    let (data, _) = input.data.objects.iter().find(|(_, data_object)| {
        data_object.source_key == buffer.source_key
            && data_object.source_statement == buffer.statement_index
    })?;

    let _ = dispatch_index;

    Some((data, buffer.byte_capacity))
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
