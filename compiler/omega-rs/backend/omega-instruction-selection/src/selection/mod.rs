use crate::{
    InstructionSelectionInput, derive_boundary_exit_indirect_result_copy_footprint,
    derive_boundary_exit_result_register_footprint,
};
use omega_abstract_operations::AbstractOperationPlan;
use omega_checked_trees::expression::ExpressionTable;
use omega_core::arena::Arena;
use omega_state_schedule::{StateScheduleContext, build_entry_state_schedule};

mod bindings;
mod host_operations;
mod instruction_sink;
mod lookups;
mod receiver_base;
mod runtime_dispatch;
mod state_bodies;
mod storage_places;
mod wire_decode;
mod wire_encode;

use self::bindings::RuntimeAliasBuffer;
use instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{
    AbstractFunctionPlan, AbstractOperation, AbstractOperationKind, AbstractValueOperand,
    InstructionOperand,
};
use runtime_dispatch::{
    select_entry_argument_register_writes, select_runtime_dispatch_loop_instructions,
};
use state_bodies::{StateBodyVisitStack, runtime_reachable_states, select_state_body_instructions};

pub fn build_instruction_plan(input: &InstructionSelectionInput<'_>) -> AbstractOperationPlan {
    let mut instruction_plan = estimated_instruction_plan(input);

    let (instructions, mut permission_realization_candidates, boundary_footprints) =
        select_entry_instructions(
            input,
            &mut instruction_plan.code.operands,
            &mut instruction_plan.code.runtime_value_operands,
            &mut instruction_plan.code.instructions,
        );
    append_trivial_affine_drop_realizations(input, &mut permission_realization_candidates);
    append_elided_no_debt_realizations(input, &mut permission_realization_candidates);

    instruction_plan
        .code
        .functions
        .insert(AbstractFunctionPlan {
            symbol: input.entry_symbol.clone(),
            source_key: input.entry_key,
            instructions,
        });
    instruction_plan.permission_realization_candidates = permission_realization_candidates;
    instruction_plan.boundary_footprints = boundary_footprints;

    instruction_plan
}

fn append_elided_no_debt_realizations(
    input: &InstructionSelectionInput<'_>,
    candidates: &mut Vec<omega_abstract_operations::PermissionRealizationCandidate>,
) {
    for (_, state) in input.control_flow.states.iter() {
        let permission_span = state.ownership.permissions;
        for (event_offset, event) in input
            .control_flow
            .semantics
            .ownership
            .permissions
            .span_or_empty(permission_span)
            .iter()
            .enumerate()
        {
            if event.obligation_live {
                continue;
            }
            let source_event_index = permission_span
                .start()
                .arena_index()
                .checked_add(u32::try_from(event_offset).expect("permission event offset overflow"))
                .expect("permission event index overflow");
            if candidates
                .iter()
                .any(|candidate| candidate.source_event_index == source_event_index)
            {
                continue;
            }
            candidates.push(omega_abstract_operations::PermissionRealizationCandidate {
                source_event_index,
                kind:
                    omega_abstract_operations::PermissionRealizationCandidateKind::CheckedNoCode {
                        reason:
                            omega_abstract_operations::CheckedNoCodePermissionReason::ElidedNoDebt,
                    },
            });
        }
    }
}

fn append_trivial_affine_drop_realizations(
    input: &InstructionSelectionInput<'_>,
    candidates: &mut Vec<omega_abstract_operations::PermissionRealizationCandidate>,
) {
    for (_, state) in input.control_flow.states.iter() {
        let permission_span = state.ownership.permissions;
        for (event_offset, event) in input
            .control_flow
            .semantics
            .ownership
            .permissions
            .span_or_empty(permission_span)
            .iter()
            .enumerate()
        {
            if !matches!(
                event.kind,
                omega_core::semantics::PermissionEventKind::AffineDrop
            ) || event.obligation_live
            {
                continue;
            }
            let source_event_index = permission_span
                .start()
                .arena_index()
                .checked_add(u32::try_from(event_offset).expect("permission event offset overflow"))
                .expect("permission event index overflow");
            if candidates
                .iter()
                .any(|candidate| candidate.source_event_index == source_event_index)
            {
                continue;
            }
            candidates.push(
                omega_abstract_operations::PermissionRealizationCandidate {
                    source_event_index,
                    kind: omega_abstract_operations::PermissionRealizationCandidateKind::CheckedNoCode {
                        reason: omega_abstract_operations::CheckedNoCodePermissionReason::TrivialAffineDrop,
                    },
                },
            );
        }
    }
}

fn estimated_instruction_plan(input: &InstructionSelectionInput<'_>) -> AbstractOperationPlan {
    let runtime_dispatch_edges = input.runtime_dispatch_loop.edges.len();
    let runtime_body_operations = input.runtime_bodies.operations.len();
    let host_operations = input.host_calls.operations.len();
    let storage_writes = input.runtime_storage.writes.len();
    let text_writes = input.runtime_text.writes.len();
    let state_guards = input.state_guards.guards.len();

    let instruction_capacity = 2usize
        .saturating_add(runtime_body_operations.saturating_mul(4))
        .saturating_add(runtime_dispatch_edges.saturating_mul(8))
        .saturating_add(host_operations.saturating_mul(2))
        .saturating_add(storage_writes.saturating_mul(4))
        .saturating_add(text_writes.saturating_mul(4))
        .saturating_add(state_guards.saturating_mul(2));
    let operand_capacity = input
        .host_calls
        .arguments
        .len()
        .saturating_add(host_operations.saturating_mul(2));
    let runtime_value_operand_capacity = input
        .runtime_storage
        .frame_slots
        .len()
        .saturating_add(input.runtime_storage.writes.len())
        .saturating_add(input.state_guards.operands.len())
        .saturating_add(runtime_dispatch_edges.saturating_mul(2));

    AbstractOperationPlan::with_capacity(
        1,
        instruction_capacity,
        operand_capacity,
        runtime_value_operand_capacity,
    )
}

fn select_entry_instructions(
    input: &InstructionSelectionInput<'_>,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<AbstractValueOperand>,
    instructions: &mut Arena<AbstractOperation>,
) -> (
    omega_core::arena::HandleSpan<AbstractOperation>,
    Vec<omega_abstract_operations::PermissionRealizationCandidate>,
    omega_abstract_operations::BoundaryFootprintPlan,
) {
    let mut selected_instructions = SelectedInstructionSink::new(instructions, input.control_flow);
    let mut boundary_footprints = omega_abstract_operations::BoundaryFootprintPlan::default();

    selected_instructions.push(entry_instruction(input));
    // The platform boundary establishes entry parameters by writing the
    // incoming argument locations into their frame slots. Associate the exact
    // prologue span with the canonical StateEntry events before either the
    // dispatching or straight-line body begins; zero-initialized storage is not
    // establishment evidence.
    selected_instructions.begin_state_entry_permission_site(input.entry_key);
    let entry_boundary = select_entry_argument_register_writes(
        input,
        &mut selected_instructions,
        &mut boundary_footprints,
    );
    selected_instructions.end_permission_site();

    if input.runtime_dispatch_loop.needed {
        select_runtime_dispatch_loop_instructions(
            input,
            operands,
            runtime_value_operands,
            &mut selected_instructions,
        );
        selected_instructions.push(exit_instruction(input));
        let (instruction_span, candidates) = selected_instructions.finish();
        retain_exit_footprints(
            &mut boundary_footprints,
            entry_boundary.as_ref(),
            input,
            instructions.span(instruction_span).unwrap_or_default(),
        );
        return (instruction_span, candidates, boundary_footprints);
    }

    let schedule_context =
        StateScheduleContext::new(&input.control_flow, &input.host_calls, &input.state_calls);
    let _state_schedule = build_entry_state_schedule(&schedule_context, input.entry_key)
        .unwrap_or_else(|_| runtime_reachable_states(input));

    let empty_aliases = RuntimeAliasBuffer::default();
    select_state_body_instructions(
        input,
        input.entry_key,
        input
            .runtime_bodies
            .bodies
            .iter()
            .find(|(_, body)| body.key == input.entry_key)
            .map(|(_, body)| body.dispatch_index),
        &empty_aliases,
        &ExpressionTable::new(),
        operands,
        runtime_value_operands,
        &mut selected_instructions,
        &mut StateBodyVisitStack::with_capacity(input.control_flow.states.len()),
    );

    selected_instructions.push(exit_instruction(input));
    let (instruction_span, candidates) = selected_instructions.finish();
    retain_exit_footprints(
        &mut boundary_footprints,
        entry_boundary.as_ref(),
        input,
        instructions.span(instruction_span).unwrap_or_default(),
    );
    (instruction_span, candidates, boundary_footprints)
}

fn retain_exit_footprints(
    plan: &mut omega_abstract_operations::BoundaryFootprintPlan,
    boundary: Option<&omega_calling_conventions::ValidatedBoundaryEntryPlan>,
    input: &InstructionSelectionInput<'_>,
    instructions: &[AbstractOperation],
) {
    let Some(boundary) = boundary else {
        return;
    };
    let evidence = derive_boundary_exit_result_register_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect("selected exit-result registers must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() {
        plan.fragments
            .push(omega_abstract_operations::BoundaryFootprintFragment {
                origin:
                    omega_abstract_operations::BoundaryFootprintFragmentOrigin::ExitResultRegisters,
                evidence,
            });
    }
    if input.runtime_storage.entry_indirect_result_pointer_size != 8 {
        return;
    }
    let evidence = derive_boundary_exit_indirect_result_copy_footprint(
        boundary,
        input.runtime_storage.entry_indirect_result_pointer_base,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect("selected indirect-result copies must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() {
        plan.fragments
            .push(omega_abstract_operations::BoundaryFootprintFragment {
            origin:
                omega_abstract_operations::BoundaryFootprintFragmentOrigin::ExitIndirectResultCopy,
            evidence,
        });
    }
}

fn entry_instruction(input: &InstructionSelectionInput<'_>) -> AbstractOperation {
    AbstractOperation {
        kind: AbstractOperationKind::EnterFunction,
        source_key: input.entry_key,
        source_statement: 0,
    }
}

fn exit_instruction(input: &InstructionSelectionInput<'_>) -> AbstractOperation {
    AbstractOperation {
        kind: AbstractOperationKind::LeaveFunction,
        source_key: input.entry_key,
        source_statement: 0,
    }
}
