use crate::plan::NativePlan;
use crate::runtime_dispatch::branching::RuntimeLeafBranchExpansion;
use crate::state_guards::StateGuardOperator;
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

use super::super::host_operations::runtime_text_input_buffer_data_for_text_place;
use super::super::storage_places::{enum_variant_value, resolve_runtime_storage_place};
use omega_target_program::{NativeDataObjectHandle, SelectedInstructionKind};

pub(super) fn select_runtime_leaf_branch_guard(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
) -> Option<SelectedInstructionKind> {
    if let Some((buffer, literal)) = runtime_text_literal_guard(native_plan, expansion) {
        return Some(SelectedInstructionKind::CompareRuntimeTextLiteral { buffer, literal });
    }

    runtime_text_storage_guard(native_plan, expansion)
        .or_else(|| runtime_storage_guard(native_plan, expansion))
}

fn runtime_text_literal_guard(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
) -> Option<(NativeDataObjectHandle, String)> {
    let omega_typed_program::statement::TransitionGuard::When(Expression::Binary(binary)) =
        &expansion.resolved_guard
    else {
        return None;
    };
    if binary.operator != omega_typed_program::expression::BinaryOperator::Equal {
        return None;
    }

    let (text_place, literal) = match (&binary.left, &binary.right) {
        (text_place, Expression::String(literal)) => (text_place, literal),
        (Expression::String(literal), text_place) => (text_place, literal),
        _ => return None,
    };

    let (buffer, _) = runtime_text_input_buffer_data_for_text_place(native_plan, text_place)?;
    Some((buffer, literal.clone()))
}

fn runtime_text_storage_guard(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
) -> Option<SelectedInstructionKind> {
    let omega_typed_program::statement::TransitionGuard::When(Expression::Binary(binary)) =
        &expansion.resolved_guard
    else {
        return None;
    };
    if binary.operator != omega_typed_program::expression::BinaryOperator::Equal {
        return None;
    }
    let operator = StateGuardOperator::Equal;
    let source_machine = source_machine_name(native_plan, expansion.source_key);
    let source_state = source_state_name(native_plan, expansion.source_key);

    let left_place = resolve_runtime_storage_place(
        native_plan,
        expansion.dispatch_index,
        expansion.source_key,
        &source_machine,
        &source_state,
        &binary.left,
    );
    let right_place = resolve_runtime_storage_place(
        native_plan,
        expansion.dispatch_index,
        expansion.source_key,
        &source_machine,
        &source_state,
        &binary.right,
    );
    let left_buffer = runtime_text_input_buffer_data_for_text_place(native_plan, &binary.left);
    let right_buffer = runtime_text_input_buffer_data_for_text_place(native_plan, &binary.right);
    let string_descriptor_size = native_plan.target.pointer_size * 2;

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
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
) -> Option<SelectedInstructionKind> {
    let omega_typed_program::statement::TransitionGuard::When(Expression::Binary(binary)) =
        &expansion.resolved_guard
    else {
        return None;
    };
    let operator = match binary.operator {
        omega_typed_program::expression::BinaryOperator::Equal => StateGuardOperator::Equal,
        omega_typed_program::expression::BinaryOperator::NotEqual => StateGuardOperator::NotEqual,
        _ => return None,
    };
    let source_machine = source_machine_name(native_plan, expansion.source_key);
    let source_state = source_state_name(native_plan, expansion.source_key);
    let left = resolve_runtime_storage_place(
        native_plan,
        expansion.dispatch_index,
        expansion.source_key,
        &source_machine,
        &source_state,
        &binary.left,
    );
    let right = resolve_runtime_storage_place(
        native_plan,
        expansion.dispatch_index,
        expansion.source_key,
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
        && let Some(expected_value) = enum_variant_value(&native_plan.layouts, &binary.right)
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
        && let Some(expected_value) = enum_variant_value(&native_plan.layouts, &binary.left)
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

fn source_machine_name(native_plan: &NativePlan, key: omega_control_flow::StateKey) -> ProgramName {
    native_plan
        .control_flow
        .state_machine_name_by_key_cloned(key)
}

fn source_state_name(native_plan: &NativePlan, key: omega_control_flow::StateKey) -> ProgramName {
    native_plan.control_flow.state_name_by_key_cloned(key)
}
