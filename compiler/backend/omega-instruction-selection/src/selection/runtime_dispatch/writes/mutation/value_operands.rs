use crate::InstructionSelectionInput;
use crate::selection::bindings::{RuntimeAliasBinding, resolve_runtime_alias_binding};
use crate::selection::storage_places::{
    resolve_runtime_frame_base_indexed_target, resolve_runtime_frame_base_indexed_target_in_table,
    resolve_runtime_frame_fixed_indexed_target_in_table, resolve_runtime_frame_indexed_target,
    resolve_runtime_frame_indexed_target_in_table, resolve_runtime_pointee_slot_offset,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_place,
    resolve_runtime_storage_place_in_table,
};
use omega_abstract_operations::{RuntimeValueOperand, RuntimeValueOperandHandle};
use omega_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use omega_control_flow::StateKey;
use omega_core::arena::Arena;

use super::super::static_values::{
    RuntimeStaticValues, resolve_runtime_static_integer_value,
    resolve_runtime_static_integer_value_in_table,
};
use super::operators::{
    builtin_runtime_call_operator, builtin_runtime_call_operator_in_table, runtime_binary_operator,
    supports_runtime_value_operand,
};
use super::resolve_runtime_call_result_source_place;

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_runtime_value_operand_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<RuntimeValueOperandHandle> {
    if let Some(value) =
        resolve_runtime_static_integer_value_in_table(input, expressions, expression, static_values)
    {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Immediate(value)));
    }

    if let ExpressionNode::Binary(binary) = expressions.expression(expression) {
        let operator = runtime_binary_operator(binary.operator)?;
        let left = resolve_runtime_value_operand_in_table(
            input,
            dispatch_index,
            source_key,
            statement_index,
            expressions,
            binary.left,
            static_values,
            runtime_value_operands,
        )?;
        let right = resolve_runtime_value_operand_in_table(
            input,
            dispatch_index,
            source_key,
            statement_index,
            expressions,
            binary.right,
            static_values,
            runtime_value_operands,
        )?;
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Binary {
            left,
            operator,
            right,
        }));
    }

    if let ExpressionNode::Call(call) = expressions.expression(expression) {
        if let Some(operator) = builtin_runtime_call_operator_in_table(input, call) {
            let left = resolve_runtime_value_operand_in_table(
                input,
                dispatch_index,
                source_key,
                statement_index,
                expressions,
                expressions.expression_handle_at_offset(call.arguments, 0),
                static_values,
                runtime_value_operands,
            )?;
            let right = resolve_runtime_value_operand_in_table(
                input,
                dispatch_index,
                source_key,
                statement_index,
                expressions,
                expressions.expression_handle_at_offset(call.arguments, 1),
                static_values,
                runtime_value_operands,
            )?;
            return Some(runtime_value_operands.insert(RuntimeValueOperand::Binary {
                left,
                operator,
                right,
            }));
        }

        if let Some(place) = resolve_runtime_call_result_source_place(
            input,
            dispatch_index,
            source_key,
            statement_index,
        ) {
            return Some(runtime_value_operands.insert(RuntimeValueOperand::Storage {
                region: place.region,
                byte_offset: place.byte_offset,
                byte_size: place.byte_count,
            }));
        }
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

    let place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
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
        let left = resolve_runtime_value_operand(
            input,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            statement_index,
            &binary.left,
            aliases,
            alias_expressions,
            static_values,
            runtime_value_operands,
        )?;
        let right = resolve_runtime_value_operand(
            input,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            statement_index,
            &binary.right,
            aliases,
            alias_expressions,
            static_values,
            runtime_value_operands,
        )?;
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Binary {
            left,
            operator,
            right,
        }));
    }

    if let Expression::Call(call) = expression
        && let Some(operator) = builtin_runtime_call_operator(input, call)
    {
        let [left, right] = &*call.arguments else {
            return None;
        };
        let left = resolve_runtime_value_operand(
            input,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            statement_index,
            left,
            aliases,
            alias_expressions,
            static_values,
            runtime_value_operands,
        )?;
        let right = resolve_runtime_value_operand(
            input,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            statement_index,
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
        }));
    }

    if matches!(expression, Expression::Call(_))
        && let Some(place) = resolve_runtime_call_result_source_place(
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
                index_offset: indexed_target.index_offset,
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
