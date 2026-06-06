use crate::InstructionSelectionInput;
use crate::selection::storage_places::{
    RuntimeStoragePlace, classify_scalar_value_type_in_table,
    resolve_runtime_frame_indexed_target_in_table,
    resolve_runtime_pointee_fixed_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_is_signed_in_table,
    resolve_runtime_storage_place_in_table, resolve_runtime_storage_primitive_type_in_table,
};
use omega_checked_trees::types::PrimitiveType;
use omega_abstract_operations::{
    RuntimeStorageRegion, RuntimeValueOperand, RuntimeValueOperandHandle, SelectedInstructionKind,
    StateGuardOperator,
};
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_control_flow::StateKey;
use omega_core::arena::Arena;

use super::super::static_values::{
    RuntimeStaticValues, invalidate_runtime_static_value_in_table,
};
use super::operators::{builtin_runtime_call_operator_in_table, runtime_binary_operator};
use super::value_operands::{
    binary_value_operands_are_float, resolve_runtime_value_operand_in_table,
};

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

    // A float target performs the operation on the SSE unit, to a machine-owned or
    // frame storage place (the indexed/pointee binary paths below stay integer-only).
    // f64 (8-byte, addsd) and f32 (4-byte, addss) — the encoder selects the scalar
    // width from the target byte_size.
    // Float-ness is the operands' (== the target's) scalar type, via the shared
    // classifier -- the same signal the pre-resolved-place entry below uses, so a
    // binary write is classified identically no matter which path reaches it.
    let is_float = binary_value_operands_are_float(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        left_expression,
        right_expression,
    );

    // A direct float-LITERAL operand was resolved to its f64 bit pattern
    // (value_operands.rs). When the target is f32 the operation runs in single
    // precision (movd reads the low dword), so narrow such a literal operand to its
    // f32 bit pattern. f32 field/var operands need no narrowing (loaded from 4-byte
    // storage); nested float-literal operands (inside an inner binary) are a separate
    // remaining gap.
    if is_float
        && matches!(
            resolve_runtime_storage_primitive_type_in_table(
                input,
                dispatch_index,
                target_source_key,
                expressions,
                target,
            ),
            Some(PrimitiveType::F32)
        )
    {
        narrow_f32_literal_operand(runtime_value_operands, expressions, left_expression, left);
        narrow_f32_literal_operand(runtime_value_operands, expressions, right_expression, right);
    }

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

    // No target expression here (pre-resolved place), so classify float vs integer
    // from the OPERAND expressions: a float-typed place or a float literal on
    // either side means the op runs on the SSE unit (addsd/...). Without this, a
    // float arithmetic into a local (`let c: f64 = a + b`) -- which reaches this
    // entry point -- emits an integer add over the IEEE bits.
    let is_float = binary_value_operands_are_float(
        input,
        dispatch_index,
        source_key,
        expressions,
        left_expression,
        right_expression,
    );
    Some(SelectedInstructionKind::WriteRuntimeStorageBinary {
        target_region,
        target_offset,
        byte_size,
        left,
        operator,
        right,
        is_float,
    })
}

/// When a binary's target is f32, a direct float-literal operand was resolved to its
/// f64 bit pattern; rewrite it to the f32 bit pattern so the single-precision SSE op
/// (`movd`, low dword) reads the correct constant. No-op for non-literal operands.
fn narrow_f32_literal_operand(
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    expressions: &ExpressionTable,
    operand_expression: ExpressionHandle,
    operand: RuntimeValueOperandHandle,
) {
    let ExpressionNode::Float(literal) = expressions.expression(operand_expression) else {
        return;
    };
    let narrowed = (literal.value() as f32).to_bits() as i64;
    if let RuntimeValueOperand::Immediate(bits) = runtime_value_operands.get_mut(operand) {
        *bits = narrowed;
    }
}

/// Byte size of a scalar primitive, or `None` for non-scalar (e.g. `String`).
fn scalar_primitive_byte_size(primitive: PrimitiveType) -> Option<usize> {
    match primitive {
        PrimitiveType::Bool => Some(1),
        PrimitiveType::F32 | PrimitiveType::I32 | PrimitiveType::U32 => Some(4),
        PrimitiveType::F64 | PrimitiveType::I64 | PrimitiveType::U64 | PrimitiveType::Usize => {
            Some(8)
        }
        PrimitiveType::String => None,
    }
}

/// A numeric `as` cast assigned to a storage place (`self.n = self.a as i32`):
/// resolve the source and target scalar primitive types, build the source as a
/// runtime value operand, and emit a converting store. The encoder picks the
/// right conversion (cvttsd2si / cvtsi2sd / cvtsd2ss / sized integer move) from
/// the float-ness, widths, and signedness recorded here. First cut: the source
/// must resolve to a storage place (so its primitive type is known); literal
/// sources are a follow-on.
#[allow(clippy::too_many_arguments)]
pub(in crate::selection::runtime_dispatch::writes) fn select_runtime_convert_mutation_write_in_table(
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
    let ExpressionNode::Cast(cast) = expressions.expression(value) else {
        return None;
    };
    let source_expression = cast.value;

    let target_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    )?;
    let target_primitive = resolve_runtime_storage_primitive_type_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    )?;
    // The source is usually a storage place, but a CONSTANT source folds to a
    // literal (`let b: f64 = 10.0; b as i32` becomes `10.0 as i32`); the shared
    // classifier resolves either (place leaf type, or the literal's node type).
    let source_primitive = classify_scalar_value_type_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        source_expression,
    )?;

    let target_byte_size = scalar_primitive_byte_size(target_primitive)?;
    let source_byte_size = scalar_primitive_byte_size(source_primitive)?;
    if !matches!(target_byte_size, 1 | 4 | 8) || !matches!(source_byte_size, 1 | 4 | 8) {
        return None;
    }

    let source = resolve_runtime_value_operand_in_table(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        source_expression,
        static_values,
        runtime_value_operands,
    )?;

    invalidate_runtime_static_value_in_table(static_values, expressions, target);

    Some(SelectedInstructionKind::WriteRuntimeStorageConvert {
        target_region: target_place.region,
        target_offset: target_place.byte_offset,
        target_byte_size,
        source,
        source_byte_size,
        source_is_float: source_primitive.accepts_float_literal(),
        target_is_float: target_primitive.accepts_float_literal(),
        source_signed: source_primitive.is_signed_integer(),
    })
}
