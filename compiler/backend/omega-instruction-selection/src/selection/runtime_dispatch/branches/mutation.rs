use crate::InstructionSelectionInput;
use omega_abstract_operations::{
    RuntimeValueOperand, RuntimeValueOperandHandle, SelectedInstruction, SelectedInstructionKind,
    StateGuardOperator,
};
use omega_checked_trees::expression::{
    BinaryOperator, Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
    TableCallExpression, TableMemberExpression,
};
use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_core::symbols::{BuiltinFunction, SymbolHandle};

use super::super::super::storage_places::{
    resolve_runtime_frame_base_indexed_target, resolve_runtime_frame_base_indexed_target_in_table,
    resolve_runtime_frame_fixed_indexed_target, resolve_runtime_frame_indexed_target,
    resolve_runtime_pointee_slot_offset, resolve_runtime_storage_place, static_integer_value,
};
use super::super::super::storage_places::{
    resolve_runtime_frame_fixed_indexed_target_in_table,
    resolve_runtime_frame_indexed_target_in_table, resolve_runtime_machine_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_place_in_table,
    static_integer_value_in_table,
};
use super::super::text_writes::{
    runtime_text_builder_write_in_table_emit, runtime_text_builder_write_without_aliases_emit,
    select_runtime_string_descriptor_write, string_literal_data_handle,
};
use super::super::writes::{
    runtime_storage_copy, runtime_storage_copy_in_table, runtime_storage_indirect_copy_in_table,
};
use crate::selection::instruction_sink::SelectedInstructionSink;

fn supports_scalar_integer_write(byte_size: usize) -> bool {
    matches!(byte_size, 1 | 4 | 8)
}

fn supports_runtime_value_operand(byte_size: usize) -> bool {
    matches!(byte_size, 1 | 4 | 8)
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
pub(crate) fn select_runtime_resolved_mutation_write(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_key: StateKey,
    _source_machine: &str,
    operation_machine: &str,
    operation_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    resolved_value: &Expression,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if let Expression::String(value) = resolved_value {
        let data = string_literal_data_handle(input, operation_key, statement_index, value);
        if data.is_valid()
            && let Some(target) = resolve_runtime_pointee_slot_offset(
                input,
                dispatch_index,
                operation_key,
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
                source_key: operation_key,
                source_statement: statement_index,
            });
            return;
        }
        select_runtime_string_descriptor_write(
            input,
            operation_key,
            operation_key,
            operation_machine,
            operation_state,
            dispatch_index,
            statement_index,
            resolved_target,
            value,
            selected_instructions,
        );
        return;
    }

    if runtime_text_builder_write_without_aliases_emit(
        input,
        dispatch_index,
        operation_key,
        operation_machine,
        operation_state,
        statement_index,
        resolved_target,
        &mut |kind| {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key: operation_key,
                source_statement: statement_index,
            });
        },
    ) {
        return;
    }

    if let Some(pointer_target) =
        resolve_runtime_pointee_slot_offset(input, dispatch_index, operation_key, resolved_target)
        && let Some(value) = static_integer_value(&input.layouts, resolved_value)
    {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WriteRuntimePointeeInteger {
                pointer_byte_offset: pointer_target.pointer_byte_offset,
                field_byte_offset: pointer_target.field_byte_offset,
                byte_size: pointer_target.pointee_byte_size,
                value,
            },
            source_key: operation_key,
            source_statement: statement_index,
        });
        return;
    }

    if let Some(target_place) = resolve_runtime_storage_place(
        input,
        dispatch_index,
        operation_key,
        operation_machine,
        operation_state,
        resolved_target,
    ) && supports_scalar_integer_write(target_place.byte_count)
        && let Some(value) = static_integer_value(&input.layouts, resolved_value)
    {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
                target_region: target_place.region,
                byte_offset: target_place.byte_offset,
                byte_size: target_place.byte_count,
                value,
            },
            source_key: operation_key,
            source_statement: statement_index,
        });
        return;
    }

    if let Some(copy) = runtime_storage_copy(
        input,
        dispatch_index,
        operation_key,
        operation_key,
        operation_machine,
        operation_state,
        resolved_target,
        resolved_value,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind: copy,
            source_key: operation_key,
            source_statement: statement_index,
        });
        return;
    }

    if let Some(pointer_target) =
        resolve_runtime_pointee_slot_offset(input, dispatch_index, operation_key, resolved_target)
        && let Some(source_place) = resolve_runtime_storage_place(
            input,
            dispatch_index,
            operation_key,
            operation_machine,
            operation_state,
            resolved_value,
        )
    {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::CopyRuntimeStorageToRuntimePointee {
                source_region: source_place.region,
                source_offset: source_place.byte_offset,
                pointer_byte_offset: pointer_target.pointer_byte_offset,
                field_byte_offset: pointer_target.field_byte_offset,
                byte_count: source_place.byte_count,
            },
            source_key: operation_key,
            source_statement: statement_index,
        });
        return;
    }

    if let Some(kind) = select_runtime_resolved_binary_mutation_write(
        input,
        dispatch_index,
        operation_key,
        operation_machine,
        operation_state,
        resolved_target,
        resolved_value,
        runtime_value_operands,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: operation_key,
            source_statement: statement_index,
        });
        return;
    }

    if let Some(indexed_target) =
        resolve_runtime_frame_indexed_target(input, dispatch_index, operation_key, resolved_target)
    {
        if let Some(source_place) = resolve_runtime_storage_place(
            input,
            dispatch_index,
            operation_key,
            operation_machine,
            operation_state,
            resolved_value,
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
                source_key: operation_key,
                source_statement: statement_index,
            });
            return;
        }

        if supports_scalar_integer_write(indexed_target.byte_count)
            && let Some(value) = static_integer_value(&input.layouts, resolved_value)
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
                source_key: operation_key,
                source_statement: statement_index,
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn select_runtime_resolved_mutation_write_in_table_with_scratch(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_key: StateKey,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    resolved_target: ExpressionHandle,
    resolved_value: ExpressionHandle,
    mutable_expressions: &mut ExpressionTable,
    resolved_segment_expressions: &mut ExpressionTable,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    if matches!(
        expressions.expression(resolved_value),
        ExpressionNode::StructLiteral(_)
    ) {
        let source_expressions = expressions;
        mutable_expressions.clear();
        let expressions = mutable_expressions;
        let resolved_target = expressions.copy_from(source_expressions, resolved_target);
        let resolved_value = expressions.copy_from(source_expressions, resolved_value);
        return select_runtime_resolved_mutation_write_in_mutable_table(
            input,
            dispatch_index,
            operation_key,
            target_source_key,
            value_source_key,
            statement_index,
            expressions,
            resolved_target,
            resolved_value,
            resolved_segment_expressions,
            runtime_value_operands,
            selected_instructions,
        );
    }

    select_runtime_resolved_scalar_mutation_write_in_table(
        input,
        dispatch_index,
        operation_key,
        target_source_key,
        value_source_key,
        statement_index,
        expressions,
        resolved_target,
        resolved_value,
        resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_resolved_mutation_write_in_mutable_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_key: StateKey,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &mut ExpressionTable,
    resolved_target: ExpressionHandle,
    resolved_value: ExpressionHandle,
    resolved_segment_expressions: &mut ExpressionTable,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    if let ExpressionNode::StructLiteral(struct_literal) =
        expressions.expression(resolved_value).clone()
    {
        for offset in 0..struct_literal.fields.count() {
            let field = expressions
                .struct_field_at_offset(struct_literal.fields, offset)
                .clone();
            let field_target = expressions.insert(ExpressionNode::Member(TableMemberExpression {
                receiver: resolved_target,
                member_symbol: SymbolHandle::invalid(),
                member: field.name,
            }));
            select_runtime_resolved_mutation_write_in_mutable_table(
                input,
                dispatch_index,
                operation_key,
                target_source_key,
                value_source_key,
                statement_index,
                expressions,
                field_target,
                field.value,
                resolved_segment_expressions,
                runtime_value_operands,
                selected_instructions,
            );
        }
        return true;
    }

    select_runtime_resolved_scalar_mutation_write_in_table_with_scratch(
        input,
        dispatch_index,
        operation_key,
        target_source_key,
        value_source_key,
        statement_index,
        expressions,
        resolved_target,
        resolved_value,
        resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_resolved_scalar_mutation_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_key: StateKey,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    resolved_target: ExpressionHandle,
    resolved_value: ExpressionHandle,
    resolved_segment_expressions: &mut ExpressionTable,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    select_runtime_resolved_scalar_mutation_write_in_table_with_scratch(
        input,
        dispatch_index,
        operation_key,
        target_source_key,
        value_source_key,
        statement_index,
        expressions,
        resolved_target,
        resolved_value,
        resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_resolved_scalar_mutation_write_in_table_with_scratch(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_key: StateKey,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    resolved_target: ExpressionHandle,
    resolved_value: ExpressionHandle,
    resolved_segment_expressions: &mut ExpressionTable,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    resolved_segment_expressions.clear();
    if runtime_text_builder_write_in_table_emit(
        input,
        dispatch_index,
        operation_key,
        target_source_key,
        statement_index,
        expressions,
        resolved_target,
        resolved_segment_expressions,
        &|_, expression| expression,
        &mut |kind| {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key: operation_key,
                source_statement: statement_index,
            });
        },
    ) {
        return true;
    }

    if let Some(kind) = select_runtime_string_mutation_write_in_table(
        input,
        dispatch_index,
        operation_key,
        target_source_key,
        statement_index,
        expressions,
        resolved_target,
        resolved_value,
    )
    .or_else(|| {
        select_runtime_static_mutation_write_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            resolved_target,
            resolved_value,
        )
    })
    .or_else(|| {
        runtime_storage_copy_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            expressions,
            resolved_target,
            resolved_value,
        )
    })
    .or_else(|| {
        runtime_storage_indirect_copy_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            expressions,
            resolved_target,
            resolved_value,
        )
    })
    .or_else(|| {
        select_runtime_binary_mutation_write_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            expressions,
            resolved_target,
            resolved_value,
            runtime_value_operands,
        )
    }) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: operation_key,
            source_statement: statement_index,
        });
        return true;
    }

    false
}

fn select_runtime_static_mutation_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
) -> Option<SelectedInstructionKind> {
    let value = static_integer_value_in_table(&input.layouts, expressions, value)?;

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
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

    if let Some(indexed_target) = resolve_runtime_frame_base_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        target,
    ) && supports_scalar_integer_write(indexed_target.byte_count)
    {
        return Some(
            SelectedInstructionKind::WriteRuntimeFrameBaseIndexedInteger {
                base_byte_offset: indexed_target.base_byte_offset,
                index_offset: indexed_target.index_offset,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                byte_size: indexed_target.byte_count,
                value,
            },
        );
    }

    if let Some(indexed_target) = resolve_runtime_machine_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        target,
    ) && supports_scalar_integer_write(indexed_target.byte_count)
    {
        return Some(SelectedInstructionKind::WriteRuntimeMachineIndexedInteger {
            base_byte_offset: indexed_target.base_byte_offset,
            index_region: indexed_target.index_region,
            index_offset: indexed_target.index_offset,
            element_byte_size: indexed_target.element_byte_size,
            field_byte_offset: indexed_target.field_byte_offset,
            byte_size: indexed_target.byte_count,
            value,
        });
    }

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        target,
    ) {
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
        source_key,
        expressions,
        target,
    )?;
    if !supports_scalar_integer_write(target_place.byte_count) {
        return None;
    }

    Some(SelectedInstructionKind::WriteRuntimeStorageInteger {
        target_region: target_place.region,
        byte_offset: target_place.byte_offset,
        byte_size: target_place.byte_count,
        value,
    })
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_string_mutation_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_key: StateKey,
    target_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
) -> Option<SelectedInstructionKind> {
    let value = expressions.string_literal_value(value)?;
    let data = string_literal_data_handle(input, operation_key, statement_index, &value);

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

#[allow(clippy::too_many_arguments)]
fn select_runtime_binary_mutation_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    target_source_key: StateKey,
    value_source_key: StateKey,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
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
        expressions,
        left_expression,
        runtime_value_operands,
    )?;
    let right = resolve_runtime_value_operand_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        right_expression,
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

    let target_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
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

fn select_runtime_resolved_binary_mutation_write(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_key: StateKey,
    operation_machine: &str,
    operation_state: &str,
    resolved_target: &Expression,
    resolved_value: &Expression,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    let Expression::Binary(binary) = resolved_value else {
        return None;
    };
    let operator = runtime_binary_operator(binary.operator)?;
    let left = resolve_runtime_value_operand(
        input,
        dispatch_index,
        operation_key,
        operation_machine,
        operation_state,
        &binary.left,
        runtime_value_operands,
    )?;
    let right = resolve_runtime_value_operand(
        input,
        dispatch_index,
        operation_key,
        operation_machine,
        operation_state,
        &binary.right,
        runtime_value_operands,
    )?;

    if let Some(indexed_target) =
        resolve_runtime_frame_indexed_target(input, dispatch_index, operation_key, resolved_target)
    {
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

    if let Some(pointer_target) =
        resolve_runtime_pointee_slot_offset(input, dispatch_index, operation_key, resolved_target)
    {
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
        operation_key,
        operation_machine,
        operation_state,
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

fn resolve_runtime_value_operand(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    expression: &Expression,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<RuntimeValueOperandHandle> {
    if let Some(value) = static_integer_value(&input.layouts, expression) {
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
            &binary.left,
            runtime_value_operands,
        )?;
        let right = resolve_runtime_value_operand(
            input,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            &binary.right,
            runtime_value_operands,
        )?;
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Binary {
            left,
            operator,
            right,
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

    if let Some(indexed_target) =
        resolve_runtime_frame_fixed_indexed_target(input, dispatch_index, source_key, expression)
    {
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

    let place = resolve_runtime_storage_place(
        input,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
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

fn resolve_runtime_value_operand_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<RuntimeValueOperandHandle> {
    if let Some(value) = static_integer_value_in_table(&input.layouts, expressions, expression) {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Immediate(value)));
    }

    match expressions.expression(expression) {
        ExpressionNode::Binary(binary) => {
            let operator = runtime_binary_operator(binary.operator)?;
            let left = resolve_runtime_value_operand_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                binary.left,
                runtime_value_operands,
            )?;
            let right = resolve_runtime_value_operand_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                binary.right,
                runtime_value_operands,
            )?;
            return Some(runtime_value_operands.insert(RuntimeValueOperand::Binary {
                left,
                operator,
                right,
            }));
        }
        ExpressionNode::Call(call) => {
            let operator = builtin_runtime_call_operator_in_table(input, call)?;
            let left = resolve_runtime_value_operand_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                expressions.expression_handle_at_offset(call.arguments, 0),
                runtime_value_operands,
            )?;
            let right = resolve_runtime_value_operand_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                expressions.expression_handle_at_offset(call.arguments, 1),
                runtime_value_operands,
            )?;
            return Some(runtime_value_operands.insert(RuntimeValueOperand::Binary {
                left,
                operator,
                right,
            }));
        }
        _ => {}
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
        BinaryOperator::Or => Some(StateGuardOperator::Or),
        BinaryOperator::Subtract => Some(StateGuardOperator::Subtract),
        BinaryOperator::Divide | BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => None,
    }
}
