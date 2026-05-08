use crate::control_flow::StateKey;
use crate::plan::NativePlan;
use crate::runtime_dispatch::bodies::{
    RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind,
};
use omega_typed_program::expression::Expression;

use super::super::bindings::{
    RuntimeAliasBinding, append_place_suffix, resolve_runtime_alias_expression,
    strip_mutable_expression,
};
use super::super::lookups::state_mutation_for_statement;
use super::super::model::{SelectedInstruction, SelectedInstructionKind};
use super::super::storage_places::{
    enum_variant_value, resolve_machine_owned_place, resolve_runtime_storage_place,
};
use super::text_writes::{runtime_text_builder_write, select_runtime_string_descriptor_write};

pub(super) fn select_runtime_storage_write_for_operation(
    native_plan: &NativePlan,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    aliases: &[RuntimeAliasBinding],
    static_values: &mut Vec<(String, i64)>,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let RuntimeDispatchBodyOperationKind::Mutation { .. } = &operation.kind else {
        return;
    };
    let Some(mutation) =
        state_mutation_for_statement(native_plan, operation.source_key, operation.statement_index)
    else {
        return;
    };

    select_runtime_mutation_writes(
        native_plan,
        dispatch_index,
        mutation.source_key,
        &operation.source_machine,
        &operation.source_state,
        mutation.statement_index,
        &mutation.target,
        &mutation.value,
        aliases,
        static_values,
        selected_instructions,
    );
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_mutation_writes(
    native_plan: &NativePlan,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    target: &Expression,
    value: &Expression,
    aliases: &[RuntimeAliasBinding],
    static_values: &mut Vec<(String, i64)>,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let resolved_target = resolve_runtime_alias_expression(target, source_key, aliases);

    if let Expression::StructLiteral(struct_literal) = value {
        for field in &struct_literal.fields {
            let field_target =
                append_place_suffix(&resolved_target, std::slice::from_ref(&field.name));
            select_runtime_mutation_writes(
                native_plan,
                dispatch_index,
                source_key,
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
            source_key,
            source_machine,
            source_state,
            statement_index,
            &resolved_target,
            value,
            selected_instructions,
        );
        return;
    }

    if let Some(instructions) = runtime_text_builder_write(
        native_plan,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        statement_index,
        &resolved_target,
        aliases,
    ) {
        for kind in instructions {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_machine: source_machine.to_owned().into(),
                source_state: source_state.to_owned().into(),
                source_statement: statement_index,
            });
        }
        return;
    }

    let resolved_value = resolve_runtime_alias_expression(value, source_key, aliases);
    if let Some(copy) = runtime_storage_copy(
        native_plan,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        &resolved_target,
        &resolved_value,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind: copy,
            source_machine: source_machine.to_owned().into(),
            source_state: source_state.to_owned().into(),
            source_statement: statement_index,
        });
        return;
    }

    let Some(value) = resolve_runtime_static_integer_value(
        native_plan,
        source_key,
        value,
        aliases,
        static_values,
    ) else {
        return;
    };
    let Some((byte_offset, byte_size)) = resolve_machine_owned_place(
        &native_plan.layouts,
        &native_plan.entry_machine,
        source_machine,
        &resolved_target,
    ) else {
        return;
    };

    set_runtime_static_value(
        static_values,
        strip_mutable_expression(resolved_target.clone()).display_name(),
        value,
    );
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeMachineInteger {
            byte_offset,
            byte_size,
            value,
        },
        source_machine: source_machine.to_owned().into(),
        source_state: source_state.to_owned().into(),
        source_statement: statement_index,
    });
}

pub(super) fn runtime_storage_copy(
    native_plan: &NativePlan,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    target: &Expression,
    value: &Expression,
) -> Option<SelectedInstructionKind> {
    let target_place = resolve_runtime_storage_place(
        native_plan,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        target,
    )?;
    let source_place = resolve_runtime_storage_place(
        native_plan,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        value,
    )?;
    if target_place.byte_count != source_place.byte_count || target_place.byte_count == 0 {
        return None;
    }

    Some(SelectedInstructionKind::CopyRuntimeStorage {
        source_symbol: source_place.symbol,
        source_offset: source_place.byte_offset,
        target_symbol: target_place.symbol,
        target_offset: target_place.byte_offset,
        byte_count: target_place.byte_count,
    })
}

fn resolve_runtime_static_integer_value(
    native_plan: &NativePlan,
    source_key: StateKey,
    expression: &Expression,
    aliases: &[RuntimeAliasBinding],
    static_values: &[(String, i64)],
) -> Option<i64> {
    match expression {
        Expression::Integer(value) => Some(*value),
        Expression::Name(_) => enum_variant_value(&native_plan.layouts, expression).or_else(|| {
            let resolved_expression =
                resolve_runtime_alias_expression(expression, source_key, aliases);
            let resolved_expression = strip_mutable_expression(resolved_expression);
            static_values
                .iter()
                .find(|(target, _)| target == &resolved_expression.display_name())
                .map(|(_, value)| *value)
        }),
        Expression::Indexed(_) | Expression::Mutable(_) => {
            let resolved_expression =
                resolve_runtime_alias_expression(expression, source_key, aliases);
            let resolved_expression = strip_mutable_expression(resolved_expression);
            static_values
                .iter()
                .find(|(target, _)| target == &resolved_expression.display_name())
                .map(|(_, value)| *value)
        }
        Expression::Boolean(value) => Some(i64::from(*value)),
        Expression::ArrayLiteral(_)
        | Expression::Binary(_)
        | Expression::Float(_)
        | Expression::String(_)
        | Expression::StructLiteral(_) => None,
    }
}

fn set_runtime_static_value(static_values: &mut Vec<(String, i64)>, target: String, value: i64) {
    if let Some((_, existing_value)) = static_values
        .iter_mut()
        .find(|(existing_target, _)| existing_target == &target)
    {
        *existing_value = value;
    } else {
        static_values.push((target, value));
    }
}
