mod mutation;
mod static_values;
mod storage_copy;

use super::super::bindings::{RuntimeAliasBinding, resolve_runtime_alias_binding_handle};
use super::super::lookups::state_mutation_for_statement;
use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_checked_trees::expression::ExpressionHandle;
use omega_checked_trees::expression::ExpressionTable;
use omega_checked_trees::name::ProgramName;
use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_runtime_bodies::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
use omega_target_operations::{RuntimeValueOperand, SelectedInstruction};
pub(super) use static_values::RuntimeStaticValues;

pub(in crate::selection::runtime_dispatch) use mutation::select_runtime_frame_slot_value_write_in_table;
pub(super) use storage_copy::{
    runtime_storage_copy, runtime_storage_copy_in_table, runtime_storage_indirect_copy_in_table,
};

pub(super) fn select_runtime_storage_write_for_operation(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    static_values: &mut RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    match &operation.kind {
        RuntimeDispatchBodyOperationKind::Mutation { .. } => {}
        RuntimeDispatchBodyOperationKind::StateCallResult {
            role,
            call_ordinal,
            target_key,
            value,
            ..
        } => {
            mutation::select_runtime_state_call_result_write(
                input,
                dispatch_index,
                operation.source_key,
                operation.statement_index,
                *role,
                *call_ordinal,
                *target_key,
                *value,
                aliases,
                alias_expressions,
                static_values,
                runtime_value_operands,
                selected_instructions,
            );
            return;
        }
        _ => return,
    }
    let Some(mutation) =
        state_mutation_for_statement(input, operation.source_key, operation.statement_index)
    else {
        return;
    };

    if aliases.is_empty() {
        if select_runtime_storage_mutation_write_in_table(
            input,
            dispatch_index,
            mutation.source_key,
            mutation.statement_index,
            mutation.target,
            mutation.value,
            static_values,
            runtime_value_operands,
            selected_instructions,
        ) {
            return;
        }
    } else {
        let mut expressions = alias_expressions.clone();
        let target = expressions.copy_from(&input.state_storage.expressions, mutation.target);
        let value = expressions.copy_from(&input.state_storage.expressions, mutation.value);
        let resolved_target = resolve_runtime_alias_binding_handle(
            target,
            mutation.source_key,
            aliases,
            &mut expressions,
        );
        let resolved_value = resolve_runtime_alias_binding_handle(
            value,
            mutation.source_key,
            aliases,
            &mut expressions,
        );
        if select_runtime_storage_resolved_mutation_write_in_table(
            input,
            dispatch_index,
            mutation.source_key,
            resolved_target.source_key,
            resolved_value.source_key,
            mutation.statement_index,
            &expressions,
            resolved_target.expression,
            resolved_value.expression,
            static_values,
            runtime_value_operands,
            selected_instructions,
        ) {
            return;
        }
    }

    let (source_machine, source_state) = state_names(input, mutation.source_key);
    let target = input.state_storage.expressions.to_tree(mutation.target);
    let value = input.state_storage.expressions.to_tree(mutation.value);
    mutation::select_runtime_mutation_writes(
        input,
        dispatch_index,
        mutation.source_key,
        &source_machine,
        &source_state,
        mutation.statement_index,
        &target,
        &value,
        aliases,
        alias_expressions,
        static_values,
        runtime_value_operands,
        selected_instructions,
    );
}

pub(in crate::selection::runtime_dispatch) fn select_runtime_storage_mutation_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    target: ExpressionHandle,
    value: ExpressionHandle,
    static_values: &mut RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    select_runtime_storage_resolved_mutation_write_in_table(
        input,
        dispatch_index,
        source_key,
        source_key,
        source_key,
        statement_index,
        &input.state_storage.expressions,
        target,
        value,
        static_values,
        runtime_value_operands,
        selected_instructions,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_storage_resolved_mutation_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_source_key: StateKey,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
    static_values: &mut RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    if let Some(kind) = storage_copy::runtime_storage_copy_in_table(
        input,
        dispatch_index,
        target_source_key,
        value_source_key,
        expressions,
        target,
        value,
    )
    .or_else(|| {
        storage_copy::runtime_storage_indirect_copy_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            expressions,
            target,
            value,
        )
    })
    .or_else(|| {
        mutation::select_runtime_static_mutation_write_in_table(
            input,
            dispatch_index,
            target_source_key,
            statement_index,
            expressions,
            target,
            value,
            static_values,
        )
    })
    .or_else(|| {
        mutation::select_runtime_string_mutation_write_in_table(
            input,
            dispatch_index,
            operation_source_key,
            target_source_key,
            statement_index,
            expressions,
            target,
            value,
        )
    })
    .or_else(|| {
        mutation::select_runtime_binary_mutation_write_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            statement_index,
            expressions,
            target,
            value,
            static_values,
            runtime_value_operands,
        )
    }) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return true;
    }

    false
}

fn state_names(
    input: &InstructionSelectionInput<'_>,
    key: omega_control_flow::StateKey,
) -> (ProgramName, ProgramName) {
    input.control_flow.state_names_by_key_cloned(key)
}
