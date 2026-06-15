use crate::InstructionSelectionInput;
use crate::selection::storage_places::{
    RuntimeStoragePlace, clamp_runtime_case_comparison_operands_in_table,
    classify_scalar_value_type_in_table, descriptor_primitive_type,
    resolve_runtime_frame_indexed_target_in_table,
    resolve_runtime_pointee_fixed_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset_in_table,
    resolve_runtime_storage_arithmetic_domain_in_table, resolve_runtime_storage_is_signed_in_table,
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
    binary_value_operands_are_float, resolve_runtime_comparison_operand_in_table,
    resolve_runtime_value_operand_in_table,
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
    let (operator, comparison_operator, left_expression, right_expression) =
        match expressions.expression(value) {
            ExpressionNode::Binary(binary) => (
                runtime_binary_operator(binary.operator)?,
                Some(binary.operator),
                binary.left,
                binary.right,
            ),
            ExpressionNode::Call(call) => {
                let operator = builtin_runtime_call_operator_in_table(input, call)?;
                let left = expressions.expression_handle_at_offset(call.arguments, 0);
                let right = expressions.expression_handle_at_offset(call.arguments, 1);
                (operator, None, left, right)
            }
            _ => return None,
        };

    // A TOP-LEVEL String `==` (`let equal: bool = a.label == b.label`, or a
    // single-String-field Equatable expansion): the whole value IS the
    // text-equals leaf. The binary write needs two operands, so the leaf is
    // passed through unchanged as `text_equals | 0`.
    if let Some(comparison_operator) = comparison_operator
        && let Some(text_equals) = super::value_operands::resolve_runtime_text_equals_operand_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            comparison_operator,
            left_expression,
            right_expression,
            runtime_value_operands,
        )
    {
        invalidate_runtime_static_value_in_table(static_values, expressions, target);
        let zero = runtime_value_operands.insert(RuntimeValueOperand::Immediate(0));
        let target_place = target_place?;
        return Some(SelectedInstructionKind::WriteRuntimeStorageBinary {
            target_region: target_place.region,
            target_offset: target_place.byte_offset,
            byte_size: target_place.byte_count,
            left: text_equals,
            operator: StateGuardOperator::Or,
            right: zero,
            is_float: false,
            domain: omega_core::arithmetic::ArithmeticDomain::Exact,
            target_signed: false,
        });
    }

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

    let left = resolve_runtime_comparison_operand_in_table(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        left_expression,
        comparison_operator,
        right_expression,
        static_values,
        runtime_value_operands,
    )?;
    let right = resolve_runtime_comparison_operand_in_table(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        right_expression,
        comparison_operator,
        left_expression,
        static_values,
        runtime_value_operands,
    )?;
    // A case-name equality (the lowered form of `in`) compares the TAG only;
    // the place operand must not read payload bytes. The encoder compares at
    // the operand's recorded width, so an unclamped enum place would fold
    // payload bytes into the comparison.
    if let Some(comparison_operator) = comparison_operator {
        clamp_runtime_case_comparison_operands_in_table(
            &input.layouts,
            expressions,
            comparison_operator,
            left_expression,
            right_expression,
            left,
            right,
            runtime_value_operands,
        );
    }

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
    let target_is_f32 = matches!(
        resolve_runtime_storage_primitive_type_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        ),
        Some(PrimitiveType::F32)
    ) || target_place
        .as_ref()
        .is_some_and(|place| place.byte_count == 4);
    if is_float && target_is_f32 {
        narrow_f32_literal_operands(runtime_value_operands, expressions, left_expression, left);
        narrow_f32_literal_operands(runtime_value_operands, expressions, right_expression, right);
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

    // Decision 17: the write's arithmetic domain + target signedness drive the
    // ISA's overflow handling (Exact/Wrapping wrap; Saturating clamps; Trapping
    // aborts). Both read from the TARGET's declared type, not the operands.
    let domain = resolve_runtime_storage_arithmetic_domain_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    );
    let target_signed = resolve_runtime_storage_is_signed_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    )
    .unwrap_or(false);

    let target_place = target_place?;
    Some(SelectedInstructionKind::WriteRuntimeStorageBinary {
        target_region: target_place.region,
        target_offset: target_place.byte_offset,
        byte_size: target_place.byte_count,
        left,
        operator,
        right,
        is_float,
        domain,
        target_signed,
    })
}

/// Replace a signed division/modulo/right-shift/min/max/comparison operator with
/// its unsigned form when the operands are an unsigned integer type. The default
/// (signed, or an undeterminable type) is correct for the dominant i32/i64 case.
/// Shared with the branch-expansion binary write (transition/call argument
/// values like `f(raw % 100)`), which selects through the same operator table.
#[allow(clippy::too_many_arguments)]
pub(in crate::selection::runtime_dispatch) fn signedness_adjusted_operator(
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
    let Some(unsigned) = unsigned_operator_form(operator) else {
        return operator;
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

/// The unsigned encoding of a signedness-sensitive operator, or `None` when the
/// operator behaves identically for signed and unsigned operands.
fn unsigned_operator_form(operator: StateGuardOperator) -> Option<StateGuardOperator> {
    match operator {
        StateGuardOperator::Divide => Some(StateGuardOperator::DivideUnsigned),
        StateGuardOperator::Modulo => Some(StateGuardOperator::ModuloUnsigned),
        StateGuardOperator::ShiftRight => Some(StateGuardOperator::ShiftRightLogical),
        StateGuardOperator::Min => Some(StateGuardOperator::MinUnsigned),
        StateGuardOperator::Max => Some(StateGuardOperator::MaxUnsigned),
        StateGuardOperator::Greater => Some(StateGuardOperator::GreaterUnsigned),
        StateGuardOperator::GreaterOrEqual => Some(StateGuardOperator::GreaterOrEqualUnsigned),
        StateGuardOperator::Less => Some(StateGuardOperator::LessUnsigned),
        StateGuardOperator::LessOrEqual => Some(StateGuardOperator::LessOrEqualUnsigned),
        _ => None,
    }
}

/// Operand-only variant of [`signedness_adjusted_operator`] for write paths that
/// carry a PRE-RESOLVED target place instead of a target expression (the
/// frame-slot value write that materializes call/transition arguments like
/// `f(raw % 100)` into a parameter slot). A u32 `raw` must select the unsigned
/// modulo: sdiv on raw >= 2^31 yields a negative remainder, which the unsigned
/// dispatch guards then read as a huge value (the dungeon seed-7 extra enemy
/// draw).
pub(in crate::selection::runtime_dispatch) fn signedness_adjusted_operator_for_operands(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    expressions: &ExpressionTable,
    left_expression: ExpressionHandle,
    right_expression: ExpressionHandle,
    operator: StateGuardOperator,
) -> StateGuardOperator {
    let Some(unsigned) = unsigned_operator_form(operator) else {
        return operator;
    };

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
    });

    match is_signed {
        Some(false) => unsigned,
        _ => operator,
    }
}

/// Tree-operand adapter over [`signedness_adjusted_operator_for_operands`] for
/// the non-table write paths (alias-resolved branch-arm expressions carry OWNED
/// `Expression` trees, not table handles). Follows the standard
/// `insert_tree`+delegate collapse pattern so the signedness decision lives once.
pub(in crate::selection::runtime_dispatch) fn signedness_adjusted_operator_for_tree_operands(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    left_expression: &omega_checked_trees::expression::Expression,
    right_expression: &omega_checked_trees::expression::Expression,
    operator: StateGuardOperator,
) -> StateGuardOperator {
    if unsigned_operator_form(operator).is_none() {
        return operator;
    }
    let mut delegated_expressions = ExpressionTable::default();
    let left = delegated_expressions.insert_tree(left_expression);
    let right = delegated_expressions.insert_tree(right_expression);
    signedness_adjusted_operator_for_operands(
        input,
        dispatch_index,
        value_source_key,
        &delegated_expressions,
        left,
        right,
        operator,
    )
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
    // Decision 17: this entry has a PRE-RESOLVED place (no target expression), so
    // the caller passes the target's arithmetic domain + signedness derived from
    // the slot/field descriptor.
    domain: omega_core::arithmetic::ArithmeticDomain,
    target_signed: bool,
    value: ExpressionHandle,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    let (operator, comparison_operator, left_expression, right_expression) =
        match expressions.expression(value) {
            ExpressionNode::Binary(binary) => (
                runtime_binary_operator(binary.operator)?,
                Some(binary.operator),
                binary.left,
                binary.right,
            ),
            ExpressionNode::Call(call) => {
                let operator = builtin_runtime_call_operator_in_table(input, call)?;
                let left = expressions.expression_handle_at_offset(call.arguments, 0);
                let right = expressions.expression_handle_at_offset(call.arguments, 1);
                (operator, None, left, right)
            }
            _ => return None,
        };

    // Same signedness policy as the targeted-mutation path above; this entry has
    // no target EXPRESSION (the place is pre-resolved), so probe the operands.
    let operator = signedness_adjusted_operator_for_operands(
        input,
        dispatch_index,
        source_key,
        expressions,
        left_expression,
        right_expression,
        operator,
    );

    let left = resolve_runtime_comparison_operand_in_table(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        left_expression,
        comparison_operator,
        right_expression,
        static_values,
        runtime_value_operands,
    )?;
    let right = resolve_runtime_comparison_operand_in_table(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        right_expression,
        comparison_operator,
        left_expression,
        static_values,
        runtime_value_operands,
    )?;
    // A case-name equality (the lowered form of `in`) compares the TAG only;
    // see the same clamp in the targeted-mutation path above.
    if let Some(comparison_operator) = comparison_operator {
        clamp_runtime_case_comparison_operands_in_table(
            &input.layouts,
            expressions,
            comparison_operator,
            left_expression,
            right_expression,
            left,
            right,
            runtime_value_operands,
        );
    }

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

    // A 4-byte float target is f32: the operation runs in single precision (movd,
    // low dword), so a float-LITERAL operand -- including a constant local folded to
    // its `Float` initializer (`let a: f32 = 2.5; ... a + b`) -- carries the wrong
    // (f64) bit pattern and must be narrowed to f32 bits. This is the LOCAL
    // float-arithmetic entry point; without it the addss reads garbage.
    if is_float && byte_size == 4 {
        narrow_f32_literal_operands(runtime_value_operands, expressions, left_expression, left);
        narrow_f32_literal_operands(runtime_value_operands, expressions, right_expression, right);
    }
    Some(SelectedInstructionKind::WriteRuntimeStorageBinary {
        target_region,
        target_offset,
        byte_size,
        left,
        operator,
        right,
        is_float,
        domain,
        target_signed,
    })
}

/// When a binary's target is f32, a float-LITERAL operand was resolved to its f64
/// bit pattern (`resolve_runtime_static_float_value_in_table` returns the literal's
/// `f64` value); the single-precision SSE op (`movd`, low dword) needs the f32 bits,
/// so rewrite such an operand's immediate from its f64 bits to its f32 bits. This is
/// keyed on the operand EXPRESSION being a float literal -- the only operand kind
/// that carries an f64-bit immediate. A folded PLACE operand (`self.a`, a constant
/// f32 field) folds through the integer static-value tracker, whose stored bits are
/// ALREADY the f32 pattern (`static_writes.rs` narrows on the way in); re-narrowing
/// those would corrupt them, so this must not touch them. Recurses through a nested
/// Binary operand expression in lockstep with the operand tree so a float literal
/// inside an inner sub-expression is narrowed too. No-op for non-literal operands.
fn narrow_f32_literal_operands(
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    expressions: &ExpressionTable,
    operand_expression: ExpressionHandle,
    operand: RuntimeValueOperandHandle,
) {
    match expressions.expression(operand_expression) {
        ExpressionNode::Float(literal) => {
            let narrowed = (literal.value() as f32).to_bits() as i64;
            if let RuntimeValueOperand::Immediate(bits) = runtime_value_operands.get_mut(operand) {
                *bits = narrowed;
            }
        }
        ExpressionNode::Binary(binary) => {
            let (left_expr, right_expr) = (binary.left, binary.right);
            if let RuntimeValueOperand::Binary { left, right, .. } =
                *runtime_value_operands.get(operand)
            {
                narrow_f32_literal_operands(runtime_value_operands, expressions, left_expr, left);
                narrow_f32_literal_operands(runtime_value_operands, expressions, right_expr, right);
            }
        }
        _ => {}
    }
}

/// Byte size of a scalar primitive, or `None` for non-scalar (e.g. `String`).
fn scalar_primitive_byte_size(primitive: PrimitiveType) -> Option<usize> {
    match primitive {
        PrimitiveType::Bool | PrimitiveType::I8 | PrimitiveType::U8 => Some(1),
        PrimitiveType::I16 | PrimitiveType::U16 => Some(2),
        PrimitiveType::F32 | PrimitiveType::I32 | PrimitiveType::U32 => Some(4),
        PrimitiveType::F64
        | PrimitiveType::I64
        | PrimitiveType::U64
        | PrimitiveType::Usize
        | PrimitiveType::Isize => Some(8),
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

    let kind = build_runtime_convert_write(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        target_place.region,
        target_place.byte_offset,
        target_primitive,
        source_expression,
        static_values,
        runtime_value_operands,
    )?;
    invalidate_runtime_static_value_in_table(static_values, expressions, target);
    Some(kind)
}

/// A numeric `as` cast assigned to a frame slot (`let n: i32 = c as i32`, where the
/// LOCAL target's place is a pre-resolved frame slot, not a target expression). The
/// cast source is often a folded arithmetic expression (`c` inlined to `a + b`), so
/// this reuses the shared convert builder, which classifies the source through a
/// binary and resolves it as a runtime value operand. The target primitive comes
/// from the slot's leaf type descriptor.
#[allow(clippy::too_many_arguments)]
pub(in crate::selection::runtime_dispatch::writes) fn select_runtime_frame_slot_convert_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    value: ExpressionHandle,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    let ExpressionNode::Cast(cast) = expressions.expression(value) else {
        return None;
    };
    let target_primitive = descriptor_primitive_type(&slot.type_descriptor)?;

    build_runtime_convert_write(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        RuntimeStorageRegion::RuntimeFrame,
        slot.byte_offset,
        target_primitive,
        cast.value,
        static_values,
        runtime_value_operands,
    )
}

/// Shared body of the two convert-write entry points: classify the source scalar
/// type, build it as a runtime value operand, narrow f32 float constants, and emit
/// the converting store. The target place (region + offset + primitive) is supplied
/// by the caller -- from a target expression or a pre-resolved frame slot.
#[allow(clippy::too_many_arguments)]
fn build_runtime_convert_write(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    target_primitive: PrimitiveType,
    source_expression: ExpressionHandle,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    // The source is usually a storage place, but a CONSTANT source folds to a
    // literal (`let b: f64 = 10.0; b as i32` becomes `10.0 as i32`) or a folded
    // arithmetic expression (`(a + b) as i32`); the shared classifier resolves a
    // place leaf type, a literal node type, or a binary's operand type.
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

    // An f32 source computed in single precision (`(a + b) as i32`, a/b folded to
    // `Float` literals) needs its float-literal constants narrowed to f32 bits before
    // the convert reads them (movd, low dword).
    if source_primitive == PrimitiveType::F32 {
        narrow_f32_literal_operands(runtime_value_operands, expressions, source_expression, source);
    }

    Some(SelectedInstructionKind::WriteRuntimeStorageConvert {
        target_region,
        target_offset,
        target_byte_size,
        source,
        source_byte_size,
        source_is_float: source_primitive.accepts_float_literal(),
        target_is_float: target_primitive.accepts_float_literal(),
        source_signed: source_primitive.is_signed_integer(),
    })
}
