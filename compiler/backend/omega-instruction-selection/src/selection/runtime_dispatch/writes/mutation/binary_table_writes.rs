use crate::InstructionSelectionInput;
use crate::selection::storage_places::{
    RuntimeStoragePlace, resolve_runtime_frame_indexed_target_in_table,
    resolve_runtime_pointee_fixed_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_place_in_table,
};
use omega_abstract_operations::{
    RuntimeStorageRegion, RuntimeValueOperand, SelectedInstructionKind,
};
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_control_flow::StateKey;
use omega_core::arena::Arena;

use super::super::static_values::{
    RuntimeStaticValues, invalidate_runtime_static_value_in_table,
};
use super::operators::{builtin_runtime_call_operator_in_table, runtime_binary_operator};
use super::value_operands::resolve_runtime_value_operand_in_table;

pub(in crate::selection::runtime_dispatch::writes) fn select_runtime_binary_mutation_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
    static_values: &mut RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    let target_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    );

    select_runtime_targeted_binary_mutation_write_in_table(
        input,
        dispatch_index,
        target_source_key,
        value_source_key,
        statement_index,
        expressions,
        target,
        target_place,
        value,
        static_values,
        runtime_value_operands,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_targeted_binary_mutation_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    target_place: Option<RuntimeStoragePlace>,
    value: ExpressionHandle,
    static_values: &mut RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    let (operator, left_expression, right_expression) = match expressions.expression(value) {
        ExpressionNode::Binary(binary) => (
            runtime_binary_operator(binary.operator)?,
            binary.left,
            binary.right,
        ),
        ExpressionNode::Call(call) => {
            let operator = builtin_runtime_call_operator_in_table(input, call)?;
            let left = expressions.expression_handle_at_offset(call.arguments, 0);
            let right = expressions.expression_handle_at_offset(call.arguments, 1);
            (operator, left, right)
        }
        _ => return None,
    };

    let left = resolve_runtime_value_operand_in_table(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        left_expression,
        static_values,
        runtime_value_operands,
    )?;
    let right = resolve_runtime_value_operand_in_table(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        right_expression,
        static_values,
        runtime_value_operands,
    )?;

    // The operands above were resolved against the pre-write static state (so a
    // first read-modify-write still folds its own operands). The write itself
    // produces a value we do not track as a constant, so forget any recorded
    // constant for the target: a later read of the same place in this state must
    // come from live storage, not the stale entry-value fold. Without this, a
    // chain like `v = v + 5; v = v - 3;` would read the entry value of `v` for
    // every statement and silently compute the wrong result.
    invalidate_runtime_static_value_in_table(static_values, expressions, target);

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) {
        return Some(SelectedInstructionKind::WriteRuntimeFrameIndexedBinary {
            descriptor_offset: indexed_target.descriptor_offset,
            index_offset: indexed_target.index_offset,
            element_byte_size: indexed_target.element_byte_size,
            field_byte_offset: indexed_target.field_byte_offset,
            byte_size: indexed_target.byte_count,
            left,
            operator,
            right,
        });
    }

    if let Some(pointer_target) = resolve_runtime_pointee_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) {
        return Some(SelectedInstructionKind::WriteRuntimePointeeBinary {
            pointer_byte_offset: pointer_target.pointer_byte_offset,
            field_byte_offset: pointer_target.field_byte_offset,
            byte_size: pointer_target.pointee_byte_size,
            left,
            operator,
            right,
        });
    }

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) {
        return Some(SelectedInstructionKind::WriteRuntimePointeeBinary {
            pointer_byte_offset: pointer_target.pointer_byte_offset,
            field_byte_offset: pointer_target.field_byte_offset,
            byte_size: pointer_target.pointee_byte_size,
            left,
            operator,
            right,
        });
    }

    let target_place = target_place?;
    Some(SelectedInstructionKind::WriteRuntimeStorageBinary {
        target_region: target_place.region,
        target_offset: target_place.byte_offset,
        byte_size: target_place.byte_count,
        left,
        operator,
        right,
    })
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection::runtime_dispatch::writes) fn select_runtime_storage_binary_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    byte_size: usize,
    value: ExpressionHandle,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    let (operator, left_expression, right_expression) = match expressions.expression(value) {
        ExpressionNode::Binary(binary) => (
            runtime_binary_operator(binary.operator)?,
            binary.left,
            binary.right,
        ),
        ExpressionNode::Call(call) => {
            let operator = builtin_runtime_call_operator_in_table(input, call)?;
            let left = expressions.expression_handle_at_offset(call.arguments, 0);
            let right = expressions.expression_handle_at_offset(call.arguments, 1);
            (operator, left, right)
        }
        _ => return None,
    };

    let left = resolve_runtime_value_operand_in_table(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        left_expression,
        static_values,
        runtime_value_operands,
    )?;
    let right = resolve_runtime_value_operand_in_table(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        right_expression,
        static_values,
        runtime_value_operands,
    )?;

    Some(SelectedInstructionKind::WriteRuntimeStorageBinary {
        target_region,
        target_offset,
        byte_size,
        left,
        operator,
        right,
    })
}
