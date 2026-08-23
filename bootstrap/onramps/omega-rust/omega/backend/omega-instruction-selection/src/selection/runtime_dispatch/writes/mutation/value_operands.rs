use crate::InstructionSelectionInput;
use crate::selection::bindings::{RuntimeAliasBinding, resolve_runtime_alias_binding};
use crate::selection::storage_places::{
    RuntimeStoragePlace, RuntimeStoredIntegerSource, clamp_runtime_case_comparison_operands,
    clamp_runtime_case_comparison_operands_in_table, classify_scalar_value_type_in_table,
    combine_binary_operand_scalar_types, enum_variant_value_in_table,
    resolve_binary_operand_arithmetic_domain_in_table,
    resolve_binary_operation_arithmetic_domain_in_table,
    resolve_runtime_assignment_value_call_result_place_by_ordinal,
    resolve_runtime_frame_base_indexed_target, resolve_runtime_frame_base_indexed_target_in_table,
    resolve_runtime_frame_fixed_indexed_target_in_table, resolve_runtime_frame_indexed_target,
    resolve_runtime_frame_indexed_target_in_table, resolve_runtime_machine_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset, resolve_runtime_pointee_slot_offset_in_table,
    resolve_runtime_storage_place, resolve_runtime_storage_place_in_table,
    resolve_runtime_storage_place_is_fat_slice_in_table,
    resolve_runtime_stored_integer_projection_in_table,
};
use omega_abstract_operations::{
    RuntimeValueOperand, RuntimeValueOperandHandle, StateGuardOperator,
};
use omega_control_flow::StateKey;
use omega_state_calls::StateCallRole;
use psi_arena::Arena;
use psi_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use psi_checked_trees::statement::StatementNode;
use psi_checked_trees::types::PrimitiveType;
use psi_checked_trees::{CheckedValueOrigin, CheckedValueStatementRole};
use psi_numerics::bignum::BigInt;

use super::super::static_values::{
    RuntimeStaticValues, resolve_runtime_static_float_value_in_table,
    resolve_runtime_static_integer_value, resolve_runtime_static_integer_value_in_table,
};
use super::operators::{
    builtin_runtime_call_operator, builtin_runtime_call_operator_in_table,
    builtin_runtime_unary_call_operator_in_table, is_float_classification_predicate,
    runtime_binary_operator, supports_runtime_value_operand,
};
use super::{
    resolve_runtime_table_call_result_source_place,
    resolve_runtime_tree_call_result_source_place_in_expression,
};

/// Whether a binary value operand built from these two operand expressions is
/// floating-point, so the encoder uses the SSE unit (addsd/...) instead of an
/// integer add over the IEEE bits. A float operand is either a float-typed place
/// or a float literal; checking both operands covers `place OP literal` in any
/// order and `place OP place`.
pub(in crate::selection::runtime_dispatch) fn binary_value_operands_are_float(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    [left, right].into_iter().any(|operand| {
        classify_scalar_value_type_in_table(input, dispatch_index, source_key, expressions, operand)
            .is_some_and(|primitive| primitive.accepts_float_literal())
    })
}

/// The resolved scalar byte width of a binary value operand's result, computed
/// ONCE here from the operands' scalar types and threaded onto
/// `RuntimeValueOperand::Binary` so the float emission picks `addss` (4) vs
/// `addsd` (8) instead of hardcoding a width at the ISA. Defaults to 8 when the
/// operand types do not resolve (the prior behavior; the integer arm ignores
/// this and keeps its own operand-derived width).
pub(in crate::selection::runtime_dispatch) fn binary_value_operand_byte_width(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> usize {
    let left_type =
        classify_scalar_value_type_in_table(input, dispatch_index, source_key, expressions, left);
    let right_type =
        classify_scalar_value_type_in_table(input, dispatch_index, source_key, expressions, right);
    combine_binary_operand_scalar_types(left_type, right_type)
        .and_then(convert_scalar_byte_size)
        .unwrap_or(8)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_runtime_value_operand_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<RuntimeValueOperandHandle> {
    resolve_runtime_value_operand_in_table_with_root(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        expression,
        expression,
        None,
        static_values,
        runtime_value_operands,
    )
}

#[allow(clippy::too_many_arguments)]
fn resolve_runtime_value_operand_in_table_with_root(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    root_expression: ExpressionHandle,
    expression: ExpressionHandle,
    minimum_call_ordinal: Option<usize>,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<RuntimeValueOperandHandle> {
    if let Some(value) =
        resolve_runtime_static_integer_value_in_table(input, expressions, expression, static_values)
    {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Immediate(value)));
    }

    // A float literal operand carries its IEEE-754 bits in the immediate; the
    // float binary write moves those bits into an XMM register for the SSE op.
    // First cut resolves the f64 bit pattern (the default float width); f32
    // arithmetic with a constant operand is gated out at the write site.
    if let Some(float_value) = resolve_runtime_static_float_value_in_table(expressions, expression)
    {
        return Some(
            runtime_value_operands
                .insert(RuntimeValueOperand::Immediate(float_value.to_bits() as i64)),
        );
    }

    if let Some(operand) = resolve_selected_ternary_float_operand_in_table_with_root(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        root_expression,
        expression,
        minimum_call_ordinal,
        static_values,
        runtime_value_operands,
    ) {
        return Some(operand);
    }

    if let ExpressionNode::Binary(binary) = expressions.expression(expression) {
        // String `==` is CONTENT equality (length AND bytes), not a scalar
        // compare of descriptor words: when both sides are String-typed
        // descriptor places this becomes the dedicated text-equals leaf.
        if let Some(text_equals) = resolve_runtime_text_equals_operand_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            binary.operator,
            binary.left,
            binary.right,
            runtime_value_operands,
        ) {
            return Some(text_equals);
        }
        let operator = runtime_binary_operator(binary.operator)?;
        let left_expr = binary.left;
        let right_expr = binary.right;
        // Nested operands carry signedness too: `hi % 199` as a SUB-expression of
        // a larger value (a convert chain, an outer binary) must pick the unsigned
        // encoding exactly like a top-level binary write.
        let operator = super::binary_table_writes::signedness_adjusted_operator_for_operands(
            input,
            dispatch_index,
            source_key,
            expressions,
            left_expr,
            right_expr,
            operator,
        );
        let left = resolve_runtime_comparison_operand_in_table_with_root_and_call_ordinal(
            input,
            dispatch_index,
            source_key,
            statement_index,
            expressions,
            root_expression,
            left_expr,
            Some(binary.operator),
            right_expr,
            minimum_call_ordinal,
            static_values,
            runtime_value_operands,
        )?;
        let right = resolve_runtime_comparison_operand_in_table_with_root_and_call_ordinal(
            input,
            dispatch_index,
            source_key,
            statement_index,
            expressions,
            root_expression,
            right_expr,
            Some(binary.operator),
            left_expr,
            minimum_call_ordinal,
            static_values,
            runtime_value_operands,
        )?;
        let is_float = binary_value_operands_are_float(
            input,
            dispatch_index,
            source_key,
            expressions,
            left_expr,
            right_expr,
        );
        let byte_width = binary_value_operand_byte_width(
            input,
            dispatch_index,
            source_key,
            expressions,
            left_expr,
            right_expr,
        );
        // Normalize f32-literal immediates at the binary node that consumes
        // them. Doing this only at a float-valued write target misses nested
        // float comparisons inside a bool expression: the outer `&&` writes a
        // one-byte bool, while each comparison still executes its operands at
        // single precision. Without this local normalization the AArch64/x86
        // encoders read the low word of an f64 bit pattern as the f32 operand.
        if is_float && byte_width == 4 {
            super::binary_table_writes::narrow_f32_literal_operands(
                input,
                dispatch_index,
                source_key,
                runtime_value_operands,
                expressions,
                left_expr,
                left,
            );
            super::binary_table_writes::narrow_f32_literal_operands(
                input,
                dispatch_index,
                source_key,
                runtime_value_operands,
                expressions,
                right_expr,
                right,
            );
        }
        // A case-name equality (the lowered form of `in`) compares the TAG
        // only; the place operand must not read payload bytes.
        clamp_runtime_case_comparison_operands_in_table(
            &input.layouts,
            expressions,
            binary.operator,
            left_expr,
            right_expr,
            left,
            right,
            runtime_value_operands,
        );
        let domain_signedness = resolve_binary_operand_arithmetic_domain_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            left_expr,
            right_expr,
        );
        let arithmetic_domain = resolve_binary_operation_arithmetic_domain_in_table(
            input,
            dispatch_index,
            source_key,
            statement_index,
            expressions,
            expression,
            left_expr,
            right_expr,
            is_float,
            byte_width,
        )?;
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Binary {
            left,
            operator,
            right,
            is_float,
            byte_width,
            // Recorded so the Saturating/Trapping operand-position lowering
            // picks its width-correct op + clamp/trap bounds.
            arithmetic_domain,
            operands_signed: domain_signedness.1,
        }));
    }

    // A numeric `as` cast in operand position (`self.a + (self.b as f64)`): resolve
    // the source operand and wrap it in a Convert, which the encoder lowers to the
    // in-place conversion (cvttsd2si / cvtsi2sd / cvtsd2ss / movsxd).
    if let ExpressionNode::Cast(cast) = expressions.expression(expression) {
        let source_expression = cast.value;
        let target_primitive = input.program.primitive_type_reference(cast.target_type)?;
        let source_primitive = classify_scalar_value_type_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            source_expression,
        )?;
        let target_byte_size = convert_scalar_byte_size(target_primitive)?;
        let source_byte_size = convert_scalar_byte_size(source_primitive)?;
        let source = resolve_runtime_value_operand_in_table_with_root(
            input,
            dispatch_index,
            source_key,
            statement_index,
            expressions,
            root_expression,
            source_expression,
            minimum_call_ordinal,
            static_values,
            runtime_value_operands,
        )?;
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Convert {
            source,
            source_byte_size,
            target_byte_size,
            source_is_float: source_primitive.accepts_float_literal(),
            target_is_float: target_primitive.accepts_float_literal(),
            source_signed: source_primitive.is_signed_integer(),
            target_signed: target_primitive.is_signed_integer(),
            // F4: a Trapping float->int cast carries its trap guard.
            trapping: cast.domain == psi_numerics::arithmetic::ArithmeticDomain::Trapping
                && source_primitive.accepts_float_literal()
                && !target_primitive.accepts_float_literal(),
            saturating: cast.domain == psi_numerics::arithmetic::ArithmeticDomain::Saturating
                && source_primitive.accepts_float_literal()
                && !target_primitive.accepts_float_literal(),
        }));
    }

    if let ExpressionNode::Call(call) = expressions.expression(expression) {
        if let Some(operator) = builtin_runtime_unary_call_operator_in_table(input, call) {
            let operand_expression = expressions.expression_handle_at_offset(call.arguments, 0);
            let left = resolve_runtime_value_operand_in_table_with_root(
                input,
                dispatch_index,
                source_key,
                statement_index,
                expressions,
                root_expression,
                operand_expression,
                minimum_call_ordinal,
                static_values,
                runtime_value_operands,
            )?;
            let byte_width = binary_value_operand_byte_width(
                input,
                dispatch_index,
                source_key,
                expressions,
                operand_expression,
                operand_expression,
            );
            let right = if is_float_classification_predicate(operator) {
                // Static metadata carrier, not another evaluation: direct
                // bool-target writes need the source float width even when
                // the authored operand folds to an untyped immediate.
                runtime_value_operands.insert(RuntimeValueOperand::Immediate(byte_width as i64))
            } else {
                // Existing sqrt representation; the encoder reads its right
                // float register. IsNan deliberately does not duplicate this.
                resolve_runtime_value_operand_in_table_with_root(
                    input,
                    dispatch_index,
                    source_key,
                    statement_index,
                    expressions,
                    root_expression,
                    operand_expression,
                    minimum_call_ordinal,
                    static_values,
                    runtime_value_operands,
                )?
            };
            let is_float = binary_value_operands_are_float(
                input,
                dispatch_index,
                source_key,
                expressions,
                operand_expression,
                operand_expression,
            );
            if is_float_classification_predicate(operator) && byte_width == 4 {
                super::binary_table_writes::narrow_f32_literal_operands(
                    input,
                    dispatch_index,
                    source_key,
                    runtime_value_operands,
                    expressions,
                    operand_expression,
                    left,
                );
            }
            let arithmetic_domain = resolve_binary_operation_arithmetic_domain_in_table(
                input,
                dispatch_index,
                source_key,
                statement_index,
                expressions,
                expression,
                operand_expression,
                operand_expression,
                is_float,
                byte_width,
            )?;
            return Some(runtime_value_operands.insert(RuntimeValueOperand::Binary {
                left,
                operator,
                right,
                is_float,
                byte_width,
                arithmetic_domain,
                operands_signed: true,
            }));
        }

        if let Some(operator) = builtin_runtime_call_operator_in_table(input, call) {
            let left_expr = expressions.expression_handle_at_offset(call.arguments, 0);
            let right_expr = expressions.expression_handle_at_offset(call.arguments, 1);
            // min/max builtins compare their operands, so they carry the same
            // signedness sensitivity as a binary comparison.
            let operator = super::binary_table_writes::signedness_adjusted_operator_for_operands(
                input,
                dispatch_index,
                source_key,
                expressions,
                left_expr,
                right_expr,
                operator,
            );
            let left = resolve_runtime_value_operand_in_table_with_root(
                input,
                dispatch_index,
                source_key,
                statement_index,
                expressions,
                root_expression,
                left_expr,
                minimum_call_ordinal,
                static_values,
                runtime_value_operands,
            )?;
            let right = resolve_runtime_value_operand_in_table_with_root(
                input,
                dispatch_index,
                source_key,
                statement_index,
                expressions,
                root_expression,
                right_expr,
                minimum_call_ordinal,
                static_values,
                runtime_value_operands,
            )?;
            let is_float = binary_value_operands_are_float(
                input,
                dispatch_index,
                source_key,
                expressions,
                left_expr,
                right_expr,
            );
            let byte_width = binary_value_operand_byte_width(
                input,
                dispatch_index,
                source_key,
                expressions,
                left_expr,
                right_expr,
            );
            let arithmetic_domain = if is_float {
                resolve_binary_operation_arithmetic_domain_in_table(
                    input,
                    dispatch_index,
                    source_key,
                    statement_index,
                    expressions,
                    expression,
                    left_expr,
                    right_expr,
                    true,
                    byte_width,
                )?
            } else {
                psi_numerics::arithmetic::ArithmeticDomain::Exact
            };
            return Some(runtime_value_operands.insert(RuntimeValueOperand::Binary {
                left,
                operator,
                right,
                is_float,
                byte_width,
                // min/max SELECTS one operand -- no overflow exists for a
                // domain to clamp/trap. Signedness already rode the operator.
                arithmetic_domain,
                operands_signed: true,
            }));
        }

        if let Some(place) = resolve_runtime_table_call_result_source_place(
            input,
            dispatch_index,
            source_key,
            statement_index,
            expressions,
            root_expression,
            expression,
            call,
            minimum_call_ordinal,
        ) {
            return Some(runtime_value_operands.insert(RuntimeValueOperand::Storage {
                region: place.region,
                byte_offset: place.byte_offset,
                byte_size: place.byte_count,
            }));
        }
    }

    if let Some(operand) = resolve_runtime_stored_integer_operand_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
        runtime_value_operands,
    ) {
        return Some(operand);
    }

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) && supports_runtime_value_operand(pointer_target.pointee_byte_size)
    {
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
                index_region: indexed_target.index_region,
                index_offset: indexed_target.index_offset,
                index_byte_size: indexed_target.index_byte_size,
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
                index_byte_size: indexed_target.index_byte_size,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                byte_size: indexed_target.byte_count,
            }),
        );
    }

    if let Some(indexed_target) = resolve_runtime_machine_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(
            runtime_value_operands.insert(RuntimeValueOperand::MachineIndexed {
                base_byte_offset: indexed_target.base_byte_offset,
                index_region: indexed_target.index_region,
                index_offset: indexed_target.index_offset,
                index_byte_size: indexed_target.index_byte_size,
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

    let place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )?;
    if std::env::var_os("OMEGA_DEBUG_RECEIVER").is_some()
        && place.region == omega_target_operations::RuntimeStorageRegion::Machine
    {
        eprintln!(
            "VOP place: dispatch {} source m{} s{} expr `{}` -> Machine@{}+{}",
            dispatch_index,
            source_key.machine.arena_index(),
            source_key.state.arena_index(),
            expressions.display_name(expression),
            place.byte_offset,
            place.byte_count,
        );
    }
    if !supports_runtime_value_operand(place.byte_count) {
        return None;
    }
    Some(runtime_value_operands.insert(RuntimeValueOperand::Storage {
        region: place.region,
        byte_offset: place.byte_offset,
        byte_size: place.byte_count,
    }))
}

pub(crate) fn resolve_runtime_stored_integer_operand_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<RuntimeValueOperandHandle> {
    let projection = resolve_runtime_stored_integer_projection_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )?;
    if !supports_runtime_value_operand(projection.stored_byte_count)
        || !supports_runtime_value_operand(projection.carrier_byte_count)
    {
        return None;
    }
    let source = match projection.source {
        RuntimeStoredIntegerSource::Direct {
            region,
            byte_offset,
        } => runtime_value_operands.insert(RuntimeValueOperand::Storage {
            region,
            byte_offset,
            byte_size: projection.stored_byte_count,
        }),
        RuntimeStoredIntegerSource::Pointee {
            pointer_byte_offset,
            field_byte_offset,
        } => runtime_value_operands.insert(RuntimeValueOperand::Pointee {
            pointer_byte_offset,
            field_byte_offset,
            byte_size: projection.stored_byte_count,
        }),
        RuntimeStoredIntegerSource::FrameIndexed {
            descriptor_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        } => runtime_value_operands.insert(RuntimeValueOperand::FrameIndexed {
            descriptor_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size: projection.stored_byte_count,
        }),
        RuntimeStoredIntegerSource::FrameBaseIndexed {
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        } => runtime_value_operands.insert(RuntimeValueOperand::FrameBaseIndexed {
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size: projection.stored_byte_count,
        }),
        RuntimeStoredIntegerSource::MachineIndexed {
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        } => runtime_value_operands.insert(RuntimeValueOperand::MachineIndexed {
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size: projection.stored_byte_count,
        }),
    };
    Some(runtime_value_operands.insert(RuntimeValueOperand::Convert {
        source,
        source_byte_size: projection.stored_byte_count,
        target_byte_size: projection.carrier_byte_count,
        source_is_float: false,
        target_is_float: false,
        source_signed: matches!(
            projection.interpretation,
            psi_layout_plans::IntegerInterpretation::Signed
        ),
        target_signed: projection.carrier_signed,
        trapping: false,
        saturating: false,
    }))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn select_runtime_stored_integer_projection_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    target_region: omega_abstract_operations::RuntimeStorageRegion,
    target_offset: usize,
    target_byte_size: usize,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<omega_abstract_operations::SelectedInstructionKind> {
    let source = resolve_runtime_stored_integer_operand_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
        runtime_value_operands,
    )?;
    let RuntimeValueOperand::Convert {
        target_byte_size: carrier_byte_size,
        target_signed,
        ..
    } = runtime_value_operands.get(source)
    else {
        return None;
    };
    if *carrier_byte_size != target_byte_size {
        return None;
    }
    Some(
        omega_abstract_operations::SelectedInstructionKind::WriteRuntimeStorageConvert {
            target_region,
            target_offset,
            target_byte_size,
            source,
            source_byte_size: target_byte_size,
            source_is_float: false,
            target_is_float: false,
            source_signed: *target_signed,
            target_signed: *target_signed,
            trapping: false,
            saturating: false,
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn select_runtime_stored_integer_mutation_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target_expression: ExpressionHandle,
    value_expression: ExpressionHandle,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<omega_abstract_operations::SelectedInstructionKind> {
    let projection = resolve_runtime_stored_integer_projection_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target_expression,
    )?;
    if !projection.write_is_total
        && !stored_integer_write_value_is_proved_fit(
            input,
            value_source_key,
            statement_index,
            expressions,
            value_expression,
            static_values,
            projection.stored_byte_count,
            projection.interpretation,
        )
    {
        return None;
    }
    let source_primitive = classify_scalar_value_type_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value_expression,
    )?;
    let source_byte_size = convert_scalar_byte_size(source_primitive)?;
    if source_byte_size != projection.carrier_byte_count {
        return None;
    }
    let source = resolve_runtime_value_operand_in_table(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        value_expression,
        static_values,
        runtime_value_operands,
    )?;
    let target_signed = matches!(
        projection.interpretation,
        psi_layout_plans::IntegerInterpretation::Signed
    );
    Some(match projection.source {
        RuntimeStoredIntegerSource::Direct {
            region,
            byte_offset,
        } => omega_abstract_operations::SelectedInstructionKind::WriteRuntimeStorageConvert {
            target_region: region,
            target_offset: byte_offset,
            target_byte_size: projection.stored_byte_count,
            source,
            source_byte_size,
            source_is_float: false,
            target_is_float: false,
            source_signed: source_primitive.is_signed_integer(),
            target_signed,
            trapping: false,
            saturating: false,
        },
        ref target_source => {
            omega_abstract_operations::SelectedInstructionKind::WritePlaceConvert {
                target: target_source.as_place()?,
                target_byte_size: projection.stored_byte_count,
                source,
                source_byte_size,
                source_is_float: false,
                target_is_float: false,
                source_signed: source_primitive.is_signed_integer(),
                target_signed,
                trapping: false,
                saturating: false,
            }
        }
    })
}

/// Admit a non-total `IntegerAt` mutation only from proof material Psi has
/// already checked. An exact compile-time integer is its own witness. A runtime
/// value must carry a checked declaration range at this exact assignment site,
/// and that complete range must fit the physical encoding. Unknown, flow-only,
/// or unbounded values remain rejected; Omega never invents a qualification.
#[allow(clippy::too_many_arguments)]
fn stored_integer_write_value_is_proved_fit(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    value_expression: ExpressionHandle,
    static_values: &RuntimeStaticValues,
    stored_byte_count: usize,
    interpretation: psi_layout_plans::IntegerInterpretation,
) -> bool {
    let Some((stored_minimum, stored_maximum)) =
        stored_integer_range(stored_byte_count, interpretation)
    else {
        return false;
    };

    if let Some(value) = resolve_runtime_static_integer_value_in_table(
        input,
        expressions,
        value_expression,
        static_values,
    ) {
        let value = BigInt::from_i64(value);
        return value >= stored_minimum && value <= stored_maximum;
    }

    let proved_range = input
        .program
        .facts
        .values
        .values
        .iter()
        .find_map(|(_, value)| match value.origin {
            CheckedValueOrigin::StateStatement {
                machine_symbol,
                state_symbol,
                statement_index: value_statement_index,
                role: CheckedValueStatementRole::AssignmentValue,
            } if machine_symbol == source_key.machine
                && state_symbol == source_key.state
                && value_statement_index == statement_index =>
            {
                value.integer_range.as_ref()
            }
            _ => None,
        });
    let Some(range) = proved_range else {
        return false;
    };
    range.minimum >= stored_minimum && range.maximum <= stored_maximum
}

fn stored_integer_range(
    stored_byte_count: usize,
    interpretation: psi_layout_plans::IntegerInterpretation,
) -> Option<(BigInt, BigInt)> {
    let bit_count = stored_byte_count.checked_mul(8)?;
    if !(1..=64).contains(&bit_count) {
        return None;
    }
    Some(match interpretation {
        psi_layout_plans::IntegerInterpretation::Signed => {
            let magnitude = 1_i128.checked_shl(u32::try_from(bit_count - 1).ok()?)?;
            (
                BigInt::from_i128(-magnitude),
                BigInt::from_i128(magnitude - 1),
            )
        }
        psi_layout_plans::IntegerInterpretation::Unsigned => {
            let cardinality = 1_i128.checked_shl(u32::try_from(bit_count).ok()?)?;
            (BigInt::zero(), BigInt::from_i128(cardinality - 1))
        }
    })
}

/// Reify a selected named multiply-then-add or fused-multiply-add compiler call
/// as one ternary runtime operand. Its unnameable, format-specific builtin
/// symbol survives
/// state-local expression copying; flattening its three authored operands here
/// lets native policy adaptation inspect all of them without evaluating any
/// operand twice.
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_selected_ternary_float_operand_in_table_with_root(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    root_expression: ExpressionHandle,
    expression: ExpressionHandle,
    minimum_call_ordinal: Option<usize>,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<RuntimeValueOperandHandle> {
    let ExpressionNode::Call(call) = expressions.expression(expression) else {
        return None;
    };
    let (byte_width, ternary_operator) =
        super::operators::builtin_runtime_ternary_float_call_operator_in_table(input, call)?;
    let [first_expression, second_expression, third_expression] =
        [0, 1, 2].map(|offset| expressions.expression_handle_at_offset(call.arguments, offset));
    let first = resolve_runtime_value_operand_in_table_with_root(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        root_expression,
        first_expression,
        minimum_call_ordinal,
        static_values,
        runtime_value_operands,
    )?;
    let second = resolve_runtime_value_operand_in_table_with_root(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        root_expression,
        second_expression,
        minimum_call_ordinal,
        static_values,
        runtime_value_operands,
    )?;
    let third = resolve_runtime_value_operand_in_table_with_root(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        root_expression,
        third_expression,
        minimum_call_ordinal,
        static_values,
        runtime_value_operands,
    )?;
    if byte_width == 4 {
        for (expression, operand) in [
            (first_expression, first),
            (second_expression, second),
            (third_expression, third),
        ] {
            super::binary_table_writes::narrow_f32_literal_operands(
                input,
                dispatch_index,
                source_key,
                runtime_value_operands,
                expressions,
                expression,
                operand,
            );
        }
    }
    let domain = resolve_binary_operation_arithmetic_domain_in_table(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        expression,
        first_expression,
        third_expression,
        true,
        byte_width,
    )?;
    let pair = runtime_value_operands.insert(RuntimeValueOperand::Binary {
        left: second,
        operator: StateGuardOperator::FloatPair,
        right: third,
        is_float: true,
        byte_width,
        arithmetic_domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
        operands_signed: true,
    });
    Some(runtime_value_operands.insert(RuntimeValueOperand::Binary {
        left: first,
        operator: ternary_operator,
        right: pair,
        is_float: true,
        byte_width,
        arithmetic_domain: domain,
        operands_signed: true,
    }))
}

/// Resolve a `String == String` compare to the dedicated `TextEquals` value
/// operand: text equality is CONTENT equality (length AND bytes), so the
/// generic scalar path -- which would compare 8 of the descriptor's 16 bytes,
/// i.e. the POINTERS -- must never see it. Applies only when BOTH sides are
/// String-typed runtime storage places (the synthesized member reads of
/// Equatable structural equality, plain field/local reads); other String
/// shapes (literals, slice elements) return `None` and surface as a hard
/// "needs runtime value lowering" emission blocker instead of comparing
/// descriptor words. `!=` is the negated leaf: the state-values simplifier
/// De-Morgans the Equatable expansion's `equality == false` spelling into
/// per-field `!=` compares, so the String term reaches selection as a direct
/// `String != String` binary -- it lowers as `text_equals(..) == 0`.
#[allow(clippy::too_many_arguments)]
pub(in crate::selection::runtime_dispatch) fn resolve_runtime_text_equals_operand_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    operator: psi_checked_trees::expression::BinaryOperator,
    left_expression: ExpressionHandle,
    right_expression: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<RuntimeValueOperandHandle> {
    let negated = match operator {
        psi_checked_trees::expression::BinaryOperator::Equal => false,
        psi_checked_trees::expression::BinaryOperator::NotEqual => true,
        _ => return None,
    };
    // One side an INLINE STRING LITERAL (`self.okf = self.name == "omega"`):
    // guard position already lowers this shape via `TextEqualsLiteral`, but in
    // value/write position it fell through to the generic scalar path (whose
    // 16-byte descriptor load never resolves) and the machine-owned store
    // blocker refused it -- the field-store leg of the texteq matrix, while
    // the `let` leg rode the frame-slot text-comparison writer and the
    // place==place field store rode `TextEquals` below. Build the literal
    // leaf here so all four legs lower identically. Owned `[u8; N]` carriers
    // use their inline `{len, bytes}` place rather than the descriptor path.
    let literal_pairing = match (
        expressions.expression(left_expression),
        expressions.expression(right_expression),
    ) {
        (ExpressionNode::String(literal), _) => Some((right_expression, literal.clone())),
        (_, ExpressionNode::String(literal)) => Some((left_expression, literal.clone())),
        _ => None,
    };
    let text_equals = if let Some((place_expression, literal)) = literal_pairing {
        let (place, place_is_bounded_buffer) = if let Some(place) =
            crate::selection::runtime_dispatch::guards::resolve_runtime_bounded_byte_buffer_place_operand_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                place_expression,
                runtime_value_operands,
            )
        {
            (place, true)
        } else {
            let place = crate::selection::runtime_dispatch::guards::resolve_runtime_text_descriptor_place_operand_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                place_expression,
                runtime_value_operands,
            )?;
            (place, false)
        };
        runtime_value_operands.insert(RuntimeValueOperand::TextEqualsLiteral {
            place,
            literal,
            place_is_bounded_buffer,
        })
    } else {
        // A `&[u8] in Utf8` view uses the TextEquals leaf (length + bounded byte
        // loop). Recognize such a fat-slice-descriptor place, or a
        // `&[u8] in Utf8 == &[u8] in Utf8` compare falls to the generic scalar path,
        // whose 16-byte runtime-operand load the encoder rejects loudly.
        let operand_is_text = |expression: ExpressionHandle| {
            resolve_runtime_storage_place_is_fat_slice_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                expression,
            ) || crate::selection::storage_places::resolve_runtime_storage_place_is_bounded_byte_buffer_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                expression,
            )
        };
        if !operand_is_text(left_expression) || !operand_is_text(right_expression) {
            return None;
        }
        let left_place = resolve_runtime_storage_place_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            left_expression,
        )?;
        let right_place = resolve_runtime_storage_place_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            right_expression,
        )?;
        let left_is_bounded_buffer = crate::selection::storage_places::resolve_runtime_storage_place_is_bounded_byte_buffer_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            left_expression,
        );
        let right_is_bounded_buffer = crate::selection::storage_places::resolve_runtime_storage_place_is_bounded_byte_buffer_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            right_expression,
        );
        debug_assert!(left_is_bounded_buffer || left_place.byte_count == 16);
        debug_assert!(right_is_bounded_buffer || right_place.byte_count == 16);
        runtime_value_operands.insert(RuntimeValueOperand::TextEquals {
            left_region: left_place.region,
            left_offset: left_place.byte_offset,
            left_is_bounded_buffer,
            right_region: right_place.region,
            right_offset: right_place.byte_offset,
            right_is_bounded_buffer,
        })
    };
    if !negated {
        return Some(text_equals);
    }
    // `!=`: invert the 0/1 text-equals result inside the operand tree
    // (`text_equals == 0`), keeping the leaf shape every consumer already
    // encodes.
    let zero = runtime_value_operands.insert(RuntimeValueOperand::Immediate(0));
    Some(runtime_value_operands.insert(RuntimeValueOperand::Binary {
        left: text_equals,
        operator: omega_abstract_operations::StateGuardOperator::Equal,
        right: zero,
        is_float: false,
        // Integer 0/1 bool compare; width is unread by the integer emission arm.
        byte_width: 1,
        // A synthesized 0/1 bool compare cannot overflow.
        arithmetic_domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
        operands_signed: false,
    }))
}

/// Resolve one operand of a TAG comparison: when the OTHER side of an
/// `==`/`!=` names a CASE (the lowered form of `in`, payload-less case
/// `==`, and the tag guards of synthesized structural equality), the place
/// side reads only its 4-byte tag prefix. Resolving the full enum value
/// would fail for sums whose payload exceeds one scalar (e.g. a two-field
/// case, 12 bytes) and silently drop the whole write.
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_runtime_comparison_operand_in_table_with_root(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    root_expression: ExpressionHandle,
    expression: ExpressionHandle,
    comparison_operator: Option<psi_checked_trees::expression::BinaryOperator>,
    other_expression: ExpressionHandle,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<RuntimeValueOperandHandle> {
    resolve_runtime_comparison_operand_in_table_with_root_and_call_ordinal(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        root_expression,
        expression,
        comparison_operator,
        other_expression,
        None,
        static_values,
        runtime_value_operands,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_runtime_comparison_operand_in_table_with_root_and_call_ordinal(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    root_expression: ExpressionHandle,
    expression: ExpressionHandle,
    comparison_operator: Option<psi_checked_trees::expression::BinaryOperator>,
    other_expression: ExpressionHandle,
    minimum_call_ordinal: Option<usize>,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<RuntimeValueOperandHandle> {
    if matches!(
        comparison_operator,
        Some(
            psi_checked_trees::expression::BinaryOperator::Equal
                | psi_checked_trees::expression::BinaryOperator::NotEqual
        )
    ) && enum_variant_value_in_table(&input.layouts, expressions, other_expression).is_some()
        && let Some(place) = resolve_runtime_storage_place_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            expression,
        )
        && !supports_runtime_value_operand(place.byte_count)
    {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Storage {
            region: place.region,
            byte_offset: place.byte_offset,
            byte_size: omega_layout::ENUM_TAG_BYTES,
        }));
    }

    resolve_runtime_value_operand_in_table_with_root(
        input,
        dispatch_index,
        source_key,
        statement_index,
        expressions,
        root_expression,
        expression,
        minimum_call_ordinal,
        static_values,
        runtime_value_operands,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_runtime_value_operand(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    expression: &Expression,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<RuntimeValueOperandHandle> {
    resolve_runtime_value_operand_with_root(
        input,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        statement_index,
        expression,
        expression,
        aliases,
        alias_expressions,
        static_values,
        runtime_value_operands,
    )
}

#[allow(clippy::too_many_arguments)]
fn resolve_runtime_value_operand_with_root(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    root_expression: &Expression,
    expression: &Expression,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<RuntimeValueOperandHandle> {
    // `(arr[a..b]).len` of a literal-bounded fixed-array subslice is the window
    // length `b - a` -- a compile-time constant. (Such a subslice is often bound
    // to a `&[u8]` local that gets inlined to this expression and never
    // materialized, so a place read has no descriptor slot and would yield
    // garbage; fold it directly.)
    if let Expression::Member(member) = expression
        && member.member.as_str() == "len"
        && let Some(length) = super::super::subslice_copy::fixed_array_subslice_length(
            input,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            &member.receiver,
        )
        && let Ok(length) = i64::try_from(length)
    {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Immediate(length)));
    }
    if let Some(value) = resolve_runtime_static_integer_value(
        input,
        source_key,
        expression,
        aliases,
        alias_expressions,
        static_values,
    ) {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Immediate(value)));
    }

    if let Expression::Binary(binary) = expression {
        let operator = runtime_binary_operator(binary.operator)?;
        // Same signedness policy as the `_in_table` nested-binary path above
        // (operand-position: no write target to fall back to).
        let operator = super::binary_table_writes::signedness_adjusted_operator_for_tree_operands(
            input,
            dispatch_index,
            source_key,
            &binary.left,
            &binary.right,
            operator,
        );
        let left = resolve_runtime_value_operand_with_root(
            input,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            statement_index,
            root_expression,
            &binary.left,
            aliases,
            alias_expressions,
            static_values,
            runtime_value_operands,
        )?;
        let right = resolve_runtime_value_operand_with_root(
            input,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            statement_index,
            root_expression,
            &binary.right,
            aliases,
            alias_expressions,
            static_values,
            runtime_value_operands,
        )?;
        // A case-name equality (the lowered form of `in`) compares the TAG
        // only; the place operand must not read payload bytes.
        clamp_runtime_case_comparison_operands(
            &input.layouts,
            binary.operator,
            &binary.left,
            &binary.right,
            left,
            right,
            runtime_value_operands,
        );
        // Domain witness through a DELEGATED table (this fallback path works
        // on expression TREES; the domain resolver is table-shaped).
        let mut domain_expressions = ExpressionTable::default();
        let domain_left = domain_expressions.insert_tree(&binary.left);
        let domain_right = domain_expressions.insert_tree(&binary.right);
        let domain_signedness = resolve_binary_operand_arithmetic_domain_in_table(
            input,
            dispatch_index,
            source_key,
            &domain_expressions,
            domain_left,
            domain_right,
        );
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Binary {
            left,
            operator,
            right,
            // Non-table value-operand path: float detection not wired here yet;
            // float arithmetic via this fallback stays a known gap.
            is_float: false,
            // The plain integer arm ignores the width; a non-Exact
            // operand needs its REAL operand width for the width-correct op +
            // clamp bounds.
            byte_width: if domain_signedness.0 != psi_numerics::arithmetic::ArithmeticDomain::Exact
            {
                crate::selection::runtime_dispatch::guards::runtime_value_compare_byte_size(
                    runtime_value_operands,
                    left,
                    right,
                )
            } else {
                8
            },
            // Recorded so the Saturating/Trapping operand-position lowering
            // picks its width-correct op + clamp/trap bounds.
            arithmetic_domain: domain_signedness.0,
            operands_signed: domain_signedness.1,
        }));
    }

    if let Expression::Call(call) = expression
        && let Some(operator) = builtin_runtime_call_operator(input, call)
    {
        let [left, right] = &*call.arguments else {
            return None;
        };
        // min/max builtins compare their operands -- same signedness policy as
        // the `_in_table` builtin-call path (operand-position: no write target
        // to fall back to).
        let operator = super::binary_table_writes::signedness_adjusted_operator_for_tree_operands(
            input,
            dispatch_index,
            source_key,
            left,
            right,
            operator,
        );
        let left = resolve_runtime_value_operand_with_root(
            input,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            statement_index,
            root_expression,
            left,
            aliases,
            alias_expressions,
            static_values,
            runtime_value_operands,
        )?;
        let right = resolve_runtime_value_operand_with_root(
            input,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            statement_index,
            root_expression,
            right,
            aliases,
            alias_expressions,
            static_values,
            runtime_value_operands,
        )?;
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Binary {
            left,
            operator,
            right,
            // Non-table value-operand path: float detection not wired here yet;
            // float arithmetic via this fallback stays a known gap.
            is_float: false,
            // Integer arm derives its own width; default 8 matches prior behavior.
            byte_width: 8,
            // min/max SELECTS one operand -- no overflow exists for a
            // domain to clamp/trap. Signedness already rode the operator.
            arithmetic_domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            operands_signed: true,
        }));
    }

    if matches!(expression, Expression::Call(_))
        && let Some(place) = resolve_prior_local_call_result_source_place(
            input,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            statement_index,
            expression,
            aliases,
            alias_expressions,
        )
    {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Storage {
            region: place.region,
            byte_offset: place.byte_offset,
            byte_size: place.byte_count,
        }));
    }

    if let Expression::Call(call) = expression
        && let Some(place) = resolve_runtime_tree_call_result_source_place_in_expression(
            input,
            dispatch_index,
            source_key,
            statement_index,
            Some(root_expression),
            call,
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
        && supports_runtime_value_operand(pointer_target.pointee_byte_size)
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
                index_region: indexed_target.index_region,
                index_offset: indexed_target.index_offset,
                index_byte_size: indexed_target.index_byte_size,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                byte_size: indexed_target.byte_count,
            }),
        );
    }

    if let Some(indexed_target) =
        resolve_runtime_frame_base_indexed_target(input, dispatch_index, source_key, expression)
    {
        return Some(
            runtime_value_operands.insert(RuntimeValueOperand::FrameBaseIndexed {
                base_byte_offset: indexed_target.base_byte_offset,
                index_offset: indexed_target.index_offset,
                index_byte_size: indexed_target.index_byte_size,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                byte_size: indexed_target.byte_count,
            }),
        );
    }

    let resolved =
        resolve_runtime_alias_binding(expression, source_key, aliases, alias_expressions);
    let place = resolve_runtime_storage_place(
        input,
        dispatch_index,
        resolved.source_key,
        source_machine,
        source_state,
        &resolved.expression,
    )?;
    if !supports_runtime_value_operand(place.byte_count) {
        return None;
    }
    Some(runtime_value_operands.insert(RuntimeValueOperand::Storage {
        region: place.region,
        byte_offset: place.byte_offset,
        byte_size: place.byte_count,
    }))
}

fn resolve_prior_local_call_result_source_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    expression: &Expression,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
) -> Option<RuntimeStoragePlace> {
    let Expression::Call(_) = expression else {
        return None;
    };

    let mut candidate_states = Vec::new();
    if let Some(candidate) = program_state_statements_by_symbol(input, source_key) {
        candidate_states.push(candidate);
    }
    if let Some((machine_name, state_name)) = input.control_flow.state_names_by_key(source_key)
        && let Some(candidate) =
            program_state_statements_by_name(input, source_key, machine_name, state_name)
    {
        candidate_states.push(candidate);
    }
    if let Some(candidate) =
        program_state_statements_by_display_name(input, source_key, source_machine, source_state)
    {
        candidate_states.push(candidate);
    }

    for (local_source_key, statements) in candidate_states {
        for (local_statement_index, statement) in
            statements.iter().enumerate().take(statement_index)
        {
            let StatementNode::LocalData(local_data) = statement else {
                continue;
            };
            if !local_data.initial_value.is_valid() {
                continue;
            }

            let initializer = input
                .program
                .expression_table
                .to_tree(local_data.initial_value);
            let resolved_initializer =
                resolve_runtime_alias_binding(&initializer, source_key, aliases, alias_expressions);
            if !prior_local_call_initializer_matches(&resolved_initializer.expression, expression) {
                continue;
            }

            for state_call in input
                .state_calls
                .calls_for_statement(local_source_key, local_statement_index)
                .filter(|state_call| state_call.role == StateCallRole::AssignmentValue)
            {
                if let Some(place) = resolve_runtime_assignment_value_call_result_place_by_ordinal(
                    input,
                    dispatch_index,
                    local_source_key,
                    local_statement_index,
                    state_call.call_ordinal,
                ) {
                    return Some(place);
                }
            }
        }
    }

    None
}

fn prior_local_call_initializer_matches(initializer: &Expression, expression: &Expression) -> bool {
    if initializer == expression {
        return true;
    }

    let (Expression::Call(initializer), Expression::Call(expression)) = (initializer, expression)
    else {
        return false;
    };

    initializer.target_symbol == expression.target_symbol
        && initializer.target == expression.target
        && initializer.receiver == expression.receiver
        && initializer.arguments.len() == expression.arguments.len()
}

fn program_state_statements_by_name<'a>(
    input: &'a InstructionSelectionInput<'_>,
    source_key: StateKey,
    machine_name: &psi_checked_trees::name::Identifier,
    state_name: &psi_checked_trees::name::Identifier,
) -> Option<(StateKey, &'a [StatementNode])> {
    for machine in input.program.machines() {
        if machine.name != *machine_name {
            continue;
        }
        for state in input.program.machine_states(machine) {
            if state.name != *state_name {
                continue;
            }
            let local_source_key = StateKey {
                machine: machine.symbol,
                state: state.symbol,
                segment_index: source_key.segment_index,
            };
            let statements = input
                .program
                .statement_table
                .statements(state.statement_nodes);
            return Some((local_source_key, statements));
        }
    }

    None
}

fn program_state_statements_by_display_name<'a>(
    input: &'a InstructionSelectionInput<'_>,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
) -> Option<(StateKey, &'a [StatementNode])> {
    for machine in input.program.machines() {
        let machine_name = machine.name.to_string();
        if source_machine != machine_name
            && !source_machine.starts_with(&format!("{machine_name}::"))
        {
            continue;
        }

        for state in input.program.machine_states(machine) {
            let state_name = state.name.to_string();
            if state_name != source_state {
                continue;
            }
            let local_source_key = StateKey {
                machine: machine.symbol,
                state: state.symbol,
                segment_index: source_key.segment_index,
            };
            let statements = input
                .program
                .statement_table
                .statements(state.statement_nodes);
            return Some((local_source_key, statements));
        }
    }

    None
}

fn program_state_statements_by_symbol<'a>(
    input: &'a InstructionSelectionInput<'_>,
    source_key: StateKey,
) -> Option<(StateKey, &'a [StatementNode])> {
    let machine = input
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == source_key.machine)?;
    let state = input
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == source_key.state)?;
    let statements = input
        .program
        .statement_table
        .statements(state.statement_nodes);
    Some((source_key, statements))
}

/// Byte size of a scalar primitive for a Convert operand (1/4/8), or `None` for
/// non-scalar (e.g. `String`).
fn convert_scalar_byte_size(primitive: PrimitiveType) -> Option<usize> {
    primitive.scalar_byte_size()
}
