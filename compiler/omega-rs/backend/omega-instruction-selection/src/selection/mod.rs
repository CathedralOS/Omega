use crate::{
    InstructionSelectionInput, derive_boundary_call_return_mechanics_footprint,
    derive_boundary_checked_assembly_footprint, derive_boundary_compiler_body_atomic_footprint,
    derive_boundary_compiler_body_constant_host_result_footprint,
    derive_boundary_compiler_body_outbound_authored_aggregate_import_footprint,
    derive_boundary_compiler_body_outbound_authored_aggregate_import_result_footprint,
    derive_boundary_compiler_body_outbound_authored_aggregate_result_footprint,
    derive_boundary_compiler_body_outbound_authored_float_import_footprint,
    derive_boundary_compiler_body_outbound_authored_float_import_result_footprint,
    derive_boundary_compiler_body_outbound_authored_import_footprint,
    derive_boundary_compiler_body_outbound_authored_import_result_footprint,
    derive_boundary_compiler_body_outbound_data_import_footprint,
    derive_boundary_compiler_body_outbound_data_import_result_footprint,
    derive_boundary_compiler_body_outbound_dereferenced_import_result_footprint,
    derive_boundary_compiler_body_outbound_float_import_result_footprint,
    derive_boundary_compiler_body_outbound_immediate_import_footprint,
    derive_boundary_compiler_body_outbound_immediate_import_result_footprint,
    derive_boundary_compiler_body_outbound_indirect_call_footprint,
    derive_boundary_compiler_body_outbound_open_create_import_footprint,
    derive_boundary_compiler_body_outbound_storage_import_footprint,
    derive_boundary_compiler_body_outbound_storage_import_result_footprint,
    derive_boundary_compiler_body_outbound_syscall_data_arguments_footprint,
    derive_boundary_compiler_body_outbound_syscall_footprint,
    derive_boundary_compiler_body_outbound_syscall_result_data_arguments_footprint,
    derive_boundary_compiler_body_outbound_syscall_result_footprint,
    derive_boundary_compiler_body_outbound_syscall_result_storage_arguments_footprint,
    derive_boundary_compiler_body_outbound_syscall_storage_arguments_footprint,
    derive_boundary_compiler_body_outbound_syscall_timespec_argument_footprint,
    derive_boundary_compiler_body_outbound_syscall_timespec_result_footprint,
    derive_boundary_compiler_body_place_address_write_footprint,
    derive_boundary_compiler_body_place_binary_write_footprint,
    derive_boundary_compiler_body_place_bounded_buffer_write_footprint,
    derive_boundary_compiler_body_place_copy_footprint,
    derive_boundary_compiler_body_place_integer_write_footprint,
    derive_boundary_compiler_body_place_string_write_footprint,
    derive_boundary_compiler_body_runtime_byte_read_footprint,
    derive_boundary_compiler_body_runtime_byte_write_footprint,
    derive_boundary_compiler_body_runtime_line_read_footprint,
    derive_boundary_compiler_body_storage_bit_field_write_footprint,
    derive_boundary_compiler_body_storage_convert_write_footprint,
    derive_boundary_compiler_body_text_assembly_write_footprint,
    derive_boundary_compiler_body_wire_byte_slice_read_footprint,
    derive_boundary_compiler_body_wire_expected_byte_read_footprint,
    derive_boundary_compiler_body_wire_literal_byte_append_footprint,
    derive_boundary_compiler_body_wire_nested_close_footprint,
    derive_boundary_compiler_body_wire_nested_open_footprint,
    derive_boundary_compiler_body_wire_repeated_scalar_varint_append_footprint,
    derive_boundary_compiler_body_wire_repeated_scalar_varint_read_footprint,
    derive_boundary_compiler_body_wire_scalar_slice_append_footprint,
    derive_boundary_compiler_body_wire_scalar_varint_append_footprint,
    derive_boundary_compiler_body_wire_scalar_varint_read_footprint,
    derive_boundary_compiler_body_wire_text_bytes_append_footprint,
    derive_boundary_dispatch_scaffold_footprint,
    derive_boundary_exit_indirect_result_copy_footprint,
    derive_boundary_exit_result_register_footprint, derive_boundary_place_guard_footprint,
    derive_boundary_runtime_text_guard_footprint, derive_boundary_runtime_value_guard_footprint,
    derive_boundary_static_guard_footprint,
};
use omega_abstract_operations::AbstractOperationPlan;
use omega_state_schedule::{StateScheduleContext, build_entry_state_schedule};
use psi_arena::Arena;
use psi_checked_trees::expression::ExpressionTable;

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
            identity: omega_control_flow::MachineFunctionIdentity::source(input.entry_key),
            instructions,
        });
    instruction_plan.permission_realization_candidates = permission_realization_candidates;
    instruction_plan.semantics.boundaries.footprints = boundary_footprints;

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
                psi_language_semantics::PermissionEventKind::AffineDrop
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
    psi_arena::HandleSpan<AbstractOperation>,
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
            operands,
            runtime_value_operands,
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
        operands,
        runtime_value_operands,
        instructions.span(instruction_span).unwrap_or_default(),
    );
    (instruction_span, candidates, boundary_footprints)
}

fn retain_exit_footprints(
    plan: &mut omega_abstract_operations::BoundaryFootprintPlan,
    boundary: Option<&omega_calling_conventions::ValidatedBoundaryEntryPlan>,
    input: &InstructionSelectionInput<'_>,
    operands: &Arena<InstructionOperand>,
    runtime_value_operands: &Arena<AbstractValueOperand>,
    instructions: &[AbstractOperation],
) {
    let Some(boundary) = boundary else {
        return;
    };
    if input.runtime_dispatch_loop.needed {
        let evidence = derive_boundary_dispatch_scaffold_footprint(
            boundary,
            instructions.iter().map(|instruction| &instruction.kind),
        )
        .expect("selected dispatch scaffold must fit the validated entry state ceiling");
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin:
                    omega_abstract_operations::BoundaryFootprintFragmentOrigin::DispatchScaffold,
                evidence,
            },
        )
        .expect("retained dispatch scaffold must name and fit the entry boundary contract");
    }
    let evidence = derive_boundary_static_guard_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect("selected static guards must fit the validated entry state ceiling");
    if !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin:
                    omega_abstract_operations::BoundaryFootprintFragmentOrigin::StaticGuardComparison,
                evidence,
            },
        )
        .expect("retained static guards must name and fit the entry boundary contract");
    }
    let evidence = derive_boundary_runtime_text_guard_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect("selected runtime text guards must fit the validated entry state ceiling");
    if !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::RuntimeTextGuardComparison,
                evidence,
            },
        )
        .expect("retained runtime text guards must name and fit the entry boundary contract");
    }
    let evidence = derive_boundary_place_guard_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect("selected place guards must fit the validated entry state ceiling");
    if !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::PlaceGuardComparison,
                evidence,
            },
        )
        .expect("retained place guards must name and fit the entry boundary contract");
    }
    let evidence = derive_boundary_runtime_value_guard_footprint(
        boundary,
        runtime_value_operands,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect("selected runtime-value guards must fit the validated entry state ceiling");
    if !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::RuntimeValueGuardComparison,
                evidence,
            },
        )
        .expect("retained runtime-value guards must name and fit the entry boundary contract");
    }
    let evidence = derive_boundary_call_return_mechanics_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect("selected function entry/return must match the validated boundary control contract");
    plan.retain_validated_fragment(
        boundary,
        omega_abstract_operations::BoundaryFootprintFragment {
            origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CallReturnMechanics,
            evidence,
        },
    )
    .expect("retained function mechanics must name and fit the entry boundary contract");

    let evidence = derive_boundary_checked_assembly_footprint(
        boundary,
        runtime_value_operands,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect("selected checked assembly must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CheckedAssemblyCatalog,
                evidence,
            },
        )
        .expect("retained checked-assembly footprint must name the entry boundary contract");
    }

    let evidence = derive_boundary_exit_result_register_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect("selected exit-result registers must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin:
                    omega_abstract_operations::BoundaryFootprintFragmentOrigin::ExitResultRegisters,
                evidence,
            },
        )
        .expect("retained result footprint must name and fit the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_place_copy_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect("selected compiler-body place copies must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyPlaceCopy,
                evidence,
            },
        )
        .expect("retained compiler-body place-copy footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_place_integer_write_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect("selected compiler-body integer writes must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyPlaceIntegerWrite,
                evidence,
            },
        )
        .expect("retained compiler-body integer-write footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_place_address_write_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect("selected compiler-body address writes must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyPlaceAddressWrite,
                evidence,
            },
        )
        .expect("retained compiler-body address-write footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_constant_host_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect(
        "selected compiler-body constant host results must fit the validated entry state ceiling",
    );
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyConstantHostResult,
                evidence,
            },
        )
        .expect("retained compiler-body constant-host-result footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_immediate_import_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect("selected compiler-body immediate imports must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImport,
                evidence,
            },
        )
        .expect("retained compiler-body immediate-import footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_immediate_import_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect("selected compiler-body result-bearing immediate imports must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImportResult,
                evidence,
            },
        )
        .expect("retained compiler-body result-bearing immediate-import footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_float_import_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect("selected compiler-body result-bearing float imports must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundFloatImportResult,
                evidence,
            },
        )
        .expect("retained compiler-body result-bearing float-import footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_dereferenced_import_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect("selected compiler-body dereferenced-result imports must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDereferencedImportResult,
                evidence,
            },
        )
        .expect("retained compiler-body dereferenced-result import footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_data_import_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect(
        "selected compiler-body data-address imports must fit the validated entry state ceiling",
    );
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImport,
                evidence,
            },
        )
        .expect("retained compiler-body data-address import footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_data_import_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect("selected compiler-body result-bearing data-address imports must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImportResult,
                evidence,
            },
        )
        .expect("retained compiler-body result-bearing data-address import footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_indirect_call_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect("selected compiler-body indirect calls must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundIndirectCall,
                evidence,
            },
        )
        .expect("retained compiler-body indirect-call footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_authored_import_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect("selected compiler-body authored imports must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImport,
                evidence,
            },
        )
        .expect("retained compiler-body authored import footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_authored_import_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect("selected compiler-body result-bearing authored imports must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImportResult,
                evidence,
            },
        )
        .expect("retained compiler-body result-bearing authored import footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_authored_float_import_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect(
        "selected compiler-body authored float imports must fit the validated entry state ceiling",
    );
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImport,
                evidence,
            },
        )
        .expect("retained compiler-body authored float import footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_authored_float_import_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect("selected compiler-body result-bearing authored float imports must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImportResult,
                evidence,
            },
        )
        .expect("retained compiler-body result-bearing authored float import footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_authored_aggregate_import_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect(
        "selected compiler-body authored aggregate imports must fit the validated entry state ceiling",
    );
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImport,
                evidence,
            },
        )
        .expect("retained compiler-body authored aggregate import footprint must name the entry boundary contract");
    }
    let evidence =
        derive_boundary_compiler_body_outbound_authored_aggregate_import_result_footprint(
            boundary,
            input,
            operands,
            instructions,
        )
        .expect("selected compiler-body result-bearing authored aggregate imports must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImportResult,
                evidence,
            },
        )
        .expect("retained compiler-body result-bearing authored aggregate import footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_authored_aggregate_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect(
        "selected compiler-body authored aggregate results must fit the validated entry state ceiling",
    );
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateResult,
                evidence,
            },
        )
        .expect("retained compiler-body authored aggregate result footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_open_create_import_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect(
        "selected compiler-body Darwin open-create imports must fit the validated entry state ceiling",
    );
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundOpenCreateImport,
                evidence,
            },
        )
        .expect("retained compiler-body Darwin open-create footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_runtime_byte_read_footprint(
        boundary,
        input,
        instructions,
    )
    .expect("selected compiler-body runtime byte reads must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteRead,
                evidence,
            },
        )
        .expect("retained compiler-body runtime byte-read footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_runtime_byte_write_footprint(
        boundary,
        input,
        instructions,
    )
    .expect(
        "selected compiler-body runtime byte writes must fit the validated entry state ceiling",
    );
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteWrite,
                evidence,
            },
        )
        .expect("retained compiler-body runtime byte-write footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_runtime_line_read_footprint(
        boundary,
        input,
        instructions,
    )
    .expect("selected compiler-body runtime line reads must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeLineRead,
                evidence,
            },
        )
        .expect("retained compiler-body runtime line-read footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_storage_import_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect("selected compiler-body storage imports must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImport,
                evidence,
            },
        )
        .expect("retained compiler-body storage-import footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_storage_import_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect("selected compiler-body result-bearing storage imports must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImportResult,
                evidence,
            },
        )
        .expect("retained compiler-body result-bearing storage-import footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_syscall_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect("selected compiler-body outbound syscalls must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscall,
                evidence,
            },
        )
        .expect("retained compiler-body outbound-syscall footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_syscall_storage_arguments_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect("selected compiler-body storage-argument outbound syscalls must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallStorageArguments,
                evidence,
            },
        )
        .expect("retained compiler-body storage-argument outbound-syscall footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_syscall_data_arguments_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect("selected compiler-body data-argument outbound syscalls must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallDataArguments,
                evidence,
            },
        )
        .expect("retained compiler-body data-argument outbound-syscall footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_syscall_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect("selected compiler-body result-bearing outbound syscalls must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResult,
                evidence,
            },
        )
        .expect("retained compiler-body result-bearing outbound-syscall footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_syscall_result_storage_arguments_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect("selected compiler-body result-bearing storage-argument syscalls must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultStorageArguments,
                evidence,
            },
        )
        .expect("retained compiler-body result-bearing storage-argument syscall footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_syscall_result_data_arguments_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect("selected compiler-body result-bearing data-argument syscalls must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultDataArguments,
                evidence,
            },
        )
        .expect("retained compiler-body result-bearing data-argument syscall footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_syscall_timespec_argument_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect("selected compiler-body timespec-argument syscalls must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecArgument,
                evidence,
            },
        )
        .expect("retained compiler-body timespec-argument syscall footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_outbound_syscall_timespec_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .expect("selected compiler-body timespec-result syscalls must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecResult,
                evidence,
            },
        )
        .expect("retained compiler-body timespec-result syscall footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_storage_bit_field_write_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect("selected compiler-body bit-field writes must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyStorageBitFieldWrite,
                evidence,
            },
        )
        .expect("retained compiler-body bit-field-write footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_place_bounded_buffer_write_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect(
        "selected compiler-body bounded-buffer writes must fit the validated entry state ceiling",
    );
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
                evidence,
            },
        )
        .expect("retained compiler-body bounded-buffer-write footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_place_string_write_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect("selected compiler-body string writes must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyPlaceStringWrite,
                evidence,
            },
        )
        .expect("retained compiler-body string-write footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_wire_literal_byte_append_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect(
        "selected compiler-body wire literal-byte appends must fit the validated entry state ceiling",
    );
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireLiteralByteAppend,
                evidence,
            },
        )
        .expect("retained compiler-body wire literal-byte footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_wire_scalar_varint_append_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect(
        "selected compiler-body wire scalar-varint appends must fit the validated entry state ceiling",
    );
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintAppend,
                evidence,
            },
        )
        .expect("retained compiler-body wire scalar-varint footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_wire_text_bytes_append_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect("selected compiler-body wire text appends must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireTextBytesAppend,
                evidence,
            },
        )
        .expect("retained compiler-body wire text-append footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_wire_scalar_slice_append_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect(
        "selected compiler-body wire scalar-slice appends must fit the validated entry state ceiling",
    );
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarSliceAppend,
                evidence,
            },
        )
        .expect("retained compiler-body wire scalar-slice append footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_wire_repeated_scalar_varint_append_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect(
        "selected compiler-body repeated scalar appends must fit the validated entry state ceiling",
    );
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintAppend,
                evidence,
            },
        )
        .expect("retained compiler-body repeated scalar append footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_wire_expected_byte_read_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect(
        "selected compiler-body wire expected-byte reads must fit the validated entry state ceiling",
    );
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireExpectedByteRead,
                evidence,
            },
        )
        .expect("retained compiler-body wire expected-byte footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_wire_scalar_varint_read_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect(
        "selected compiler-body wire scalar-varint reads must fit the validated entry state ceiling",
    );
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintRead,
                evidence,
            },
        )
        .expect("retained compiler-body wire scalar-varint read footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_wire_byte_slice_read_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect(
        "selected compiler-body wire byte-slice reads must fit the validated entry state ceiling",
    );
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireByteSliceRead,
                evidence,
            },
        )
        .expect("retained compiler-body wire byte-slice read footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_wire_nested_open_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect("selected compiler-body nested-open checks must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedOpen,
                evidence,
            },
        )
        .expect("retained compiler-body nested-open footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_wire_nested_close_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect(
        "selected compiler-body nested-close checks must fit the validated entry state ceiling",
    );
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedClose,
                evidence,
            },
        )
        .expect("retained compiler-body nested-close footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_wire_repeated_scalar_varint_read_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect(
        "selected compiler-body repeated scalar reads must fit the validated entry state ceiling",
    );
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintRead,
                evidence,
            },
        )
        .expect("retained compiler-body repeated scalar read footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_atomic_footprint(
        boundary,
        runtime_value_operands,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect("selected compiler-body atomic operations must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyAtomicOperation,
                evidence,
            },
        )
        .expect("retained compiler-body atomic footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_text_assembly_write_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect(
        "selected compiler-body text assembly writes must fit the validated entry state ceiling",
    );
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                evidence,
            },
        )
        .expect("retained compiler-body text-assembly footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_place_binary_write_footprint(
        boundary,
        runtime_value_operands,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect("selected compiler-body binary writes must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBinaryWrite,
                evidence,
            },
        )
        .expect("retained compiler-body binary-write footprint must name the entry boundary contract");
    }
    let evidence = derive_boundary_compiler_body_storage_convert_write_footprint(
        boundary,
        runtime_value_operands,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .expect("selected compiler-body conversion writes must fit the validated entry state ceiling");
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyStorageConvertWrite,
                evidence,
            },
        )
        .expect("retained compiler-body conversion-write footprint must name the entry boundary contract");
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
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin:
                    omega_abstract_operations::BoundaryFootprintFragmentOrigin::ExitIndirectResultCopy,
                evidence,
            },
        )
        .expect("retained indirect-result footprint must name the entry boundary contract");
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
