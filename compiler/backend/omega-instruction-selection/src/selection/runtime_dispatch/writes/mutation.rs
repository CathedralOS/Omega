use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_control_flow::StateKey;
use omega_target_operations::{RuntimeValueOperand, SelectedInstruction, SelectedInstructionKind, StateGuardOperator};
use omega_checked_trees::expression::{BinaryOperator, Expression, ExpressionHandle, ExpressionTable, NamePath};

use super::super::super::bindings::{
    RuntimeAliasBinding, append_place_suffix, resolve_runtime_alias_binding,
    strip_mutable_expression,
};
use super::super::super::storage_places::resolve_runtime_storage_place;
use super::super::super::storage_places::{
    resolve_runtime_assignment_value_call_result_place, resolve_runtime_frame_indexed_target,
    resolve_runtime_pointee_slot_offset,
};
use super::super::text_writes::{
    runtime_text_builder_write_emit, select_runtime_string_descriptor_write,
    string_literal_data_handle,
};
use super::static_values::{
    RuntimeStaticValues, resolve_runtime_static_integer_value, set_runtime_static_value,
};
use super::storage_copy::runtime_storage_copy;

fn supports_scalar_integer_write(byte_size: usize) -> bool {
    matches!(byte_size, 1 | 4 | 8)
}

fn supports_runtime_value_operand(byte_size: usize) -> bool {
    matches!(byte_size, 1 | 4 | 8)
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
        selected_instructions,
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn select_runtime_state_call_result_write(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_source_key: StateKey,
    statement_index: usize,
    value_source_key: StateKey,
    value: ExpressionHandle,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    static_values: &mut RuntimeStaticValues,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(slot) = input
        .runtime_storage
        .assignment_value_result_slot(dispatch_index, operation_source_key, statement_index)
    else {
        return;
    };
    let (value_machine, value_state) = input.control_flow.state_names_by_key_cloned(value_source_key);
    let target = Expression::Name(NamePath::unresolved(vec![slot.name.clone()]));
    let value = input.runtime_bodies.expressions.to_tree(value);

    select_runtime_resolved_target_value_source_mutation_writes(
        input,
        dispatch_index,
        operation_source_key,
        operation_source_key,
        value_source_key,
        &value_machine,
        &value_state,
        statement_index,
        &target,
        &value,
        aliases,
        alias_expressions,
        static_values,
        selected_instructions,
    );
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
    selected_instructions: &mut SelectedInstructionSink,
) {
    if let Expression::StructLiteral(struct_literal) = value {
        for field in &struct_literal.fields {
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
                selected_instructions,
            );
        }
        return;
    }

    if let Expression::String(value) = value {
        if let Some((pointer_byte_offset, field_byte_offset, data)) = resolve_runtime_pointee_slot_offset(
            input,
            dispatch_index,
            target_source_key,
            resolved_target,
        ).and_then(|target| {
            string_literal_data_handle(input, operation_source_key, statement_index, value).map(
                |data| (target.pointer_byte_offset, target.field_byte_offset, data),
            )
        }) {
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

    if runtime_text_builder_write_emit(
        input,
        dispatch_index,
        operation_source_key,
        source_machine,
        source_state,
        statement_index,
        resolved_target,
        aliases,
        alias_expressions,
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

    if let Some(kind) = select_runtime_binary_mutation_write(
        input,
        dispatch_index,
        operation_source_key,
        target_source_key,
        source_machine,
        source_state,
        statement_index,
        resolved_target,
        &resolved_value.expression,
        aliases,
        alias_expressions,
        static_values,
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
        ) {
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
    operation_source_key: StateKey,
    target_source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    value: &Expression,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    static_values: &RuntimeStaticValues,
) -> Option<SelectedInstructionKind> {
    let (operator, left_expression, right_expression) = match value {
        Expression::Binary(binary) => (
            runtime_binary_operator(binary.operator)?,
            &binary.left,
            &binary.right,
        ),
        Expression::Call(call) => {
            let operator = builtin_runtime_call_operator(call)?;
            let [left, right] = call.arguments.as_slice() else {
                return None;
            };
            (operator, left, right)
        }
        _ => return None,
    };
    let left = resolve_runtime_value_operand(
        input,
        dispatch_index,
        operation_source_key,
        source_machine,
        source_state,
        statement_index,
        left_expression,
        aliases,
        alias_expressions,
        static_values,
    )?;
    let right = resolve_runtime_value_operand(
        input,
        dispatch_index,
        operation_source_key,
        source_machine,
        source_state,
        statement_index,
        right_expression,
        aliases,
        alias_expressions,
        static_values,
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
) -> Option<RuntimeValueOperand> {
    if let Some(value) = resolve_runtime_static_integer_value(
        input,
        source_key,
        expression,
        aliases,
        alias_expressions,
        static_values,
    ) {
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
            statement_index,
            &binary.left,
            aliases,
            alias_expressions,
            static_values,
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
        )?;
        return Some(RuntimeValueOperand::Binary {
            left: Box::new(left),
            operator,
            right: Box::new(right),
        });
    }

    if let Expression::Call(call) = expression
        && let Some(operator) = builtin_runtime_call_operator(call)
    {
        let [left, right] = call.arguments.as_slice() else {
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
        )?;
        return Some(RuntimeValueOperand::Binary {
            left: Box::new(left),
            operator,
            right: Box::new(right),
        });
    }

    if let Expression::Call(call) = expression
        && let Some(place) = resolve_runtime_assignment_value_call_result_place(
            input,
            dispatch_index,
            source_key,
            statement_index,
            call,
        )
    {
        return Some(RuntimeValueOperand::Storage {
            region: place.region,
            byte_offset: place.byte_offset,
            byte_size: place.byte_count,
        });
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

fn builtin_runtime_call_operator(
    call: &omega_checked_trees::expression::CallExpression,
) -> Option<StateGuardOperator> {
    if call.receiver.is_some() || call.arguments.len() != 2 {
        return None;
    }

    match call.target.as_str() {
        "max" => Some(StateGuardOperator::Max),
        "min" => Some(StateGuardOperator::Min),
        _ => None,
    }
}
