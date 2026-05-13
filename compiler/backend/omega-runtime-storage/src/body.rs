use super::{RuntimeFrameSlot, RuntimeStorageBodyInput, RuntimeStorageContext, RuntimeStoragePlan};
use omega_control_flow::StateKey;
use omega_core::arena::HandleSpan;
use omega_runtime_bodies::RuntimeDispatchBodyOperationKind;
use omega_state_storage::{StateMutation, StateMutationLowering};

use super::layout::{align_to, layout_for_type};

pub(super) fn build_runtime_storage_body_plan(
    context: &RuntimeStorageContext,
    body_input: &RuntimeStorageBodyInput,
) -> RuntimeStoragePlan {
    let mut plan = RuntimeStoragePlan::default();
    let mut next_frame_offset = 0usize;
    append_parameter_slots(
        context,
        body_input,
        &mut plan,
        &mut next_frame_offset,
    );
    let Some(operations) = context
        .runtime_bodies
        .operations
        .paged_span(body_input.body.operations)
    else {
        return plan;
    };

    for operation in operations.iter() {
        match &operation.kind {
            RuntimeDispatchBodyOperationKind::LocalStorage {
                symbol,
                name,
                type_symbol,
                type_name,
                invariant_names,
            } => {
                let layout = layout_for_type(context, *type_symbol, type_name);
                let byte_offset = align_to(next_frame_offset, layout.alignment);
                next_frame_offset = byte_offset
                    .checked_add(layout.size)
                    .expect("runtime frame slot size overflow");

                plan.frame_slots.insert(RuntimeFrameSlot {
                    dispatch_index: body_input.body.dispatch_index,
                    source_key: operation.source_key,
                    statement_index: operation.statement_index,
                    symbol: *symbol,
                    name: name.clone(),
                    type_symbol: *type_symbol,
                    type_name: type_name.clone(),
                    invariant_names: plan.invariant_names.insert_many(
                        context
                            .runtime_bodies
                            .invariant_names
                            .span_or_empty(*invariant_names)
                            .iter()
                            .cloned(),
                    ),
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
                    plan.writes.insert(super::RuntimeStorageWrite {
                        dispatch_index: body_input.body.dispatch_index,
                        source_key: operation.source_key,
                        statement_index: operation.statement_index,
                        target: plan
                            .expressions
                            .copy_from(&context.state_storage.expressions, mutation.target),
                        value: plan
                            .expressions
                            .copy_from(&context.state_storage.expressions, mutation.value),
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

fn append_parameter_slots(
    context: &RuntimeStorageContext,
    body_input: &RuntimeStorageBodyInput,
    plan: &mut RuntimeStoragePlan,
    next_frame_offset: &mut usize,
) {
    let Some(state) = context.control_flow.state_by_key(body_input.body.key) else {
        return;
    };

    for parameter in &state.parameters {
        let layout = layout_for_type(context, parameter.type_symbol, parameter.type_name.as_str());
        let byte_offset = align_to(*next_frame_offset, layout.alignment);
        *next_frame_offset = byte_offset
            .checked_add(layout.size)
            .expect("runtime parameter slot size overflow");

        plan.frame_slots.insert(RuntimeFrameSlot {
            dispatch_index: body_input.body.dispatch_index,
            source_key: body_input.body.key,
            statement_index: usize::MAX,
            symbol: parameter.symbol,
            name: parameter.name.clone(),
            type_symbol: parameter.type_symbol,
            type_name: parameter.type_name.to_string(),
            invariant_names: HandleSpan::empty(),
            byte_offset,
            byte_size: layout.size,
            alignment: layout.alignment,
        });
    }
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
