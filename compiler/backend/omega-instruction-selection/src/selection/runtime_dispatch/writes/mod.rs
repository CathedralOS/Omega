mod mutation;
mod static_values;
mod storage_copy;

use super::super::bindings::RuntimeAliasBinding;
use super::super::lookups::state_mutation_for_statement;
use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_checked_trees::expression::ExpressionTable;
use omega_checked_trees::name::ProgramName;
use omega_core::arena::Arena;
use omega_runtime_bodies::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
use omega_target_operations::{RuntimeValueOperand, SelectedInstruction};
pub(super) use static_values::RuntimeStaticValues;

pub(super) use storage_copy::runtime_storage_copy;

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

    let (source_machine, source_state) = state_names(input, mutation.source_key);
    if aliases.is_empty()
        && let Some(copy) = storage_copy::runtime_storage_copy_in_table(
            input,
            dispatch_index,
            mutation.source_key,
            mutation.source_key,
            &input.state_storage.expressions,
            mutation.target,
            mutation.value,
        )
    {
        selected_instructions.push(SelectedInstruction {
            kind: copy,
            source_key: mutation.source_key,
            source_statement: mutation.statement_index,
        });
        return;
    }
    if aliases.is_empty()
        && let Some(copy) = storage_copy::runtime_storage_indirect_copy_in_table(
            input,
            dispatch_index,
            mutation.source_key,
            mutation.source_key,
            &input.state_storage.expressions,
            mutation.target,
            mutation.value,
        )
    {
        selected_instructions.push(SelectedInstruction {
            kind: copy,
            source_key: mutation.source_key,
            source_statement: mutation.statement_index,
        });
        return;
    }
    if aliases.is_empty()
        && let Some(kind) = mutation::select_runtime_static_mutation_write_in_table(
            input,
            dispatch_index,
            mutation.source_key,
            mutation.statement_index,
            mutation.target,
            mutation.value,
            static_values,
        )
    {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: mutation.source_key,
            source_statement: mutation.statement_index,
        });
        return;
    }
    if aliases.is_empty()
        && let Some(kind) = mutation::select_runtime_binary_mutation_write_in_table(
            input,
            dispatch_index,
            mutation.source_key,
            mutation.statement_index,
            mutation.target,
            mutation.value,
            static_values,
            runtime_value_operands,
        )
    {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: mutation.source_key,
            source_statement: mutation.statement_index,
        });
        return;
    }

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

fn state_names(
    input: &InstructionSelectionInput<'_>,
    key: omega_control_flow::StateKey,
) -> (ProgramName, ProgramName) {
    input.control_flow.state_names_by_key_cloned(key)
}
