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
mod callback_functions;
mod dynamic_calls;
mod host_operations;
mod instruction_sink;
mod lookups;
mod private_dynamic_functions;
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

pub fn build_instruction_plan(
    input: &InstructionSelectionInput<'_>,
) -> Result<AbstractOperationPlan, psi_diagnostics::Diagnostic> {
    dynamic_calls::validate_dynamic_calls(input)?;
    let mut instruction_plan = estimated_instruction_plan(input);

    let (
        instructions,
        mut permission_realization_candidates,
        mut boundary_footprints,
        entry_boundary,
    ) = select_entry_instructions(
        input,
        &mut instruction_plan.code.operands,
        &mut instruction_plan.code.runtime_value_operands,
        &mut instruction_plan.code.instructions,
    )?;
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
    private_dynamic_functions::select_private_dynamic_realization_functions(
        input,
        &mut instruction_plan,
        &mut permission_realization_candidates,
        &mut boundary_footprints,
        entry_boundary.as_ref(),
    )?;
    callback_functions::select_payloadless_callback_functions(input, &mut instruction_plan)?;
    instruction_plan.permission_realization_candidates = permission_realization_candidates;
    instruction_plan.semantics.boundaries.footprints = boundary_footprints;

    Ok(instruction_plan)
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
) -> Result<
    (
        psi_arena::HandleSpan<AbstractOperation>,
        Vec<omega_abstract_operations::PermissionRealizationCandidate>,
        omega_abstract_operations::BoundaryFootprintPlan,
        Option<omega_calling_conventions::ValidatedBoundaryEntryPlan>,
    ),
    psi_diagnostics::Diagnostic,
> {
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
        )?;
        return Ok((
            instruction_span,
            candidates,
            boundary_footprints,
            entry_boundary,
        ));
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
    )?;
    Ok((
        instruction_span,
        candidates,
        boundary_footprints,
        entry_boundary,
    ))
}

fn retain_exit_footprints(
    plan: &mut omega_abstract_operations::BoundaryFootprintPlan,
    boundary: Option<&omega_calling_conventions::ValidatedBoundaryEntryPlan>,
    input: &InstructionSelectionInput<'_>,
    operands: &Arena<InstructionOperand>,
    runtime_value_operands: &Arena<AbstractValueOperand>,
    instructions: &[AbstractOperation],
) -> Result<(), psi_diagnostics::Diagnostic> {
    let Some(boundary) = boundary else {
        return Ok(());
    };
    if input.runtime_dispatch_loop.needed
        && instructions.iter().any(|instruction| {
            matches!(
                instruction.kind,
                AbstractOperationKind::EnterDispatchLoop { .. }
                    | AbstractOperationKind::LeaveDispatchLoop
            )
        })
    {
        let evidence = derive_boundary_dispatch_scaffold_footprint(
            boundary,
            instructions.iter().map(|instruction| &instruction.kind),
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin:
                    omega_abstract_operations::BoundaryFootprintFragmentOrigin::DispatchScaffold,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_static_guard_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin:
                    omega_abstract_operations::BoundaryFootprintFragmentOrigin::StaticGuardComparison,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_runtime_text_guard_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::RuntimeTextGuardComparison,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_place_guard_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::PlaceGuardComparison,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_runtime_value_guard_footprint(
        boundary,
        runtime_value_operands,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::RuntimeValueGuardComparison,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_call_return_mechanics_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    retain_boundary_footprint_fragment(
        plan,
        boundary,
        omega_abstract_operations::BoundaryFootprintFragment {
            origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CallReturnMechanics,
            evidence,
        },
    )?;

    let evidence = derive_boundary_checked_assembly_footprint(
        boundary,
        runtime_value_operands,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CheckedAssemblyCatalog,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }

    let evidence = derive_boundary_exit_result_register_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin:
                    omega_abstract_operations::BoundaryFootprintFragmentOrigin::ExitResultRegisters,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_place_copy_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyPlaceCopy,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_place_integer_write_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyPlaceIntegerWrite,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_place_address_write_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyPlaceAddressWrite,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_constant_host_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyConstantHostResult,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_immediate_import_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImport,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_immediate_import_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImportResult,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_float_import_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundFloatImportResult,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_dereferenced_import_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDereferencedImportResult,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_data_import_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImport,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_data_import_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImportResult,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_indirect_call_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundIndirectCall,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_authored_import_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImport,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_authored_import_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImportResult,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_authored_float_import_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImport,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_authored_float_import_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImportResult,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_authored_aggregate_import_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImport,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence =
        derive_boundary_compiler_body_outbound_authored_aggregate_import_result_footprint(
            boundary,
            input,
            operands,
            instructions,
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImportResult,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_authored_aggregate_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateResult,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_open_create_import_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundOpenCreateImport,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence =
        derive_boundary_compiler_body_runtime_byte_read_footprint(boundary, input, instructions)
            .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteRead,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence =
        derive_boundary_compiler_body_runtime_byte_write_footprint(boundary, input, instructions)
            .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteWrite,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence =
        derive_boundary_compiler_body_runtime_line_read_footprint(boundary, input, instructions)
            .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeLineRead,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_storage_import_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImport,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_storage_import_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImportResult,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_syscall_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscall,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_syscall_storage_arguments_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallStorageArguments,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_syscall_data_arguments_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallDataArguments,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_syscall_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResult,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence =
        derive_boundary_compiler_body_outbound_syscall_result_storage_arguments_footprint(
            boundary,
            input,
            operands,
            instructions,
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultStorageArguments,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_syscall_result_data_arguments_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultDataArguments,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_syscall_timespec_argument_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecArgument,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_outbound_syscall_timespec_result_footprint(
        boundary,
        input,
        operands,
        instructions,
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecResult,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_storage_bit_field_write_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyStorageBitFieldWrite,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_place_bounded_buffer_write_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_place_string_write_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyPlaceStringWrite,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_wire_literal_byte_append_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireLiteralByteAppend,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_wire_scalar_varint_append_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintAppend,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_wire_text_bytes_append_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireTextBytesAppend,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_wire_scalar_slice_append_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarSliceAppend,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_wire_repeated_scalar_varint_append_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintAppend,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_wire_expected_byte_read_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireExpectedByteRead,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_wire_scalar_varint_read_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintRead,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_wire_byte_slice_read_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireByteSliceRead,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_wire_nested_open_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedOpen,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_wire_nested_close_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedClose,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_wire_repeated_scalar_varint_read_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintRead,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_atomic_footprint(
        boundary,
        runtime_value_operands,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() || !evidence.machine_state().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyAtomicOperation,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_text_assembly_write_footprint(
        boundary,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_place_binary_write_footprint(
        boundary,
        runtime_value_operands,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBinaryWrite,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    let evidence = derive_boundary_compiler_body_storage_convert_write_footprint(
        boundary,
        runtime_value_operands,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin::CompilerBodyStorageConvertWrite,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    if input.runtime_storage.entry_indirect_result_pointer_size != 8 {
        return Ok(());
    }
    let evidence = derive_boundary_exit_indirect_result_copy_footprint(
        boundary,
        input.runtime_storage.entry_indirect_result_pointer_base,
        instructions.iter().map(|instruction| &instruction.kind),
    )
    .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    if !evidence.registers().as_slice().is_empty() {
        plan.retain_validated_fragment(
            boundary,
            omega_abstract_operations::BoundaryFootprintFragment {
                origin:
                    omega_abstract_operations::BoundaryFootprintFragmentOrigin::ExitIndirectResultCopy,
                evidence,
            },
        )
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))?;
    }
    Ok(())
}

fn retain_boundary_footprint_fragment(
    plan: &mut omega_abstract_operations::BoundaryFootprintPlan,
    boundary: &omega_calling_conventions::ValidatedBoundaryEntryPlan,
    fragment: omega_abstract_operations::BoundaryFootprintFragment,
) -> Result<(), psi_diagnostics::Diagnostic> {
    plan.retain_validated_fragment(boundary, fragment)
        .map_err(|error| psi_diagnostics::Diagnostic::error(error.0))
}

#[cfg(test)]
mod footprint_error_tests {
    use super::retain_boundary_footprint_fragment;
    use omega_abstract_operations::{
        BoundaryFootprintFragment, BoundaryFootprintFragmentOrigin, BoundaryFootprintPlan,
    };
    use omega_calling_conventions::{
        CallSignature, CallingPolicy, MachineState, MachineStateSet, RegisterSet,
        StateFootprintEvidence, evaluate_ordinary_boundary_entry_plan,
    };

    #[test]
    fn root_ceiling_mismatch_returns_a_diagnostic() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("ordinary boundary");
        let mut plan = BoundaryFootprintPlan::default();
        let diagnostic = retain_boundary_footprint_fragment(
            &mut plan,
            &boundary,
            BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::StaticGuardComparison,
                evidence: StateFootprintEvidence::new(
                    RegisterSet::default(),
                    MachineStateSet::new([MachineState::DebugState]),
                ),
            },
        )
        .expect_err("a fragment beyond the root ceiling must fail closed");

        assert!(
            diagnostic
                .message
                .contains("exceeds the entry plan ceiling")
        );
        assert!(plan.fragments.is_empty());
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
