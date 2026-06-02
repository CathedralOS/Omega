mod binary_table_writes;
mod frame_slots;
mod normalization;
mod operators;
mod static_writes;
mod value_operands;

use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{
    RuntimeValueOperand, SelectedInstruction, SelectedInstructionKind,
};
use omega_checked_trees::expression::{Expression, ExpressionHandle, ExpressionTable};
use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_state_calls::StateCallRole;

use super::super::super::bindings::{
    RuntimeAliasBinding, RuntimeAliasBuffer, append_place_suffix,
    resolve_runtime_alias_binding_handle, strip_mutable_expression,
};
use super::super::super::storage_places::resolve_runtime_storage_place;
use super::super::super::storage_places::{
    resolve_runtime_assignment_value_call_result_place, resolve_runtime_frame_base_indexed_target,
    resolve_runtime_frame_fixed_indexed_target, resolve_runtime_frame_indexed_target,
    resolve_runtime_machine_indexed_target, resolve_runtime_pointee_slot_offset,
    resolve_runtime_transition_argument_call_result_place,
};
use super::super::text_writes::{
    runtime_text_builder_write_in_table_emit, runtime_text_builder_write_with_scratch_emit,
    select_runtime_string_descriptor_write, string_literal_data_handle,
};
use super::slice_descriptors::emit_runtime_frame_slot_slice_descriptor_write_in_table;
use super::static_values::{
    RuntimeStaticValues, resolve_runtime_static_integer_value, set_runtime_static_value,
};
use super::storage_copy::runtime_storage_copy;
use super::subslice_copy::runtime_fixed_array_subslice_indexed_source_copy;
pub(super) use binary_table_writes::{
    select_runtime_binary_mutation_write_in_table, select_runtime_storage_binary_write_in_table,
};
pub(in crate::selection) use frame_slots::{
    runtime_frame_slot_target_expression, select_runtime_frame_slot_value_write_in_table,
};
pub(super) use normalization::simplify_runtime_expression_with_state_locals;
use normalization::{normalize_runtime_mutation_expression, resolve_runtime_mutation_target};
use operators::{
    builtin_runtime_call_operator, runtime_binary_operator, supports_scalar_integer_write,
};
pub(super) use static_writes::select_runtime_static_mutation_write_in_table;
use value_operands::resolve_runtime_value_operand;

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

#[allow(clippy::too_many_arguments)]
pub(super) fn select_runtime_mutation_writes(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    value_source_key: StateKey,
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
    let resolved_target = resolve_runtime_mutation_target(
        input,
        dispatch_index,
        source_key,
        target,
        aliases,
        alias_expressions,
    );
    select_runtime_resolved_target_value_source_mutation_writes(
        input,
        dispatch_index,
        source_key,
        resolved_target.source_key,
        value_source_key,
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

    let resolved_value = normalize_runtime_mutation_expression(
        input,
        value_source_key,
        statement_index,
        value,
        aliases,
        alias_expressions,
    );
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

    if let Some(copy) = runtime_fixed_array_subslice_indexed_source_copy(
        input,
        dispatch_index,
        target_source_key,
        resolved_value.source_key,
        source_machine,
        source_state,
        resolved_target,
        &resolved_value.expression,
    )
    .or_else(|| {
        runtime_storage_copy(
            input,
            dispatch_index,
            target_source_key,
            resolved_value.source_key,
            source_machine,
            source_state,
            resolved_target,
            &resolved_value.expression,
        )
    }) {
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

    if let Some(indexed_target) = resolve_runtime_frame_base_indexed_target(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) {
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
                kind: SelectedInstructionKind::WriteRuntimeFrameBaseIndexedInteger {
                    base_byte_offset: indexed_target.base_byte_offset,
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

    if let Some(indexed_target) = resolve_runtime_machine_indexed_target(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) {
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
                kind: SelectedInstructionKind::WriteRuntimeMachineIndexedInteger {
                    base_byte_offset: indexed_target.base_byte_offset,
                    index_region: indexed_target.index_region,
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

    if let Some(indexed_target) = resolve_runtime_frame_fixed_indexed_target(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) && supports_scalar_integer_write(indexed_target.byte_count)
        && let Some(value) = resolve_runtime_static_integer_value(
            input,
            operation_source_key,
            value,
            aliases,
            alias_expressions,
            static_values,
        )
        && let Some(field_byte_offset) = indexed_target.pointee_field_byte_offset()
    {
        set_runtime_static_value(
            static_values,
            strip_mutable_expression(resolved_target.clone()),
            value,
        );
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WriteRuntimePointeeInteger {
                pointer_byte_offset: indexed_target.descriptor_offset,
                field_byte_offset,
                byte_size: indexed_target.byte_count,
                value,
            },
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return;
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

    if let Some(indexed_target) = resolve_runtime_frame_base_indexed_target(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) {
        return Some(
            SelectedInstructionKind::WriteRuntimeFrameBaseIndexedBinary {
                base_byte_offset: indexed_target.base_byte_offset,
                index_offset: indexed_target.index_offset,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                byte_size: indexed_target.byte_count,
                left,
                operator,
                right,
            },
        );
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
