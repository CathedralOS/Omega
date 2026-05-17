use super::{RuntimeFrameSlot, RuntimeStorageBodyInput, RuntimeStorageContext, RuntimeStoragePlan};
use crate::model::RuntimeFrameSlotKind;
use omega_checked_trees::name::ProgramName;
use omega_checked_trees::types::TypeReferenceHandle;
use omega_control_flow::StateKey;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use omega_runtime_bodies::RuntimeDispatchBodyOperationKind;
use omega_state_calls::StateCallRole;
use omega_state_storage::{StateMutation, StateMutationLowering};

use super::layout::{align_to, layout_for_type, layout_for_type_reference};

pub(super) fn build_runtime_storage_body_plan(
    context: &RuntimeStorageContext,
    body_input: &RuntimeStorageBodyInput,
) -> RuntimeStoragePlan {
    let mut plan = RuntimeStoragePlan::default();
    let mut next_frame_offset = 0usize;
    append_parameter_slots(context, body_input, &mut plan, &mut next_frame_offset);
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
                type_reference,
                invariant_names,
            } => {
                let layout = layout_for_type_reference(
                    context,
                    &context.runtime_bodies.type_references,
                    *type_reference,
                );
                let byte_offset = align_to(next_frame_offset, layout.alignment);
                next_frame_offset = byte_offset
                    .checked_add(layout.size)
                    .expect("runtime frame slot size overflow");

                plan.frame_slots.insert(RuntimeFrameSlot {
                    dispatch_index: body_input.body.dispatch_index,
                    source_key: operation.source_key,
                    statement_index: operation.statement_index,
                    kind: RuntimeFrameSlotKind::LocalStorage,
                    symbol: *symbol,
                    name: name.clone(),
                    type_symbol: *type_symbol,
                    type_name: context
                        .runtime_bodies
                        .type_references
                        .display_name(*type_reference),
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
            RuntimeDispatchBodyOperationKind::InlineLeafStateCall {
                role,
                call_ordinal,
                target_key,
                ..
            }
            | RuntimeDispatchBodyOperationKind::InlineStateCall {
                role,
                call_ordinal,
                target_key,
                ..
            }
            | RuntimeDispatchBodyOperationKind::StateCall {
                role,
                call_ordinal,
                target_key,
                ..
            } => append_state_call_result_slot(
                context,
                body_input,
                &mut plan,
                &mut next_frame_offset,
                operation.source_key,
                operation.statement_index,
                *role,
                *call_ordinal,
                *target_key,
            ),
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

    for parameter in context.control_flow.state_parameters(state) {
        let layout = layout_for_type(context, parameter.type_symbol, parameter.type_name.as_str());
        let byte_offset = align_to(*next_frame_offset, layout.alignment);
        *next_frame_offset = byte_offset
            .checked_add(layout.size)
            .expect("runtime parameter slot size overflow");

        plan.frame_slots.insert(RuntimeFrameSlot {
            dispatch_index: body_input.body.dispatch_index,
            source_key: body_input.body.key,
            statement_index: usize::MAX,
            kind: RuntimeFrameSlotKind::Parameter,
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

#[allow(clippy::too_many_arguments)]
fn append_state_call_result_slot(
    context: &RuntimeStorageContext,
    body_input: &RuntimeStorageBodyInput,
    plan: &mut RuntimeStoragePlan,
    next_frame_offset: &mut usize,
    source_key: StateKey,
    statement_index: usize,
    role: StateCallRole,
    call_ordinal: usize,
    target_key: StateKey,
) {
    if !matches!(
        role,
        StateCallRole::AssignmentValue
            | StateCallRole::TransitionArgument
            | StateCallRole::TransitionGuard
    ) {
        return;
    }

    let return_type = state_return_type(context, target_key);
    if !return_type.is_valid() {
        return;
    };
    let type_symbol = context.program.type_reference_symbol(return_type);
    let type_name = context.program.display_type_reference(return_type);
    let layout = layout_for_type(context, type_symbol, &type_name);
    if layout.size == 0 {
        return;
    }

    let byte_offset = align_to(*next_frame_offset, layout.alignment);
    *next_frame_offset = byte_offset
        .checked_add(layout.size)
        .expect("runtime state-call result slot size overflow");

    plan.frame_slots.insert(RuntimeFrameSlot {
        dispatch_index: body_input.body.dispatch_index,
        source_key,
        statement_index,
        kind: RuntimeFrameSlotKind::StateCallResult {
            role,
            call_ordinal,
            target_key,
        },
        symbol: SymbolHandle::invalid(),
        name: ProgramName::generated(&format!(
            "__call_result_{}_{}",
            statement_index, call_ordinal
        )),
        type_symbol,
        type_name,
        invariant_names: HandleSpan::empty(),
        byte_offset,
        byte_size: layout.size,
        alignment: layout.alignment,
    });
}

fn state_return_type(context: &RuntimeStorageContext, target_key: StateKey) -> TypeReferenceHandle {
    let Some(machine) = context
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == target_key.machine)
    else {
        return TypeReferenceHandle::invalid();
    };
    let Some(state) = context
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == target_key.state)
    else {
        return TypeReferenceHandle::invalid();
    };
    state.return_type
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
