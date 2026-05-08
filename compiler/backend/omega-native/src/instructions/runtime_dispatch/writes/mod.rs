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

    mutation::select_runtime_mutation_writes(
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
