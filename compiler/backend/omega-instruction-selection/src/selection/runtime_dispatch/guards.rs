use crate::InstructionSelectionInput;
use omega_runtime_branching::{RuntimeLeafBranchExpansion, RuntimeStraightLineBranchExpansion};
use omega_state_guards::StateGuardOperator;
use omega_typed_trees::expression::Expression;
use omega_typed_trees::name::ProgramName;

use omega_runtime_text::places::expression_name_with_suffix_eq_tree;
use super::super::storage_places::{enum_variant_value, resolve_runtime_storage_place};
use omega_target_operations::{SelectedInstructionKind, TargetDataObjectHandle};

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
        runtime_storage_guard(
            input,
            expansion.dispatch_index,
            expansion.source_key,
            &expansion.resolved_guard,
        )
    })
}

fn runtime_text_literal_guard(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    guard: &omega_typed_trees::statement::TransitionGuard,
) -> Option<(TargetDataObjectHandle, String)> {
    let omega_typed_trees::statement::TransitionGuard::When(Expression::Binary(binary)) = guard
    else {
        return None;
    };
    if binary.operator != omega_typed_trees::expression::BinaryOperator::Equal {
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
    guard: &omega_typed_trees::statement::TransitionGuard,
) -> Option<SelectedInstructionKind> {
    let omega_typed_trees::statement::TransitionGuard::When(Expression::Binary(binary)) = guard
    else {
        return None;
    };
    if binary.operator != omega_typed_trees::expression::BinaryOperator::Equal {
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
    guard: &omega_typed_trees::statement::TransitionGuard,
) -> Option<SelectedInstructionKind> {
    let omega_typed_trees::statement::TransitionGuard::When(Expression::Binary(binary)) = guard
    else {
        return None;
    };
    let operator = match binary.operator {
        omega_typed_trees::expression::BinaryOperator::Equal => StateGuardOperator::Equal,
        omega_typed_trees::expression::BinaryOperator::NotEqual => StateGuardOperator::NotEqual,
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
