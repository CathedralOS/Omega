use crate::plan::NativePlan;
use omega_control_flow::StateKey;
use omega_typed_program::expression::Expression;

use super::super::super::bindings::{
    RuntimeAliasBinding, append_place_suffix, resolve_runtime_alias_binding,
    strip_mutable_expression,
};
use super::super::super::model::{SelectedInstruction, SelectedInstructionKind};
use super::super::super::storage_places::resolve_machine_owned_place;
use super::super::text_writes::{
    runtime_text_builder_write, select_runtime_string_descriptor_write,
};
use super::static_values::{
    RuntimeStaticValues, resolve_runtime_static_integer_value, set_runtime_static_value,
};
use super::storage_copy::runtime_storage_copy;

#[allow(clippy::too_many_arguments)]
pub(super) fn select_runtime_mutation_writes(
    native_plan: &NativePlan,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    target: &Expression,
    value: &Expression,
    aliases: &[RuntimeAliasBinding],
    static_values: &mut RuntimeStaticValues,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let resolved_target = resolve_runtime_alias_binding(target, source_key, aliases);
    select_runtime_resolved_target_mutation_writes(
        native_plan,
        dispatch_index,
        source_key,
        resolved_target.source_key,
        source_machine,
        source_state,
        statement_index,
        &resolved_target.expression,
        value,
        aliases,
        static_values,
        selected_instructions,
    );
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_resolved_target_mutation_writes(
    native_plan: &NativePlan,
    dispatch_index: u32,
    operation_source_key: StateKey,
    target_source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    value: &Expression,
    aliases: &[RuntimeAliasBinding],
    static_values: &mut RuntimeStaticValues,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    if let Expression::StructLiteral(struct_literal) = value {
        for field in &struct_literal.fields {
            let field_target =
                append_place_suffix(resolved_target, std::slice::from_ref(&field.name));
            select_runtime_resolved_target_mutation_writes(
                native_plan,
                dispatch_index,
                operation_source_key,
                target_source_key,
                source_machine,
                source_state,
                statement_index,
                &field_target,
                &field.value,
                aliases,
                static_values,
                selected_instructions,
            );
        }
        return;
    }

    if let Expression::String(value) = value {
        select_runtime_string_descriptor_write(
            native_plan,
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

    if let Some(instructions) = runtime_text_builder_write(
        native_plan,
        dispatch_index,
        operation_source_key,
        source_machine,
        source_state,
        statement_index,
        resolved_target,
        aliases,
    ) {
        for kind in instructions {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key: operation_source_key,
                source_statement: statement_index,
            });
        }
        return;
    }

    let resolved_value = resolve_runtime_alias_binding(value, operation_source_key, aliases);
    if let Some(copy) = runtime_storage_copy(
        native_plan,
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

    let Some(value) = resolve_runtime_static_integer_value(
        native_plan,
        operation_source_key,
        value,
        aliases,
        static_values,
    ) else {
        return;
    };
    let Some((byte_offset, byte_size)) = resolve_machine_owned_place(
        &native_plan.layouts,
        native_plan.entry_key.machine,
        target_source_key.machine,
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
        kind: SelectedInstructionKind::WriteRuntimeMachineInteger {
            byte_offset,
            byte_size,
            value,
        },
        source_key: operation_source_key,
        source_statement: statement_index,
    });
}
