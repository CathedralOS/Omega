mod host;
mod runtime_storage;
mod runtime_text;

use crate::MachineEmissionContext;
use crate::layout::LaidOutMachineInstruction;
use crate::selected_instruction_queries::{selected_host_operation, selected_host_text_read};
use omega_assigned_target_operations::SelectedInstructionKind;
use psi_diagnostics::Diagnostic;

pub(super) fn encode_machine_instruction_bytes(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
    kind: &SelectedInstructionKind,
) -> Result<Vec<u8>, Diagnostic> {
    if let Some(host_operation) = selected_host_operation(kind) {
        let Some(operands) = input
            .assigned_target_operations
            .instruction_operands(host_operation.operands)
        else {
            return Err(Diagnostic::error(
                "cannot encode host operation: missing operand span",
            ));
        };

        return host::encode_host_operation(input, host_operation.operation_key, operands);
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
        return omega_instruction_selection::encode_table_function_call_sequence_with_plan(
            input.target,
            operands,
            *byte_offset,
            *result_present,
            call_plan,
        );
    }

    match kind {
        SelectedInstructionKind::CompareRuntimeTextLiteral { literal, .. } => {
            runtime_text::encode_runtime_text_literal_compare(
                input,
                machine_instructions,
                machine_instruction_index,
                literal,
            )
        }
        SelectedInstructionKind::CompareRuntimeValues {
            left,
            right,
            byte_size,
            operator,
        } => runtime_storage::encode_runtime_value_compare(
            input,
            machine_instructions,
            machine_instruction_index,
            *left,
            *right,
            *byte_size,
            *operator,
        ),
        SelectedInstructionKind::WriteRuntimeTextLiteral { literal, .. } => {
            runtime_text::encode_runtime_text_literal_write(input, literal)
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
            byte_offset,
            literal,
            ..
        } => runtime_text::encode_runtime_text_literal_segment_write(input, *byte_offset, literal),
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
            buffer_offset,
            source_offset,
            target_offset,
            length_delta,
            ..
        } => runtime_text::encode_runtime_text_stored_suffix_append(
            input,
            *buffer_offset,
            *source_offset,
            *target_offset,
            *length_delta,
        ),
        // Task #132: the place-shaped text-crossing survivors DECOMPOSE by
        // place shape to the retained per-shape encoders on both
        // architectures (the transitional pattern); unsupported shapes
        // refuse loudly.
        SelectedInstructionKind::MaterializeTextBufferToPlace { target, .. } => {
            omega_instruction_selection::encode_runtime_text_buffer_materialize_to_place(
                input.target.architecture,
                target,
            )
        }
        SelectedInstructionKind::AppendTextStoredToPlace {
            source_offset,
            target,
            ..
        } => omega_instruction_selection::encode_runtime_text_stored_place_append_to_place(
            input.target.architecture,
            *source_offset,
            target,
        ),
        SelectedInstructionKind::AppendTextLiteralToPlace {
            target, literal, ..
        } => omega_instruction_selection::encode_runtime_text_literal_append_to_place(
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
        } => runtime_storage::encode_runtime_storage_convert(
            input,
            *target_offset,
            *target_byte_size,
            *source,
            *source_byte_size,
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
        } => omega_instruction_selection::encode_write_place_convert(
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
        ),
        SelectedInstructionKind::AtomicLoad {
            source_offset,
            byte_size,
            result_offset,
            ordering,
            ..
        } => runtime_storage::encode_atomic_load_to_storage(
            input,
            *source_offset,
            *byte_size,
            *result_offset,
            *ordering,
        ),
        SelectedInstructionKind::AtomicStore {
            target_offset,
            byte_size,
            value,
            ordering,
            ..
        } => runtime_storage::encode_atomic_store_from_operand(
            input,
            *target_offset,
            *byte_size,
            *value,
            *ordering,
        ),
        SelectedInstructionKind::AtomicFetchAdd {
            target_offset,
            byte_size,
            result_offset,
            delta,
            ordering,
            ..
        } => runtime_storage::encode_atomic_fetch_add(
            input,
            *target_offset,
            *byte_size,
            *result_offset,
            *delta,
            *ordering,
        ),
        SelectedInstructionKind::AtomicFetchSub {
            target_offset,
            byte_size,
            result_offset,
            delta,
            ordering,
            ..
        } => runtime_storage::encode_atomic_fetch_sub(
            input,
            *target_offset,
            *byte_size,
            *result_offset,
            *delta,
            *ordering,
        ),
        SelectedInstructionKind::AtomicFetchXor {
            target_offset,
            byte_size,
            result_offset,
            value,
            ordering,
            ..
        } => runtime_storage::encode_atomic_fetch_xor(
            input,
            *target_offset,
            *byte_size,
            *result_offset,
            *value,
            *ordering,
        ),
        SelectedInstructionKind::AtomicFetchOr {
            target_offset,
            byte_size,
            result_offset,
            value,
            ordering,
            ..
        } => runtime_storage::encode_atomic_fetch_or(
            input,
            *target_offset,
            *byte_size,
            *result_offset,
            *value,
            *ordering,
        ),
        SelectedInstructionKind::AtomicFetchAnd {
            target_offset,
            byte_size,
            result_offset,
            value,
            ordering,
            ..
        } => runtime_storage::encode_atomic_fetch_and(
            input,
            *target_offset,
            *byte_size,
            *result_offset,
            *value,
            *ordering,
        ),
        SelectedInstructionKind::AtomicSwap {
            target_offset,
            byte_size,
            result_offset,
            new_value,
            ordering,
            ..
        } => runtime_storage::encode_atomic_swap(
            input,
            *target_offset,
            *byte_size,
            *result_offset,
            *new_value,
            *ordering,
        ),
        SelectedInstructionKind::AtomicCompareExchange {
            target_offset,
            byte_size,
            result_offset,
            expected,
            new_value,
            ordering,
            ..
        } => runtime_storage::encode_atomic_compare_exchange(
            input,
            *target_offset,
            *byte_size,
            *result_offset,
            *expected,
            *new_value,
            *ordering,
        ),
        SelectedInstructionKind::AppendWireLiteralByte {
            out_offset,
            written_offset,
            value,
            ..
        } => omega_instruction_selection::encode_append_wire_literal_byte(
            input.target.architecture,
            *out_offset,
            *written_offset,
            *value,
        ),
        SelectedInstructionKind::AppendWireScalarVarint {
            source_region,
            source_offset,
            byte_size,
            zigzag,
            out_offset,
            written_offset,
            ..
        } => omega_instruction_selection::encode_append_wire_scalar_varint(
            input.target.architecture,
            *source_region,
            *source_offset,
            *byte_size,
            *zigzag,
            *out_offset,
            *written_offset,
        ),
        SelectedInstructionKind::AppendWireTextBytes {
            source_region,
            source_offset,
            out_offset,
            out_length,
            written_offset,
            ..
        } => omega_instruction_selection::encode_append_wire_text_bytes(
            input.target.architecture,
            *source_region,
            *source_offset,
            *out_offset,
            *out_length,
            *written_offset,
        ),
        SelectedInstructionKind::AppendWireScalarSlice {
            source_region,
            source_offset,
            element_byte_size,
            zigzag,
            out_offset,
            out_length,
            written_offset,
            ..
        } => omega_instruction_selection::encode_append_wire_scalar_slice(
            input.target.architecture,
            *source_region,
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
            expected,
            ..
        } => omega_instruction_selection::encode_read_wire_expected_byte(
            input.target.architecture,
            *buffer_offset,
            *buffer_length,
            *read_offset,
            *ok_offset,
            *expected,
        ),
        SelectedInstructionKind::ReadWireScalarVarint {
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_region,
            target_offset,
            byte_size,
            zigzag,
            range,
            ..
        } => omega_instruction_selection::encode_read_wire_scalar_varint(
            input.target.architecture,
            *buffer_offset,
            *buffer_length,
            *read_offset,
            *ok_offset,
            *target_region,
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
            target_region,
            target_offset,
            predicate_mask,
            ..
        } => omega_instruction_selection::encode_read_wire_byte_slice(
            input.target.architecture,
            *buffer_offset,
            *buffer_length,
            *read_offset,
            *ok_offset,
            *target_region,
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
        } => omega_instruction_selection::encode_read_wire_nested_open(
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
        } => omega_instruction_selection::encode_read_wire_nested_close(
            input.target.architecture,
            *buffer_offset,
            *read_offset,
            *ok_offset,
            *end_offset,
        ),
        SelectedInstructionKind::AppendWireRepeatedScalarVarint {
            source_region,
            source_offset,
            byte_size,
            zigzag,
            index,
            count_region,
            count_offset,
            out_offset,
            written_offset,
            ..
        } => omega_instruction_selection::encode_append_wire_repeated_scalar_varint(
            input.target.architecture,
            *source_region,
            *source_offset,
            *byte_size,
            *zigzag,
            *index,
            *count_region,
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
            count_region,
            count_offset,
            target_region,
            target_offset,
            byte_size,
            zigzag,
            range,
            ..
        } => omega_instruction_selection::encode_read_wire_repeated_scalar_varint(
            input.target.architecture,
            *buffer_offset,
            *buffer_length,
            *read_offset,
            *ok_offset,
            *end_offset,
            *count_region,
            *count_offset,
            *target_region,
            *target_offset,
            *byte_size,
            *zigzag,
            *range,
        ),
        SelectedInstructionKind::AppendPlaceBoundedBufferSource { target, source } => {
            omega_instruction_selection::encode_append_place_bounded_buffer_source(
                input.target.architecture,
                target,
                source,
            )
        }
        SelectedInstructionKind::AppendPlaceBoundedBufferLiteral { target, literal } => {
            omega_instruction_selection::encode_append_place_bounded_buffer_literal(
                input.target.architecture,
                target,
                literal,
            )
        }
        SelectedInstructionKind::ReadRuntimeTextLine { .. } => {
            let Some(read) = selected_host_text_read(kind) else {
                return Err(Diagnostic::error(
                    "cannot encode runtime text read: missing host operation source",
                ));
            };
            runtime_text::encode_runtime_text_line_read(
                input,
                read.target_offset,
                read.byte_capacity,
                read.source,
                read.target,
            )
        }
        SelectedInstructionKind::ReadRuntimeByte {
            target_offset,
            payload_offset,
            source,
            ..
        } => runtime_text::encode_runtime_byte_read(input, *target_offset, *payload_offset, source),
        SelectedInstructionKind::WriteRuntimeByte {
            source_offset,
            source,
            source_is_place,
            ..
        } => {
            // A literal source relocates the adrp pair to the 1-byte data
            // object, whose byte sits at offset 0.
            let offset = if *source_is_place { *source_offset } else { 0 };
            runtime_text::encode_runtime_byte_write(input, offset, source)
        }
        SelectedInstructionKind::CopyPlaces {
            source,
            target,
            byte_count,
            ..
        } => omega_instruction_selection::encode_copy_places(
            input.target.architecture,
            source,
            target,
            *byte_count,
        ),
        SelectedInstructionKind::WritePlaceInteger {
            target,
            value,
            byte_size,
        } => omega_instruction_selection::encode_write_place_integer(
            input.target.architecture,
            target,
            *value,
            *byte_size,
        ),
        SelectedInstructionKind::WriteStorageBitField {
            base_byte_offset,
            fragments,
            value,
            ..
        } => omega_instruction_selection::encode_runtime_storage_bit_field_write(
            input.target.architecture,
            *base_byte_offset,
            fragments,
            *value,
        ),
        SelectedInstructionKind::WritePlaceString {
            target,
            byte_length,
            ..
        } => omega_instruction_selection::encode_write_place_string(
            input.target.architecture,
            target,
            *byte_length,
        ),
        SelectedInstructionKind::WritePlaceBoundedBuffer { target, literal } => {
            omega_instruction_selection::encode_write_place_bounded_buffer(
                input.target.architecture,
                target,
                literal,
            )
        }
        SelectedInstructionKind::WritePlaceAddress {
            source,
            target_offset,
        } => omega_instruction_selection::encode_write_place_address(
            input.target.architecture,
            source,
            *target_offset,
        ),
        SelectedInstructionKind::WriteDataAddressToRuntimeFrame { target_offset, .. } => {
            omega_instruction_selection::encode_runtime_frame_data_address_write(
                input.target.architecture,
                *target_offset,
            )
        }
        SelectedInstructionKind::WriteFunctionAddressToRuntimeStorage {
            target_region,
            target_offset,
            ..
        } => omega_instruction_selection::encode_runtime_storage_function_address_write(
            input.target.architecture,
            *target_region,
            *target_offset,
        ),
        SelectedInstructionKind::WritePlaceBinary {
            target,
            byte_size,
            left,
            operator,
            right,
            is_float,
            domain,
            target_signed,
        } => runtime_storage::encode_write_place_binary(
            input,
            target,
            *byte_size,
            *left,
            *operator,
            *right,
            *is_float,
            *domain,
            *target_signed,
        ),
        SelectedInstructionKind::MachineHalt => Ok(
            omega_instruction_selection::encode_machine_halt_bytes(input.target.architecture),
        ),
        SelectedInstructionKind::MemoryFence(kind) => {
            omega_instruction_selection::encode_memory_fence_bytes(input.target.architecture, *kind)
                .ok_or_else(|| {
                    psi_diagnostics::Diagnostic::error(format!(
                        "asm instruction `{}` is x86_64-only",
                        kind.mnemonic(),
                    ))
                })
        }
        SelectedInstructionKind::InterruptControl(kind) => {
            omega_instruction_selection::encode_interrupt_control_bytes(
                input.target.architecture,
                *kind,
            )
            .ok_or_else(|| {
                psi_diagnostics::Diagnostic::error(format!(
                    "asm instruction `{}` is x86_64-only",
                    kind.mnemonic(),
                ))
            })
        }
        SelectedInstructionKind::FlagsSnapshot {
            dest_byte_offset, ..
        } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(psi_diagnostics::Diagnostic::error(
                    "asm instruction `pushfq` is x86_64-only",
                ));
            }
            omega_instruction_selection::encode_flags_snapshot_bytes(*dest_byte_offset)
        }
        SelectedInstructionKind::FlagsRestore { source } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(psi_diagnostics::Diagnostic::error(
                    "asm instruction `popfq` is x86_64-only",
                ));
            }
            omega_instruction_selection::encode_flags_restore_bytes(
                input.assigned_target_operations,
                *source,
            )
        }
        SelectedInstructionKind::MsrRead {
            index,
            dest_byte_offset,
            ..
        } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(psi_diagnostics::Diagnostic::error(
                    "asm instruction `rdmsr` is x86_64-only",
                ));
            }
            omega_instruction_selection::encode_msr_read_bytes(
                input.assigned_target_operations,
                *index,
                *dest_byte_offset,
            )
        }
        SelectedInstructionKind::MsrWrite { index, value } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(psi_diagnostics::Diagnostic::error(
                    "asm instruction `wrmsr` is x86_64-only",
                ));
            }
            omega_instruction_selection::encode_msr_write_bytes(
                input.assigned_target_operations,
                *index,
                *value,
            )
        }
        SelectedInstructionKind::ControlRegisterRead {
            register,
            dest_byte_offset,
            ..
        } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "asm instruction `{}` is x86_64-only",
                    register.read_mnemonic()
                )));
            }
            omega_instruction_selection::encode_control_register_read_bytes(
                *register,
                *dest_byte_offset,
            )
        }
        SelectedInstructionKind::ControlRegisterWrite { register, source } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(psi_diagnostics::Diagnostic::error(format!(
                    "asm instruction `{}` is x86_64-only",
                    register
                        .write_mnemonic()
                        .expect("writable control register")
                )));
            }
            omega_instruction_selection::encode_control_register_write_bytes(
                input.assigned_target_operations,
                *register,
                *source,
            )
        }
        // Port I/O (`asm { out .. }` / `asm { in .. }`) has no branch distance;
        // its storage operands carry relocations, applied by omega-relocations
        // against the offsets pinned in the ISA encoders.
        SelectedInstructionKind::PortWrite { port, value } => {
            omega_instruction_selection::encode_port_write_bytes(
                input.assigned_target_operations,
                *port,
                *value,
            )
        }
        SelectedInstructionKind::PortRead {
            port,
            dest_byte_offset,
            ..
        } => omega_instruction_selection::encode_port_read_bytes(
            input.assigned_target_operations,
            *port,
            *dest_byte_offset,
        ),
        SelectedInstructionKind::CallInternalFunction { target } => {
            if !target.is_valid() {
                return Err(Diagnostic::error(
                    "internal direct call has no exact target function identity",
                ));
            }
            Ok(match input.target.architecture {
                omega_target::Architecture::X86_64 => {
                    omega_isa_x86_64::encode_internal_function_call_bytes().to_vec()
                }
                omega_target::Architecture::Aarch64 => {
                    omega_isa_aarch64::encode_internal_function_call_bytes().to_vec()
                }
            })
        }
        SelectedInstructionKind::LoadOutgoingStackAddress {
            register,
            stack_byte_offset,
        } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(Diagnostic::error(
                    "outgoing stack-address loads are supported only on x86-64",
                ));
            }
            Ok(omega_isa_x86_64::encode_outgoing_stack_address_load_bytes(
                *register,
                *stack_byte_offset,
            )?
            .to_vec())
        }
        SelectedInstructionKind::ReserveOutgoingStackFrame { byte_count } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(Diagnostic::error(
                    "outgoing stack frames are supported only on x86-64",
                ));
            }
            omega_isa_x86_64::encode_outgoing_stack_frame_reserve_bytes(*byte_count)
        }
        SelectedInstructionKind::WriteOutgoingStackU64 {
            stack_byte_offset,
            value,
        } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(Diagnostic::error(
                    "outgoing stack u64 writes are supported only on x86-64",
                ));
            }
            Ok(
                omega_isa_x86_64::encode_outgoing_stack_u64_write_bytes(
                    *stack_byte_offset,
                    *value,
                )?
                .to_vec(),
            )
        }
        SelectedInstructionKind::CopyEntryIndirectU64ToOutgoingStack {
            source_register,
            source_byte_offset,
            stack_byte_offset,
        } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(Diagnostic::error(
                    "entry-indirect outgoing stack copies are supported only on x86-64",
                ));
            }
            Ok(
                omega_isa_x86_64::encode_entry_indirect_u64_to_outgoing_stack_copy_bytes(
                    *source_register,
                    *source_byte_offset,
                    *stack_byte_offset,
                )?
                .to_vec(),
            )
        }
        SelectedInstructionKind::ReleaseOutgoingStackFrame { byte_count } => {
            if input.target.architecture != omega_target::Architecture::X86_64 {
                return Err(Diagnostic::error(
                    "outgoing stack frames are supported only on x86-64",
                ));
            }
            omega_isa_x86_64::encode_outgoing_stack_frame_release_bytes(*byte_count)
        }
        SelectedInstructionKind::EnterFunction
        | SelectedInstructionKind::EnterDispatchLoop { .. }
        | SelectedInstructionKind::EnterDispatchCase { .. }
        | SelectedInstructionKind::EvaluateDispatchGuard { .. }
        | SelectedInstructionKind::CompareRuntimeTextStorage { .. }
        | SelectedInstructionKind::ComparePlaces { .. }
        | SelectedInstructionKind::ComparePlaceValue { .. }
        | SelectedInstructionKind::SetDispatchState { .. }
        | SelectedInstructionKind::WriteReturnRegisterInteger { .. }
        | SelectedInstructionKind::CopyRuntimeStorageToReturnRegister { .. }
        | SelectedInstructionKind::WriteEntryArgumentRegister { .. }
        | SelectedInstructionKind::WriteEntryStackArgument { .. }
        | SelectedInstructionKind::WriteEntryIndirectArgument { .. }
        | SelectedInstructionKind::WriteEntryArgumentsSliceDescriptor { .. }
        | SelectedInstructionKind::TerminateDispatch
        | SelectedInstructionKind::LeaveDispatchCase
        | SelectedInstructionKind::LeaveDispatchLoop
        | SelectedInstructionKind::LeaveFunction
        | SelectedInstructionKind::BeginPlatformCall => Err(Diagnostic::error(
            "internal error: zero-width machine instruction reached byte encoder",
        )),
        SelectedInstructionKind::HostOperation { .. }
        | SelectedInstructionKind::DynamicTableCall { .. } => Err(Diagnostic::error(
            "internal error: host operation was not handled by machine encoder host query",
        )),
    }
}
