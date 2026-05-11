use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_control_flow::StateKey;
use omega_typed_trees::expression::{Expression, ExpressionTable};

use super::super::super::bindings::{
    RuntimeAliasBinding, append_place_suffix, resolve_runtime_alias_binding,
    strip_mutable_expression,
};
use super::super::super::storage_places::resolve_runtime_storage_place;
use super::super::super::storage_places::resolve_runtime_frame_indexed_target;
use super::super::text_writes::{
    runtime_text_builder_write_emit, select_runtime_string_descriptor_write,
};
use super::static_values::{
    RuntimeStaticValues, resolve_runtime_static_integer_value, set_runtime_static_value,
};
use super::storage_copy::runtime_storage_copy;
use omega_target_operations::{SelectedInstruction, SelectedInstructionKind};

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
    select_runtime_resolved_target_mutation_writes(
        input,
        dispatch_index,
        source_key,
        resolved_target.source_key,
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
fn select_runtime_resolved_target_mutation_writes(
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
    static_values: &mut RuntimeStaticValues,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if let Expression::StructLiteral(struct_literal) = value {
        for field in &struct_literal.fields {
            let field_target =
                append_place_suffix(resolved_target, std::slice::from_ref(&field.name));
            select_runtime_resolved_target_mutation_writes(
                input,
                dispatch_index,
                operation_source_key,
                target_source_key,
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
        select_runtime_string_descriptor_write(
            input,
            operation_source_key,
            target_source_key,
            source_machine,
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
        resolve_runtime_alias_binding(value, operation_source_key, aliases, alias_expressions);
    if let Expression::String(value) = &resolved_value.expression {
        select_runtime_string_descriptor_write(
            input,
            operation_source_key,
            target_source_key,
            source_machine,
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

        if let Some(value) = resolve_runtime_static_integer_value(
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
