mod mutation;
mod static_values;
mod storage_copy;

use super::super::bindings::RuntimeAliasBinding;
use super::super::lookups::state_mutation_for_statement;
use super::super::model::SelectedInstruction;
use crate::plan::NativePlan;
use crate::runtime_dispatch::bodies::{
    RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind,
};
use omega_typed_program::name::ProgramName;
use static_values::RuntimeStaticValues;

pub(super) use storage_copy::runtime_storage_copy;

pub(super) fn select_runtime_storage_write_for_operation(
    native_plan: &NativePlan,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    aliases: &[RuntimeAliasBinding],
    static_values: &mut RuntimeStaticValues,
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

    let (source_machine, source_state) = state_names(native_plan, mutation.source_key);
    mutation::select_runtime_mutation_writes(
        native_plan,
        dispatch_index,
        mutation.source_key,
        &source_machine,
        &source_state,
        mutation.statement_index,
        &mutation.target,
        &mutation.value,
        aliases,
        static_values,
        selected_instructions,
    );
}

fn state_names(
    native_plan: &NativePlan,
    key: crate::control_flow::StateKey,
) -> (ProgramName, ProgramName) {
    native_plan
        .control_flow
        .state_names_by_key(key)
        .map(|(machine, state)| (machine.clone(), state.clone()))
        .unwrap_or_default()
}
