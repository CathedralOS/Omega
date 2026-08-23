use super::{RuntimeFrameSlot, RuntimeStorageContext, RuntimeStoragePlan};
use crate::model::RuntimeFrameSlotKind;
use omega_control_flow::{PlannedTransitionTarget, StateKey};
use omega_runtime_bodies::{RuntimeDispatchBody, RuntimeDispatchBodyOperationKind};
use omega_state_calls::{StateCall, StateCallLowering, StateCallRole};
use omega_state_storage::{StateLocalStorage, StateMutation, StateMutationLowering};
use psi_arena::HandleSpan;
use psi_checked_trees::expression::{ExpressionNode, ExpressionTable, ExpressionTableCapacity};
use psi_checked_trees::name::Identifier;
use psi_checked_trees::statement::StatementNode;
use psi_checked_trees::types::{FixedArrayLength, TypeReferenceHandle};
use psi_symbols::SymbolHandle;
use std::sync::Arc;

use super::layout::{align_to, bounded_byte_buffer_shape, layout_for_type_reference};

const BRANCH_STORAGE_VISITING_COUNT: usize = 32;

fn state_key_matches_statement_source(expected: StateKey, actual: StateKey) -> bool {
    expected == actual || (expected.machine == actual.machine && expected.state == actual.state)
}

struct BranchStorageVisitingStates {
    inline: [Option<StateKey>; BRANCH_STORAGE_VISITING_COUNT],
    len: usize,
    overflow: Vec<StateKey>,
}

impl BranchStorageVisitingStates {
    fn with_capacity(state_capacity: usize) -> Self {
        Self {
            inline: [None; BRANCH_STORAGE_VISITING_COUNT],
            len: 0,
            overflow: Vec::with_capacity(
                state_capacity.saturating_sub(BRANCH_STORAGE_VISITING_COUNT),
            ),
        }
    }

    fn contains(&self, key: StateKey) -> bool {
        self.inline
            .iter()
            .take(self.len.min(BRANCH_STORAGE_VISITING_COUNT))
            .flatten()
            .any(|candidate| *candidate == key)
            || self.overflow.contains(&key)
    }

    fn push(&mut self, key: StateKey) {
        if self.len < BRANCH_STORAGE_VISITING_COUNT {
            self.inline[self.len] = Some(key);
        } else {
            self.overflow.push(key);
        }

        self.len += 1;
    }

    fn pop(&mut self) {
        if self.len == 0 {
            return;
        }

        self.len -= 1;
        if self.len < BRANCH_STORAGE_VISITING_COUNT {
            self.inline[self.len] = None;
        } else {
            self.overflow.pop();
        }
    }
}

pub(super) fn build_runtime_storage_body_plan(
    context: &RuntimeStorageContext,
    body: &RuntimeDispatchBody,
) -> RuntimeStoragePlan {
    let Some(operations) = context
        .runtime_bodies
        .operations
        .paged_span(body.operations)
    else {
        return RuntimeStoragePlan::default();
    };
    let parameter_count = context
        .control_flow
        .state_by_key(body.key)
        .map(|state| context.control_flow.state_parameters(state).len())
        .unwrap_or(0);
    let local_count = operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.kind,
                RuntimeDispatchBodyOperationKind::LocalStorage { .. }
            )
        })
        .count();
    let write_count = operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.kind,
                RuntimeDispatchBodyOperationKind::Mutation { lowering, .. }
                    if lowering != StateMutationLowering::AlreadyLowered
            )
        })
        .count();
    let mut plan = RuntimeStoragePlan::with_capacities(
        ExpressionTableCapacity {
            expressions: write_count.saturating_mul(2),
            ..ExpressionTableCapacity::default()
        },
        local_count,
        parameter_count.saturating_add(operations.len()),
        write_count,
    );
    let mut next_frame_offset = 0usize;
    append_parameter_slots(context, body, &mut plan, &mut next_frame_offset);

    for operation in operations.iter() {
        match &operation.kind {
            RuntimeDispatchBodyOperationKind::LocalStorage {
                symbol,
                name,
                type_symbol,
                type_reference,
                invariant_names,
            } => {
                // Dispatch-root locals take this direct arm instead of the
                // branch/straight-line helper below. Preserve the same recast
                // record-view rule here: `let d: &Record = &bytes[K] as
                // &Record` reserves the referee width used to distinguish its
                // address-bearing carrier from an ordinary pointer slot.
                let layout = local_storage_for_operation(
                    context,
                    operation.source_key,
                    operation.statement_index,
                )
                .and_then(|local| recast_view_slot_layout(context, local))
                .unwrap_or_else(|| {
                    layout_for_type_reference(
                        context,
                        &context.runtime_bodies.type_references,
                        *type_reference,
                    )
                });
                append_local_slot(
                    &mut plan,
                    body.dispatch_index,
                    operation.source_key,
                    operation.statement_index,
                    *symbol,
                    name.clone(),
                    *type_symbol,
                    context
                        .runtime_bodies
                        .type_references
                        .display_name(*type_reference)
                        .into(),
                    type_descriptor(&context.runtime_bodies.type_references, *type_reference),
                    context
                        .runtime_bodies
                        .invariant_names
                        .span_or_empty(*invariant_names)
                        .iter()
                        .cloned(),
                    layout.size,
                    layout.alignment,
                    is_static_boundary_capability(context, *type_symbol),
                    &mut next_frame_offset,
                );
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
            } => append_state_call_result_slot(
                context,
                &mut plan,
                &mut next_frame_offset,
                body.dispatch_index,
                operation.source_key,
                operation.statement_index,
                *role,
                *call_ordinal,
                *target_key,
            ),
            RuntimeDispatchBodyOperationKind::StateCall {
                role,
                call_ordinal,
                target_key,
                lowering,
                ..
            } => {
                append_state_call_result_slot(
                    context,
                    &mut plan,
                    &mut next_frame_offset,
                    body.dispatch_index,
                    operation.source_key,
                    operation.statement_index,
                    *role,
                    *call_ordinal,
                    *target_key,
                );
                if *lowering == StateCallLowering::InlineBranching {
                    append_branch_expanded_storage(
                        context,
                        &mut plan,
                        &mut next_frame_offset,
                        body.dispatch_index,
                        *target_key,
                        &mut BranchStorageVisitingStates::with_capacity(
                            context.control_flow.states.len(),
                        ),
                    );
                }
            }
            // A host-call RESULT bound to a LOCAL (`let rc = self.host.close(fd)`)
            // in a DISPATCHING state: the statement lowered to a HostCall body op
            // (not a LocalStorage op), so without this arm the result local never
            // got a frame slot and its result operand resolved to nothing -- the
            // encoder then hard-errored ("no encodable call sequence"). Fields
            // dodged it (machine region, not dispatch-keyed); the straight-line
            // builder dodged it (its own `locals` scan). Idempotent: the shared
            // `append_local_slot` skips a slot that already exists.
            RuntimeDispatchBodyOperationKind::HostCall { .. } => {
                if let Some(local_storage) = local_storage_for_operation(
                    context,
                    operation.source_key,
                    operation.statement_index,
                ) {
                    append_branch_local_slot(
                        context,
                        &mut plan,
                        &mut next_frame_offset,
                        body.dispatch_index,
                        local_storage,
                    );
                }
            }
            RuntimeDispatchBodyOperationKind::Mutation { lowering, .. }
                if *lowering != StateMutationLowering::AlreadyLowered =>
            {
                if let Some(mutation) =
                    mutation_for_operation(context, operation.source_key, operation.statement_index)
                {
                    plan.writes.insert(super::RuntimeStorageWrite {
                        dispatch_index: body.dispatch_index,
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

pub(super) fn build_straight_line_runtime_storage_plan(
    context: &RuntimeStorageContext,
) -> RuntimeStoragePlan {
    let local_count = context.state_storage.locals.len();
    let parameter_count = context.control_flow.state_parameters.len();
    let write_count = context
        .state_storage
        .mutations
        .iter()
        .filter(|(_, mutation)| mutation.lowering != StateMutationLowering::AlreadyLowered)
        .count();
    let mut plan = RuntimeStoragePlan::with_capacities(
        ExpressionTableCapacity {
            expressions: write_count.saturating_mul(2),
            ..ExpressionTableCapacity::default()
        },
        local_count,
        parameter_count
            .saturating_add(local_count)
            .saturating_add(context.state_calls.calls.len()),
        write_count,
    );
    let mut next_frame_offset = 0usize;
    let dispatch_index = 0u32;

    for (_, machine) in context.control_flow.machines.iter() {
        let Some(states) = context.control_flow.states.span(machine.states) else {
            continue;
        };

        for state in states {
            append_parameter_slots_for_state(
                context,
                dispatch_index,
                state.key,
                &mut plan,
                &mut next_frame_offset,
            );
            append_straight_line_local_slots_for_state(
                context,
                dispatch_index,
                state.key,
                &mut plan,
                &mut next_frame_offset,
            );

            let Some(operations) = context.control_flow.operations.span(state.operations) else {
                continue;
            };

            for operation in operations {
                if let Some(local_storage) =
                    local_storage_for_operation(context, state.key, operation.statement_index)
                {
                    append_branch_local_slot(
                        context,
                        &mut plan,
                        &mut next_frame_offset,
                        dispatch_index,
                        local_storage,
                    );
                }

                for state_call in context
                    .state_calls
                    .calls_for_statement(state.key, operation.statement_index)
                    .filter(|state_call| state_call.role == StateCallRole::CallArgument)
                {
                    append_state_call_result_slot(
                        context,
                        &mut plan,
                        &mut next_frame_offset,
                        dispatch_index,
                        state_call.source_key,
                        state_call.statement_index,
                        state_call.role,
                        state_call.call_ordinal,
                        state_call.target_key,
                    );
                }

                if let Some(state_call) = context
                    .state_calls
                    .statement_call(state.key, operation.statement_index)
                {
                    append_state_call_result_slot(
                        context,
                        &mut plan,
                        &mut next_frame_offset,
                        dispatch_index,
                        state_call.source_key,
                        state_call.statement_index,
                        state_call.role,
                        state_call.call_ordinal,
                        state_call.target_key,
                    );
                }

                for state_call in context
                    .state_calls
                    .calls_for_statement(state.key, operation.statement_index)
                    .filter(|state_call| state_call.role == StateCallRole::AssignmentValue)
                {
                    append_state_call_result_slot(
                        context,
                        &mut plan,
                        &mut next_frame_offset,
                        dispatch_index,
                        state_call.source_key,
                        state_call.statement_index,
                        state_call.role,
                        state_call.call_ordinal,
                        state_call.target_key,
                    );
                }

                if let Some(mutation) =
                    mutation_for_operation(context, state.key, operation.statement_index)
                    && mutation.lowering != StateMutationLowering::AlreadyLowered
                {
                    plan.writes.insert(super::RuntimeStorageWrite {
                        dispatch_index,
                        source_key: state.key,
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

            for (_, state_call) in context.state_calls.calls.iter() {
                if state_call.source_key == state.key
                    && matches!(
                        state_call.role,
                        StateCallRole::TransitionArgument | StateCallRole::TransitionGuard
                    )
                {
                    append_state_call_result_slot(
                        context,
                        &mut plan,
                        &mut next_frame_offset,
                        dispatch_index,
                        state_call.source_key,
                        state_call.statement_index,
                        state_call.role,
                        state_call.call_ordinal,
                        state_call.target_key,
                    );
                }
            }
        }
    }

    plan
}

fn append_straight_line_local_slots_for_state(
    context: &RuntimeStorageContext,
    dispatch_index: u32,
    state_key: StateKey,
    plan: &mut RuntimeStoragePlan,
    next_frame_offset: &mut usize,
) {
    for (_, local_storage) in context.state_storage.locals.iter() {
        if !state_key_matches_statement_source(local_storage.source_key, state_key) {
            continue;
        }

        append_branch_local_slot(
            context,
            plan,
            next_frame_offset,
            dispatch_index,
            local_storage,
        );
    }
}

fn append_parameter_slots(
    context: &RuntimeStorageContext,
    body: &RuntimeDispatchBody,
    plan: &mut RuntimeStoragePlan,
    next_frame_offset: &mut usize,
) {
    append_parameter_slots_for_state(
        context,
        body.dispatch_index,
        body.key,
        plan,
        next_frame_offset,
    );
}

fn append_parameter_slots_for_state(
    context: &RuntimeStorageContext,
    dispatch_index: u32,
    state_key: StateKey,
    plan: &mut RuntimeStoragePlan,
    next_frame_offset: &mut usize,
) {
    let Some(state) = context.control_flow.state_by_key(state_key) else {
        return;
    };

    for parameter in context.control_flow.state_parameters(state) {
        let layout = layout_for_type_reference(
            context,
            &context.program.type_reference_table,
            parameter.type_reference,
        );
        let byte_offset = align_to(*next_frame_offset, layout.alignment);
        *next_frame_offset = byte_offset
            .checked_add(layout.size)
            .expect("runtime parameter slot size overflow");

        plan.frame_slots.insert(RuntimeFrameSlot {
            dispatch_index,
            source_key: state_key,
            statement_index: usize::MAX,
            kind: RuntimeFrameSlotKind::Parameter,
            symbol: parameter.symbol,
            name: parameter.name.clone(),
            type_symbol: parameter.type_symbol,
            type_name: parameter.type_name.as_str().into(),
            type_descriptor: type_descriptor(
                &context.program.type_reference_table,
                parameter.type_reference,
            ),
            invariant_names: HandleSpan::empty(),
            byte_offset,
            byte_size: layout.size,
            alignment: layout.alignment,
            is_static_boundary_capability: is_static_boundary_capability(
                context,
                parameter.type_symbol,
            ),
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn append_local_slot(
    plan: &mut RuntimeStoragePlan,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    symbol: SymbolHandle,
    name: Identifier,
    type_symbol: SymbolHandle,
    type_name: Arc<str>,
    type_descriptor: omega_layout::TypeLayoutDescriptor,
    invariant_names: impl IntoIterator<Item = Identifier>,
    byte_size: usize,
    alignment: usize,
    is_static_boundary_capability: bool,
    next_frame_offset: &mut usize,
) {
    if local_slot_exists(plan, dispatch_index, source_key, statement_index, symbol) {
        return;
    }

    let byte_offset = align_to(*next_frame_offset, alignment);
    *next_frame_offset = byte_offset
        .checked_add(byte_size)
        .expect("runtime frame slot size overflow");

    plan.frame_slots.insert(RuntimeFrameSlot {
        dispatch_index,
        source_key,
        statement_index,
        kind: RuntimeFrameSlotKind::LocalStorage,
        symbol,
        name,
        type_symbol,
        type_name,
        type_descriptor,
        invariant_names: plan.invariant_names.insert_many(invariant_names),
        byte_offset,
        byte_size,
        alignment,
        is_static_boundary_capability,
    });
}

fn append_branch_expanded_storage(
    context: &RuntimeStorageContext,
    plan: &mut RuntimeStoragePlan,
    next_frame_offset: &mut usize,
    dispatch_index: u32,
    state_key: StateKey,
    visiting: &mut BranchStorageVisitingStates,
) {
    if visiting.contains(state_key) {
        return;
    }
    visiting.push(state_key);

    let Some(state) = context.control_flow.state_by_key(state_key) else {
        visiting.pop();
        return;
    };
    let Some(operations) = context.control_flow.operations.span(state.operations) else {
        visiting.pop();
        return;
    };

    for operation in operations {
        append_branch_operation_storage(
            context,
            plan,
            next_frame_offset,
            dispatch_index,
            state_key,
            operation.statement_index,
            visiting,
        );
    }

    for (_, state_call) in context.state_calls.calls.iter() {
        if state_call.source_key == state_key
            && matches!(
                state_call.role,
                StateCallRole::TransitionArgument | StateCallRole::TransitionGuard
            )
        {
            append_branch_state_call_storage(
                context,
                plan,
                next_frame_offset,
                dispatch_index,
                state_call,
                visiting,
            );
        }
    }

    if let Some(transitions) = context.control_flow.transitions.span(state.transitions) {
        for transition in transitions {
            append_branch_transition_target_storage(
                context,
                plan,
                next_frame_offset,
                dispatch_index,
                state_key,
                &transition.target,
                visiting,
            );
            append_branch_transition_target_storage(
                context,
                plan,
                next_frame_offset,
                dispatch_index,
                state_key,
                &transition.continuation,
                visiting,
            );
        }
    }

    visiting.pop();
}

fn append_branch_operation_storage(
    context: &RuntimeStorageContext,
    plan: &mut RuntimeStoragePlan,
    next_frame_offset: &mut usize,
    dispatch_index: u32,
    state_key: StateKey,
    statement_index: usize,
    visiting: &mut BranchStorageVisitingStates,
) {
    if let Some(local_storage) = local_storage_for_operation(context, state_key, statement_index) {
        append_branch_local_slot(
            context,
            plan,
            next_frame_offset,
            dispatch_index,
            local_storage,
        );
    }

    for state_call in context
        .state_calls
        .calls_for_statement(state_key, statement_index)
        .filter(|state_call| state_call.role == StateCallRole::CallArgument)
    {
        append_branch_state_call_storage(
            context,
            plan,
            next_frame_offset,
            dispatch_index,
            state_call,
            visiting,
        );
    }

    if let Some(state_call) = context
        .state_calls
        .statement_call(state_key, statement_index)
    {
        append_branch_state_call_storage(
            context,
            plan,
            next_frame_offset,
            dispatch_index,
            state_call,
            visiting,
        );
        return;
    }

    for state_call in context
        .state_calls
        .calls_for_statement(state_key, statement_index)
        .filter(|state_call| state_call.role == StateCallRole::AssignmentValue)
    {
        append_branch_state_call_storage(
            context,
            plan,
            next_frame_offset,
            dispatch_index,
            state_call,
            visiting,
        );
    }
}

/// Rung C2: a reference-typed local INITIALIZED BY A JUDGED RECAST sizes by
/// its referee record (the wide-view carrier). `None` keeps the ordinary layout
/// -- the gate is the recast initializer, so boundary pointer-model locals
/// are untouched.
fn recast_view_slot_layout(
    context: &RuntimeStorageContext,
    local_storage: &StateLocalStorage,
) -> Option<omega_layout::TypeLayout> {
    if !local_storage.initial_value.is_valid() {
        return None;
    }
    let expressions = &context.state_storage.expressions;
    let initializer = match expressions.expression(local_storage.initial_value) {
        psi_checked_trees::expression::ExpressionNode::Mutable(inner) => *inner,
        _ => local_storage.initial_value,
    };
    let psi_checked_trees::expression::ExpressionNode::Cast(cast) =
        expressions.expression(initializer)
    else {
        return None;
    };
    if !cast.form.is_recast() {
        return None;
    }
    // Shared recast views may reserve referee-sized content slots for flat
    // reads. A mutable recast is always pointer-bearing: writes and reads must
    // reach the backing place even when the referee fits in a machine word.
    if cast.form == psi_language_core::CastForm::RecastMutable {
        return None;
    }
    super::layout::recast_view_layout(
        context,
        &context.state_storage.type_references,
        local_storage.type_reference,
    )
}

fn append_branch_local_slot(
    context: &RuntimeStorageContext,
    plan: &mut RuntimeStoragePlan,
    next_frame_offset: &mut usize,
    dispatch_index: u32,
    local_storage: &StateLocalStorage,
) {
    let layout = recast_view_slot_layout(context, local_storage).unwrap_or_else(|| {
        layout_for_type_reference(
            context,
            &context.state_storage.type_references,
            local_storage.type_reference,
        )
    });
    append_local_slot(
        plan,
        dispatch_index,
        local_storage.source_key,
        local_storage.statement_index,
        local_storage.symbol,
        local_storage.name.clone(),
        local_storage.type_symbol,
        context
            .state_storage
            .type_references
            .display_name(local_storage.type_reference)
            .into(),
        type_descriptor(
            &context.state_storage.type_references,
            local_storage.type_reference,
        ),
        context
            .state_storage
            .invariant_names
            .span_or_empty(local_storage.invariant_names)
            .iter()
            .cloned(),
        layout.size,
        layout.alignment,
        is_static_boundary_capability(context, local_storage.type_symbol),
        next_frame_offset,
    );
}

fn append_branch_state_call_storage(
    context: &RuntimeStorageContext,
    plan: &mut RuntimeStoragePlan,
    next_frame_offset: &mut usize,
    dispatch_index: u32,
    state_call: &StateCall,
    visiting: &mut BranchStorageVisitingStates,
) {
    append_state_call_result_slot(
        context,
        plan,
        next_frame_offset,
        dispatch_index,
        state_call.source_key,
        state_call.statement_index,
        state_call.role,
        state_call.call_ordinal,
        state_call.target_key,
    );

    if matches!(
        state_call.lowering,
        StateCallLowering::InlineLeaf
            | StateCallLowering::InlineExpansion
            | StateCallLowering::InlineBranching
    ) {
        append_branch_expanded_storage(
            context,
            plan,
            next_frame_offset,
            dispatch_index,
            state_call.target_key,
            visiting,
        );
    }
}

fn append_branch_transition_target_storage(
    context: &RuntimeStorageContext,
    plan: &mut RuntimeStoragePlan,
    next_frame_offset: &mut usize,
    dispatch_index: u32,
    current_state: StateKey,
    target: &PlannedTransitionTarget,
    visiting: &mut BranchStorageVisitingStates,
) {
    let Some(target_key) = branch_transition_target_key(context, current_state, target) else {
        return;
    };
    append_branch_expanded_storage(
        context,
        plan,
        next_frame_offset,
        dispatch_index,
        target_key,
        visiting,
    );
}

fn branch_transition_target_key(
    context: &RuntimeStorageContext,
    current_state: StateKey,
    target: &PlannedTransitionTarget,
) -> Option<StateKey> {
    match target {
        PlannedTransitionTarget::State { key, .. } => Some(*key),
        PlannedTransitionTarget::Nested {
            receiver_symbol,
            state_symbol,
            receiver,
            ..
        } => {
            let machine = context
                .control_flow
                .machine_by_symbol(current_state.machine)?;
            // A SELF/sibling-machine target (`-> self.e1()`, e1 a machine on
            // the SAME attached data): resolve within the current machine's
            // states, then across sibling attached machines -- mirrors
            // state-calls' runtime_transition_target. Without this arm the
            // target key resolved to None, the arm-target callee's body was
            // never storage-walked for the inlining dispatch case, and its
            // locals (a host-call result `let rc`) had no frame slot there --
            // "host operation has no encodable call sequence" on every deep
            // wrapper walk. (An earlier landing was reverted on suite noise
            // that three clean re-runs + a byte-identical emitted-code diff
            // later attributed to the recorded parallel temp-dir race.)
            if *receiver_symbol == machine.symbol || receiver.as_str() == "self" {
                let own_state = context
                    .control_flow
                    .states
                    .span(machine.states)
                    .and_then(|states| {
                        states.iter().find(|candidate| {
                            state_symbol.is_valid() && candidate.key.state == *state_symbol
                        })
                    })
                    .map(|target_state| target_state.key);
                if own_state.is_some() {
                    return own_state;
                }
                let attached = machine.attached_data.as_ref().map(|data| data.as_str());
                return context
                    .control_flow
                    .machines
                    .iter()
                    .filter(|(_, sibling)| {
                        sibling.symbol != machine.symbol
                            && attached.is_some()
                            && sibling.attached_data.as_ref().map(|data| data.as_str()) == attached
                    })
                    .find_map(|(_, sibling)| {
                        context
                            .control_flow
                            .states
                            .span(sibling.states)
                            .and_then(|states| {
                                states.iter().find(|candidate| {
                                    state_symbol.is_valid() && candidate.key.state == *state_symbol
                                })
                            })
                            .map(|target_state| target_state.key)
                    });
            }
            context
                .control_flow
                .machine_contains(machine)
                .iter()
                .find(|contained| {
                    receiver_symbol.is_valid() && contained.symbol == *receiver_symbol
                })
                .and_then(|contained| {
                    context
                        .control_flow
                        .machine_by_symbol(contained.type_symbol)
                })
                .and_then(|target_machine| {
                    context
                        .control_flow
                        .states
                        .span(target_machine.states)
                        .and_then(|states| {
                            states.iter().find(|candidate| {
                                state_symbol.is_valid() && candidate.key.state == *state_symbol
                            })
                        })
                })
                .map(|target_state| target_state.key)
        }
        PlannedTransitionTarget::SelfTarget => Some(current_state),
        PlannedTransitionTarget::None | PlannedTransitionTarget::Terminal => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_state_call_result_slot_for_plan(
    context: &RuntimeStorageContext,
    plan: &mut RuntimeStoragePlan,
    next_frame_offset: &mut usize,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    role: StateCallRole,
    call_ordinal: usize,
    target_key: StateKey,
) {
    append_state_call_result_slot(
        context,
        plan,
        next_frame_offset,
        dispatch_index,
        source_key,
        statement_index,
        role,
        call_ordinal,
        target_key,
    )
}

fn append_state_call_result_slot(
    context: &RuntimeStorageContext,
    plan: &mut RuntimeStoragePlan,
    next_frame_offset: &mut usize,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    role: StateCallRole,
    call_ordinal: usize,
    target_key: StateKey,
) {
    if !matches!(
        role,
        StateCallRole::AssignmentValue
            | StateCallRole::CallArgument
            | StateCallRole::TransitionArgument
            | StateCallRole::TransitionGuard
    ) {
        return;
    }

    let return_type = state_return_type(context, target_key);
    if !return_type.is_valid() {
        return;
    };
    if call_result_slot_exists(
        plan,
        dispatch_index,
        source_key,
        statement_index,
        role,
        call_ordinal,
    ) {
        return;
    }

    let type_symbol = context.program.type_reference_symbol(return_type);
    let type_name = context.program.display_type_reference(return_type);
    let layout =
        layout_for_type_reference(context, &context.program.type_reference_table, return_type);
    if layout.size == 0 {
        return;
    }

    // A transition's guard SUBJECT evaluates exactly once per transition
    // evaluation, but the parser copies the subject call into every arm's guard,
    // so each arm statement asks for its own result slot. Arms re-testing a
    // structurally equal subject SHARE the first arm's slot: the first arm's
    // prelude writes it once and every arm's comparison reads the same bytes
    // (the runtime-branching plan suppresses the later arms' execution
    // machinery to match).
    let byte_offset = if role == StateCallRole::TransitionGuard
        && let Some(shared_offset) = shared_transition_guard_slot_offset(
            context,
            plan,
            dispatch_index,
            source_key,
            statement_index,
        ) {
        shared_offset
    } else {
        let byte_offset = align_to(*next_frame_offset, layout.alignment);
        *next_frame_offset = byte_offset
            .checked_add(layout.size)
            .expect("runtime state-call result slot size overflow");
        byte_offset
    };

    let (symbol, name) =
        call_result_slot_symbol_and_name(context, source_key, statement_index, role)
            // A bare-call binding names its call-result slot after the binding
            // ONLY when the binding has no LocalStorage slot of its own (then
            // the call-result slot IS the local, and name-based reads must
            // find it). When the local ALSO has storage, the pair is kept in
            // sync by the call-result->local copy and name-based reads must
            // resolve to the LOCAL alone -- a name-carrying call-result slot
            // made the guarded arms' terminal writes (which rebuild the slot
            // as a NAME expression and re-resolve it) land in whichever slot
            // the name matched FIRST, so the guard read the other slot's
            // pre-store ZII tag (the nested-value-call guard-ZII miscompile).
            .filter(|(symbol, _)| {
                !local_slot_exists(plan, dispatch_index, source_key, statement_index, *symbol)
                    && !local_storage_for_operation(context, source_key, statement_index)
                        .is_some_and(|local| local.symbol == *symbol)
            })
            .unwrap_or_else(|| {
                // The anonymous scratch name must be UNIQUE per call site: the
                // struct-decomposition write strategy rebuilds the slot as a
                // NAME expression (`runtime_frame_slot_target_expression`) and
                // re-resolves it BY NAME, so a shared "__call_result" made
                // every same-callee site's struct terminal construct write
                // into the FIRST site's slot (the second call then captured
                // its own slot's untouched ZII zero).
                (
                    SymbolHandle::invalid(),
                    Identifier::generated(format!(
                        "__call_result_{statement_index}_{role:?}_{call_ordinal}"
                    )),
                )
            });

    plan.frame_slots.insert(RuntimeFrameSlot {
        dispatch_index,
        source_key,
        statement_index,
        kind: RuntimeFrameSlotKind::StateCallResult {
            role,
            call_ordinal,
            target_key,
        },
        symbol,
        name,
        type_symbol,
        type_name: type_name.into(),
        type_descriptor: type_descriptor(&context.program.type_reference_table, return_type),
        invariant_names: HandleSpan::empty(),
        byte_offset,
        byte_size: layout.size,
        alignment: layout.alignment,
        is_static_boundary_capability: is_static_boundary_capability(context, type_symbol),
    });
}

fn is_static_boundary_capability(
    context: &RuntimeStorageContext,
    type_symbol: SymbolHandle,
) -> bool {
    context
        .program
        .traits()
        .iter()
        .any(|definition| definition.symbol == type_symbol && definition.is_boundary)
}

/// The byte offset of an already-allocated transition-guard result slot in the
/// same state whose guard subject call is structurally equal to this
/// statement's -- i.e. another arm of the same subject-form transition. Copies
/// of one subject must land in ONE slot so later arms read the value the first
/// arm's single evaluation produced.
fn shared_transition_guard_slot_offset(
    context: &RuntimeStorageContext,
    plan: &RuntimeStoragePlan,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
) -> Option<usize> {
    let subject = context
        .control_flow
        .transition_guard_subject_call(source_key, statement_index);
    if !subject.is_valid() {
        return None;
    }
    plan.frame_slots.iter().find_map(|(_, slot)| {
        if slot.dispatch_index != dispatch_index
            || slot.source_key != source_key
            || slot.statement_index == statement_index
            || !matches!(
                slot.kind,
                RuntimeFrameSlotKind::StateCallResult {
                    role: StateCallRole::TransitionGuard,
                    ..
                }
            )
        {
            return None;
        }
        let seen_subject = context
            .control_flow
            .transition_guard_subject_call(source_key, slot.statement_index);
        (seen_subject.is_valid()
            && context
                .control_flow
                .expressions
                .expressions_structurally_equal(seen_subject, subject))
        .then_some(slot.byte_offset)
    })
}

pub(super) fn call_result_slot_symbol_and_name(
    context: &RuntimeStorageContext,
    source_key: StateKey,
    statement_index: usize,
    role: StateCallRole,
) -> Option<(SymbolHandle, Identifier)> {
    if role != StateCallRole::AssignmentValue {
        return None;
    }

    let machine = context
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == source_key.machine)?;
    let state = context
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == source_key.state)?;
    let statement = context
        .program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)?;
    let StatementNode::LocalData(local_data) = statement else {
        return None;
    };

    // Only a BARE-call binding (`let dv6 = self.f(6)`) may name its call-result
    // slot after the binding: there the binding's value IS the call result.
    // When the call is EMBEDDED in a larger initializer (`let r1 = self.base +
    // self.f(6) * 3`), the binding's value lives in its own LocalStorage slot
    // (where the full expression is written); the call-result slot is only
    // scratch for the embedded call. Naming that scratch after the binding made
    // every name-based read of `r1` (guards, transition arguments) resolve to
    // the scratch slot -- which holds the partial (call) value -- instead of the
    // local. Leave the scratch anonymous (the caller falls back to a generated
    // `__call_result` name); the call result is written by POSITION
    // (call_result_slot_by_ordinal), never by this name, so nothing else breaks.
    if !local_data.initial_value.is_valid() {
        return None;
    }
    let initializer = strip_mutable_expression_node(
        context
            .program
            .expression_table
            .expression(local_data.initial_value),
        &context.program.expression_table,
    );
    if !matches!(initializer, ExpressionNode::Call(_)) {
        return None;
    }

    Some((local_data.symbol, local_data.name.clone()))
}

/// Peel `Mutable` wrappers so a `let x = self.f()` whose initializer is parsed as
/// `Mutable(Call)` is still recognised as a bare call.
fn strip_mutable_expression_node<'tree>(
    node: &'tree ExpressionNode,
    table: &'tree ExpressionTable,
) -> &'tree ExpressionNode {
    let mut node = node;
    while let ExpressionNode::Mutable(inner) = node {
        node = table.expression(*inner);
    }
    node
}

fn local_slot_exists(
    plan: &RuntimeStoragePlan,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    symbol: SymbolHandle,
) -> bool {
    plan.frame_slots.iter().any(|(_, slot)| {
        slot.dispatch_index == dispatch_index
            && state_key_matches_statement_source(slot.source_key, source_key)
            && slot.statement_index == statement_index
            && slot.symbol == symbol
            && matches!(slot.kind, RuntimeFrameSlotKind::LocalStorage)
    })
}

fn call_result_slot_exists(
    plan: &RuntimeStoragePlan,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    role: StateCallRole,
    call_ordinal: usize,
) -> bool {
    plan.frame_slots.iter().any(|(_, slot)| {
        slot.dispatch_index == dispatch_index
            && state_key_matches_statement_source(slot.source_key, source_key)
            && slot.statement_index == statement_index
            && matches!(
                slot.kind,
                RuntimeFrameSlotKind::StateCallResult {
                    role: slot_role,
                    call_ordinal: slot_call_ordinal,
                    ..
                } if slot_role == role && slot_call_ordinal == call_ordinal
            )
    })
}

fn type_descriptor(
    table: &psi_checked_trees::types::TypeReferenceTable,
    type_reference: TypeReferenceHandle,
) -> omega_layout::TypeLayoutDescriptor {
    use psi_checked_trees::types::{TypeConstraintNode, TypeReferenceNode};

    match table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee, access, ..
        } => omega_layout::TypeLayoutDescriptor::Reference {
            referee: Box::new(type_descriptor(table, *referee)),
            is_mutable: access.is_exclusive(),
        },
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            if let Some((element_type, capacity)) =
                bounded_byte_buffer_shape(table, *base_type, *constraints)
            {
                omega_layout::TypeLayoutDescriptor::BoundedByteBuffer {
                    element_type: Box::new(type_descriptor(table, element_type)),
                    capacity,
                }
            } else {
                omega_layout::TypeLayoutDescriptor::Constrained {
                    base_type: Box::new(type_descriptor(table, *base_type)),
                    domain: table
                        .constraints(*constraints)
                        .iter()
                        .find_map(|constraint| match constraint {
                            TypeConstraintNode::ArithmeticDomain(domain) => Some(*domain),
                            _ => None,
                        })
                        .unwrap_or(psi_numerics::arithmetic::ArithmeticDomain::Exact),
                }
            }
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => match length {
            FixedArrayLength::Literal(length) => omega_layout::TypeLayoutDescriptor::FixedArray {
                element_type: Box::new(type_descriptor(table, *element_type)),
                length: *length,
            },
            // ConstCall lengths are substituted to literals by the
            // orchestration const-eval pass long before runtime storage;
            // a survivor degrades like an unresolved const parameter.
            FixedArrayLength::ConstParameter { .. } | FixedArrayLength::ConstCall { .. } => {
                omega_layout::TypeLayoutDescriptor::Unit
            }
        },
        TypeReferenceNode::Slice { element_type } => omega_layout::TypeLayoutDescriptor::Slice {
            element_type: Box::new(type_descriptor(table, *element_type)),
        },
        TypeReferenceNode::DynamicTrait {
            symbol,
            name,
            conformance,
            conformance_carrier,
            conformance_name,
        } => omega_layout::TypeLayoutDescriptor::DynamicTrait {
            symbol: *symbol,
            name: name.clone(),
            conformance: *conformance,
            conformance_carrier: conformance_carrier.clone(),
            conformance_name: conformance_name.clone(),
        },
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } => omega_layout::TypeLayoutDescriptor::Named {
            symbol: *base_symbol,
            name: base_name.clone(),
        },
        TypeReferenceNode::Named { symbol, name } => omega_layout::TypeLayoutDescriptor::Named {
            symbol: *symbol,
            name: name.clone(),
        },
        TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => {
            omega_layout::TypeLayoutDescriptor::Unit
        }
    }
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
            state_key_matches_statement_source(mutation.source_key, source_key)
                && mutation.statement_index == statement_index
        })
        .map(|(_, mutation)| mutation)
}

fn local_storage_for_operation<'plan>(
    context: &'plan RuntimeStorageContext,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan StateLocalStorage> {
    context
        .state_storage
        .locals
        .iter()
        .find(|(_, local)| {
            state_key_matches_statement_source(local.source_key, source_key)
                && local.statement_index == statement_index
        })
        .map(|(_, local)| local)
}
