use super::{RuntimeFrameSlot, RuntimeStorageContext, RuntimeStoragePlan};
use crate::model::RuntimeFrameSlotKind;
use omega_checked_trees::expression::ExpressionTableCapacity;
use omega_checked_trees::name::Identifier;
use omega_checked_trees::types::TypeReferenceHandle;
use omega_control_flow::{PlannedTransitionTarget, StateKey};
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use omega_runtime_bodies::{RuntimeDispatchBody, RuntimeDispatchBodyOperationKind};
use omega_state_calls::{StateCall, StateCallLowering, StateCallRole};
use omega_state_storage::{StateLocalStorage, StateMutation, StateMutationLowering};
use std::sync::Arc;

use super::layout::{align_to, layout_for_type_reference};

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
                let layout = layout_for_type_reference(
                    context,
                    &context.runtime_bodies.type_references,
                    *type_reference,
                );
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

fn append_parameter_slots(
    context: &RuntimeStorageContext,
    body: &RuntimeDispatchBody,
    plan: &mut RuntimeStoragePlan,
    next_frame_offset: &mut usize,
) {
    let Some(state) = context.control_flow.state_by_key(body.key) else {
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
            dispatch_index: body.dispatch_index,
            source_key: body.key,
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

    if let Some(state_call) = context
        .state_calls
        .assignment_value_call(state_key, statement_index)
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

fn append_branch_local_slot(
    context: &RuntimeStorageContext,
    plan: &mut RuntimeStoragePlan,
    next_frame_offset: &mut usize,
    dispatch_index: u32,
    local_storage: &StateLocalStorage,
) {
    let layout = layout_for_type_reference(
        context,
        &context.state_storage.type_references,
        local_storage.type_reference,
    );
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
            ..
        } => {
            let machine = context
                .control_flow
                .machine_by_symbol(current_state.machine)?;
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

    let byte_offset = align_to(*next_frame_offset, layout.alignment);
    *next_frame_offset = byte_offset
        .checked_add(layout.size)
        .expect("runtime state-call result slot size overflow");

    plan.frame_slots.insert(RuntimeFrameSlot {
        dispatch_index,
        source_key,
        statement_index,
        kind: RuntimeFrameSlotKind::StateCallResult {
            role,
            call_ordinal,
            target_key,
        },
        symbol: SymbolHandle::invalid(),
        name: Identifier::generated_static("__call_result"),
        type_symbol,
        type_name: type_name.into(),
        type_descriptor: type_descriptor(&context.program.type_reference_table, return_type),
        invariant_names: HandleSpan::empty(),
        byte_offset,
        byte_size: layout.size,
        alignment: layout.alignment,
    });
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
    table: &omega_checked_trees::types::TypeReferenceTable,
    type_reference: TypeReferenceHandle,
) -> omega_layout::TypeLayoutDescriptor {
    use omega_checked_trees::types::TypeReferenceNode;

    match table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee,
            is_mutable,
        } => omega_layout::TypeLayoutDescriptor::Reference {
            referee: Box::new(type_descriptor(table, *referee)),
            is_mutable: *is_mutable,
        },
        TypeReferenceNode::Constrained { base_type, .. } => {
            omega_layout::TypeLayoutDescriptor::Constrained {
                base_type: Box::new(type_descriptor(table, *base_type)),
            }
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => omega_layout::TypeLayoutDescriptor::FixedArray {
            element_type: Box::new(type_descriptor(table, *element_type)),
            length: *length,
        },
        TypeReferenceNode::Slice { element_type } => omega_layout::TypeLayoutDescriptor::Slice {
            element_type: Box::new(type_descriptor(table, *element_type)),
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
        TypeReferenceNode::Unit => omega_layout::TypeLayoutDescriptor::Unit,
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
