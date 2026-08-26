use crate::MachineEmissionContext;
use crate::host_bindings::{
    field_model_result_present, host_binding, instruction_requires_float_control_restore,
    omega_result_present, runtime_text_call_plans,
};
use crate::selected_instruction_queries::{selected_host_operation, selected_host_text_read};
use omega_assigned_target_operations::{
    RuntimeTextReadSource, SelectedInstructionKind, StateGuardLowering, StateGuardOperator,
};
use omega_calling_conventions::HostBindingMechanism;
use omega_instruction_selection::{
    authored_import_call_sequence_width, control_register_read_width, control_register_write_width,
    dispatch_case_enter_width, dispatch_case_leave_width, dispatch_guard_compare_static_width,
    dispatch_loop_enter_width, dispatch_state_write_width, encode_host_call_sequence_with_plan,
    encode_syscall_sequence_with_plan, entry_argument_register_write_width,
    entry_arguments_slice_descriptor_write_width, entry_indirect_argument_write_width,
    entry_stack_argument_write_width, flags_restore_width, flags_snapshot_width,
    function_enter_width, host_call_sequence_width_with_plan, interrupt_control_width,
    machine_halt_width, memory_fence_width, msr_read_width, msr_write_width, port_read_width,
    port_write_width, return_register_integer_write_width, return_width,
    runtime_atomic_compare_exchange_width, runtime_atomic_fetch_add_width,
    runtime_atomic_fetch_and_width, runtime_atomic_fetch_or_width, runtime_atomic_fetch_sub_width,
    runtime_atomic_fetch_xor_width, runtime_atomic_load_to_storage_width,
    runtime_atomic_store_from_operand_width, runtime_atomic_swap_width,
    runtime_byte_read_width_with_plans, runtime_byte_write_width_with_plans,
    runtime_storage_convert_width, runtime_storage_copy_to_return_register_width,
    runtime_text_line_read_width_with_plans, runtime_text_literal_compare_width,
    runtime_text_literal_segment_write_width, runtime_text_literal_write_width,
    runtime_text_storage_compare_width, runtime_text_stored_suffix_append_width,
    runtime_value_compare_width, vtable_call_sequence_width_with_plan,
};
use omega_machine_instructions::{MachineInstruction, MachineInstructionKind};
use psi_diagnostics::Diagnostic;

#[derive(Debug, Clone)]
pub(crate) struct LaidOutMachineInstruction {
    pub selected_instruction_index: u32,
    pub offset: usize,
    pub byte_width: usize,
    pub kind: MachineInstructionKind,
    pub source_kind: SelectedInstructionKind,
}

pub(crate) fn layout_machine_instructions(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[MachineInstruction],
) -> Result<Vec<LaidOutMachineInstruction>, Diagnostic> {
    let mut laid_out = Vec::with_capacity(machine_instructions.len());
    let mut offset = 0usize;

    for machine_instruction in machine_instructions {
        let byte_width = machine_instruction_width(input, &machine_instruction.source_kind)?;

        // A zero width is legitimate ONLY for STRUCTURAL kinds that may
        // genuinely emit no bytes: residual guard lowerings, loop-leave
        // markers, platform-call boundaries, and function enter/leave (a
        // syscall-only ELF entry needs no prologue). For every OPERATIONAL
        // kind a zero width is an arch-specific "unimplemented" marker from a
        // width function -- and emission SKIPS zero-width instructions without
        // ever calling their encoder, so letting one through silently DROPS
        // the operation (the historic `arr[i] = arr[j]` no-op miscompile on
        // aarch64). Refuse loudly instead; the workaround for the indexed
        // family is staging through a machine field temp.
        if byte_width == 0
            && !matches!(
                machine_instruction.source_kind,
                SelectedInstructionKind::EvaluateDispatchGuard { .. }
                    | SelectedInstructionKind::LeaveDispatchLoop
                    | SelectedInstructionKind::BeginPlatformCall
                    | SelectedInstructionKind::EnterFunction
                    | SelectedInstructionKind::LeaveFunction
            )
        {
            return Err(Diagnostic::error(format!(
                "{:?} has no native lowering for this target (its layout width is zero); \
                 refusing to emit -- a zero-width instruction is silently skipped, which \
                 would drop the operation instead of failing loudly",
                machine_instruction.source_kind,
            )));
        }

        laid_out.push(LaidOutMachineInstruction {
            selected_instruction_index: machine_instruction.selected_instruction_index,
            offset,
            byte_width,
            kind: machine_instruction.kind,
            source_kind: machine_instruction.source_kind.clone(),
        });
        offset += byte_width;
    }

    Ok(laid_out)
}

fn machine_instruction_width(
    input: MachineEmissionContext<'_>,
    kind: &SelectedInstructionKind,
) -> Result<usize, Diagnostic> {
    if let Some(host_operation) = selected_host_operation(kind) {
        let operands = input
            .assigned_target_operations
            .instruction_operands(host_operation.operands)
            .unwrap_or(&[]);
        let binding = host_binding(input, host_operation.operation_key);
        let width = if binding.is_none() && host_operation.operation_key.lowers_to_constant_result()
        {
            omega_instruction_selection::constant_host_result_sequence_width(
                input.target.architecture,
                operands,
            )
        } else {
            let binding = binding.ok_or_else(|| {
                Diagnostic::error(format!(
                    "host operation {}.{} has no selected host binding",
                    host_operation.operation_key.capability_name(),
                    host_operation.operation_key.operation_name(),
                ))
            })?;
            let plan = binding.call_plan();
            match &binding.mechanism {
                HostBindingMechanism::Syscall { number, .. }
                    if host_operation.operation_key.uses_linux_timespec_result() =>
                {
                    omega_instruction_selection::linux_timespec_syscall_sequence_width_with_plan(
                        input.target.architecture,
                        operands,
                        *number,
                        plan,
                    )
                }
                HostBindingMechanism::Syscall { number, .. }
                    if host_operation
                        .operation_key
                        .uses_linux_timespec_argument() =>
                {
                    omega_instruction_selection::linux_timespec_argument_syscall_sequence_width_with_plan(
                        input.target.architecture,
                        operands,
                        *number,
                        plan,
                    )
                }
                HostBindingMechanism::Syscall { number, .. }
                    if omega_result_present(host_operation.operation_key, plan) =>
                {
                    omega_instruction_selection::value_syscall_sequence_width_with_plan(
                        input.target.architecture,
                        operands,
                        *number,
                        plan,
                    )
                }
                HostBindingMechanism::Syscall { number, .. } => {
                    encode_syscall_sequence_with_plan(
                        input.target.architecture,
                        operands,
                        *number,
                        plan,
                    )
                    .map_err(|error| {
                        Diagnostic::error(format!(
                            "host operation {}.{} has no encodable syscall sequence: {}",
                            host_operation.operation_key.capability_name(),
                            host_operation.operation_key.operation_name(),
                            error.message,
                        ))
                    })?
                    .len()
                }
                HostBindingMechanism::VtableSlot { index } => {
                    vtable_call_sequence_width_with_plan(
                        input.target,
                        operands,
                        *index,
                        false,
                        plan,
                    )
                }
                HostBindingMechanism::VtableField {
                    byte_offset,
                    ..
                } => {
                    let result_present =
                        field_model_result_present(operands.len(), plan, 0, "vtable-field")?;
                    omega_instruction_selection::encode_vtable_call_sequence_at_offset_with_plan(
                        input.target,
                        operands,
                        *byte_offset,
                        result_present,
                        plan,
                    )
                    .map_err(|error| {
                        Diagnostic::error(format!(
                            "host operation {}.{} has no encodable vtable-field call: {}",
                            host_operation.operation_key.capability_name(),
                            host_operation.operation_key.operation_name(),
                            error.message,
                        ))
                    })?
                    .len()
                }
                HostBindingMechanism::TableFunction {
                    byte_offset,
                    ..
                } => {
                    omega_instruction_selection::encode_table_function_call_sequence_with_plan(
                        input.target,
                        operands,
                        *byte_offset,
                        field_model_result_present(operands.len(), plan, 1, "table-function")?,
                        plan,
                    )
                    .map_err(|error| {
                        Diagnostic::error(format!(
                            "host operation {}.{} has no encodable table-function call: {}",
                            host_operation.operation_key.capability_name(),
                            host_operation.operation_key.operation_name(),
                            error.message,
                        ))
                    })?
                    .len()
                }
                HostBindingMechanism::Import { .. }
                    if matches!(
                        host_operation.operation_key.capability,
                        omega_calling_conventions::HostCapability::Custom(_)
                            | omega_calling_conventions::HostCapability::Unknown
                    ) =>
                {
                    authored_import_call_sequence_width(input.target, operands, plan)
                }
                HostBindingMechanism::Import { .. } => host_call_sequence_width_with_plan(
                    input.target,
                    host_operation.operation_key,
                    operands,
                    plan,
                ),
            }
        };
        // A host call is never legitimately empty: a zero width means the
        // encoder rejected the operands (e.g. an argument that failed to
        // marshal). Refuse loudly -- a zero-byte call would still have its
        // import relocation applied, which corrupts whatever instruction
        // lands at that offset and crashes the binary at runtime.
        if width == 0 {
            if matches!(
                binding.map(|binding| &binding.mechanism),
                Some(HostBindingMechanism::Import { .. })
            ) && !matches!(
                host_operation.operation_key.capability,
                omega_calling_conventions::HostCapability::Custom(_)
                    | omega_calling_conventions::HostCapability::Unknown
            ) && let Err(error) = encode_host_call_sequence_with_plan(
                input.target,
                host_operation.operation_key,
                operands,
                binding
                    .map(omega_calling_conventions::HostBinding::call_plan)
                    .expect("selected import binding was resolved above"),
            ) {
                return Err(Diagnostic::error(format!(
                    "host operation {}.{} has no encodable call sequence: {}",
                    host_operation.operation_key.capability_name(),
                    host_operation.operation_key.operation_name(),
                    error.message,
                )));
            }
            return Err(Diagnostic::error(format!(
                "host operation {}.{} has no encodable call sequence: a host-call argument \
                 must be a simple value (a local, field, parameter, or literal). Bind a computed \
                 argument -- arithmetic, a cast, or a value-call result -- to a local or field \
                 first, then pass that. (refusing to emit a zero-byte host call)",
                host_operation.operation_key.capability_name(),
                host_operation.operation_key.operation_name(),
            )));
        }
        let control_restore_width =
            if binding.is_some_and(|binding| binding.mechanism.requires_float_control_restore()) {
                omega_instruction_selection::foreign_float_control_trampoline_width(
                    input.target.architecture,
                )
            } else {
                0
            };
        return Ok(width + control_restore_width);
    }

    if let SelectedInstructionKind::DynamicTableCall {
        byte_offset,
        result_present,
        call_plan,
        operands,
        ..
    } = kind
    {
        let operands = input
            .assigned_target_operations
            .instruction_operands(*operands)
            .ok_or_else(|| Diagnostic::error("dynamic table call lost its operand span"))?;
        return Ok(
            omega_instruction_selection::table_function_call_sequence_width_with_plan(
                input.target,
                operands,
                *byte_offset,
                *result_present,
                call_plan,
            ),
        );
    }

    let width = match kind {
        SelectedInstructionKind::EnterDispatchLoop { .. } => {
            dispatch_loop_enter_width(input.target.architecture)
        }
        SelectedInstructionKind::EnterDispatchCase { .. } => {
            dispatch_case_enter_width(input.target.architecture)
        }
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::CompareStaticValue,
            operator:
                StateGuardOperator::Equal
                | StateGuardOperator::NotEqual
                | StateGuardOperator::Greater
                | StateGuardOperator::GreaterOrEqual
                | StateGuardOperator::Less
                | StateGuardOperator::LessOrEqual
                | StateGuardOperator::GreaterUnsigned
                | StateGuardOperator::GreaterOrEqualUnsigned
                | StateGuardOperator::LessUnsigned
                | StateGuardOperator::LessOrEqualUnsigned,
            has_storage: true,
            byte_offset,
            byte_size,
            is_float,
            ..
        } => dispatch_guard_compare_static_width(
            input.target.architecture,
            *byte_offset,
            *byte_size,
            *is_float,
        ),
        // A forward skip-jump is a plain unconditional `jmp rel32` -- same shape as a
        // dispatch-case leave, so it reuses that width/encoder.
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::ForwardBranchSkip,
            ..
        } => dispatch_case_leave_width(input.target.architecture),
        SelectedInstructionKind::CompareRuntimeTextLiteral { literal, .. } => {
            runtime_text_literal_compare_width(input.target.architecture, literal)
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer,
            source_offset,
            ..
        } => {
            let literal_len = input.data.objects.get(*buffer).bytes.len();
            runtime_text_storage_compare_width(
                input.target.architecture,
                *source_offset,
                literal_len,
            )
        }
        SelectedInstructionKind::ComparePlaces {
            left,
            right,
            byte_size,
            operator,
            is_float,
        } => omega_instruction_selection::place_compare_width(
            input.target.architecture,
            left,
            right,
            *byte_size,
            *operator,
            *is_float,
        )?,
        SelectedInstructionKind::ComparePlaceValue {
            place,
            byte_size,
            expected_value,
            operator,
        } => omega_instruction_selection::place_value_compare_width(
            input.target.architecture,
            place,
            *byte_size,
            *expected_value,
            *operator,
        )?,
        SelectedInstructionKind::CompareRuntimeValues {
            left,
            right,
            byte_size,
            ..
        } => runtime_value_compare_width(
            input.target.architecture,
            input.assigned_target_operations,
            *byte_size,
            *left,
            *right,
        ),
        SelectedInstructionKind::WriteRuntimeTextLiteral { literal, .. } => {
            runtime_text_literal_write_width(input.target.architecture, literal)
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment { literal, .. } => {
            runtime_text_literal_segment_write_width(input.target.architecture, literal)
        }
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
            buffer_offset,
            source_offset,
            target_offset,
            length_delta,
            ..
        } => runtime_text_stored_suffix_append_width(
            input.target.architecture,
            *buffer_offset,
            *source_offset,
            *target_offset,
            *length_delta,
        ),
        // Task #132: the place-shaped survivors decompose by shape; the
        // width fns mirror the encoders exactly.
        SelectedInstructionKind::MaterializeTextBufferToPlace { target, .. } => {
            omega_instruction_selection::runtime_text_buffer_materialize_to_place_width(
                input.target.architecture,
                target,
            )
        }
        SelectedInstructionKind::AppendTextStoredToPlace {
            source_offset,
            target,
            ..
        } => omega_instruction_selection::runtime_text_stored_place_append_to_place_width(
            input.target.architecture,
            *source_offset,
            target,
        ),
        SelectedInstructionKind::AppendTextLiteralToPlace {
            target, literal, ..
        } => omega_instruction_selection::runtime_text_literal_append_to_place_width(
            input.target.architecture,
            target,
            literal,
        ),
        SelectedInstructionKind::WriteRuntimeStorageConvert {
            target_offset,
            target_byte_size,
            source,
            source_byte_size,
            source_is_float,
            target_is_float,
            source_signed,
            target_signed,
            trapping,
            saturating,
            ..
        } => runtime_storage_convert_width(
            input.target.architecture,
            input.assigned_target_operations,
            *target_offset,
            *source,
            *source_byte_size,
            *target_byte_size,
            *source_is_float,
            *target_is_float,
            *source_signed,
            *target_signed,
            *trapping,
            *saturating,
        ),
        SelectedInstructionKind::WritePlaceConvert {
            target,
            target_byte_size,
            source,
            source_byte_size,
            source_is_float,
            target_is_float,
            source_signed,
            target_signed,
            trapping,
            saturating,
        } => omega_instruction_selection::write_place_convert_width(
            input.target.architecture,
            input.assigned_target_operations,
            target,
            *target_byte_size,
            *source,
            *source_byte_size,
            *source_is_float,
            *target_is_float,
            *source_signed,
            *target_signed,
            *trapping,
            *saturating,
        )?,
        SelectedInstructionKind::AtomicLoad {
            source_offset,
            byte_size,
            result_offset,
            ..
        } => runtime_atomic_load_to_storage_width(
            input.target.architecture,
            *source_offset,
            *byte_size,
            *result_offset,
        ),
        SelectedInstructionKind::AtomicStore {
            target_offset,
            byte_size,
            value,
            ..
        } => runtime_atomic_store_from_operand_width(
            input.target.architecture,
            input.assigned_target_operations,
            *target_offset,
            *byte_size,
            *value,
        ),
        SelectedInstructionKind::AtomicFetchAdd {
            target_offset,
            byte_size,
            result_offset,
            delta,
            ..
        } => runtime_atomic_fetch_add_width(
            input.target.architecture,
            input.assigned_target_operations,
            *target_offset,
            *byte_size,
            *result_offset,
            *delta,
        ),
        SelectedInstructionKind::AtomicFetchSub {
            target_offset,
            byte_size,
            result_offset,
            delta,
            ..
        } => runtime_atomic_fetch_sub_width(
            input.target.architecture,
            input.assigned_target_operations,
            *target_offset,
            *byte_size,
            *result_offset,
            *delta,
        ),
        SelectedInstructionKind::AtomicFetchXor {
            target_offset,
            byte_size,
            result_offset,
            value,
            ..
        } => runtime_atomic_fetch_xor_width(
            input.target.architecture,
            input.assigned_target_operations,
            *target_offset,
            *byte_size,
            *result_offset,
            *value,
        ),
        SelectedInstructionKind::AtomicFetchOr {
            target_offset,
            byte_size,
            result_offset,
            value,
            ..
        } => runtime_atomic_fetch_or_width(
            input.target.architecture,
            input.assigned_target_operations,
            *target_offset,
            *byte_size,
            *result_offset,
            *value,
        ),
        SelectedInstructionKind::AtomicFetchAnd {
            target_offset,
            byte_size,
            result_offset,
            value,
            ..
        } => runtime_atomic_fetch_and_width(
            input.target.architecture,
            input.assigned_target_operations,
            *target_offset,
            *byte_size,
            *result_offset,
            *value,
        ),
        SelectedInstructionKind::AtomicSwap {
            target_offset,
            byte_size,
            result_offset,
            new_value,
            ..
        } => runtime_atomic_swap_width(
            input.target.architecture,
            input.assigned_target_operations,
            *target_offset,
            *byte_size,
            *result_offset,
            *new_value,
        ),
        SelectedInstructionKind::AtomicCompareExchange {
            target_offset,
            byte_size,
            result_offset,
            expected,
            new_value,
            ..
        } => runtime_atomic_compare_exchange_width(
            input.target.architecture,
            input.assigned_target_operations,
            *target_offset,
            *byte_size,
            *result_offset,
            *expected,
            *new_value,
        ),
        SelectedInstructionKind::AppendWireLiteralByte {
            out_offset,
            written_offset,
            ..
        } => omega_instruction_selection::append_wire_literal_byte_width(
            input.target.architecture,
            *out_offset,
            *written_offset,
        ),
        SelectedInstructionKind::AppendWireScalarVarint {
            source_offset,
            byte_size,
            zigzag,
            out_offset,
            written_offset,
            ..
        } => omega_instruction_selection::append_wire_scalar_varint_width(
            input.target.architecture,
            *source_offset,
            *byte_size,
            *zigzag,
            *out_offset,
            *written_offset,
        ),
        SelectedInstructionKind::AppendWireTextBytes {
            source_offset,
            out_offset,
            out_length,
            written_offset,
            ..
        } => omega_instruction_selection::append_wire_text_bytes_width(
            input.target.architecture,
            *source_offset,
            *out_offset,
            *out_length,
            *written_offset,
        ),
        SelectedInstructionKind::AppendWireScalarSlice {
            source_offset,
            element_byte_size,
            zigzag,
            out_offset,
            out_length,
            written_offset,
            ..
        } => omega_instruction_selection::append_wire_scalar_slice_width(
            input.target.architecture,
            *source_offset,
            *element_byte_size,
            *zigzag,
            *out_offset,
            *out_length,
            *written_offset,
        ),
        SelectedInstructionKind::ReadWireExpectedByte {
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            ..
        } => omega_instruction_selection::read_wire_expected_byte_width(
            input.target.architecture,
            *buffer_offset,
            *buffer_length,
            *read_offset,
            *ok_offset,
        ),
        SelectedInstructionKind::ReadWireScalarVarint {
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_offset,
            byte_size,
            zigzag,
            range,
            ..
        } => omega_instruction_selection::read_wire_scalar_varint_width(
            input.target.architecture,
            *buffer_offset,
            *buffer_length,
            *read_offset,
            *ok_offset,
            *target_offset,
            *byte_size,
            *zigzag,
            *range,
        ),
        SelectedInstructionKind::ReadWireByteSlice {
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_offset,
            predicate_mask,
            ..
        } => omega_instruction_selection::read_wire_byte_slice_width(
            input.target.architecture,
            *buffer_offset,
            *buffer_length,
            *read_offset,
            *ok_offset,
            *target_offset,
            *predicate_mask,
        ),
        SelectedInstructionKind::ReadWireNestedOpen {
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            end_offset,
            ..
        } => omega_instruction_selection::read_wire_nested_open_width(
            input.target.architecture,
            *buffer_offset,
            *buffer_length,
            *read_offset,
            *ok_offset,
            *end_offset,
        ),
        SelectedInstructionKind::ReadWireNestedClose {
            buffer_offset,
            read_offset,
            ok_offset,
            end_offset,
            ..
        } => omega_instruction_selection::read_wire_nested_close_width(
            input.target.architecture,
            *buffer_offset,
            *read_offset,
            *ok_offset,
            *end_offset,
        ),
        SelectedInstructionKind::AppendWireRepeatedScalarVarint {
            source_offset,
            byte_size,
            zigzag,
            index,
            count_offset,
            out_offset,
            written_offset,
            ..
        } => omega_instruction_selection::append_wire_repeated_scalar_varint_width(
            input.target.architecture,
            *source_offset,
            *byte_size,
            *zigzag,
            *index,
            *count_offset,
            *out_offset,
            *written_offset,
        ),
        SelectedInstructionKind::ReadWireRepeatedScalarVarint {
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            end_offset,
            count_offset,
            target_offset,
            byte_size,
            zigzag,
            range,
            ..
        } => omega_instruction_selection::read_wire_repeated_scalar_varint_width(
            input.target.architecture,
            *buffer_offset,
            *buffer_length,
            *read_offset,
            *ok_offset,
            *end_offset,
            *count_offset,
            *target_offset,
            *byte_size,
            *zigzag,
            *range,
        ),
        SelectedInstructionKind::AppendPlaceBoundedBufferSource { target, source } => {
            omega_instruction_selection::append_place_bounded_buffer_source_width(
                input.target.architecture,
                target,
                source,
            )?
        }
        SelectedInstructionKind::AppendPlaceBoundedBufferLiteral { target, literal } => {
            omega_instruction_selection::append_place_bounded_buffer_literal_width(
                input.target.architecture,
                target,
                literal,
            )?
        }
        SelectedInstructionKind::ReadRuntimeTextLine { .. } => {
            let Some(read) = selected_host_text_read(kind) else {
                return Err(Diagnostic::error(
                    "cannot lay out runtime text read: missing host operation source",
                ));
            };
            let Some(binding) = input
                .assigned_target_operations
                .host_binding(read.operation_key)
            else {
                return Err(Diagnostic::error(format!(
                    "missing host binding for runtime text read operation {}.{}",
                    read.operation_key.capability_name(),
                    read.operation_key.operation_name()
                )));
            };
            runtime_text_line_read_width_with_plans(
                input.target.architecture,
                read.byte_capacity,
                &binding.mechanism,
                read.target,
                read.target_offset,
                runtime_text_call_plans(input, read.operation_key, binding)?,
            )
        }
        SelectedInstructionKind::ReadRuntimeByte {
            target_offset,
            payload_offset,
            source,
            ..
        } => {
            let RuntimeTextReadSource::HostOperation { operation_key } = source;
            let Some(binding) = input
                .assigned_target_operations
                .host_binding(*operation_key)
            else {
                return Err(Diagnostic::error(
                    "missing host binding for runtime byte read",
                ));
            };
            runtime_byte_read_width_with_plans(
                input.target.architecture,
                &binding.mechanism,
                *target_offset,
                *payload_offset,
                runtime_text_call_plans(input, *operation_key, binding)?,
            )
        }
        SelectedInstructionKind::WriteRuntimeByte {
            source,
            source_offset,
            ..
        } => {
            let RuntimeTextReadSource::HostOperation { operation_key } = source;
            let Some(binding) = input
                .assigned_target_operations
                .host_binding(*operation_key)
            else {
                return Err(Diagnostic::error(
                    "missing host binding for runtime byte write",
                ));
            };
            runtime_byte_write_width_with_plans(
                input.target.architecture,
                &binding.mechanism,
                *source_offset,
                runtime_text_call_plans(input, *operation_key, binding)?,
            )
        }
        SelectedInstructionKind::CopyPlaces {
            source,
            target,
            byte_count,
            ..
        } => omega_instruction_selection::copy_places_width(
            input.target.architecture,
            source,
            target,
            *byte_count,
        )?,
        SelectedInstructionKind::WritePlaceInteger {
            target,
            value,
            byte_size,
        } => omega_instruction_selection::write_place_integer_width(
            input.target.architecture,
            target,
            *value,
            *byte_size,
        )?,
        SelectedInstructionKind::WriteStorageBitField {
            base_byte_offset,
            fragments,
            ..
        } => omega_instruction_selection::runtime_storage_bit_field_write_width(
            input.target.architecture,
            *base_byte_offset,
            fragments,
        )?,
        SelectedInstructionKind::WritePlaceString {
            target,
            byte_length,
            ..
        } => omega_instruction_selection::write_place_string_width(
            input.target.architecture,
            target,
            *byte_length,
        )?,
        SelectedInstructionKind::WritePlaceBoundedBuffer { target, literal } => {
            omega_instruction_selection::write_place_bounded_buffer_width(
                input.target.architecture,
                target,
                literal,
            )?
        }
        SelectedInstructionKind::WritePlaceAddress {
            source,
            target_offset,
        } => omega_instruction_selection::write_place_address_width(
            input.target.architecture,
            source,
            *target_offset,
        )?,
        SelectedInstructionKind::WriteDataAddressToRuntimeFrame { target_offset, .. } => {
            omega_instruction_selection::runtime_frame_data_address_write_width(
                input.target.architecture,
                *target_offset,
            )?
        }
        SelectedInstructionKind::WritePlaceBinary {
            target,
            byte_size,
            left,
            operator,
            right,
            is_float,
            domain,
            target_signed,
        } => omega_instruction_selection::write_place_binary_width(
            input.target.architecture,
            input.assigned_target_operations,
            target,
            *byte_size,
            *left,
            *operator,
            *right,
            *is_float,
            *domain,
            *target_signed,
        )?,
        SelectedInstructionKind::SetDispatchState { .. }
        | SelectedInstructionKind::TerminateDispatch => {
            dispatch_state_write_width(input.target.architecture)
        }
        SelectedInstructionKind::WriteReturnRegisterInteger {
            register,
            byte_size,
            ..
        } => return_register_integer_write_width(input.target.architecture, *register, *byte_size),
        SelectedInstructionKind::CopyRuntimeStorageToReturnRegister {
            register,
            byte_offset,
            byte_size,
            ..
        } => runtime_storage_copy_to_return_register_width(
            input.target.architecture,
            *register,
            *byte_offset,
            *byte_size,
        ),
        SelectedInstructionKind::WriteEntryArgumentRegister {
            register,
            byte_size,
            ..
        } => entry_argument_register_write_width(input.target.architecture, *register, *byte_size),
        SelectedInstructionKind::WriteEntryStackArgument { byte_size, .. } => {
            entry_stack_argument_write_width(input.target.architecture, *byte_size)
        }
        SelectedInstructionKind::WriteEntryIndirectArgument {
            pointer,
            byte_offset,
            byte_size,
        } => entry_indirect_argument_write_width(
            input.target.architecture,
            *pointer,
            *byte_offset,
            *byte_size,
        ),
        SelectedInstructionKind::WriteEntryArgumentsSliceDescriptor { .. } => {
            entry_arguments_slice_descriptor_write_width(input.target.architecture)
        }
        SelectedInstructionKind::LeaveDispatchCase => {
            dispatch_case_leave_width(input.target.architecture)
        }
        SelectedInstructionKind::EnterFunction => function_enter_width(input.target.architecture),
        SelectedInstructionKind::MachineHalt => machine_halt_width(input.target.architecture),
        SelectedInstructionKind::MemoryFence(kind) => {
            return memory_fence_width(input.target.architecture).ok_or_else(|| {
                Diagnostic::error(format!(
                    "asm instruction `{}` is x86_64-only",
                    kind.mnemonic(),
                ))
            });
        }
        SelectedInstructionKind::InterruptControl(kind) => {
            return interrupt_control_width(input.target.architecture).ok_or_else(|| {
                Diagnostic::error(format!(
                    "asm instruction `{}` is x86_64-only",
                    kind.mnemonic(),
                ))
            });
        }
        SelectedInstructionKind::FlagsSnapshot { .. } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(Diagnostic::error("asm instruction `pushfq` is x86_64-only"));
            }
            flags_snapshot_width()
        }
        SelectedInstructionKind::FlagsRestore { source } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(Diagnostic::error("asm instruction `popfq` is x86_64-only"));
            }
            flags_restore_width(input.assigned_target_operations, *source)
        }
        SelectedInstructionKind::MsrRead { index, .. } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(Diagnostic::error("asm instruction `rdmsr` is x86_64-only"));
            }
            msr_read_width(input.assigned_target_operations, *index)
        }
        SelectedInstructionKind::MsrWrite { index, value } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(Diagnostic::error("asm instruction `wrmsr` is x86_64-only"));
            }
            msr_write_width(input.assigned_target_operations, *index, *value)
        }
        SelectedInstructionKind::ControlRegisterRead { register, .. } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(Diagnostic::error(format!(
                    "asm instruction `{}` is x86_64-only",
                    register.read_mnemonic()
                )));
            }
            control_register_read_width()
        }
        SelectedInstructionKind::ControlRegisterWrite { register, source } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(Diagnostic::error(format!(
                    "asm instruction `{}` is x86_64-only",
                    register
                        .write_mnemonic()
                        .expect("writable control register")
                )));
            }
            control_register_write_width(input.assigned_target_operations, *source)
        }
        SelectedInstructionKind::PortWrite { port, value } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(Diagnostic::error(
                    "port I/O (`asm { out .. }`) is x86_64-only; ARM has no port space",
                ));
            }
            port_write_width(input.assigned_target_operations, *port, *value)
        }
        SelectedInstructionKind::PortRead { port, .. } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(Diagnostic::error(
                    "port I/O (`asm { in .. }`) is x86_64-only; ARM has no port space",
                ));
            }
            port_read_width(input.assigned_target_operations, *port)
        }
        SelectedInstructionKind::CallInternalFunction { .. } => match input.target.architecture {
            omega_target::Architecture::X86_64 => omega_isa_x86_64::internal_function_call_width(),
            omega_target::Architecture::Aarch64 => {
                omega_isa_aarch64::internal_function_call_width()
            }
        },
        SelectedInstructionKind::LoadOutgoingStackAddress { .. } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(Diagnostic::error(
                    "outgoing stack-address loads are supported only on x86-64",
                ));
            }
            omega_isa_x86_64::outgoing_stack_address_load_width()
        }
        SelectedInstructionKind::ReserveOutgoingStackFrame { byte_count }
        | SelectedInstructionKind::ReleaseOutgoingStackFrame { byte_count } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(Diagnostic::error(
                    "outgoing stack frames are supported only on x86-64",
                ));
            }
            omega_isa_x86_64::outgoing_stack_frame_adjust_width(*byte_count)?
        }
        SelectedInstructionKind::WriteOutgoingStackU64 { .. } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(Diagnostic::error(
                    "outgoing stack u64 writes are supported only on x86-64",
                ));
            }
            omega_isa_x86_64::outgoing_stack_u64_write_width()
        }
        SelectedInstructionKind::CopyEntryIndirectU64ToOutgoingStack { .. } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(Diagnostic::error(
                    "entry-indirect outgoing stack copies are supported only on x86-64",
                ));
            }
            omega_isa_x86_64::entry_indirect_u64_to_outgoing_stack_copy_width()
        }
        SelectedInstructionKind::LeaveFunction => return_width(input.target.architecture),
        SelectedInstructionKind::EvaluateDispatchGuard { .. }
        | SelectedInstructionKind::LeaveDispatchLoop
        | SelectedInstructionKind::BeginPlatformCall => 0,
        SelectedInstructionKind::HostOperation { .. }
        | SelectedInstructionKind::DynamicTableCall { .. } => {
            return Err(Diagnostic::error(
                "internal error: host operation was not handled by machine layout host query",
            ));
        }
    };
    let control_restore_width = if instruction_requires_float_control_restore(input, kind) {
        omega_instruction_selection::foreign_float_control_trampoline_width(
            input.target.architecture,
        )
    } else {
        0
    };
    Ok(width + control_restore_width)
}
