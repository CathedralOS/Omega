use crate::InstructionSelectionInput;
use omega_control_flow::StateKey;
use omega_target_operations::{RuntimeValueOperand, SelectedInstruction, SelectedInstructionKind, StateGuardOperator};
use omega_checked_trees::expression::{BinaryOperator, Expression};

use super::super::super::storage_places::{
    resolve_runtime_frame_indexed_target, resolve_runtime_pointee_slot_offset,
    resolve_runtime_storage_place, static_integer_value,
};
use super::super::text_writes::{
    runtime_text_builder_write_with_resolver_emit, select_runtime_string_descriptor_write,
    string_literal_data_handle,
};
use super::super::writes::runtime_storage_copy;
use crate::selection::instruction_sink::SelectedInstructionSink;

fn supports_scalar_integer_write(byte_size: usize) -> bool {
    matches!(byte_size, 1 | 4 | 8)
}

fn supports_runtime_value_operand(byte_size: usize) -> bool {
    matches!(byte_size, 1 | 4 | 8)
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
    selected_instructions: &mut SelectedInstructionSink,
) {
    if let Expression::String(value) = resolved_value {
        if let Some((pointer_byte_offset, field_byte_offset, data)) = resolve_runtime_pointee_slot_offset(
            input,
            dispatch_index,
            operation_key,
            resolved_target,
        ).and_then(|target| {
            string_literal_data_handle(input, operation_key, statement_index, value).map(|data| {
                (target.pointer_byte_offset, target.field_byte_offset, data)
            })
        }) {
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

    if runtime_text_builder_write_with_resolver_emit(
        input,
        dispatch_index,
        operation_key,
        operation_machine,
        operation_state,
        statement_index,
        resolved_target,
        &|expression| expression.clone(),
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

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset(
        input,
        dispatch_index,
        operation_key,
        resolved_target,
    ) && let Some(value) = static_integer_value(&input.layouts, resolved_value)
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

    if let Some(kind) = select_runtime_resolved_binary_mutation_write(
        input,
        dispatch_index,
        operation_key,
        operation_machine,
        operation_state,
        resolved_target,
        resolved_value,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: operation_key,
            source_statement: statement_index,
        });
        return;
    }

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target(
        input,
        dispatch_index,
        operation_key,
        resolved_target,
    ) {
        if let Some(source_place) = resolve_runtime_storage_place(
            input,
            dispatch_index,
            operation_key,
            operation_machine,
            operation_state,
            resolved_value,
        ) && source_place.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
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

fn select_runtime_resolved_binary_mutation_write(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_key: StateKey,
    operation_machine: &str,
    operation_state: &str,
    resolved_target: &Expression,
    resolved_value: &Expression,
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
    )?;
    let right = resolve_runtime_value_operand(
        input,
        dispatch_index,
        operation_key,
        operation_machine,
        operation_state,
        &binary.right,
    )?;

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target(
        input,
        dispatch_index,
        operation_key,
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
        operation_key,
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
) -> Option<RuntimeValueOperand> {
    if let Some(value) = static_integer_value(&input.layouts, expression) {
        return Some(RuntimeValueOperand::Immediate(value));
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
        )?;
        let right = resolve_runtime_value_operand(
            input,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            &binary.right,
        )?;
        return Some(RuntimeValueOperand::Binary {
            left: Box::new(left),
            operator,
            right: Box::new(right),
        });
    }

    if let Some(indexed_target) =
        resolve_runtime_frame_indexed_target(input, dispatch_index, source_key, expression)
    {
        return Some(RuntimeValueOperand::FrameIndexed {
            descriptor_offset: indexed_target.descriptor_offset,
            index_offset: indexed_target.index_offset,
            element_byte_size: indexed_target.element_byte_size,
            field_byte_offset: indexed_target.field_byte_offset,
            byte_size: indexed_target.byte_count,
        });
    }

    if let Some(pointer_target) =
        resolve_runtime_pointee_slot_offset(input, dispatch_index, source_key, expression)
        && supports_runtime_value_operand(pointer_target.pointee_byte_size)
    {
        return Some(RuntimeValueOperand::Pointee {
            pointer_byte_offset: pointer_target.pointer_byte_offset,
            field_byte_offset: pointer_target.field_byte_offset,
            byte_size: pointer_target.pointee_byte_size,
        });
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
    Some(RuntimeValueOperand::Storage {
        region: place.region,
        byte_offset: place.byte_offset,
        byte_size: place.byte_count,
    })
}

fn runtime_binary_operator(operator: BinaryOperator) -> Option<StateGuardOperator> {
    match operator {
        BinaryOperator::Add => Some(StateGuardOperator::Add),
        BinaryOperator::Equal => Some(StateGuardOperator::Equal),
        BinaryOperator::NotEqual => Some(StateGuardOperator::NotEqual),
        BinaryOperator::Multiply => Some(StateGuardOperator::Multiply),
        BinaryOperator::Modulo => Some(StateGuardOperator::Modulo),
        BinaryOperator::Subtract => Some(StateGuardOperator::Subtract),
        BinaryOperator::And
        | BinaryOperator::Divide
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight => None,
    }
}
