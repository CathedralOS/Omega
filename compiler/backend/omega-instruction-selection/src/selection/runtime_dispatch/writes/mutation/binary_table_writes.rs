use crate::InstructionSelectionInput;
use crate::selection::storage_places::{
    RuntimeStoragePlace, resolve_runtime_frame_indexed_target_in_table,
    resolve_runtime_pointee_fixed_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_is_signed_in_table,
    resolve_runtime_storage_place_in_table, resolve_runtime_storage_primitive_type_in_table,
};
use omega_checked_trees::types::PrimitiveType;
use omega_abstract_operations::{
    RuntimeStorageRegion, RuntimeValueOperand, SelectedInstructionKind, StateGuardOperator,
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

    // Division, modulo, right shift, min/max, and comparisons differ by
    // signedness; pick the unsigned encoding when the operands are unsigned.
    // Comparisons read their signedness from an operand (the target is bool);
    // the others share the target's type.
    let operator = signedness_adjusted_operator(
        input,
        dispatch_index,
        target_source_key,
        value_source_key,
        expressions,
        target,
        left_expression,
        right_expression,
        operator,
    );

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

    // A float target performs the operation on the SSE unit. First cut: f64 only,
    // and only to a machine-owned/frame storage place (the indexed/pointee binary
    // paths below stay integer-only). An f32 target bails to avoid emitting an
    // integer op over float bits — float arithmetic on f32 is not wired yet.
    let primitive_type = resolve_runtime_storage_primitive_type_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    );
    if matches!(primitive_type, Some(PrimitiveType::F32)) {
        return None;
    }
    let is_float = matches!(primitive_type, Some(PrimitiveType::F64));

    if !is_float {
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
    }

    let target_place = target_place?;
    Some(SelectedInstructionKind::WriteRuntimeStorageBinary {
        target_region: target_place.region,
        target_offset: target_place.byte_offset,
        byte_size: target_place.byte_count,
        left,
        operator,
        right,
        is_float,
    })
}

/// Replace a signed division/modulo/right-shift/min/max/comparison operator with
/// its unsigned form when the operands are an unsigned integer type. The default
/// (signed, or an undeterminable type) is correct for the dominant i32/i64 case.
#[allow(clippy::too_many_arguments)]
fn signedness_adjusted_operator(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    target_source_key: StateKey,
    value_source_key: StateKey,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    left_expression: ExpressionHandle,
    right_expression: ExpressionHandle,
    operator: StateGuardOperator,
) -> StateGuardOperator {
    let unsigned = match operator {
        StateGuardOperator::Divide => StateGuardOperator::DivideUnsigned,
        StateGuardOperator::Modulo => StateGuardOperator::ModuloUnsigned,
        StateGuardOperator::ShiftRight => StateGuardOperator::ShiftRightLogical,
        StateGuardOperator::Min => StateGuardOperator::MinUnsigned,
        StateGuardOperator::Max => StateGuardOperator::MaxUnsigned,
        StateGuardOperator::Greater => StateGuardOperator::GreaterUnsigned,
        StateGuardOperator::GreaterOrEqual => StateGuardOperator::GreaterOrEqualUnsigned,
        StateGuardOperator::Less => StateGuardOperator::LessUnsigned,
        StateGuardOperator::LessOrEqual => StateGuardOperator::LessOrEqualUnsigned,
        _ => return operator,
    };

    // For comparisons the result place is a bool, so the signedness lives on the
    // operands; for the others the target shares the operand type. Probe the
    // operands first (works for both), then fall back to the target.
    let is_signed = resolve_runtime_storage_is_signed_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        left_expression,
    )
    .or_else(|| {
        resolve_runtime_storage_is_signed_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            right_expression,
        )
    })
    .or_else(|| {
        resolve_runtime_storage_is_signed_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
    });

    match is_signed {
        Some(false) => unsigned,
        _ => operator,
    }
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
        // This entry point receives a pre-resolved storage place without the
        // target expression, so it cannot classify float vs integer; float
        // arithmetic is routed through the targeted binary-write path instead.
        is_float: false,
    })
}
