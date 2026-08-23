use crate::InstructionSelectionInput;
use crate::selection::storage_places::{
    RuntimeStoragePlace, clamp_runtime_case_comparison_operands_in_table,
    classify_scalar_value_type_in_table, descriptor_primitive_type,
    resolve_binary_operation_arithmetic_domain_in_table,
    resolve_runtime_frame_base_indexed_target_with_index_region_in_table,
    resolve_runtime_frame_indexed_target_in_table,
    resolve_runtime_pointee_fixed_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_is_signed_in_table,
    resolve_runtime_storage_place_in_table, resolve_runtime_storage_primitive_type_in_table,
    runtime_storage_target_is_atomic_in_table,
};
use omega_abstract_operations::{
    Place, PlaceStep, RuntimeStorageRegion, RuntimeValueOperand, RuntimeValueOperandHandle,
    SelectedInstructionKind, StateGuardOperator,
};
use omega_control_flow::StateKey;
use psi_arena::Arena;
use psi_checked_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use psi_checked_trees::types::PrimitiveType;

use super::super::static_values::{
    RuntimeStaticValues, invalidate_runtime_static_value_in_table,
    resolve_runtime_static_integer_landing_in_table,
};
use super::operators::{
    builtin_runtime_call_operator_in_table, is_float_classification_predicate,
    runtime_binary_operator,
};
use super::value_operands::{
    binary_value_operand_byte_width, binary_value_operands_are_float,
    resolve_runtime_comparison_operand_in_table_with_root,
    resolve_runtime_comparison_operand_in_table_with_root_and_call_ordinal,
    resolve_runtime_value_operand_in_table,
};

#[allow(clippy::too_many_arguments)]
pub(in crate::selection::runtime_dispatch::writes) fn select_runtime_atomic_load_or_store_in_table(
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
    let ExpressionNode::Atomic(atomic) = expressions.expression(value) else {
        return None;
    };
    match atomic.ordering {
        psi_language_core::AtomicOrderingPlan::Load(_) => {
            if !runtime_storage_target_is_atomic_in_table(
                input,
                dispatch_index,
                value_source_key,
                expressions,
                atomic.value,
            ) {
                return None;
            }
            let source = resolve_runtime_storage_place_in_table(
                input,
                dispatch_index,
                value_source_key,
                expressions,
                atomic.value,
            )?;
            let result = resolve_runtime_storage_place_in_table(
                input,
                dispatch_index,
                target_source_key,
                expressions,
                target,
            )?;
            if source.byte_count == 0 || source.byte_count != result.byte_count {
                return None;
            }
            invalidate_runtime_static_value_in_table(static_values, expressions, target);
            Some(SelectedInstructionKind::AtomicLoad {
                source_region: source.region,
                source_offset: source.byte_offset,
                byte_size: source.byte_count,
                result_region: result.region,
                result_offset: result.byte_offset,
                ordering: atomic.ordering,
            })
        }
        psi_language_core::AtomicOrderingPlan::Store(_) => {
            if !runtime_storage_target_is_atomic_in_table(
                input,
                dispatch_index,
                target_source_key,
                expressions,
                target,
            ) {
                return None;
            }
            let target_place = resolve_runtime_storage_place_in_table(
                input,
                dispatch_index,
                target_source_key,
                expressions,
                target,
            )?;
            if target_place.byte_count == 0 {
                return None;
            }
            let stored = resolve_runtime_value_operand_in_table(
                input,
                dispatch_index,
                value_source_key,
                statement_index,
                expressions,
                atomic.value,
                static_values,
                runtime_value_operands,
            )?;
            invalidate_runtime_static_value_in_table(static_values, expressions, target);
            Some(SelectedInstructionKind::AtomicStore {
                target_region: target_place.region,
                target_offset: target_place.byte_offset,
                byte_size: target_place.byte_count,
                value: stored,
                ordering: atomic.ordering,
            })
        }
        psi_language_core::AtomicOrderingPlan::Swap(_) => {
            if !runtime_storage_target_is_atomic_in_table(
                input,
                dispatch_index,
                target_source_key,
                expressions,
                target,
            ) {
                return None;
            }
            let target_place = resolve_runtime_storage_place_in_table(
                input,
                dispatch_index,
                target_source_key,
                expressions,
                target,
            )?;
            let result_place = resolve_runtime_storage_place_in_table(
                input,
                dispatch_index,
                value_source_key,
                expressions,
                atomic.result,
            )?;
            if target_place.byte_count == 0 || target_place.byte_count != result_place.byte_count {
                return None;
            }
            let new_value = resolve_runtime_value_operand_in_table(
                input,
                dispatch_index,
                value_source_key,
                statement_index,
                expressions,
                atomic.value,
                static_values,
                runtime_value_operands,
            )?;
            invalidate_runtime_static_value_in_table(static_values, expressions, target);
            Some(SelectedInstructionKind::AtomicSwap {
                target_region: target_place.region,
                target_offset: target_place.byte_offset,
                byte_size: target_place.byte_count,
                result_region: result_place.region,
                result_offset: result_place.byte_offset,
                new_value,
                ordering: atomic.ordering,
            })
        }
        psi_language_core::AtomicOrderingPlan::ReadModifyWrite(_)
        | psi_language_core::AtomicOrderingPlan::CompareExchange { .. } => None,
    }
}

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

    // Atomic compare_exchange: the parser desugars `place.compare_exchange(
    // expected, new, ..)` to `place = prior + (prior == expected) * (new -
    // prior)`. Recognize that exact shape on an atomic target FIRST (before
    // fetch_add, whose looser `place + delta` gate this Add also satisfies) and
    // lower it to one `LOCK CMPXCHG` / `CASAL` instead of an xadd of the delta.
    if let Some(cas) = select_runtime_atomic_compare_exchange_in_table(
        input,
        dispatch_index,
        target_source_key,
        value_source_key,
        statement_index,
        expressions,
        target,
        target_place.clone(),
        value,
        static_values,
        runtime_value_operands,
    ) {
        return Some(cas);
    }

    // Atomic fetch arithmetic lowers through a target RMW and writes the
    // instruction-observed prior into the compiler-authored result place.
    if let Some(atomic) = select_runtime_atomic_fetch_arithmetic_in_table(
        input,
        dispatch_index,
        target_source_key,
        value_source_key,
        statement_index,
        expressions,
        target,
        target_place.clone(),
        value,
        static_values,
        runtime_value_operands,
    ) {
        return Some(atomic);
    }

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
fn select_runtime_atomic_compare_exchange_in_table(
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
    let atomic = match expressions.expression(value) {
        ExpressionNode::Atomic(atomic)
            if matches!(
                atomic.ordering,
                psi_language_core::AtomicOrderingPlan::CompareExchange { .. }
            ) =>
        {
            atomic
        }
        _ => return None,
    };
    let ordering = atomic.ordering;
    let value = atomic.value;
    // Match the compare_exchange desugar tree:
    //   Add(_, Multiply(Equal(_, expected), Subtract(new_value, _)))
    // extracting `expected` (the Equal's right) and `new_value` (the Subtract's
    // left). Handles are copied out of each node before the next lookup to keep
    // the immutable borrows from overlapping.
    let add_right = match expressions.expression(value) {
        ExpressionNode::Binary(add) if add.operator == BinaryOperator::Add => add.right,
        _ => return None,
    };
    let (mul_left, mul_right) = match expressions.expression(add_right) {
        ExpressionNode::Binary(mul) if mul.operator == BinaryOperator::Multiply => {
            (mul.left, mul.right)
        }
        _ => return None,
    };
    let expected_expr = match expressions.expression(mul_left) {
        ExpressionNode::Binary(eq) if eq.operator == BinaryOperator::Equal => eq.right,
        _ => return None,
    };
    let new_value_expr = match expressions.expression(mul_right) {
        ExpressionNode::Binary(sub) if sub.operator == BinaryOperator::Subtract => sub.left,
        _ => return None,
    };
    if !runtime_storage_target_is_atomic_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) {
        return None;
    }
    let target_place = target_place?;
    let result_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        atomic.result,
    )?;
    if target_place.byte_count == 0 {
        return None;
    }
    let expected = resolve_runtime_value_operand_in_table(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        expected_expr,
        static_values,
        runtime_value_operands,
    )?;
    let new_value = resolve_runtime_value_operand_in_table(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        new_value_expr,
        static_values,
        runtime_value_operands,
    )?;
    invalidate_runtime_static_value_in_table(static_values, expressions, target);
    Some(SelectedInstructionKind::AtomicCompareExchange {
        target_region: target_place.region,
        target_offset: target_place.byte_offset,
        byte_size: target_place.byte_count,
        result_region: result_place.region,
        result_offset: result_place.byte_offset,
        expected,
        new_value,
        ordering,
    })
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_atomic_fetch_arithmetic_in_table(
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
    let atomic = match expressions.expression(value) {
        ExpressionNode::Atomic(atomic)
            if matches!(
                atomic.ordering,
                psi_language_core::AtomicOrderingPlan::ReadModifyWrite(_)
            ) =>
        {
            atomic
        }
        _ => return None,
    };
    let ordering = atomic.ordering;
    let value = atomic.value;
    let ExpressionNode::Binary(binary) = expressions.expression(value) else {
        return None;
    };
    let operator = runtime_binary_operator(binary.operator)?;
    if !matches!(
        operator,
        StateGuardOperator::Add
            | StateGuardOperator::Subtract
            | StateGuardOperator::BitwiseXor
            | StateGuardOperator::BitwiseOr
            | StateGuardOperator::BitwiseAnd
    ) {
        return None;
    }
    let right = binary.right;
    if !runtime_storage_target_is_atomic_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) {
        return None;
    }
    let target_place = target_place?;
    let result_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        atomic.result,
    )?;
    if target_place.byte_count == 0 {
        return None;
    }
    let delta = resolve_runtime_value_operand_in_table(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        right,
        static_values,
        runtime_value_operands,
    )?;
    // The atomic field's value is volatile; drop any tracked static value so a
    // later read does not fold to a stale constant.
    invalidate_runtime_static_value_in_table(static_values, expressions, target);
    let common = (
        target_place.region,
        target_place.byte_offset,
        target_place.byte_count,
        result_place.region,
        result_place.byte_offset,
        delta,
        ordering,
    );
    Some(match operator {
        StateGuardOperator::Add => SelectedInstructionKind::AtomicFetchAdd {
            target_region: common.0,
            target_offset: common.1,
            byte_size: common.2,
            result_region: common.3,
            result_offset: common.4,
            delta: common.5,
            ordering: common.6,
        },
        StateGuardOperator::Subtract => SelectedInstructionKind::AtomicFetchSub {
            target_region: common.0,
            target_offset: common.1,
            byte_size: common.2,
            result_region: common.3,
            result_offset: common.4,
            delta: common.5,
            ordering: common.6,
        },
        StateGuardOperator::BitwiseXor => SelectedInstructionKind::AtomicFetchXor {
            target_region: common.0,
            target_offset: common.1,
            byte_size: common.2,
            result_region: common.3,
            result_offset: common.4,
            value: common.5,
            ordering: common.6,
        },
        StateGuardOperator::BitwiseOr => SelectedInstructionKind::AtomicFetchOr {
            target_region: common.0,
            target_offset: common.1,
            byte_size: common.2,
            result_region: common.3,
            result_offset: common.4,
            value: common.5,
            ordering: common.6,
        },
        StateGuardOperator::BitwiseAnd => SelectedInstructionKind::AtomicFetchAnd {
            target_region: common.0,
            target_offset: common.1,
            byte_size: common.2,
            result_region: common.3,
            result_offset: common.4,
            value: common.5,
            ordering: common.6,
        },
        _ => unreachable!("fetch arithmetic gate accepts add/sub/xor/or/and only"),
    })
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
    if std::env::var_os("OMEGA_DEBUG_RECEIVER").is_some() {
        eprintln!(
            "BTW targeted: dispatch {} valsrc m{} s{} stmt {} value `{}`",
            dispatch_index,
            value_source_key.machine.arena_index(),
            value_source_key.state.arena_index(),
            statement_index,
            expressions.display_name(value)
        );
    }
    if let Some(ternary) =
        super::value_operands::resolve_selected_ternary_float_operand_in_table_with_root(
            input,
            dispatch_index,
            value_source_key,
            statement_index,
            expressions,
            value,
            value,
            None,
            static_values,
            runtime_value_operands,
        )
    {
        let RuntimeValueOperand::Binary {
            left,
            operator,
            right,
            is_float,
            arithmetic_domain,
            ..
        } = runtime_value_operands.get(ternary).clone()
        else {
            return None;
        };
        invalidate_runtime_static_value_in_table(static_values, expressions, target);
        let target_place = target_place?;
        return Some(
            crate::selection::runtime_dispatch::write_place_binary_direct(
                target_place.region,
                target_place.byte_offset,
                target_place.byte_count,
                left,
                operator,
                right,
                is_float,
                arithmetic_domain,
                false,
            ),
        );
    }
    let (operator, comparison_operator, left_expression, right_expression) =
        match expressions.expression(value) {
            ExpressionNode::Binary(binary) => (
                runtime_binary_operator(binary.operator)?,
                Some(binary.operator),
                binary.left,
                binary.right,
            ),
            ExpressionNode::Call(call) => {
                // `sqrt(x)` (a unary builtin) rides the binary float path with
                // BOTH operands = x; the encoder's Sqrt arm reads xmm0 only.
                if let Some(operator) =
                    super::operators::builtin_runtime_unary_call_operator_in_table(input, call)
                {
                    let x = expressions.expression_handle_at_offset(call.arguments, 0);
                    (operator, None, x, x)
                } else {
                    let operator = builtin_runtime_call_operator_in_table(input, call)?;
                    let left = expressions.expression_handle_at_offset(call.arguments, 0);
                    let right = expressions.expression_handle_at_offset(call.arguments, 1);
                    (operator, None, left, right)
                }
            }
            _ => return None,
        };

    // A TOP-LEVEL String `==` (`let equal: bool = a.label == b.label`, or a
    // single-String-field Equatable expansion): the whole value IS the
    // text-equals leaf. The binary write needs two operands, so the leaf is
    // passed through unchanged as `text_equals | 0`.
    if let Some(comparison_operator) = comparison_operator
        && let Some(text_equals) =
            super::value_operands::resolve_runtime_text_equals_operand_in_table(
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
        return Some(
            crate::selection::runtime_dispatch::write_place_binary_direct(
                target_place.region,
                target_place.byte_offset,
                target_place.byte_count,
                text_equals,
                StateGuardOperator::Or,
                zero,
                false,
                psi_numerics::arithmetic::ArithmeticDomain::Exact,
                false,
            ),
        );
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

    let left = resolve_runtime_comparison_operand_in_table_with_root(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        value,
        left_expression,
        comparison_operator,
        right_expression,
        static_values,
        runtime_value_operands,
    )?;
    let operand_byte_width = binary_value_operand_byte_width(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        left_expression,
        right_expression,
    );
    let classification_byte_width =
        is_float_classification_predicate(operator).then_some(operand_byte_width);
    let right = if let Some(byte_width) = classification_byte_width {
        // The encoder compares the left float register with itself. Keep an
        // ignored metadata placeholder here so the authored unary argument is
        // never evaluated a second time; 4/8 retains its source format when
        // the operand itself folds to an untyped immediate.
        runtime_value_operands.insert(RuntimeValueOperand::Immediate(byte_width as i64))
    } else {
        resolve_runtime_comparison_operand_in_table_with_root(
            input,
            dispatch_index,
            value_source_key,
            statement_index,
            expressions,
            value,
            right_expression,
            comparison_operator,
            left_expression,
            static_values,
            runtime_value_operands,
        )?
    };
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
    if is_float && (target_is_f32 || operand_byte_width == 4) {
        narrow_f32_literal_operands(
            input,
            dispatch_index,
            value_source_key,
            runtime_value_operands,
            expressions,
            left_expression,
            left,
        );
        narrow_f32_literal_operands(
            input,
            dispatch_index,
            value_source_key,
            runtime_value_operands,
            expressions,
            right_expression,
            right,
        );
    }

    if !is_float {
        if let Some(indexed_target) = resolve_runtime_frame_indexed_target_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        ) {
            return Some(
                crate::selection::runtime_dispatch::write_place_binary_frame_indexed(
                    indexed_target.descriptor_offset,
                    indexed_target.index_region,
                    indexed_target.index_offset,
                    indexed_target.index_byte_size,
                    indexed_target.element_byte_size,
                    indexed_target.field_byte_offset,
                    indexed_target.byte_count,
                    left,
                    operator,
                    right,
                ),
            );
        }

        if let Some(pointer_target) = resolve_runtime_pointee_fixed_indexed_target_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        ) {
            return Some(
                crate::selection::runtime_dispatch::write_place_binary_pointee(
                    pointer_target.pointer_byte_offset,
                    pointer_target.field_byte_offset,
                    pointer_target.pointee_byte_size,
                    left,
                    operator,
                    right,
                ),
            );
        }

        if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        ) {
            return Some(
                crate::selection::runtime_dispatch::write_place_binary_pointee(
                    pointer_target.pointer_byte_offset,
                    pointer_target.field_byte_offset,
                    pointer_target.pointee_byte_size,
                    left,
                    operator,
                    right,
                ),
            );
        }
    }

    let target_place = target_place?;
    // Float operations consume exact checked provider and adapter evidence
    // carried through control flow. Only integer operations use the ordinary
    // operand-domain path inside the shared resolver.
    let operation_byte_width = classification_byte_width.unwrap_or(target_place.byte_count);
    let domain = resolve_binary_operation_arithmetic_domain_in_table(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        value,
        left_expression,
        right_expression,
        is_float,
        operation_byte_width,
    )?;
    let target_signed = resolve_runtime_storage_is_signed_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    )
    .unwrap_or(false);

    Some(
        crate::selection::runtime_dispatch::write_place_binary_direct(
            target_place.region,
            target_place.byte_offset,
            target_place.byte_count,
            left,
            operator,
            right,
            is_float,
            domain,
            target_signed,
        ),
    )
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

fn resolve_runtime_operand_signedness_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    static_values: &RuntimeStaticValues,
) -> Option<bool> {
    resolve_runtime_storage_is_signed_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )
    .or_else(|| {
        resolve_runtime_static_integer_landing_in_table(expressions, expression, static_values)
            .map(|landing| landing.landed_type.is_signed())
    })
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

/// Tree-operand adapter over the signedness adjustment for the non-table
/// write paths (alias-resolved branch-arm expressions carry OWNED
/// `Expression` trees, not table handles). Follows the standard
/// `insert_tree`+delegate collapse pattern so the signedness decision lives
/// once. Signedness comes from the operands themselves; a write destination
/// must not reinterpret an already-landed constant expression.
pub(in crate::selection::runtime_dispatch) fn signedness_adjusted_operator_for_tree_operands(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    left_expression: &psi_checked_trees::expression::Expression,
    right_expression: &psi_checked_trees::expression::Expression,
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
pub(in crate::selection::runtime_dispatch) fn select_runtime_storage_binary_write_in_table(
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
    select_runtime_storage_binary_write_in_table_with_call_ordinal(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        target_region,
        target_offset,
        byte_size,
        value,
        None,
        static_values,
        runtime_value_operands,
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection::runtime_dispatch) fn select_runtime_storage_binary_write_in_table_with_call_ordinal(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    byte_size: usize,
    value: ExpressionHandle,
    minimum_call_ordinal: Option<usize>,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    select_runtime_storage_binary_write_in_table_with_evidence_source_key_and_call_ordinal(
        input,
        dispatch_index,
        source_key,
        source_key,
        statement_index,
        expressions,
        target_region,
        target_offset,
        byte_size,
        value,
        minimum_call_ordinal,
        static_values,
        runtime_value_operands,
    )
}

/// Select a provider-backed binary/unary write whose operands were substituted
/// into `source_key` but whose checked operator still belongs to
/// `evidence_source_key`. Generated provider calls can have no source span, so
/// the two identities cannot always be recovered from the rebuilt expression.
#[allow(clippy::too_many_arguments)]
pub(in crate::selection::runtime_dispatch) fn select_runtime_storage_binary_write_in_table_with_evidence_source_key(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    evidence_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    byte_size: usize,
    value: ExpressionHandle,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    select_runtime_storage_binary_write_in_table_with_evidence_source_key_and_call_ordinal(
        input,
        dispatch_index,
        source_key,
        evidence_source_key,
        statement_index,
        expressions,
        target_region,
        target_offset,
        byte_size,
        value,
        None,
        static_values,
        runtime_value_operands,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_storage_binary_write_in_table_with_evidence_source_key_and_call_ordinal(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    evidence_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    byte_size: usize,
    value: ExpressionHandle,
    minimum_call_ordinal: Option<usize>,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    if let Some(ternary) =
        super::value_operands::resolve_selected_ternary_float_operand_in_table_with_root(
            input,
            dispatch_index,
            source_key,
            statement_index,
            expressions,
            value,
            value,
            minimum_call_ordinal,
            static_values,
            runtime_value_operands,
        )
    {
        let RuntimeValueOperand::Binary {
            left,
            operator,
            right,
            is_float,
            arithmetic_domain,
            ..
        } = runtime_value_operands.get(ternary).clone()
        else {
            return None;
        };
        return Some(
            crate::selection::runtime_dispatch::write_place_binary_direct(
                target_region,
                target_offset,
                byte_size,
                left,
                operator,
                right,
                is_float,
                arithmetic_domain,
                false,
            ),
        );
    }
    let (operator, comparison_operator, left_expression, right_expression) =
        match expressions.expression(value) {
            ExpressionNode::Binary(binary) => (
                runtime_binary_operator(binary.operator)?,
                Some(binary.operator),
                binary.left,
                binary.right,
            ),
            ExpressionNode::Call(call) => {
                // `sqrt(x)` (a unary builtin) rides the binary float path with
                // BOTH operands = x; the encoder's Sqrt arm reads xmm0 only.
                if let Some(operator) =
                    super::operators::builtin_runtime_unary_call_operator_in_table(input, call)
                {
                    let x = expressions.expression_handle_at_offset(call.arguments, 0);
                    (operator, None, x, x)
                } else {
                    let operator = builtin_runtime_call_operator_in_table(input, call)?;
                    let left = expressions.expression_handle_at_offset(call.arguments, 0);
                    let right = expressions.expression_handle_at_offset(call.arguments, 1);
                    (operator, None, left, right)
                }
            }
            _ => return None,
        };

    // Same signedness policy as the targeted-mutation path above; this entry has
    // no target EXPRESSION (the place is pre-resolved), so probe the operands.
    // Typed alias materialization retains landed constants on this expression,
    // so the destination never reinterprets an anonymous value as a fallback.
    let resolved_signed = resolve_runtime_operand_signedness_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        left_expression,
        static_values,
    )
    .or_else(|| {
        resolve_runtime_operand_signedness_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            right_expression,
            static_values,
        )
    });
    let operator = match (unsigned_operator_form(operator), resolved_signed) {
        (Some(unsigned), Some(false)) => unsigned,
        _ => operator,
    };
    let left = resolve_runtime_comparison_operand_in_table_with_root_and_call_ordinal(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        value,
        left_expression,
        comparison_operator,
        right_expression,
        minimum_call_ordinal,
        static_values,
        runtime_value_operands,
    )?;
    let operand_byte_width = binary_value_operand_byte_width(
        input,
        dispatch_index,
        source_key,
        expressions,
        left_expression,
        right_expression,
    );
    let classification_byte_width =
        is_float_classification_predicate(operator).then_some(operand_byte_width);
    let right = if let Some(byte_width) = classification_byte_width {
        runtime_value_operands.insert(RuntimeValueOperand::Immediate(byte_width as i64))
    } else {
        resolve_runtime_comparison_operand_in_table_with_root_and_call_ordinal(
            input,
            dispatch_index,
            source_key,
            statement_index,
            expressions,
            value,
            right_expression,
            comparison_operator,
            left_expression,
            minimum_call_ordinal,
            static_values,
            runtime_value_operands,
        )?
    };
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
    if is_float && (byte_size == 4 || operand_byte_width == 4) {
        narrow_f32_literal_operands(
            input,
            dispatch_index,
            source_key,
            runtime_value_operands,
            expressions,
            left_expression,
            left,
        );
        narrow_f32_literal_operands(
            input,
            dispatch_index,
            source_key,
            runtime_value_operands,
            expressions,
            right_expression,
            right,
        );
    }

    // Consume carried checked adapter evidence for normalized float
    // operations; only compatibility-only shapes reconstruct from operand
    // domains.
    let operation_byte_width = classification_byte_width.unwrap_or(byte_size);
    let domain = resolve_binary_operation_arithmetic_domain_in_table(
        input,
        dispatch_index,
        evidence_source_key,
        statement_index,
        expressions,
        value,
        left_expression,
        right_expression,
        is_float,
        operation_byte_width,
    )?;
    let target_signed = resolved_signed.unwrap_or(false);
    Some(
        crate::selection::runtime_dispatch::write_place_binary_direct(
            target_region,
            target_offset,
            byte_size,
            left,
            operator,
            right,
            is_float,
            domain,
            target_signed,
        ),
    )
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
pub(super) fn narrow_f32_literal_operands(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    expressions: &ExpressionTable,
    operand_expression: ExpressionHandle,
    operand: RuntimeValueOperandHandle,
) {
    match expressions.expression(operand_expression) {
        ExpressionNode::Float(literal) => {
            let narrowed = literal.f32_bits() as i64;
            if let RuntimeValueOperand::Immediate(bits) = runtime_value_operands.get_mut(operand) {
                *bits = narrowed;
            }
        }
        ExpressionNode::Name(_) => {
            // A materialized f32 place records its already-narrow bits in the
            // static-value tracker. An elided constant local has no place and
            // retains the source literal's f64 carrier there; normalize only
            // that latter provenance. Treating every immediate Name as f64
            // would corrupt a folded machine/frame f32 value.
            if resolve_runtime_storage_place_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                operand_expression,
            )
            .is_none()
                && let RuntimeValueOperand::Immediate(bits) =
                    runtime_value_operands.get_mut(operand)
            {
                *bits = i64::from((f64::from_bits(*bits as u64) as f32).to_bits());
            }
        }
        ExpressionNode::Mutable(inner) => narrow_f32_literal_operands(
            input,
            dispatch_index,
            source_key,
            runtime_value_operands,
            expressions,
            *inner,
            operand,
        ),
        ExpressionNode::Binary(binary) => {
            let (left_expr, right_expr) = (binary.left, binary.right);
            if let RuntimeValueOperand::Binary { left, right, .. } =
                *runtime_value_operands.get(operand)
            {
                narrow_f32_literal_operands(
                    input,
                    dispatch_index,
                    source_key,
                    runtime_value_operands,
                    expressions,
                    left_expr,
                    left,
                );
                narrow_f32_literal_operands(
                    input,
                    dispatch_index,
                    source_key,
                    runtime_value_operands,
                    expressions,
                    right_expr,
                    right,
                );
            }
        }
        _ => {}
    }
}

/// Byte size of a scalar primitive, or `None` for non-scalar (e.g. `String`).
/// Delegates to the single source of truth on `PrimitiveType`.
fn scalar_primitive_byte_size(primitive: PrimitiveType) -> Option<usize> {
    primitive.scalar_byte_size()
}

/// A numeric `as` cast assigned to a storage place (`self.n = self.a as i32`):
/// resolve the source and target scalar primitive types, build the source as a
/// runtime value operand, and emit a converting store. The encoder picks the
/// right conversion (cvttsd2si / cvtsi2sd / cvtsd2ss / sized integer move) from
/// the float-ness, widths, and signedness recorded here. First cut: the source
/// must resolve to a storage place (so its primitive type is known); literal
/// sources are a follow-on.
#[allow(clippy::too_many_arguments)]
pub(in crate::selection::runtime_dispatch) fn select_runtime_convert_mutation_write_in_table(
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

    let target_primitive = resolve_runtime_storage_primitive_type_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    )
    // A runtime-indexed target is intentionally not flattened to a direct
    // storage leaf, so the ordinary target-descriptor resolver may not reach
    // it. The cast result carries the exact primitive type that the checked
    // assignment stores.
    .or_else(|| input.program.primitive_type_reference(cast.target_type))?;

    if let Some(target_place) = runtime_indexed_convert_target_place(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) {
        let kind = build_runtime_convert_write(
            input,
            dispatch_index,
            value_source_key,
            statement_index,
            expressions,
            RuntimeStorageRegion::Machine,
            0,
            Some(target_place),
            target_primitive,
            source_expression,
            cast.domain,
            static_values,
            runtime_value_operands,
        )?;
        invalidate_runtime_static_value_in_table(static_values, expressions, target);
        return Some(kind);
    }

    let target_place = resolve_runtime_storage_place_in_table(
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
        None,
        target_primitive,
        source_expression,
        cast.domain,
        static_values,
        runtime_value_operands,
    )?;
    invalidate_runtime_static_value_in_table(static_values, expressions, target);
    Some(kind)
}

fn runtime_indexed_convert_target_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
) -> Option<Place> {
    if let Some(indexed) = resolve_runtime_frame_base_indexed_target_with_index_region_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        target,
    ) {
        return Place::at(RuntimeStorageRegion::RuntimeFrame, indexed.base_byte_offset)
            .with_step(PlaceStep::ScaledIndex {
                index_region: indexed.index_region,
                index_offset: indexed.index_offset,
                index_byte_size: indexed.index_byte_size,
                element_byte_size: indexed.element_byte_size,
            })?
            .with_step(PlaceStep::ConstOffset(indexed.field_byte_offset));
    }
    let (place, _) = super::super::resolve_struct_target_place(
        input,
        dispatch_index,
        source_key,
        expressions,
        target,
    )?;
    // Direct places retain the compact WriteRuntimeStorageConvert operation.
    // Every walked place uses the canonical target algebra assembled by the
    // shared mutation resolver, so conversion targeting cannot lag integer,
    // copy, and binary writes by independently re-enumerating the grammar.
    place.const_offset().is_none().then_some(place)
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
        None,
        target_primitive,
        cast.value,
        cast.domain,
        static_values,
        runtime_value_operands,
    )
}

/// Shared body of the two convert-write entry points: classify the source scalar
/// type, build it as a runtime value operand, narrow f32 float constants, and emit
/// the converting store. The target place (region + offset + primitive) is supplied
/// by the caller -- from a target expression or a pre-resolved frame slot.
#[allow(clippy::too_many_arguments)]
pub(in crate::selection::runtime_dispatch) fn build_runtime_convert_write(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    target_place: Option<Place>,
    target_primitive: PrimitiveType,
    source_expression: ExpressionHandle,
    cast_domain: psi_numerics::arithmetic::ArithmeticDomain,
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
    // The convert encoder handles 1/2/4/8-byte integer widths: a 2-byte source is
    // movzx/movsx-extended like a 1-byte one, and a 2-byte target stores through
    // the 0x66-prefixed word form (same pipeline as 16-bit arithmetic). 16-byte+
    // and unmapped widths are still unsupported.
    if !matches!(target_byte_size, 1 | 2 | 4 | 8) || !matches!(source_byte_size, 1 | 2 | 4 | 8) {
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
        narrow_f32_literal_operands(
            input,
            dispatch_index,
            value_source_key,
            runtime_value_operands,
            expressions,
            source_expression,
            source,
        );
    }

    let source_is_float = source_primitive.accepts_float_literal();
    let target_is_float = target_primitive.accepts_float_literal();
    let source_signed = source_primitive.is_signed_integer();
    let target_signed = target_primitive.is_signed_integer();
    let trapping = cast_domain == psi_numerics::arithmetic::ArithmeticDomain::Trapping
        && source_is_float
        && !target_is_float;
    let saturating = cast_domain == psi_numerics::arithmetic::ArithmeticDomain::Saturating
        && source_is_float
        && !target_is_float;
    Some(if let Some(target) = target_place {
        SelectedInstructionKind::WritePlaceConvert {
            target,
            target_byte_size,
            source,
            source_byte_size,
            source_is_float,
            target_is_float,
            source_signed,
            target_signed,
            trapping,
            saturating,
        }
    } else {
        SelectedInstructionKind::WriteRuntimeStorageConvert {
            target_region,
            target_offset,
            target_byte_size,
            source,
            source_byte_size,
            source_is_float,
            target_is_float,
            source_signed,
            target_signed,
            trapping,
            saturating,
        }
    })
}
