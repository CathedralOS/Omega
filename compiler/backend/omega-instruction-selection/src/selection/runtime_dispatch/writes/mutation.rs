use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{
    RuntimeStorageRegion, RuntimeValueOperand, RuntimeValueOperandHandle, SelectedInstruction,
    SelectedInstructionKind, StateGuardOperator,
};
use omega_checked_trees::expression::{
    BinaryOperator, Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
    TableCallExpression, TableNamePath,
};
use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::BuiltinFunction;
use omega_state_calls::StateCallRole;

use super::super::super::bindings::{
    RuntimeAliasBinding, RuntimeAliasBuffer, append_place_suffix, resolve_runtime_alias_binding,
    resolve_runtime_alias_binding_handle, strip_mutable_expression,
};
use super::super::super::storage_places::resolve_runtime_storage_place;
use super::super::super::storage_places::{
    resolve_fixed_array_length_in_table, resolve_runtime_assignment_value_call_result_place,
    resolve_runtime_frame_fixed_indexed_target_in_table, resolve_runtime_frame_indexed_target,
    resolve_runtime_frame_indexed_target_in_table, resolve_runtime_machine_indexed_target_in_table,
    resolve_runtime_pointee_fixed_indexed_target_in_table, resolve_runtime_pointee_slot_offset,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_place_in_table,
    resolve_runtime_transition_argument_call_result_place,
};
use super::super::text_writes::{
    runtime_text_builder_write_in_table_emit, runtime_text_builder_write_with_scratch_emit,
    select_runtime_string_descriptor_write, string_literal_data_handle,
};
use super::static_values::{
    RuntimeStaticValues, resolve_runtime_static_integer_value,
    resolve_runtime_static_integer_value_in_table, set_runtime_static_value,
    set_runtime_static_value_in_table,
};
use super::storage_copy::runtime_storage_copy;

fn supports_scalar_integer_write(byte_size: usize) -> bool {
    matches!(byte_size, 1 | 4 | 8)
}

fn supports_runtime_value_operand(byte_size: usize) -> bool {
    matches!(byte_size, 1 | 4 | 8)
}

fn resolve_runtime_call_result_source_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
) -> Option<super::super::super::storage_places::RuntimeStoragePlace> {
    resolve_runtime_assignment_value_call_result_place(
        input,
        dispatch_index,
        source_key,
        statement_index,
    )
    .or_else(|| {
        resolve_runtime_transition_argument_call_result_place(
            input,
            dispatch_index,
            source_key,
            statement_index,
        )
    })
}

fn builtin_runtime_call_operator_in_table(
    input: &InstructionSelectionInput<'_>,
    call: &TableCallExpression,
) -> Option<StateGuardOperator> {
    if call.receiver.is_valid() || call.arguments.count() != 2 {
        return None;
    }

    let symbols = &input.program.symbols;
    if Some(call.target_symbol) == symbols.builtin_function_symbol(BuiltinFunction::Max) {
        return Some(StateGuardOperator::Max);
    }
    if Some(call.target_symbol) == symbols.builtin_function_symbol(BuiltinFunction::Min) {
        return Some(StateGuardOperator::Min);
    }

    None
}

#[allow(clippy::too_many_arguments)]
pub(super) fn select_runtime_mutation_writes(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    target: &Expression,
    value: &Expression,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    static_values: &mut RuntimeStaticValues,
    resolved_segment_expressions: &mut ExpressionTable,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let resolved_target =
        resolve_runtime_alias_binding(target, source_key, aliases, alias_expressions);
    select_runtime_resolved_target_value_source_mutation_writes(
        input,
        dispatch_index,
        source_key,
        resolved_target.source_key,
        source_key,
        source_machine,
        source_state,
        statement_index,
        &resolved_target.expression,
        value,
        aliases,
        alias_expressions,
        static_values,
        resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn select_runtime_state_call_result_write(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_source_key: StateKey,
    statement_index: usize,
    role: StateCallRole,
    call_ordinal: usize,
    value_source_key: StateKey,
    value: ExpressionHandle,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    static_values: &mut RuntimeStaticValues,
    scratch: &mut super::RuntimeStorageWriteScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(slot) = input.runtime_storage.call_result_slot_by_ordinal(
        dispatch_index,
        operation_source_key,
        statement_index,
        role,
        call_ordinal,
    ) else {
        return;
    };
    scratch.expressions.clear();
    let value_expressions = &mut scratch.expressions;
    let copied_aliases =
        RuntimeAliasBuffer::copy_from_bindings(alias_expressions, aliases, value_expressions);
    let value_expression = value_expressions.copy_from(&input.runtime_bodies.expressions, value);
    let resolved_value = resolve_runtime_alias_binding_handle(
        value_expression,
        value_source_key,
        copied_aliases.bindings(),
        value_expressions,
    );
    if emit_runtime_frame_slot_slice_descriptor_write_in_table(
        input,
        dispatch_index,
        resolved_value.source_key,
        statement_index,
        &value_expressions,
        slot,
        resolved_value.expression,
        selected_instructions,
    ) {
        return;
    }
    if let Some(kind) = select_runtime_frame_slot_value_write_in_table(
        input,
        dispatch_index,
        resolved_value.source_key,
        statement_index,
        &value_expressions,
        slot,
        resolved_value.expression,
        static_values,
        runtime_value_operands,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return;
    }

    let target = runtime_frame_slot_target_expression(value_expressions, slot);
    scratch.resolved_segment_expressions.clear();
    let copied_segment_aliases = RuntimeAliasBuffer::copy_from_bindings(
        value_expressions,
        copied_aliases.bindings(),
        &mut scratch.resolved_segment_expressions,
    );
    if runtime_text_builder_write_in_table_emit(
        input,
        dispatch_index,
        operation_source_key,
        operation_source_key,
        statement_index,
        value_expressions,
        target,
        &mut scratch.resolved_segment_expressions,
        &|expressions, expression| {
            resolve_runtime_alias_binding_handle(
                expression,
                operation_source_key,
                copied_segment_aliases.bindings(),
                expressions,
            )
            .expression
        },
        &mut |kind| {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key: operation_source_key,
                source_statement: statement_index,
            });
        },
    ) {
        return;
    }

    let (value_machine, value_state) = input
        .control_flow
        .state_names_by_key_cloned(resolved_value.source_key);

    let target = value_expressions.to_tree(target);
    let value = value_expressions.to_tree(resolved_value.expression);
    scratch.resolved_segment_expressions.clear();

    select_runtime_resolved_target_value_source_mutation_writes(
        input,
        dispatch_index,
        operation_source_key,
        operation_source_key,
        resolved_value.source_key,
        &value_machine,
        &value_state,
        statement_index,
        &target,
        &value,
        aliases,
        alias_expressions,
        static_values,
        &mut scratch.resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    );
}

pub(in crate::selection) fn runtime_frame_slot_target_expression(
    expressions: &mut ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
) -> ExpressionHandle {
    let mut members = HandleSpan::empty();
    expressions.push_name_path_member(&mut members, slot.name.clone());

    let mut member_symbols = HandleSpan::empty();
    expressions.push_name_path_member_symbol(&mut member_symbols, slot.symbol);

    expressions.insert(ExpressionNode::Name(TableNamePath {
        members,
        member_symbols,
        head_symbol: slot.symbol,
        symbol: slot.symbol,
    }))
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn emit_runtime_frame_slot_slice_descriptor_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    value: ExpressionHandle,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    if slot.byte_size != input.runtime_abi.slice_descriptor_size() {
        return false;
    }

    let ExpressionNode::Call(call) = expressions.expression(value) else {
        return false;
    };
    if !call.receiver.is_valid()
        || !call.arguments.is_empty()
        || (call.target.as_str() != "as_slice" && call.target.as_str() != "as_mut_slice")
    {
        return false;
    }

    let Some(source_place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    ) else {
        return false;
    };
    let Some(length) = resolve_fixed_array_length_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    ) else {
        return false;
    };

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame {
            source_region: source_place.region,
            source_offset: source_place.byte_offset,
            target_offset: slot.byte_offset,
        },
        source_key: value_source_key,
        source_statement: statement_index,
    });
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
            target_region: RuntimeStorageRegion::RuntimeFrame,
            byte_offset: slot.byte_offset + input.runtime_abi.pointer_size,
            byte_size: input.runtime_abi.pointer_size,
            value: length as i64,
        },
        source_key: value_source_key,
        source_statement: statement_index,
    });
    true
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn select_runtime_frame_slot_value_write_in_table(
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
    if slot.byte_size == input.runtime_abi.pointer_size
        && let Some(kind) = select_runtime_frame_slot_address_write_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            slot,
            value,
        )
    {
        return Some(kind);
    }

    if supports_scalar_integer_write(slot.byte_size)
        && let Some(value) =
            resolve_runtime_static_integer_value_in_table(input, expressions, value, static_values)
    {
        return Some(SelectedInstructionKind::WriteRuntimeStorageInteger {
            target_region: RuntimeStorageRegion::RuntimeFrame,
            byte_offset: slot.byte_offset,
            byte_size: slot.byte_size,
            value,
        });
    }

    if let Some(source_place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && source_place.byte_count == slot.byte_size
        && source_place.byte_count > 0
    {
        return Some(SelectedInstructionKind::CopyRuntimeStorage {
            source_region: source_place.region,
            source_offset: source_place.byte_offset,
            target_region: RuntimeStorageRegion::RuntimeFrame,
            target_offset: slot.byte_offset,
            byte_count: slot.byte_size,
        });
    }

    if let Some(indexed_source) = resolve_runtime_frame_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && indexed_source.byte_count == slot.byte_size
        && indexed_source.byte_count > 0
    {
        return Some(
            SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimeFrame {
                descriptor_offset: indexed_source.descriptor_offset,
                element_index: indexed_source.element_index,
                element_byte_size: indexed_source.element_byte_size,
                field_byte_offset: indexed_source.field_byte_offset,
                target_offset: slot.byte_offset,
                byte_count: slot.byte_size,
            },
        );
    }

    if let Some(indexed_source) = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && indexed_source.byte_count == slot.byte_size
        && indexed_source.byte_count > 0
    {
        return Some(
            SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimeFrame {
                descriptor_offset: indexed_source.descriptor_offset,
                index_offset: indexed_source.index_offset,
                element_byte_size: indexed_source.element_byte_size,
                field_byte_offset: indexed_source.field_byte_offset,
                target_offset: slot.byte_offset,
                byte_count: slot.byte_size,
            },
        );
    }

    select_runtime_storage_binary_write_in_table(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        RuntimeStorageRegion::RuntimeFrame,
        slot.byte_offset,
        slot.byte_size,
        value,
        static_values,
        runtime_value_operands,
    )
}

fn select_runtime_frame_slot_address_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    value: ExpressionHandle,
) -> Option<SelectedInstructionKind> {
    let ExpressionNode::Call(call) = expressions.expression(value) else {
        return None;
    };
    if !call.receiver.is_valid()
        || !call.arguments.is_empty()
        || (call.target.as_str() != "as_slice" && call.target.as_str() != "as_mut_slice")
    {
        return None;
    }

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    ) {
        return Some(
            SelectedInstructionKind::WriteRuntimePointeeAddressToRuntimeFrame {
                pointer_byte_offset: pointer_target.pointer_byte_offset,
                field_byte_offset: pointer_target.field_byte_offset,
                target_offset: slot.byte_offset,
            },
        );
    }

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    ) {
        return Some(
            SelectedInstructionKind::WriteRuntimeFrameIndexedAddressToRuntimeFrame {
                descriptor_offset: indexed_target.descriptor_offset,
                index_offset: indexed_target.index_offset,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                target_offset: slot.byte_offset,
            },
        );
    }

    let source_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    )?;
    Some(
        SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame {
            source_region: source_place.region,
            source_offset: source_place.byte_offset,
            target_offset: slot.byte_offset,
        },
    )
}

pub(super) fn select_runtime_static_mutation_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    target_source_key: StateKey,
    _statement_index: usize,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
    static_values: &mut RuntimeStaticValues,
) -> Option<SelectedInstructionKind> {
    let value =
        resolve_runtime_static_integer_value_in_table(input, expressions, value, static_values)?;

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && supports_scalar_integer_write(indexed_target.byte_count)
    {
        return Some(SelectedInstructionKind::WriteRuntimeFrameIndexedInteger {
            descriptor_offset: indexed_target.descriptor_offset,
            index_offset: indexed_target.index_offset,
            element_byte_size: indexed_target.element_byte_size,
            field_byte_offset: indexed_target.field_byte_offset,
            byte_size: indexed_target.byte_count,
            value,
        });
    }

    if let Some(indexed_target) = resolve_runtime_machine_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && supports_scalar_integer_write(indexed_target.byte_count)
    {
        set_runtime_static_value_in_table(static_values, expressions, target, value);
        return Some(SelectedInstructionKind::WriteRuntimeMachineIndexedInteger {
            base_byte_offset: indexed_target.base_byte_offset,
            index_offset: indexed_target.index_offset,
            element_byte_size: indexed_target.element_byte_size,
            field_byte_offset: indexed_target.field_byte_offset,
            byte_size: indexed_target.byte_count,
            value,
        });
    }

    if let Some(pointer_target) = resolve_runtime_pointee_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && supports_scalar_integer_write(pointer_target.pointee_byte_size)
    {
        set_runtime_static_value_in_table(static_values, expressions, target, value);
        return Some(SelectedInstructionKind::WriteRuntimePointeeInteger {
            pointer_byte_offset: pointer_target.pointer_byte_offset,
            field_byte_offset: pointer_target.field_byte_offset,
            byte_size: pointer_target.pointee_byte_size,
            value,
        });
    }

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) {
        set_runtime_static_value_in_table(static_values, expressions, target, value);
        return Some(SelectedInstructionKind::WriteRuntimePointeeInteger {
            pointer_byte_offset: pointer_target.pointer_byte_offset,
            field_byte_offset: pointer_target.field_byte_offset,
            byte_size: pointer_target.pointee_byte_size,
            value,
        });
    }

    let target_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    )?;
    if !supports_scalar_integer_write(target_place.byte_count) {
        return None;
    }

    set_runtime_static_value_in_table(static_values, expressions, target, value);
    Some(SelectedInstructionKind::WriteRuntimeStorageInteger {
        target_region: target_place.region,
        byte_offset: target_place.byte_offset,
        byte_size: target_place.byte_count,
        value,
    })
}

pub(super) fn select_runtime_string_mutation_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_source_key: StateKey,
    target_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
) -> Option<SelectedInstructionKind> {
    let value = expressions.string_literal_value(value)?;
    let data = string_literal_data_handle(input, operation_source_key, statement_index, &value);

    if data.is_valid()
        && let Some(pointer_target) = resolve_runtime_pointee_fixed_indexed_target_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
    {
        return Some(SelectedInstructionKind::WriteRuntimePointeeString {
            pointer_byte_offset: pointer_target.pointer_byte_offset,
            field_byte_offset: pointer_target.field_byte_offset,
            data,
            byte_length: value.len(),
        });
    }

    if data.is_valid()
        && let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
    {
        return Some(SelectedInstructionKind::WriteRuntimePointeeString {
            pointer_byte_offset: pointer_target.pointer_byte_offset,
            field_byte_offset: pointer_target.field_byte_offset,
            data,
            byte_length: value.len(),
        });
    }

    if data.is_valid()
        && let Some(indexed_target) = resolve_runtime_frame_indexed_target_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
        && indexed_target.byte_count == input.runtime_abi.string_descriptor_size()
    {
        return Some(SelectedInstructionKind::WriteRuntimeFrameIndexedString {
            descriptor_offset: indexed_target.descriptor_offset,
            index_offset: indexed_target.index_offset,
            element_byte_size: indexed_target.element_byte_size,
            field_byte_offset: indexed_target.field_byte_offset,
            data,
            byte_length: value.len(),
        });
    }

    let target_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    )?;
    if target_place.byte_count != input.runtime_abi.string_descriptor_size() || !data.is_valid() {
        return None;
    }

    Some(SelectedInstructionKind::WriteRuntimeMachineString {
        byte_offset: target_place.byte_offset,
        data,
        byte_length: value.len(),
    })
}

pub(super) fn select_runtime_binary_mutation_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
    static_values: &RuntimeStaticValues,
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
    target_place: Option<super::super::super::storage_places::RuntimeStoragePlace>,
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
fn select_runtime_storage_binary_write_in_table(
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

fn resolve_runtime_value_operand_in_table(
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
fn select_runtime_resolved_target_value_source_mutation_writes(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_source_key: StateKey,
    target_source_key: StateKey,
    value_source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    value: &Expression,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    static_values: &mut RuntimeStaticValues,
    resolved_segment_expressions: &mut ExpressionTable,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if let Expression::StructLiteral(struct_literal) = value {
        for field in struct_literal.fields.iter() {
            let field_target =
                append_place_suffix(resolved_target, std::slice::from_ref(&field.name));
            select_runtime_resolved_target_value_source_mutation_writes(
                input,
                dispatch_index,
                operation_source_key,
                target_source_key,
                value_source_key,
                source_machine,
                source_state,
                statement_index,
                &field_target,
                &field.value,
                aliases,
                alias_expressions,
                static_values,
                resolved_segment_expressions,
                runtime_value_operands,
                selected_instructions,
            );
        }
        return;
    }

    if let Expression::String(value) = value {
        let data = string_literal_data_handle(input, operation_source_key, statement_index, value);
        if data.is_valid()
            && let Some(target) = resolve_runtime_pointee_slot_offset(
                input,
                dispatch_index,
                target_source_key,
                resolved_target,
            )
        {
            let pointer_byte_offset = target.pointer_byte_offset;
            let field_byte_offset = target.field_byte_offset;
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::WriteRuntimePointeeString {
                    pointer_byte_offset,
                    field_byte_offset,
                    data,
                    byte_length: value.len(),
                },
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }
        select_runtime_string_descriptor_write(
            input,
            operation_source_key,
            target_source_key,
            source_machine,
            source_state,
            dispatch_index,
            statement_index,
            resolved_target,
            value,
            selected_instructions,
        );
        return;
    }

    if runtime_text_builder_write_with_scratch_emit(
        input,
        dispatch_index,
        operation_source_key,
        source_machine,
        source_state,
        statement_index,
        resolved_target,
        aliases,
        alias_expressions,
        resolved_segment_expressions,
        &mut |kind| {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key: operation_source_key,
                source_statement: statement_index,
            });
        },
    ) {
        return;
    }

    let resolved_value =
        resolve_runtime_alias_binding(value, value_source_key, aliases, alias_expressions);
    if let Expression::String(value) = &resolved_value.expression {
        select_runtime_string_descriptor_write(
            input,
            operation_source_key,
            target_source_key,
            source_machine,
            source_state,
            dispatch_index,
            statement_index,
            resolved_target,
            value,
            selected_instructions,
        );
        return;
    }

    if let Some(copy) = runtime_storage_copy(
        input,
        dispatch_index,
        target_source_key,
        resolved_value.source_key,
        source_machine,
        source_state,
        resolved_target,
        &resolved_value.expression,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind: copy,
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return;
    }

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) && let Some(source_place) = resolve_runtime_storage_place(
        input,
        dispatch_index,
        resolved_value.source_key,
        source_machine,
        source_state,
        &resolved_value.expression,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::CopyRuntimeStorageToRuntimePointee {
                source_region: source_place.region,
                source_offset: source_place.byte_offset,
                pointer_byte_offset: pointer_target.pointer_byte_offset,
                field_byte_offset: pointer_target.field_byte_offset,
                byte_count: source_place.byte_count,
            },
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return;
    }

    if let Some(source_place) = resolve_runtime_call_result_source_place(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
    ) {
        if let Some(pointer_target) = resolve_runtime_pointee_slot_offset(
            input,
            dispatch_index,
            target_source_key,
            resolved_target,
        ) && source_place.byte_count > 0
        {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::CopyRuntimeStorageToRuntimePointee {
                    source_region: source_place.region,
                    source_offset: source_place.byte_offset,
                    pointer_byte_offset: pointer_target.pointer_byte_offset,
                    field_byte_offset: pointer_target.field_byte_offset,
                    byte_count: source_place.byte_count,
                },
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }

        if let Some(indexed_target) = resolve_runtime_frame_indexed_target(
            input,
            dispatch_index,
            target_source_key,
            resolved_target,
        ) && source_place.region == omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame
            && source_place.byte_count == indexed_target.byte_count
        {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::CopyRuntimeStorageToRuntimeFrameIndexed {
                    source_region: source_place.region,
                    source_offset: source_place.byte_offset,
                    descriptor_offset: indexed_target.descriptor_offset,
                    index_offset: indexed_target.index_offset,
                    element_byte_size: indexed_target.element_byte_size,
                    field_byte_offset: indexed_target.field_byte_offset,
                    byte_count: indexed_target.byte_count,
                },
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }

        if let Some(target_place) = resolve_runtime_storage_place(
            input,
            dispatch_index,
            target_source_key,
            source_machine,
            source_state,
            resolved_target,
        ) && target_place.byte_count == source_place.byte_count
        {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::CopyRuntimeStorage {
                    source_region: source_place.region,
                    source_offset: source_place.byte_offset,
                    target_region: target_place.region,
                    target_offset: target_place.byte_offset,
                    byte_count: target_place.byte_count,
                },
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }
    }

    if let Some(kind) = select_runtime_binary_mutation_write(
        input,
        dispatch_index,
        operation_source_key,
        target_source_key,
        resolved_value.source_key,
        source_machine,
        source_state,
        statement_index,
        resolved_target,
        &resolved_value.expression,
        aliases,
        alias_expressions,
        static_values,
        runtime_value_operands,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return;
    }

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) {
        if let Some(source_place) = resolve_runtime_storage_place(
            input,
            dispatch_index,
            resolved_value.source_key,
            source_machine,
            source_state,
            &resolved_value.expression,
        ) && source_place.region == omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame
        {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::CopyRuntimeStorageToRuntimeFrameIndexed {
                    source_region: source_place.region,
                    source_offset: source_place.byte_offset,
                    descriptor_offset: indexed_target.descriptor_offset,
                    index_offset: indexed_target.index_offset,
                    element_byte_size: indexed_target.element_byte_size,
                    field_byte_offset: indexed_target.field_byte_offset,
                    byte_count: indexed_target.byte_count,
                },
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }

        if supports_scalar_integer_write(indexed_target.byte_count)
            && let Some(value) = resolve_runtime_static_integer_value(
                input,
                operation_source_key,
                value,
                aliases,
                alias_expressions,
                static_values,
            )
        {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::WriteRuntimeFrameIndexedInteger {
                    descriptor_offset: indexed_target.descriptor_offset,
                    index_offset: indexed_target.index_offset,
                    element_byte_size: indexed_target.element_byte_size,
                    field_byte_offset: indexed_target.field_byte_offset,
                    byte_size: indexed_target.byte_count,
                    value,
                },
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }
    }

    let Some(value) = resolve_runtime_static_integer_value(
        input,
        operation_source_key,
        value,
        aliases,
        alias_expressions,
        static_values,
    ) else {
        return;
    };
    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) {
        set_runtime_static_value(
            static_values,
            strip_mutable_expression(resolved_target.clone()),
            value,
        );
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WriteRuntimePointeeInteger {
                pointer_byte_offset: pointer_target.pointer_byte_offset,
                field_byte_offset: pointer_target.field_byte_offset,
                byte_size: pointer_target.pointee_byte_size,
                value,
            },
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return;
    }
    let Some(target_place) = resolve_runtime_storage_place(
        input,
        dispatch_index,
        target_source_key,
        source_machine,
        source_state,
        resolved_target,
    ) else {
        return;
    };
    if !supports_scalar_integer_write(target_place.byte_count) {
        return;
    }

    set_runtime_static_value(
        static_values,
        strip_mutable_expression(resolved_target.clone()),
        value,
    );
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
            target_region: target_place.region,
            byte_offset: target_place.byte_offset,
            byte_size: target_place.byte_count,
            value,
        },
        source_key: operation_source_key,
        source_statement: statement_index,
    });
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_binary_mutation_write(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    _operation_source_key: StateKey,
    target_source_key: StateKey,
    value_source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    value: &Expression,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    let (operator, left_expression, right_expression) = match value {
        Expression::Binary(binary) => (
            runtime_binary_operator(binary.operator)?,
            &binary.left,
            &binary.right,
        ),
        Expression::Call(call) => {
            let operator = builtin_runtime_call_operator(input, call)?;
            let [left, right] = &*call.arguments else {
                return None;
            };
            (operator, left, right)
        }
        _ => return None,
    };
    let left = resolve_runtime_value_operand(
        input,
        dispatch_index,
        value_source_key,
        source_machine,
        source_state,
        statement_index,
        left_expression,
        aliases,
        alias_expressions,
        static_values,
        runtime_value_operands,
    )?;
    let right = resolve_runtime_value_operand(
        input,
        dispatch_index,
        value_source_key,
        source_machine,
        source_state,
        statement_index,
        right_expression,
        aliases,
        alias_expressions,
        static_values,
        runtime_value_operands,
    )?;

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
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

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
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

    let target_place = resolve_runtime_storage_place(
        input,
        dispatch_index,
        target_source_key,
        source_machine,
        source_state,
        resolved_target,
    )?;

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
fn resolve_runtime_value_operand(
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

fn runtime_binary_operator(operator: BinaryOperator) -> Option<StateGuardOperator> {
    match operator {
        BinaryOperator::Add => Some(StateGuardOperator::Add),
        BinaryOperator::And => Some(StateGuardOperator::And),
        BinaryOperator::Equal => Some(StateGuardOperator::Equal),
        BinaryOperator::Greater => Some(StateGuardOperator::Greater),
        BinaryOperator::GreaterOrEqual => Some(StateGuardOperator::GreaterOrEqual),
        BinaryOperator::Less => Some(StateGuardOperator::Less),
        BinaryOperator::LessOrEqual => Some(StateGuardOperator::LessOrEqual),
        BinaryOperator::NotEqual => Some(StateGuardOperator::NotEqual),
        BinaryOperator::Multiply => Some(StateGuardOperator::Multiply),
        BinaryOperator::Modulo => Some(StateGuardOperator::Modulo),
        BinaryOperator::Subtract => Some(StateGuardOperator::Subtract),
        BinaryOperator::Divide
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight => None,
    }
}

fn builtin_runtime_call_operator(
    input: &InstructionSelectionInput<'_>,
    call: &omega_checked_trees::expression::CallExpression,
) -> Option<StateGuardOperator> {
    if call.receiver.is_some() || call.arguments.len() != 2 {
        return None;
    }

    let symbols = &input.program.symbols;
    if Some(call.target_symbol) == symbols.builtin_function_symbol(BuiltinFunction::Max) {
        return Some(StateGuardOperator::Max);
    }
    if Some(call.target_symbol) == symbols.builtin_function_symbol(BuiltinFunction::Min) {
        return Some(StateGuardOperator::Min);
    }

    None
}
