use super::{RuntimeFrameSlot, RuntimeStorageBodyInput, RuntimeStorageContext, RuntimeStoragePlan};
use crate::control_flow::StateKey;
use crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperationKind;
use crate::state_storage::{StateMutation, StateMutationLowering};
use omega_typed_program::name::ProgramName;

use super::layout::{align_to, layout_for_type_name};

pub(super) fn build_runtime_storage_body_plan(
    context: &RuntimeStorageContext,
    body_input: &RuntimeStorageBodyInput,
) -> RuntimeStoragePlan {
    let mut plan = RuntimeStoragePlan::default();
    let mut next_frame_offset = 0usize;

    for operation in &body_input.operations {
        match &operation.kind {
            RuntimeDispatchBodyOperationKind::LocalStorage { name, type_name } => {
                let (source_machine, source_state) = state_names(context, operation.source_key);
                let layout = layout_for_type_name(context, type_name);
                let byte_offset = align_to(next_frame_offset, layout.alignment);
                next_frame_offset = byte_offset
                    .checked_add(layout.size)
                    .expect("runtime frame slot size overflow");

                plan.frame_slots.insert(RuntimeFrameSlot {
                    dispatch_index: body_input.body.dispatch_index,
                    source_key: operation.source_key,
                    source_machine,
                    source_state,
                    statement_index: operation.statement_index,
                    name: name.clone(),
                    type_name: type_name.clone(),
                    byte_offset,
                    byte_size: layout.size,
                    alignment: layout.alignment,
                });
            }
            RuntimeDispatchBodyOperationKind::Mutation { lowering, .. }
                if *lowering != StateMutationLowering::AlreadyLowered =>
            {
                if let Some(mutation) =
                    mutation_for_operation(context, operation.source_key, operation.statement_index)
                {
                    let (source_machine, source_state) = state_names(context, operation.source_key);
                    plan.writes.insert(super::RuntimeStorageWrite {
                        dispatch_index: body_input.body.dispatch_index,
                        source_key: operation.source_key,
                        source_machine,
                        source_state,
                        statement_index: operation.statement_index,
                        target: mutation.target.clone(),
                        value: mutation.value.clone(),
                        mutation_kind: mutation.mutation_kind,
                        lowering: mutation.lowering,
                    });
                }
            }
            _ => {}
        }
    }

    plan
}

fn state_names(context: &RuntimeStorageContext, key: StateKey) -> (ProgramName, ProgramName) {
    let machine = context
        .control_flow
        .machine_by_symbol(key.machine)
        .map(|machine| machine.name.clone())
        .unwrap_or_default();
    let state = context
        .control_flow
        .state_by_key(key)
        .map(|state| state.name.clone())
        .unwrap_or_default();
    (machine, state)
}

fn mutation_for_operation<'plan>(
    context: &'plan RuntimeStorageContext,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan StateMutation> {
    context
        .state_storage
        .mutations
        .iter()
        .find(|(_, mutation)| {
            mutation.source_key == source_key && mutation.statement_index == statement_index
        })
        .map(|(_, mutation)| mutation)
}
