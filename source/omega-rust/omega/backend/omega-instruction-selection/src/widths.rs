use crate::aarch64_call_operand;
use omega_calling_conventions::HostBindingMechanism;
use omega_calling_conventions::HostOperationKey;
use omega_isa_aarch64::aarch64;
use omega_isa_x86_64 as x86_64;
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::{
    InstructionOperandLike, RuntimeBitFieldFragment, RuntimeTextReadTarget,
    RuntimeValueOperandHandle, RuntimeValueOperandSource, StateGuardOperator,
};
use psi_diagnostics::Diagnostic;

pub fn vtable_call_sequence_width_with_plan<T: InstructionOperandLike>(
    target: NativeTarget,
    operands: &[T],
    index: i64,
    result_present: bool,
    authoritative_plan: &omega_calling_conventions::CallPlan,
) -> usize {
    let dispatch_width = match target.architecture {
        Architecture::Aarch64 => aarch64::vtable_call_dispatch_width(index),
        Architecture::X86_64 => Some(0),
    };
    vtable_call_sequence_width_with_dispatch(
        target,
        operands,
        index,
        result_present,
        dispatch_width,
        authoritative_plan,
    )
}

pub fn vtable_call_sequence_width_at_offset_with_plan<T: InstructionOperandLike>(
    target: NativeTarget,
    operands: &[T],
    byte_offset: usize,
    result_present: bool,
    authoritative_plan: &omega_calling_conventions::CallPlan,
) -> usize {
    let dispatch_width = match target.architecture {
        Architecture::Aarch64 => aarch64::vtable_call_dispatch_width_at_offset(byte_offset),
        Architecture::X86_64 => Some(0),
    };
    vtable_call_sequence_width_with_dispatch(
        target,
        operands,
        i64::try_from(byte_offset).unwrap_or(i64::MAX),
        result_present,
        dispatch_width,
        authoritative_plan,
    )
}

fn vtable_call_sequence_width_with_dispatch<T: InstructionOperandLike>(
    target: NativeTarget,
    operands: &[T],
    index_or_offset: i64,
    result_present: bool,
    dispatch_width: Option<usize>,
    authoritative_plan: &omega_calling_conventions::CallPlan,
) -> usize {
    match target.architecture {
        Architecture::Aarch64 => {
            let Ok((placements, _)) = crate::normalized_aarch64_vtable_plan_with_plan(
                operands,
                result_present,
                authoritative_plan,
            ) else {
                return 0;
            };
            let Some(dispatch_width) = dispatch_width else {
                return 0;
            };
            aarch64_call_operands_width(operands, result_present)
                + aarch64::host_call_stack_total_width_for_placements(&placements)
                + dispatch_width
        }
        Architecture::X86_64
            if authoritative_plan.policy
                == omega_calling_conventions::CallingPolicy::MicrosoftX64 =>
        {
            x86_64::win64_vtable_call_width_with_plan(
                operands,
                index_or_offset,
                result_present,
                authoritative_plan,
            )
        }
        Architecture::X86_64
            if authoritative_plan.policy
                == omega_calling_conventions::CallingPolicy::SystemVAMD64 =>
        {
            x86_64::sysv_vtable_call_width_with_plan(
                operands,
                index_or_offset,
                result_present,
                authoritative_plan,
            )
        }
        Architecture::X86_64 => unreachable!(
            "x86 string-descriptor relocations come from the generic place materializer"
        ),
    }
}

pub fn table_function_call_sequence_width_with_plan<T: InstructionOperandLike>(
    target: NativeTarget,
    operands: &[T],
    byte_offset: usize,
    result_present: bool,
    authoritative_plan: &omega_calling_conventions::CallPlan,
) -> usize {
    match target.architecture {
        Architecture::Aarch64 => {
            let Ok((placements, _)) = crate::normalized_aarch64_table_function_plan_with_plan(
                operands,
                result_present,
                authoritative_plan,
            ) else {
                return 0;
            };
            let Some(dispatch_width) = aarch64::vtable_call_dispatch_width_at_offset(byte_offset)
            else {
                return 0;
            };
            aarch64_call_operands_width(operands, result_present)
                + aarch64::host_call_stack_total_width_for_placements(&placements)
                + dispatch_width
        }
        Architecture::X86_64
            if authoritative_plan.policy
                == omega_calling_conventions::CallingPolicy::MicrosoftX64 =>
        {
            x86_64::win64_table_function_call_width_with_plan(
                operands,
                i64::try_from(byte_offset).unwrap_or(i64::MAX),
                result_present,
                authoritative_plan,
            )
        }
        Architecture::X86_64
            if authoritative_plan.policy
                == omega_calling_conventions::CallingPolicy::SystemVAMD64 =>
        {
            x86_64::sysv_table_function_call_width_with_plan(
                operands,
                i64::try_from(byte_offset).unwrap_or(i64::MAX),
                result_present,
                authoritative_plan,
            )
        }
        Architecture::X86_64 => 0,
    }
}

pub fn host_call_sequence_width_no_plan<T: InstructionOperandLike>(
    target: NativeTarget,
    operation_key: HostOperationKey,
    operands: &[T],
) -> usize {
    match target.architecture {
        Architecture::Aarch64 => {
            // Selection signals an UNRESOLVABLE argument (or buffer) with an
            // EMPTY operand span so the architecture encoder hard-errors (see
            // select_host_operation_operands). x86_64's encoder rejects such a
            // call and this width becomes 0, tripping layout's loud zero-byte
            // host-call refusal; the aarch64 encoder would happily emit a bare
            // `bl` that DROPS the argument (`exit_process(a + b)` silently
            // exited garbage). Mirror the x86_64 contract.
            if operands.is_empty() {
                return 0;
            }
            // Constant-result ops (std::time calibration) have their own
            // no-call layout; keyed here exactly as in the encoder routing.
            if operation_key.lowers_to_constant_result()
                && let Some((_, byte_offset, byte_count)) = operands
                    .first()
                    .and_then(InstructionOperandLike::runtime_scalar_integer)
            {
                return aarch64::constant_result_sequence_width(byte_offset, byte_count);
            }
            let authored_import = matches!(
                operation_key.capability,
                omega_calling_conventions::HostCapability::Unknown
                    | omega_calling_conventions::HostCapability::Custom(_)
            );
            let base = if authored_import
                && let Some(result_width) = operands
                    .first()
                    .map(aarch64_call_operand)
                    .and_then(aarch64::indirect_result_address_width)
            {
                result_width
                    + operands[1..]
                        .iter()
                        .map(|operand| crate::operand_width(Architecture::Aarch64, operand))
                        .sum::<usize>()
                    + 4
            } else {
                aarch64::host_call_sequence_width_from_operands(
                    operands.iter().map(aarch64_call_operand),
                )
            };
            let Ok(argument_placements) =
                crate::normalized_aarch64_host_argument_placements_no_plan(
                    operation_key,
                    operands,
                    authored_import,
                )
            else {
                return 0;
            };
            let planned_stack =
                aarch64::host_call_stack_total_width_for_placements(&argument_placements);
            // A deref-result op (errno) emits one extra `ldr w0,[x0]` (4 bytes)
            // between the BL and the result store; keep the layout width in
            // lockstep with the encoder + the data-address relocation offset.
            // A float-returning op (sqrt/hypot) likewise emits one extra `fmov
            // x0,d0` (4 bytes) in the same slot (same lockstep discipline).
            // Outgoing stack reserve/store/restore widths come from the same
            // normalized placements consumed by emission and relocation.
            let deref = if operation_key.dereferences_result() {
                4
            } else {
                0
            };
            let float_return = if operation_key.returns_float() { 4 } else { 0 };
            base + planned_stack + deref + float_return
        }
        Architecture::X86_64 => x86_64::host_call_sequence_width_no_plan(
            omega_calling_conventions::CallingPolicy::native_for_target(target),
            operation_key,
            operands,
        ),
    }
}

pub fn host_call_sequence_width_with_plan<T: InstructionOperandLike>(
    target: NativeTarget,
    operation_key: HostOperationKey,
    operands: &[T],
    authoritative_plan: &omega_calling_conventions::CallPlan,
) -> usize {
    crate::encode_host_call_sequence_with_plan(target, operation_key, operands, authoritative_plan)
        .map(|bytes| bytes.len())
        .unwrap_or(0)
}

pub fn authored_import_call_sequence_width<T: InstructionOperandLike>(
    target: NativeTarget,
    operands: &[T],
    authoritative_plan: &omega_calling_conventions::CallPlan,
) -> usize {
    match target.architecture {
        Architecture::X86_64 => {
            x86_64::encode_authored_import_call_sequence(authoritative_plan, operands)
                .map(|bytes| bytes.len())
                .unwrap_or(0)
        }
        Architecture::Aarch64 => {
            crate::encode_authored_import_call_sequence(target, operands, authoritative_plan)
                .map(|bytes| bytes.len())
                .unwrap_or(0)
        }
    }
}

fn aarch64_call_operands_width<T: InstructionOperandLike>(
    operands: &[T],
    result_present: bool,
) -> usize {
    if result_present
        && let Some(result_width) = operands
            .first()
            .map(aarch64_call_operand)
            .and_then(aarch64::indirect_result_address_width)
    {
        result_width
            + operands[1..]
                .iter()
                .map(|operand| crate::operand_width(Architecture::Aarch64, operand))
                .sum::<usize>()
    } else {
        operands
            .iter()
            .map(|operand| crate::operand_width(Architecture::Aarch64, operand))
            .sum()
    }
}

pub fn syscall_sequence_width_no_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::syscall_sequence_width_from_operands(
            operands.iter().map(aarch64_call_operand),
            syscall_number,
        ),
        Architecture::X86_64 => x86_64::syscall_sequence_width(operands),
    }
}

pub fn syscall_sequence_width_with_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
    authoritative_plan: &omega_calling_conventions::CallPlan,
) -> usize {
    crate::encode_syscall_sequence_with_plan(
        architecture,
        operands,
        syscall_number,
        authoritative_plan,
    )
    .map(|bytes| bytes.len())
    .unwrap_or(0)
}

pub fn value_syscall_sequence_width_no_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
) -> usize {
    crate::encode_value_syscall_sequence_no_plan(architecture, operands, syscall_number)
        .map(|bytes| bytes.len())
        .unwrap_or(0)
}

pub fn value_syscall_sequence_width_with_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
    authoritative_plan: &omega_calling_conventions::CallPlan,
) -> usize {
    crate::encode_value_syscall_sequence_with_plan(
        architecture,
        operands,
        syscall_number,
        authoritative_plan,
    )
    .map(|bytes| bytes.len())
    .unwrap_or(0)
}

pub fn linux_timespec_syscall_sequence_width_no_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
) -> usize {
    crate::encode_linux_timespec_syscall_no_plan(architecture, operands, syscall_number)
        .map(|bytes| bytes.len())
        .unwrap_or(0)
}

pub fn linux_timespec_syscall_sequence_width_with_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
    authoritative_plan: &omega_calling_conventions::CallPlan,
) -> usize {
    crate::encode_linux_timespec_syscall_with_plan(
        architecture,
        operands,
        syscall_number,
        authoritative_plan,
    )
    .map(|bytes| bytes.len())
    .unwrap_or(0)
}

pub fn linux_timespec_argument_syscall_sequence_width_no_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
) -> usize {
    crate::encode_linux_timespec_argument_syscall_no_plan(architecture, operands, syscall_number)
        .map(|bytes| bytes.len())
        .unwrap_or(0)
}

pub fn linux_timespec_argument_syscall_sequence_width_with_plan<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
    authoritative_plan: &omega_calling_conventions::CallPlan,
) -> usize {
    crate::encode_linux_timespec_argument_syscall_with_plan(
        architecture,
        operands,
        syscall_number,
        authoritative_plan,
    )
    .map(|bytes| bytes.len())
    .unwrap_or(0)
}

pub fn constant_host_result_sequence_width<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
) -> usize {
    crate::encode_constant_host_result(architecture, operands)
        .map(|bytes| bytes.len())
        .unwrap_or(0)
}

pub fn function_enter_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::function_enter_width(),
        Architecture::X86_64 => x86_64::function_enter_width(),
    }
}

pub fn return_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::return_width(),
        Architecture::X86_64 => x86_64::return_width(),
    }
}

pub fn machine_halt_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::machine_halt_width(),
        Architecture::X86_64 => x86_64::machine_halt_width(),
    }
}

pub fn memory_fence_width(architecture: Architecture) -> Option<usize> {
    match architecture {
        Architecture::Aarch64 => None,
        Architecture::X86_64 => Some(x86_64::memory_fence_width()),
    }
}

pub fn interrupt_control_width(architecture: Architecture) -> Option<usize> {
    match architecture {
        Architecture::Aarch64 => None,
        Architecture::X86_64 => Some(x86_64::interrupt_control_width()),
    }
}

/// RFLAGS value operations are x86_64-only. Layout/encoding reject other
/// architectures before using these target-specific helpers.
pub fn flags_snapshot_width() -> usize {
    x86_64::flags_snapshot_width()
}

pub fn flags_restore_width(
    source: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> usize {
    x86_64::flags_restore_width(source, operand)
}

pub fn encode_flags_snapshot_bytes(
    dest_byte_offset: usize,
) -> Result<Vec<u8>, psi_diagnostics::Diagnostic> {
    x86_64::encode_flags_snapshot(dest_byte_offset)
}

pub fn encode_flags_restore_bytes(
    source: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, psi_diagnostics::Diagnostic> {
    x86_64::encode_flags_restore(source, operand)
}

pub fn msr_read_width(
    source: &impl RuntimeValueOperandSource,
    index: RuntimeValueOperandHandle,
) -> usize {
    x86_64::msr_read_width(source, index)
}

pub fn msr_write_width(
    source: &impl RuntimeValueOperandSource,
    index: RuntimeValueOperandHandle,
    value: RuntimeValueOperandHandle,
) -> usize {
    x86_64::msr_write_width(source, index, value)
}

pub fn encode_msr_read_bytes(
    source: &impl RuntimeValueOperandSource,
    index: RuntimeValueOperandHandle,
    dest_byte_offset: usize,
) -> Result<Vec<u8>, psi_diagnostics::Diagnostic> {
    x86_64::encode_msr_read(source, index, dest_byte_offset)
}

pub fn encode_msr_write_bytes(
    source: &impl RuntimeValueOperandSource,
    index: RuntimeValueOperandHandle,
    value: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, psi_diagnostics::Diagnostic> {
    x86_64::encode_msr_write(source, index, value)
}

pub fn control_register_read_width() -> usize {
    x86_64::control_register_read_width()
}

pub fn encode_control_register_read_bytes(
    register: psi_language_core::inline_assembly::AsmControlRegister,
    dest_byte_offset: usize,
) -> Result<Vec<u8>, psi_diagnostics::Diagnostic> {
    x86_64::encode_control_register_read(register, dest_byte_offset)
}

pub fn control_register_write_width(
    source: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> usize {
    x86_64::control_register_write_width(source, operand)
}

pub fn encode_control_register_write_bytes(
    source: &impl RuntimeValueOperandSource,
    register: psi_language_core::inline_assembly::AsmControlRegister,
    operand: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, psi_diagnostics::Diagnostic> {
    x86_64::encode_control_register_write(source, register, operand)
}

/// Port I/O is x86_64-only (ARM has no port space -- MMIO instead), so these
/// take no architecture: the layout/encoding sites reject a non-x86_64 target
/// before calling them.
pub fn port_write_width(
    source: &impl RuntimeValueOperandSource,
    port: RuntimeValueOperandHandle,
    value: RuntimeValueOperandHandle,
) -> usize {
    x86_64::port_write_width(source, port, value)
}

pub fn port_read_width(
    source: &impl RuntimeValueOperandSource,
    port: RuntimeValueOperandHandle,
) -> usize {
    x86_64::port_read_width(source, port)
}

pub fn encode_port_write_bytes(
    source: &impl RuntimeValueOperandSource,
    port: RuntimeValueOperandHandle,
    value: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, psi_diagnostics::Diagnostic> {
    x86_64::encode_port_write(source, port, value)
}

pub fn encode_port_read_bytes(
    source: &impl RuntimeValueOperandSource,
    port: RuntimeValueOperandHandle,
    dest_byte_offset: usize,
) -> Result<Vec<u8>, psi_diagnostics::Diagnostic> {
    x86_64::encode_port_read(source, port, dest_byte_offset)
}

pub fn return_register_integer_write_width(
    architecture: Architecture,
    register: omega_calling_conventions::MachineRegister,
    byte_size: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::return_register_integer_write_width(),
        Architecture::X86_64 => x86_64::return_register_integer_write_width(register, byte_size),
    }
}

pub fn runtime_storage_copy_to_return_register_width(
    architecture: Architecture,
    register: omega_calling_conventions::MachineRegister,
    byte_offset: usize,
    byte_size: usize,
) -> usize {
    debug_assert_eq!(register.architecture(), architecture);
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_to_return_register_width(byte_offset, byte_size)
        }
        Architecture::X86_64 => {
            x86_64::runtime_storage_copy_to_return_register_width(register, byte_offset, byte_size)
        }
    }
}

/// Width of the entry prologue's exact argument-register store.
pub fn entry_argument_register_write_width(
    architecture: Architecture,
    register: omega_calling_conventions::MachineRegister,
    byte_size: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::entry_argument_register_write_width(),
        Architecture::X86_64 => x86_64::entry_argument_register_write_width(register, byte_size),
    }
}

/// Width of one incoming stack-fragment copy into entry-frame storage.
pub fn entry_stack_argument_write_width(architecture: Architecture, byte_size: usize) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::entry_stack_argument_write_width(),
        Architecture::X86_64 => x86_64::entry_stack_argument_write_width(byte_size),
    }
}

pub fn entry_indirect_argument_write_width(
    architecture: Architecture,
    pointer: omega_calling_conventions::IndirectPointerLocation,
    byte_offset: usize,
    byte_size: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::entry_indirect_argument_write_width(pointer, byte_offset, byte_size)
        }
        Architecture::X86_64 => x86_64::entry_indirect_argument_write_width(pointer, byte_size),
    }
}

pub fn entry_indirect_argument_frame_base_offset(
    architecture: Architecture,
    pointer: omega_calling_conventions::IndirectPointerLocation,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::entry_indirect_argument_frame_base_offset(pointer),
        Architecture::X86_64 => x86_64::entry_indirect_argument_frame_base_offset(pointer),
    }
}

/// Width of the entry prologue's `args: &[u8]` descriptor write.
pub fn entry_arguments_slice_descriptor_write_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::entry_arguments_slice_descriptor_write_width(),
        Architecture::X86_64 => x86_64::entry_arguments_slice_descriptor_write_width(),
    }
}

pub fn dispatch_loop_enter_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::dispatch_loop_enter_width(),
        Architecture::X86_64 => x86_64::dispatch_loop_enter_width(),
    }
}

pub fn dispatch_case_enter_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::dispatch_case_enter_width(),
        Architecture::X86_64 => x86_64::dispatch_case_enter_width(),
    }
}

pub fn dispatch_state_write_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::dispatch_state_write_width(),
        Architecture::X86_64 => x86_64::dispatch_state_write_width(),
    }
}

pub fn dispatch_case_leave_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::dispatch_case_leave_width(),
        Architecture::X86_64 => x86_64::dispatch_case_leave_width(),
    }
}

pub fn dispatch_guard_compare_static_width(
    architecture: Architecture,
    byte_offset: usize,
    byte_size: usize,
    is_float: bool,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::dispatch_guard_compare_static_width(byte_offset, byte_size, is_float)
        }
        Architecture::X86_64 => x86_64::dispatch_guard_compare_static_width(is_float, byte_size),
    }
}

pub fn runtime_text_literal_compare_width(architecture: Architecture, literal: &[u8]) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_literal_compare_width(literal),
        Architecture::X86_64 => x86_64::runtime_text_literal_compare_width(literal),
    }
}

pub fn runtime_text_storage_compare_width(
    architecture: Architecture,
    source_offset: usize,
    literal_len: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            let _ = literal_len;
            aarch64::runtime_text_storage_compare_width(source_offset)
        }
        Architecture::X86_64 => x86_64::runtime_text_storage_compare_width_x86(literal_len),
    }
}

/// Byte offset within a `CompareRuntimeTextStorage` of the failure branch the
/// emitter must target with the compare-failure distance.
pub fn runtime_text_storage_compare_failure_branch_offset(
    architecture: Architecture,
    source_offset: usize,
    literal_len: usize,
) -> usize {
    match architecture {
        // AArch64's terminal MATCH branch is the last instruction of the op
        // (head 16 + loads + the 8-byte length immediate + 21 body
        // instructions precede it).
        Architecture::Aarch64 => {
            let _ = literal_len;
            aarch64::runtime_text_storage_compare_width(source_offset) - 4
        }
        Architecture::X86_64 => {
            x86_64::runtime_text_storage_compare_failure_branch_offset(literal_len)
        }
    }
}

/// Byte offset of the delimiter-failure branch (aarch64 uses `width - 4`; on
/// x86_64 both failure paths funnel through the same trampoline jmp).
pub fn runtime_text_storage_compare_delimiter_branch_offset(
    architecture: Architecture,
    source_offset: usize,
    literal_len: usize,
) -> usize {
    match architecture {
        // Unused by the aarch64 encoder (mismatch falls through like x86_64);
        // anchor at the MISMATCH trampoline so the computed distance stays
        // in-bounds.
        Architecture::Aarch64 => {
            aarch64::runtime_text_storage_compare_width(source_offset).saturating_sub(8)
        }
        Architecture::X86_64 => {
            x86_64::runtime_text_storage_compare_failure_branch_offset(literal_len)
        }
    }
}

pub fn runtime_text_literal_write_width(architecture: Architecture, literal: &[u8]) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_literal_write_width(literal),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_literal_segment_write_width(
    architecture: Architecture,
    literal: &[u8],
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_literal_segment_write_width(literal),
        Architecture::X86_64 => x86_64::runtime_text_literal_segment_write_width(literal),
    }
}

pub fn runtime_text_stored_suffix_append_width(
    architecture: Architecture,
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
    length_delta: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_stored_suffix_append_width(
            buffer_offset,
            source_offset,
            target_offset,
            length_delta,
        ),
        Architecture::X86_64 => x86_64::runtime_text_stored_suffix_append_width(),
    }
}

pub fn runtime_text_stored_place_append_width(
    architecture: Architecture,
    source_offset: usize,
    target_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_text_stored_place_append_width(source_offset, target_offset)
        }
        Architecture::X86_64 => x86_64::runtime_text_stored_place_append_width(),
    }
}

pub fn runtime_text_stored_place_append_to_runtime_frame_indexed_width(
    architecture: Architecture,
    source_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_text_stored_place_append_to_runtime_frame_indexed_width(
                source_offset,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => {
            let _ = (source_offset, element_byte_size, field_byte_offset);
            x86_64::runtime_text_stored_place_append_to_runtime_frame_indexed_width(index_byte_size)
        }
    }
}

pub fn runtime_text_stored_place_append_to_place_width(
    architecture: Architecture,
    source_offset: usize,
    target: &omega_target_operations::Place,
) -> usize {
    use crate::{WritePlaceShape, classify_write_place_shape};

    if architecture == Architecture::Aarch64
        && crate::classify_frame_base_double_indexed_text_assembly_shape(target).is_some()
    {
        return aarch64::runtime_text_stored_place_append_to_runtime_frame_base_double_indexed_width(
            source_offset,
        );
    }

    match (architecture, classify_write_place_shape(target)) {
        (architecture, WritePlaceShape::Direct { byte_offset }) => {
            runtime_text_stored_place_append_width(architecture, source_offset, byte_offset)
        }
        (
            architecture,
            WritePlaceShape::Pointee {
                pointer_byte_offset,
                field_byte_offset,
            },
        ) => runtime_text_stored_place_append_to_runtime_pointee_width(
            architecture,
            source_offset,
            pointer_byte_offset,
            field_byte_offset,
        ),
        (
            architecture,
            WritePlaceShape::FrameIndexed {
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                ..
            },
        ) => runtime_text_stored_place_append_to_runtime_frame_indexed_width(
            architecture,
            source_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        ),
        (
            Architecture::Aarch64,
            WritePlaceShape::FrameBaseIndexed {
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            },
        ) => aarch64::runtime_text_stored_place_append_to_runtime_frame_base_indexed_width(
            source_offset,
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        ),
        (Architecture::X86_64, _) => x86_64::encode_place_text_stored_append(target, source_offset)
            .map_or(0, |(bytes, _, _, _)| bytes.len()),
        _ => 0,
    }
}

pub fn runtime_text_stored_place_append_to_runtime_pointee_width(
    architecture: Architecture,
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_text_stored_place_append_to_runtime_pointee_width(
                source_offset,
                pointer_byte_offset,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => {
            let _ = (source_offset, pointer_byte_offset, field_byte_offset);
            x86_64::runtime_text_stored_place_append_to_runtime_pointee_width()
        }
    }
}

pub fn runtime_text_literal_append_width(
    architecture: Architecture,
    target_offset: usize,
    literal: &[u8],
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_literal_append_width(target_offset, literal),
        Architecture::X86_64 => x86_64::runtime_text_literal_append_width(literal),
    }
}

pub fn runtime_text_literal_append_to_runtime_pointee_width(
    architecture: Architecture,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_literal_append_to_runtime_pointee_width(
            pointer_byte_offset,
            field_byte_offset,
            literal,
        ),
        Architecture::X86_64 => {
            let _ = (pointer_byte_offset, field_byte_offset);
            x86_64::runtime_text_literal_append_to_runtime_pointee_width(literal)
        }
    }
}

pub fn runtime_text_literal_append_to_runtime_frame_indexed_width(
    architecture: Architecture,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_text_literal_append_to_runtime_frame_indexed_width(
                element_byte_size,
                field_byte_offset,
                literal,
            )
        }
        Architecture::X86_64 => x86_64::runtime_text_literal_append_to_runtime_frame_indexed_width(
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            literal,
        ),
    }
}

pub fn runtime_text_literal_append_to_place_width(
    architecture: Architecture,
    target: &omega_target_operations::Place,
    literal: &[u8],
) -> usize {
    use crate::{WritePlaceShape, classify_write_place_shape};

    if architecture == Architecture::Aarch64
        && crate::classify_frame_base_double_indexed_text_assembly_shape(target).is_some()
    {
        return aarch64::runtime_text_literal_append_to_runtime_frame_base_double_indexed_width(
            literal,
        );
    }

    match (architecture, classify_write_place_shape(target)) {
        (architecture, WritePlaceShape::Direct { byte_offset }) => {
            runtime_text_literal_append_width(architecture, byte_offset, literal)
        }
        (
            architecture,
            WritePlaceShape::Pointee {
                pointer_byte_offset,
                field_byte_offset,
            },
        ) => runtime_text_literal_append_to_runtime_pointee_width(
            architecture,
            pointer_byte_offset,
            field_byte_offset,
            literal,
        ),
        (
            architecture,
            WritePlaceShape::FrameIndexed {
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                ..
            },
        ) => runtime_text_literal_append_to_runtime_frame_indexed_width(
            architecture,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            literal,
        ),
        (
            Architecture::Aarch64,
            WritePlaceShape::FrameBaseIndexed {
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            },
        ) => aarch64::runtime_text_literal_append_to_runtime_frame_base_indexed_width(
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            literal,
        ),
        (Architecture::X86_64, _) => x86_64::encode_place_text_literal_append(target, literal)
            .map_or(0, |(bytes, _, _)| bytes.len()),
        _ => 0,
    }
}

pub fn runtime_text_buffer_materialize_width(
    architecture: Architecture,
    target_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_buffer_materialize_width(target_offset),
        Architecture::X86_64 => {
            let _ = target_offset;
            x86_64::runtime_text_buffer_materialize_width()
        }
    }
}

pub fn runtime_text_buffer_materialize_to_runtime_pointee_width(
    architecture: Architecture,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_buffer_materialize_to_runtime_pointee_width(
            pointer_byte_offset,
            field_byte_offset,
        ),
        Architecture::X86_64 => {
            let _ = (pointer_byte_offset, field_byte_offset);
            x86_64::runtime_text_buffer_materialize_to_runtime_pointee_width()
        }
    }
}

pub fn runtime_text_buffer_materialize_to_runtime_frame_indexed_width(
    architecture: Architecture,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_text_buffer_materialize_to_runtime_frame_indexed_width(
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => {
            let _ = (element_byte_size, field_byte_offset);
            x86_64::runtime_text_buffer_materialize_to_runtime_frame_indexed_width(index_byte_size)
        }
    }
}

pub fn runtime_text_buffer_materialize_to_place_width(
    architecture: Architecture,
    target: &omega_target_operations::Place,
) -> usize {
    use crate::{WritePlaceShape, classify_write_place_shape};

    if architecture == Architecture::Aarch64
        && crate::classify_frame_base_double_indexed_text_assembly_shape(target).is_some()
    {
        return aarch64::runtime_text_buffer_materialize_to_runtime_frame_base_double_indexed_width(
        );
    }

    match (architecture, classify_write_place_shape(target)) {
        (architecture, WritePlaceShape::Direct { byte_offset }) => {
            runtime_text_buffer_materialize_width(architecture, byte_offset)
        }
        (
            architecture,
            WritePlaceShape::Pointee {
                pointer_byte_offset,
                field_byte_offset,
            },
        ) => runtime_text_buffer_materialize_to_runtime_pointee_width(
            architecture,
            pointer_byte_offset,
            field_byte_offset,
        ),
        (
            architecture,
            WritePlaceShape::FrameIndexed {
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                ..
            },
        ) => runtime_text_buffer_materialize_to_runtime_frame_indexed_width(
            architecture,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        ),
        (
            Architecture::Aarch64,
            WritePlaceShape::FrameIndexedByRegion {
                index_region,
                element_byte_size,
                field_byte_offset,
                ..
            },
        ) => aarch64::runtime_text_buffer_materialize_to_runtime_frame_indexed_with_index_region_width(
            index_region,
            element_byte_size,
            field_byte_offset,
        ),
        (
            Architecture::Aarch64,
            WritePlaceShape::FrameBaseIndexed {
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            },
        ) => aarch64::runtime_text_buffer_materialize_to_runtime_frame_base_indexed_width(
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        ),
        (Architecture::X86_64, _) => x86_64::encode_place_text_buffer_materialize(target)
            .map_or(0, |(bytes, _, _)| bytes.len()),
        _ => 0,
    }
}

pub const fn runtime_text_frame_base_double_indexed_materialize_buffer_address_offset() -> usize {
    aarch64::runtime_text_frame_base_double_indexed_materialize_buffer_address_offset()
}

pub fn runtime_text_frame_base_indexed_literal_append_buffer_address_offset(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    aarch64::runtime_text_frame_base_indexed_literal_append_buffer_address_offset(
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )
}

pub const fn runtime_text_frame_base_double_indexed_literal_append_buffer_address_offset() -> usize
{
    aarch64::runtime_text_frame_base_double_indexed_literal_append_buffer_address_offset()
}

pub fn runtime_text_frame_base_indexed_stored_place_buffer_address_offset(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    aarch64::runtime_text_frame_base_indexed_stored_place_buffer_address_offset(
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )
}

pub const fn runtime_text_frame_base_double_indexed_stored_place_buffer_address_offset() -> usize {
    aarch64::runtime_text_frame_base_double_indexed_stored_place_buffer_address_offset()
}

pub fn runtime_text_frame_base_indexed_stored_place_source_address_offset(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    aarch64::runtime_text_frame_base_indexed_stored_place_source_address_offset(
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )
}

pub const fn runtime_text_frame_base_double_indexed_stored_place_source_address_offset() -> usize {
    aarch64::runtime_text_frame_base_double_indexed_stored_place_source_address_offset()
}

pub fn runtime_machine_integer_write_width(
    architecture: Architecture,
    byte_offset: usize,
    byte_size: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_machine_integer_write_width(byte_offset, byte_size)
        }
        Architecture::X86_64 => x86_64::runtime_machine_integer_write_width(byte_offset, byte_size),
    }
}

pub fn runtime_storage_bit_field_write_width(
    architecture: Architecture,
    base_byte_offset: usize,
    fragments: &[RuntimeBitFieldFragment],
) -> Result<usize, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_bit_field_write_width(base_byte_offset, fragments)
        }
        Architecture::X86_64 => {
            let _ = base_byte_offset;
            x86_64::runtime_storage_bit_field_write_width(fragments)
        }
    }
}

pub fn runtime_pointee_integer_write_width(
    architecture: Architecture,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_pointee_integer_write_width(
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
        ),
        Architecture::X86_64 => {
            let _ = pointer_byte_offset;
            x86_64::runtime_pointee_integer_write_width(field_byte_offset, byte_size)
        }
    }
}

/// Bytes inserted between the left and right operand evaluations of a binary
/// write so the left result survives the right evaluation. Zero on aarch64 (it
/// uses distinct result registers); on x86_64 it is a single `push r10`.
pub fn runtime_binary_right_operand_gap(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 0,
        Architecture::X86_64 => x86_64::BINARY_RIGHT_OPERAND_PUSH_WIDTH,
    }
}

/// Where a `MachineIndexed` operand's FRAME-base materialization sits relative
/// to the operand start, when its index is frame-resident: after the machine
/// adrp/add pair (aarch64) or the machine mov-imm64 + the rax accumulator move
/// (x86_64). Both encoders emit this prefix unconditionally in that order, so
/// the relocation walker can pin the second symbol here.
pub fn machine_indexed_operand_frame_index_base_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::MACHINE_INDEXED_OPERAND_FRAME_INDEX_BASE_OFFSET,
        Architecture::X86_64 => x86_64::MACHINE_INDEXED_OPERAND_FRAME_INDEX_BASE_OFFSET,
    }
}

/// Where a `FrameIndexed` operand's MACHINE-index base materialization sits:
/// after loading the pointee address from the frame-resident descriptor.
pub fn frame_indexed_operand_machine_index_base_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::FRAME_INDEXED_OPERAND_MACHINE_INDEX_BASE_OFFSET,
        Architecture::X86_64 => x86_64::FRAME_INDEXED_OPERAND_MACHINE_INDEX_BASE_OFFSET,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_storage_binary_write_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
    domain: psi_numerics::arithmetic::ArithmeticDomain,
    target_signed: bool,
) -> usize {
    match architecture {
        // aarch64 implements the saturating/trapping domains for add/sub/mul, so
        // the domain and target signedness change the emitted width (the clamp/
        // trap sequence). They must be threaded through for the relocation layout.
        Architecture::Aarch64 => aarch64::runtime_storage_binary_write_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            left,
            operator,
            right,
            is_float,
            domain,
            target_signed,
        ),
        Architecture::X86_64 => x86_64::runtime_storage_binary_write_width(
            runtime_value_operands,
            byte_size,
            left,
            operator,
            right,
            is_float,
            domain,
            target_signed,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_storage_convert_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    source: RuntimeValueOperandHandle,
    source_byte_size: usize,
    target_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
    target_signed: bool,
    trapping: bool,
    saturating: bool,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_convert_width(
            runtime_value_operands,
            target_offset,
            source,
            source_byte_size,
            target_byte_size,
            source_is_float,
            target_is_float,
            source_signed,
            target_signed,
            trapping,
            saturating,
        ),
        Architecture::X86_64 => {
            // The x86_64 converting store reaches the target through `r14`-relative
            // addressing with a fixed-width displacement, so its width is
            // offset-independent; ignore `target_offset` here.
            let _ = target_offset;
            x86_64::runtime_storage_convert_width(
                runtime_value_operands,
                source,
                source_byte_size,
                target_byte_size,
                source_is_float,
                target_is_float,
                source_signed,
                target_signed,
                trapping,
                saturating,
            )
        }
    }
}

pub fn runtime_atomic_load_to_storage_width(
    architecture: Architecture,
    source_offset: usize,
    byte_size: usize,
    result_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_atomic_load_to_storage_width(source_offset, byte_size, result_offset)
        }
        Architecture::X86_64 => {
            x86_64::runtime_atomic_load_to_storage_width(source_offset, byte_size, result_offset)
        }
    }
}

pub fn runtime_atomic_load_result_address_offset(
    architecture: Architecture,
    source_offset: usize,
    byte_size: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_atomic_load_result_address_offset(source_offset),
        Architecture::X86_64 => x86_64::runtime_atomic_load_result_address_offset(byte_size),
    }
}

pub fn runtime_atomic_store_from_operand_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_atomic_store_from_operand_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            value,
        ),
        Architecture::X86_64 => x86_64::runtime_atomic_store_from_operand_width(
            runtime_value_operands,
            byte_size,
            value,
        ),
    }
}

pub fn runtime_atomic_fetch_add_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    delta: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_atomic_fetch_add_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            delta,
        ),
        // x86 `lock xadd` carries the offset as a fixed disp32, so its width is
        // offset-independent.
        Architecture::X86_64 => x86_64::runtime_atomic_fetch_add_width(
            runtime_value_operands,
            byte_size,
            result_offset,
            delta,
        ),
    }
}

pub fn runtime_atomic_fetch_add_result_address_offset(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    delta: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_atomic_fetch_add_result_address_offset(
            runtime_value_operands,
            target_offset,
            delta,
        ),
        Architecture::X86_64 => x86_64::runtime_atomic_fetch_add_result_address_offset(
            runtime_value_operands,
            byte_size,
            delta,
        ),
    }
}

pub fn runtime_atomic_fetch_sub_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    delta: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_atomic_fetch_sub_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            delta,
        ),
        Architecture::X86_64 => x86_64::runtime_atomic_fetch_sub_width(
            runtime_value_operands,
            byte_size,
            result_offset,
            delta,
        ),
    }
}

pub fn runtime_atomic_fetch_sub_result_address_offset(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    delta: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_atomic_fetch_sub_result_address_offset(
            runtime_value_operands,
            target_offset,
            delta,
        ),
        Architecture::X86_64 => x86_64::runtime_atomic_fetch_sub_result_address_offset(
            runtime_value_operands,
            byte_size,
            delta,
        ),
    }
}

pub fn runtime_atomic_fetch_xor_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_atomic_fetch_xor_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            value,
        ),
        Architecture::X86_64 => x86_64::runtime_atomic_fetch_xor_width(
            runtime_value_operands,
            byte_size,
            result_offset,
            value,
        ),
    }
}

pub fn runtime_atomic_fetch_xor_result_address_offset(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_atomic_fetch_xor_result_address_offset(
            runtime_value_operands,
            target_offset,
            value,
        ),
        Architecture::X86_64 => x86_64::runtime_atomic_fetch_xor_result_address_offset(
            runtime_value_operands,
            byte_size,
            value,
        ),
    }
}

pub fn runtime_atomic_fetch_or_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_atomic_fetch_or_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            value,
        ),
        Architecture::X86_64 => x86_64::runtime_atomic_fetch_or_width(
            runtime_value_operands,
            byte_size,
            result_offset,
            value,
        ),
    }
}

pub fn runtime_atomic_fetch_or_result_address_offset(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_atomic_fetch_or_result_address_offset(
            runtime_value_operands,
            target_offset,
            value,
        ),
        Architecture::X86_64 => x86_64::runtime_atomic_fetch_or_result_address_offset(
            runtime_value_operands,
            byte_size,
            value,
        ),
    }
}

pub fn runtime_atomic_fetch_and_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_atomic_fetch_and_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            value,
        ),
        Architecture::X86_64 => x86_64::runtime_atomic_fetch_and_width(
            runtime_value_operands,
            byte_size,
            result_offset,
            value,
        ),
    }
}

pub fn runtime_atomic_fetch_and_result_address_offset(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    value: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_atomic_fetch_and_result_address_offset(
            runtime_value_operands,
            target_offset,
            value,
        ),
        Architecture::X86_64 => x86_64::runtime_atomic_fetch_and_result_address_offset(
            runtime_value_operands,
            byte_size,
            value,
        ),
    }
}

pub fn runtime_atomic_swap_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    new_value: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_atomic_swap_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            new_value,
        ),
        Architecture::X86_64 => x86_64::runtime_atomic_swap_width(
            runtime_value_operands,
            byte_size,
            result_offset,
            new_value,
        ),
    }
}

pub fn runtime_atomic_swap_result_address_offset(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    new_value: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_atomic_swap_result_address_offset(
            runtime_value_operands,
            target_offset,
            new_value,
        ),
        Architecture::X86_64 => x86_64::runtime_atomic_swap_result_address_offset(
            runtime_value_operands,
            byte_size,
            new_value,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_atomic_compare_exchange_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    expected: RuntimeValueOperandHandle,
    new_value: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_atomic_compare_exchange_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            expected,
            new_value,
        ),
        // x86 `lock cmpxchg` carries the offset as a fixed disp32, so its width is
        // offset-independent.
        Architecture::X86_64 => x86_64::runtime_atomic_compare_exchange_width(
            runtime_value_operands,
            byte_size,
            result_offset,
            expected,
            new_value,
        ),
    }
}

pub fn runtime_atomic_compare_exchange_result_address_offset(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    expected: RuntimeValueOperandHandle,
    new_value: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_atomic_compare_exchange_result_address_offset(
            runtime_value_operands,
            target_offset,
            expected,
            new_value,
        ),
        Architecture::X86_64 => x86_64::runtime_atomic_compare_exchange_result_address_offset(
            runtime_value_operands,
            byte_size,
            expected,
            new_value,
        ),
    }
}

pub fn runtime_pointee_binary_write_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_pointee_binary_write_width(
            runtime_value_operands,
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        ),
        Architecture::X86_64 => {
            let _ = (pointer_byte_offset, field_byte_offset);
            x86_64::runtime_pointee_binary_write_width(
                runtime_value_operands,
                byte_size,
                left,
                operator,
                right,
            )
        }
    }
}

pub fn runtime_pointee_operand_start_width(
    architecture: Architecture,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_pointee_operand_start_width(pointer_byte_offset, field_byte_offset)
        }
        Architecture::X86_64 => {
            let _ = pointer_byte_offset;
            let _ = field_byte_offset;
            x86_64::runtime_pointee_binary_operand_start_width()
        }
    }
}

pub fn runtime_value_compare_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_value_compare_width(runtime_value_operands, byte_size, left, right)
        }
        Architecture::X86_64 => {
            x86_64::runtime_value_compare_width(runtime_value_operands, byte_size, left, right)
        }
    }
}

/// Byte offset of the RIGHT descriptor base materialization inside a
/// `TextEquals` value operand (the left base sits at the operand start). The
/// relocation planner pins both region symbols through these offsets.
pub fn runtime_text_equals_right_base_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET,
        Architecture::X86_64 => x86_64::RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET,
    }
}

pub fn runtime_value_operand_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_value_operand_width(runtime_value_operands, operand)
        }
        Architecture::X86_64 => {
            x86_64::runtime_value_operand_width(runtime_value_operands, operand)
        }
    }
}

pub fn runtime_frame_indexed_integer_write_width(
    architecture: Architecture,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_frame_indexed_integer_write_width(
            element_byte_size,
            field_byte_offset,
            byte_size,
        ),
        Architecture::X86_64 => x86_64::runtime_frame_indexed_integer_write_width(
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        ),
    }
}

pub fn runtime_frame_base_indexed_integer_write_width(
    architecture: Architecture,
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_frame_base_indexed_integer_write_width(
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        ),
        Architecture::X86_64 => x86_64::runtime_frame_base_indexed_integer_write_width(
            base_byte_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        ),
    }
}

pub fn runtime_frame_base_indexed_operand_start_width(
    architecture: Architecture,
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_frame_base_indexed_operand_start_width(
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        ),
        Architecture::X86_64 => {
            x86_64::runtime_frame_base_indexed_binary_left_operand_offset(index_byte_size)
        }
    }
}

pub fn runtime_frame_base_indexed_machine_index_base_offset(
    architecture: Architecture,
    base_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_frame_base_indexed_machine_index_base_offset(base_byte_offset)
        }
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_frame_base_indexed_operand_start_width_with_index_region(
    architecture: Architecture,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_frame_base_indexed_operand_start_width_with_index_region(
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => {
            x86_64::runtime_frame_base_indexed_binary_left_operand_offset(index_byte_size)
        }
    }
}

pub fn runtime_frame_base_indexed_binary_write_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_frame_base_indexed_binary_write_width(
            runtime_value_operands,
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        ),
        Architecture::X86_64 => x86_64::runtime_frame_base_indexed_binary_write_width(
            runtime_value_operands,
            index_byte_size,
            byte_size,
            left,
            operator,
            right,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_machine_indexed_binary_write_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_machine_indexed_binary_write_width(
            runtime_value_operands,
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        ),
        Architecture::X86_64 => {
            let _ = (
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
            );
            x86_64::runtime_machine_indexed_binary_write_width(
                runtime_value_operands,
                index_region,
                index_byte_size,
                byte_size,
                left,
                operator,
                right,
            )
        }
    }
}

/// Byte offset of the left value operand within a frame-base-indexed binary
/// write (i.e. the length of the target-address-computation prefix).
pub fn runtime_frame_base_indexed_binary_left_operand_offset(
    architecture: Architecture,
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            // aarch64 reuses the integer-write prefix length (its store tail is a
            // separate trailing instruction, unlike x86_64's interleaved layout).
            aarch64::runtime_frame_base_indexed_integer_write_width(
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                0,
            )
        }
        Architecture::X86_64 => {
            let _ = (
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
            );
            x86_64::runtime_frame_base_indexed_binary_left_operand_offset(index_byte_size)
        }
    }
}

pub fn runtime_machine_indexed_integer_write_width(
    architecture: Architecture,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_machine_indexed_integer_write_width(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        ),
        Architecture::X86_64 => {
            let _ = (base_byte_offset, index_offset);
            x86_64::runtime_machine_indexed_integer_write_width(
                index_region,
                index_byte_size,
                element_byte_size,
                byte_size,
            )
        }
    }
}

pub fn runtime_machine_indexed_integer_runtime_frame_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_machine_indexed_integer_runtime_frame_address_offset(base_byte_offset)
        }
        Architecture::X86_64 => {
            let _ = base_byte_offset;
            x86_64::runtime_machine_indexed_integer_runtime_frame_address_offset()
        }
    }
}

pub fn runtime_frame_indexed_binary_write_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_frame_indexed_binary_write_width(
            runtime_value_operands,
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        ),
        Architecture::X86_64 => x86_64::runtime_frame_indexed_binary_write_width(
            runtime_value_operands,
            index_byte_size,
            byte_size,
            left,
            operator,
            right,
        ),
    }
}

/// Where a frame-INDEXED (slice-descriptor) binary write's LEFT operand
/// starts, relative to the instruction start -- the relocation walker pins
/// operand relocations here. The aarch64 encoder's operand block starts where
/// the frame-indexed INTEGER write's value would go (its historical
/// derivation); the x86_64 encoder has a fixed address-computation prefix.
pub fn runtime_frame_indexed_binary_left_operand_offset(
    architecture: Architecture,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_frame_indexed_integer_write_width(
            element_byte_size,
            field_byte_offset,
            0,
        ),
        Architecture::X86_64 => {
            x86_64::runtime_frame_indexed_binary_left_operand_offset(index_byte_size)
        }
    }
}

pub fn runtime_machine_bounded_buffer_source_append_width(
    architecture: Architecture,
    target_byte_offset: usize,
    source_byte_offset: usize,
    source_in_frame: bool,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_machine_bounded_buffer_source_append_width(
            target_byte_offset,
            source_byte_offset,
            source_in_frame,
        ),
        Architecture::X86_64 => {
            let _ = (target_byte_offset, source_byte_offset);
            x86_64::runtime_machine_bounded_buffer_source_append_width(source_in_frame)
        }
    }
}

pub fn runtime_machine_bounded_buffer_literal_append_width(
    architecture: Architecture,
    target_byte_offset: usize,
    literal: &[u8],
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_machine_bounded_buffer_literal_append_width(
            target_byte_offset,
            literal,
        ),
        Architecture::X86_64 => {
            let _ = target_byte_offset;
            x86_64::runtime_machine_bounded_buffer_literal_append_width(literal)
        }
    }
}

pub fn runtime_machine_indexed_string_runtime_frame_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_machine_indexed_string_runtime_frame_address_offset(base_byte_offset)
        }
        Architecture::X86_64 => {
            let _ = base_byte_offset;
            x86_64::MACHINE_INDEXED_STRING_FRAME_IMM_OFFSET
        }
    }
}

pub fn runtime_machine_indexed_string_data_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_machine_indexed_string_data_address_offset(
            base_byte_offset,
            element_byte_size,
            field_byte_offset,
        ),
        Architecture::X86_64 => {
            let _ = (base_byte_offset, element_byte_size, field_byte_offset);
            x86_64::MACHINE_INDEXED_STRING_DATA_IMM_OFFSET
        }
    }
}

pub fn runtime_machine_indexed_string_data_address_offset_with_index_region(
    architecture: Architecture,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_machine_indexed_string_data_address_offset_with_index_region(
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => unreachable!(
            "x86 string-descriptor relocations come from the generic place materializer"
        ),
    }
}

pub fn runtime_frame_base_indexed_string_data_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_frame_base_indexed_string_data_address_offset(
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        ),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_frame_base_indexed_string_data_address_offset_with_index_region(
    architecture: Architecture,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_frame_base_indexed_string_data_address_offset_with_index_region(
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => unreachable!(
            "x86 string-descriptor relocations come from the generic place materializer"
        ),
    }
}

pub fn runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
                base_byte_offset,
            )
        }
        Architecture::X86_64 => {
            // The frame-base mov sits after the 10-byte source mov in both the
            // single-value and chunked layouts (pre-+2; the record context
            // adds the imm64 offset).
            let _ = base_byte_offset;
            10
        }
    }
}

pub fn runtime_storage_copy_from_runtime_machine_indexed_target_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            let _ = byte_count;
            aarch64::runtime_storage_copy_from_runtime_machine_indexed_target_address_offset(
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => {
            // Pre-+2 offsets of the target-base mov (see the x86_64 width fn's
            // layout comment): the single-value layout has it after the
            // element load (+44 frame-index / +34 machine-index); the chunked
            // layout puts it right after `add r15,rax` (+37 / +27).
            let _ = (
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            );
            let frame_index =
                index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
            if matches!(byte_count, 1 | 4 | 8) {
                if frame_index { 44 } else { 34 }
            } else if frame_index {
                37
            } else {
                27
            }
        }
    }
}

pub fn runtime_storage_address_to_runtime_frame_target_frame_offset(
    architecture: Architecture,
    source_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_address_to_runtime_frame_target_frame_offset(source_offset)
        }
        Architecture::X86_64 => {
            let _ = source_offset;
            17
        }
    }
}

/// Relocation imm offset (pre-`+2`) of the second runtime-frame base load in the
/// frame-base-indexed address write, when the architecture loads the frame base
/// more than once. `None` when a single load is reused (aarch64).
pub fn runtime_frame_base_indexed_address_target_frame_offset(
    architecture: Architecture,
) -> Option<usize> {
    match architecture {
        Architecture::Aarch64 => None,
        Architecture::X86_64 => Some(x86_64::FRAME_BASE_INDEXED_ADDRESS_TARGET_FRAME_IMM_OFFSET),
    }
}

/// Width of the ByteRead stdin read (0 = the refuse-to-emit convention for
/// mechanisms the op can never bind to).
pub fn runtime_byte_read_width_no_plan(
    architecture: Architecture,
    binding: &HostBindingMechanism,
) -> usize {
    match architecture {
        Architecture::Aarch64 => match binding {
            HostBindingMechanism::Import { .. } => aarch64::runtime_byte_read_import_width(),
            HostBindingMechanism::Syscall { number, .. } => {
                aarch64::runtime_byte_read_syscall_width(*number)
            }
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. } => 0,
        },
        Architecture::X86_64 => match binding {
            HostBindingMechanism::Import { .. } => x86_64::runtime_byte_read_import_width(),
            HostBindingMechanism::Syscall { .. } => x86_64::runtime_byte_read_syscall_width(),
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. } => 0,
        },
    }
}

pub fn runtime_byte_read_width_with_plan(
    architecture: Architecture,
    binding: &HostBindingMechanism,
    target_offset: usize,
    payload_offset: usize,
    authoritative_plan: &omega_calling_conventions::CallPlan,
) -> usize {
    runtime_byte_read_width_with_plans(
        architecture,
        binding,
        target_offset,
        payload_offset,
        crate::RuntimeTextCallPlans::Direct(authoritative_plan),
    )
}

pub fn runtime_byte_read_width_with_plans(
    architecture: Architecture,
    binding: &HostBindingMechanism,
    target_offset: usize,
    payload_offset: usize,
    plans: crate::RuntimeTextCallPlans<'_>,
) -> usize {
    crate::encode_runtime_byte_read_with_plans(
        architecture,
        target_offset,
        payload_offset,
        binding,
        plans,
    )
    .map(|bytes| bytes.len())
    .unwrap_or(0)
}

/// Width of the stdout byte write; same conventions as the read.
pub fn runtime_byte_write_width_no_plan(
    architecture: Architecture,
    binding: &HostBindingMechanism,
    source_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => match binding {
            HostBindingMechanism::Import { .. } => {
                aarch64::runtime_byte_write_import_width(source_offset)
            }
            HostBindingMechanism::Syscall { number, .. } => {
                aarch64::runtime_byte_write_syscall_width(*number, source_offset)
            }
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. } => 0,
        },
        Architecture::X86_64 => match binding {
            HostBindingMechanism::Import { .. } => x86_64::runtime_byte_write_import_width(),
            HostBindingMechanism::Syscall { .. } => x86_64::runtime_byte_write_syscall_width(),
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. } => 0,
        },
    }
}

pub fn runtime_byte_write_width_with_plan(
    architecture: Architecture,
    binding: &HostBindingMechanism,
    source_offset: usize,
    authoritative_plan: &omega_calling_conventions::CallPlan,
) -> usize {
    runtime_byte_write_width_with_plans(
        architecture,
        binding,
        source_offset,
        crate::RuntimeTextCallPlans::Direct(authoritative_plan),
    )
}

pub fn runtime_byte_write_width_with_plans(
    architecture: Architecture,
    binding: &HostBindingMechanism,
    source_offset: usize,
    plans: crate::RuntimeTextCallPlans<'_>,
) -> usize {
    crate::encode_runtime_byte_write_with_plans(architecture, source_offset, binding, plans)
        .map(|bytes| bytes.len())
        .unwrap_or(0)
}

pub fn runtime_text_line_read_width_no_plan(
    architecture: Architecture,
    byte_capacity: usize,
    binding: &HostBindingMechanism,
    target: RuntimeTextReadTarget,
    target_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => match binding {
            HostBindingMechanism::Import { .. } => match target {
                RuntimeTextReadTarget::BoundedByteBuffer => {
                    aarch64::runtime_text_line_read_carrier_import_width(target_offset)
                }
                RuntimeTextReadTarget::FixedByteArray => {
                    aarch64::runtime_text_line_read_fixed_array_import_width(target_offset)
                }
                RuntimeTextReadTarget::StringDescriptor => {
                    aarch64::runtime_text_line_read_import_width(byte_capacity, target_offset)
                }
            },
            HostBindingMechanism::Syscall { number, .. } => match target {
                RuntimeTextReadTarget::BoundedByteBuffer => {
                    aarch64::runtime_text_line_read_carrier_syscall_width(*number, target_offset)
                }
                RuntimeTextReadTarget::FixedByteArray => {
                    aarch64::runtime_text_line_read_fixed_array_syscall_width(
                        *number,
                        target_offset,
                    )
                }
                RuntimeTextReadTarget::StringDescriptor => {
                    aarch64::runtime_text_line_read_syscall_width(
                        byte_capacity,
                        *number,
                        target_offset,
                    )
                }
            },
            // read_line is never vtable-bound; 0 = the refuse-to-emit convention.
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. } => 0,
        },
        Architecture::X86_64 => match binding {
            HostBindingMechanism::Import { .. } => match target {
                RuntimeTextReadTarget::BoundedByteBuffer => {
                    x86_64::runtime_text_line_read_carrier_width(byte_capacity)
                }
                RuntimeTextReadTarget::FixedByteArray => {
                    x86_64::runtime_text_line_read_fixed_array_width(byte_capacity)
                }
                RuntimeTextReadTarget::StringDescriptor => {
                    x86_64::runtime_text_line_read_width(byte_capacity)
                }
            },
            HostBindingMechanism::Syscall { .. } => match target {
                RuntimeTextReadTarget::BoundedByteBuffer => {
                    x86_64::runtime_text_line_read_syscall_carrier_width()
                }
                RuntimeTextReadTarget::FixedByteArray => {
                    x86_64::runtime_text_line_read_syscall_fixed_array_width()
                }
                RuntimeTextReadTarget::StringDescriptor => {
                    x86_64::runtime_text_line_read_syscall_width()
                }
            },
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. } => 0,
        },
    }
}

pub fn runtime_text_line_read_width_with_plan(
    architecture: Architecture,
    byte_capacity: usize,
    binding: &HostBindingMechanism,
    target: RuntimeTextReadTarget,
    target_offset: usize,
    authoritative_plan: &omega_calling_conventions::CallPlan,
) -> usize {
    runtime_text_line_read_width_with_plans(
        architecture,
        byte_capacity,
        binding,
        target,
        target_offset,
        crate::RuntimeTextCallPlans::Direct(authoritative_plan),
    )
}

pub fn runtime_text_line_read_width_with_plans(
    architecture: Architecture,
    byte_capacity: usize,
    binding: &HostBindingMechanism,
    target: RuntimeTextReadTarget,
    target_offset: usize,
    plans: crate::RuntimeTextCallPlans<'_>,
) -> usize {
    crate::encode_runtime_text_line_read_with_plans(
        architecture,
        target_offset,
        byte_capacity,
        binding,
        target,
        plans,
    )
    .map(|bytes| bytes.len())
    .unwrap_or(0)
}

pub fn runtime_text_line_read_target_address_offset(
    architecture: Architecture,
    binding: &HostBindingMechanism,
) -> usize {
    match architecture {
        Architecture::Aarch64 => match binding {
            HostBindingMechanism::Import { .. } => {
                aarch64::runtime_text_line_read_import_target_address_offset()
            }
            HostBindingMechanism::Syscall { number, .. } => {
                aarch64::runtime_text_line_read_syscall_target_address_offset(*number)
            }
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. } => 0,
        },
        Architecture::X86_64 => match binding {
            HostBindingMechanism::Import { .. } => {
                x86_64::runtime_text_line_read_target_imm_offset()
            }
            HostBindingMechanism::Syscall { .. } => {
                x86_64::runtime_text_line_read_syscall_target_imm_offset()
            }
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. } => 0,
        },
    }
}

/// Offset of the import call fixup inside the ByteRead stdin read (aarch64:
/// the `bl` instruction; x86_64: the ReadFile rel32 bytes).
pub fn runtime_byte_read_import_call_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_byte_read_import_call_offset(),
        Architecture::X86_64 => x86_64::runtime_byte_read_read_file_offset(),
    }
}

/// Offset of the import call fixup inside the stdout byte write (aarch64: the
/// `bl`; x86_64: the WriteFile rel32 bytes).
pub fn runtime_byte_write_import_call_offset(
    architecture: Architecture,
    source_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_byte_write_import_call_offset(source_offset),
        Architecture::X86_64 => x86_64::runtime_byte_write_write_file_offset(),
    }
}

/// x86_64-only: the GetStdHandle rel32 fixup inside the byte ops (aarch64 has
/// no separate handle call; callers gate on architecture).
pub fn runtime_byte_read_get_std_handle_offset() -> usize {
    x86_64::runtime_byte_read_get_std_handle_offset()
}

pub fn runtime_byte_write_get_std_handle_offset() -> usize {
    x86_64::runtime_byte_write_get_std_handle_offset()
}

pub fn runtime_text_line_read_import_call_offset(
    architecture: Architecture,
    target: RuntimeTextReadTarget,
    target_offset: usize,
) -> usize {
    match architecture {
        // The carrier prologue adds a fixed bytes-base `add` after the region
        // materialization, shifting the `bl` by 4 (28 -> 32).
        Architecture::Aarch64 => match target {
            RuntimeTextReadTarget::BoundedByteBuffer => {
                aarch64::runtime_text_line_read_carrier_import_call_offset(target_offset)
            }
            RuntimeTextReadTarget::FixedByteArray => {
                aarch64::runtime_text_line_read_fixed_array_import_call_offset(target_offset)
            }
            RuntimeTextReadTarget::StringDescriptor => {
                aarch64::runtime_text_line_read_import_call_offset()
            }
        },
        // x86_64 ReadFile call rel32 displacement (shifted for the carrier prologue).
        Architecture::X86_64 => match target {
            RuntimeTextReadTarget::BoundedByteBuffer => {
                x86_64::runtime_text_line_read_carrier_read_file_call_offset()
            }
            RuntimeTextReadTarget::FixedByteArray => {
                x86_64::runtime_text_line_read_fixed_array_read_file_call_offset()
            }
            RuntimeTextReadTarget::StringDescriptor => {
                x86_64::runtime_text_line_read_read_file_call_offset()
            }
        },
    }
}

/// x86_64-only: rel32 displacement offset of the GetStdHandle call within the
/// runtime line-read instruction (aarch64 has no separate handle call).
pub fn runtime_text_line_read_get_std_handle_call_offset(
    architecture: Architecture,
    target: RuntimeTextReadTarget,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 0,
        Architecture::X86_64 => match target {
            RuntimeTextReadTarget::BoundedByteBuffer => {
                x86_64::runtime_text_line_read_carrier_get_std_handle_call_offset()
            }
            RuntimeTextReadTarget::FixedByteArray => {
                x86_64::runtime_text_line_read_fixed_array_get_std_handle_call_offset()
            }
            RuntimeTextReadTarget::StringDescriptor => {
                x86_64::runtime_text_line_read_get_std_handle_call_offset()
            }
        },
    }
}

/// The `CopyPlaces` width IS the encoder's output length -- one source of
/// truth, no hand-maintained width math to move in lockstep. Copies are tens
/// of bytes; the extra encode at layout time is noise.
pub fn copy_places_width(
    architecture: Architecture,
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
    byte_count: usize,
) -> Result<usize, psi_diagnostics::Diagnostic> {
    crate::encoding::encode_copy_places(architecture, source, target, byte_count)
        .map(|bytes| bytes.len())
}

/// aarch64 SOURCE-adrp offset inside the store (for the relocation planner).
pub fn runtime_storage_copy_to_runtime_machine_indexed_source_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_to_runtime_machine_indexed_source_address_offset(
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => {
            let _ = (
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            );
            0
        }
    }
}

/// The machine-base relocation offset inside the FRAME-SOURCE variant of the
/// write half (the second `mov r15,imm64`). x86_64 only.
pub fn runtime_storage_copy_to_runtime_machine_indexed_frame_source_machine_base_offset(
    architecture: Architecture,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 0,
        Architecture::X86_64 => {
            x86_64::runtime_storage_copy_to_runtime_machine_indexed_frame_source_machine_base_offset(
            )
        }
    }
}

pub fn runtime_storage_copy_machine_indexed_to_machine_indexed_width(
    architecture: Architecture,
    source_index_region: omega_target_operations::RuntimeStorageRegion,
    target_index_region: omega_target_operations::RuntimeStorageRegion,
    byte_count: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_machine_indexed_to_machine_indexed_width(
                source_index_region,
                target_index_region,
                byte_count,
            )
        }
        Architecture::X86_64 => {
            let _ = byte_count;
            x86_64::runtime_storage_copy_machine_indexed_to_machine_indexed_width(
                source_index_region,
                target_index_region,
            )
        }
    }
}

/// Offset of the second relocated machine base inside the dual-indexed copy
/// (`arr[i] = arr[j]`), arch-dispatched for the relocation planner.
pub fn runtime_storage_copy_machine_indexed_to_machine_indexed_second_base_offset(
    architecture: Architecture,
    source_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_machine_indexed_to_machine_indexed_second_base_offset(
                source_index_region,
            )
        }
        // The read part (10+7+7+3+7 = 34) precedes it; a FRAME-resident source
        // index inserts its frame-base `mov r10,imm64` (+10).
        Architecture::X86_64 => {
            if source_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                44
            } else {
                34
            }
        }
    }
}

/// Offset of a FRAME-resident index's relocated base inside the dual-indexed
/// copy, arch-dispatched for the relocation planner.
pub fn runtime_storage_copy_machine_indexed_frame_index_offset(
    architecture: Architecture,
    source_index_region: omega_target_operations::RuntimeStorageRegion,
    target_side: bool,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_copy_machine_indexed_frame_index_offset(
            source_index_region,
            target_side,
        ),
        Architecture::X86_64 => {
            if target_side {
                runtime_storage_copy_machine_indexed_to_machine_indexed_second_base_offset(
                    architecture,
                    source_index_region,
                ) + 10
            } else {
                10
            }
        }
    }
}

pub fn aarch64_runtime_storage_copy_machine_double_indexed_pair_second_base_offset(
    source_outer_index_region: omega_target_operations::RuntimeStorageRegion,
    source_inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    aarch64::runtime_storage_copy_machine_double_indexed_pair_second_base_offset(
        source_outer_index_region,
        source_inner_index_region,
    )
}

pub fn aarch64_runtime_storage_copy_machine_double_indexed_pair_target_frame_base_offset(
    source_outer_index_region: omega_target_operations::RuntimeStorageRegion,
    source_inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    aarch64::runtime_storage_copy_machine_double_indexed_pair_target_frame_base_offset(
        source_outer_index_region,
        source_inner_index_region,
    )
}

pub fn runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage_width(
    architecture: Architecture,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage_width(
                outer_index_region,
                inner_index_region,
            )
        }
        Architecture::X86_64 => {
            x86_64::runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage_width(
                outer_index_region,
                inner_index_region,
            )
        }
    }
}

/// Frame-base relocation start inside the double-indexed read (pre-`+2`;
/// present only when an index is frame-resident). x86_64 only.
/// Write rung 1c (x86-only; aarch64 keeps the shared-base layout): the
/// canonicalized double-indexed integer WRITE's per-index frame bases.
pub fn runtime_machine_double_indexed_integer_write_outer_frame_offset() -> usize {
    x86_64::runtime_machine_double_indexed_integer_write_outer_frame_offset()
}

pub fn runtime_machine_double_indexed_integer_write_inner_frame_offset(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    x86_64::runtime_machine_double_indexed_integer_write_inner_frame_offset(outer_index_region)
}

pub fn runtime_storage_copy_from_runtime_machine_double_indexed_frame_base_offset(
    architecture: Architecture,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_machine_double_indexed_frame_base_offset(),
        Architecture::X86_64 => {
            x86_64::runtime_storage_copy_from_runtime_machine_double_indexed_frame_base_offset()
        }
    }
}

pub fn runtime_machine_double_indexed_address_frame_base_offset(
    architecture: Architecture,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    assert_eq!(
        architecture,
        Architecture::Aarch64,
        "the x86 place materializer reports its address sites directly"
    );
    aarch64::runtime_machine_double_indexed_address_frame_base_offset(
        outer_index_region,
        inner_index_region,
    )
}

pub fn runtime_machine_double_indexed_string_data_address_offset(
    architecture: Architecture,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_machine_double_indexed_string_data_address_offset(
                outer_index_region,
                inner_index_region,
            )
        }
        Architecture::X86_64 => unreachable!(
            "x86 string-descriptor relocations come from the generic place materializer"
        ),
    }
}

pub fn runtime_frame_base_double_indexed_string_data_address_offset(
    architecture: Architecture,
) -> usize {
    assert_eq!(
        architecture,
        Architecture::Aarch64,
        "x86 string-descriptor relocations come from the generic place materializer"
    );
    aarch64::runtime_frame_base_double_indexed_string_data_address_offset()
}

/// Target-region relocation start (the write-half `mov r15,imm64`, pre-`+2`)
/// inside the double-indexed read. x86_64 only.
pub fn runtime_storage_copy_from_runtime_machine_double_indexed_target_base_offset(
    architecture: Architecture,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_from_runtime_machine_double_indexed_target_base_offset(
                outer_index_region,
                inner_index_region,
            )
        }
        Architecture::X86_64 => {
            x86_64::runtime_storage_copy_from_runtime_machine_double_indexed_target_base_offset(
                outer_index_region,
                inner_index_region,
            )
        }
    }
}

pub fn runtime_storage_copy_to_runtime_machine_double_indexed_from_runtime_storage_width(
    architecture: Architecture,
    source_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_to_runtime_machine_double_indexed_from_runtime_storage_width(
                source_region,
                outer_index_region,
                inner_index_region,
            )
        }
        Architecture::X86_64 => {
            x86_64::runtime_storage_copy_to_runtime_machine_double_indexed_from_runtime_storage_width(
                source_region,
                outer_index_region,
                inner_index_region,
            )
        }
    }
}

pub fn runtime_machine_double_indexed_integer_write_width(
    architecture: Architecture,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_byte_size: usize,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_byte_size: usize,
    value: i64,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_machine_double_indexed_integer_write_width(
            outer_index_region,
            inner_index_region,
            value,
        ),
        Architecture::X86_64 => x86_64::runtime_machine_double_indexed_integer_write_width(
            outer_index_region,
            outer_index_byte_size,
            inner_index_region,
            inner_index_byte_size,
        ),
    }
}

pub fn runtime_machine_double_indexed_binary_write_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_byte_size: usize,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_byte_size: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: omega_target_operations::StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_machine_double_indexed_binary_write_width(
            runtime_value_operands,
            outer_index_region,
            inner_index_region,
            byte_size,
            left,
            operator,
            right,
        ),
        Architecture::X86_64 => x86_64::runtime_machine_double_indexed_binary_write_width(
            runtime_value_operands,
            outer_index_region,
            outer_index_byte_size,
            inner_index_region,
            inner_index_byte_size,
            byte_size,
            left,
            operator,
            right,
        ),
    }
}

pub fn runtime_machine_double_indexed_binary_left_operand_offset(
    architecture: Architecture,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_byte_size: usize,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_byte_size: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_machine_double_indexed_binary_left_operand_offset(
                outer_index_region,
                inner_index_region,
            )
        }
        Architecture::X86_64 => x86_64::runtime_machine_double_indexed_binary_left_operand_offset(
            outer_index_region,
            outer_index_byte_size,
            inner_index_region,
            inner_index_byte_size,
        ),
    }
}

pub fn runtime_frame_base_double_indexed_binary_left_operand_offset() -> usize {
    aarch64::runtime_frame_base_double_indexed_binary_left_operand_offset()
}

pub fn runtime_frame_base_double_indexed_convert_operand_offset() -> usize {
    aarch64::runtime_frame_base_double_indexed_convert_operand_offset()
}

pub fn runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage_width(
    architecture: Architecture,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage_width(
                outer_index_region,
                inner_index_region,
                target_offset,
                byte_count,
            )
        }
        Architecture::X86_64 => {
            let _ = (outer_index_region, inner_index_region, target_offset, byte_count);
            x86_64::runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage_width()
        }
    }
}

/// Target-region relocation start (pre-`+2`). x86_64 only.
pub fn runtime_storage_copy_from_runtime_frame_base_double_indexed_target_base_offset(
    architecture: Architecture,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_from_runtime_frame_base_double_indexed_target_base_offset(
                outer_index_region,
                inner_index_region,
            )
        }
        Architecture::X86_64 => {
            let _ = (outer_index_region, inner_index_region);
            x86_64::runtime_storage_copy_from_runtime_frame_base_double_indexed_target_base_offset()
        }
    }
}

pub fn runtime_storage_copy_to_runtime_frame_base_double_indexed_source_base_offset() -> usize {
    aarch64::runtime_storage_copy_to_runtime_frame_base_double_indexed_source_base_offset()
}

pub fn append_wire_literal_byte_width(
    architecture: Architecture,
    out_offset: usize,
    written_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::append_wire_literal_byte_width(out_offset, written_offset)
        }
        Architecture::X86_64 => x86_64::append_wire_literal_byte_width(out_offset, written_offset),
    }
}

pub fn append_wire_scalar_varint_width(
    architecture: Architecture,
    source_offset: usize,
    byte_size: usize,
    zigzag: bool,
    out_offset: usize,
    written_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::append_wire_scalar_varint_width(
            source_offset,
            byte_size,
            zigzag,
            out_offset,
            written_offset,
        ),
        Architecture::X86_64 => x86_64::append_wire_scalar_varint_width(
            source_offset,
            byte_size,
            zigzag,
            out_offset,
            written_offset,
        ),
    }
}

pub fn append_wire_text_bytes_width(
    architecture: Architecture,
    source_offset: usize,
    out_offset: usize,
    out_length: usize,
    written_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::append_wire_text_bytes_width(
            source_offset,
            out_offset,
            out_length,
            written_offset,
        ),
        Architecture::X86_64 => x86_64::append_wire_text_bytes_width(
            source_offset,
            out_offset,
            out_length,
            written_offset,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn append_wire_scalar_slice_width(
    architecture: Architecture,
    source_offset: usize,
    element_byte_size: usize,
    zigzag: bool,
    out_offset: usize,
    out_length: usize,
    written_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::append_wire_scalar_slice_width(
            source_offset,
            element_byte_size,
            zigzag,
            out_offset,
            out_length,
            written_offset,
        ),
        Architecture::X86_64 => x86_64::append_wire_scalar_slice_width(
            source_offset,
            element_byte_size,
            zigzag,
            out_offset,
            out_length,
            written_offset,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn append_wire_repeated_scalar_varint_width(
    architecture: Architecture,
    source_offset: usize,
    byte_size: usize,
    zigzag: bool,
    index: u64,
    count_offset: usize,
    out_offset: usize,
    written_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::append_wire_repeated_scalar_varint_width(
            source_offset,
            byte_size,
            zigzag,
            index,
            count_offset,
            out_offset,
            written_offset,
        ),
        Architecture::X86_64 => x86_64::append_wire_repeated_scalar_varint_width(
            source_offset,
            byte_size,
            zigzag,
            index,
            count_offset,
            out_offset,
            written_offset,
        ),
    }
}

/// Byte offset of the FixedVec LENGTH page address materialization inside the
/// repeated append (relocated to the carrier's region symbol).
pub fn wire_append_repeated_count_page_offset(
    architecture: Architecture,
    out_offset: usize,
    written_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::wire_append_repeated_count_page_offset(out_offset, written_offset)
        }
        Architecture::X86_64 => {
            x86_64::wire_append_repeated_count_page_offset(out_offset, written_offset)
        }
    }
}

/// Byte offset of the SOURCE page address materialization inside the repeated
/// append (relocated to the element's region symbol).
pub fn wire_append_repeated_source_page_offset(
    architecture: Architecture,
    out_offset: usize,
    written_offset: usize,
    count_offset: usize,
    index: u64,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::wire_append_repeated_source_page_offset(
            out_offset,
            written_offset,
            count_offset,
            index,
        ),
        Architecture::X86_64 => x86_64::wire_append_repeated_source_page_offset(
            out_offset,
            written_offset,
            count_offset,
            index,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn read_wire_repeated_scalar_varint_width(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    end_offset: usize,
    count_offset: usize,
    target_offset: usize,
    byte_size: usize,
    zigzag: bool,
    range: Option<psi_language_semantics::wire::WireScalarRange>,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::read_wire_repeated_scalar_varint_width(
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
        ),
        Architecture::X86_64 => x86_64::read_wire_repeated_scalar_varint_width(
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
        ),
    }
}

/// Byte offset of the TARGET page address materialization inside the repeated
/// read (relocated to the element slot's region symbol).
pub fn wire_decode_repeated_target_page_offset(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    end_offset: usize,
    zigzag: bool,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::wire_decode_repeated_target_page_offset(
            buffer_offset,
            buffer_length,
            read_offset,
            end_offset,
            zigzag,
        ),
        Architecture::X86_64 => x86_64::wire_decode_repeated_target_page_offset(
            buffer_offset,
            buffer_length,
            read_offset,
            end_offset,
            zigzag,
        ),
    }
}

/// Byte offset of the FixedVec LENGTH page address materialization inside the
/// repeated read (relocated to the carrier's region symbol).
#[allow(clippy::too_many_arguments)]
pub fn wire_decode_repeated_count_page_offset(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    end_offset: usize,
    target_offset: usize,
    byte_size: usize,
    zigzag: bool,
    range: Option<psi_language_semantics::wire::WireScalarRange>,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::wire_decode_repeated_count_page_offset(
            buffer_offset,
            buffer_length,
            read_offset,
            end_offset,
            target_offset,
            byte_size,
            zigzag,
            range,
        ),
        Architecture::X86_64 => x86_64::wire_decode_repeated_count_page_offset(
            buffer_offset,
            buffer_length,
            read_offset,
            end_offset,
            target_offset,
            byte_size,
            zigzag,
            range,
        ),
    }
}

/// Byte offset of the WRITTEN page address materialization inside both wire
/// appends (relocated to the written slot's region symbol).
pub fn wire_append_written_page_offset(architecture: Architecture, out_offset: usize) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::wire_append_written_page_offset(out_offset),
        Architecture::X86_64 => x86_64::wire_append_written_page_offset(out_offset),
    }
}

/// Byte offset of the SOURCE page address materialization inside the varint
/// append (relocated to the scalar's region symbol).
pub fn wire_append_varint_source_page_offset(
    architecture: Architecture,
    out_offset: usize,
    written_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::wire_append_varint_source_page_offset(out_offset, written_offset)
        }
        Architecture::X86_64 => {
            x86_64::wire_append_varint_source_page_offset(out_offset, written_offset)
        }
    }
}

pub fn read_wire_expected_byte_width(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::read_wire_expected_byte_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
        ),
        Architecture::X86_64 => x86_64::read_wire_expected_byte_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn read_wire_scalar_varint_width(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    target_offset: usize,
    byte_size: usize,
    zigzag: bool,
    range: Option<psi_language_semantics::wire::WireScalarRange>,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::read_wire_scalar_varint_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_offset,
            byte_size,
            zigzag,
            range,
        ),
        Architecture::X86_64 => x86_64::read_wire_scalar_varint_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_offset,
            byte_size,
            zigzag,
            range,
        ),
    }
}

pub fn read_wire_byte_slice_width(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    target_offset: usize,
    predicate_mask: u8,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::read_wire_byte_slice_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_offset,
            predicate_mask,
        ),
        Architecture::X86_64 => x86_64::read_wire_byte_slice_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_offset,
            predicate_mask,
        ),
    }
}

/// Byte offset of the READ (cursor) page address materialization inside both
/// wire decodes (relocated to the read slot's region symbol).
pub fn wire_decode_read_page_offset(architecture: Architecture, buffer_offset: usize) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::wire_decode_read_page_offset(buffer_offset),
        Architecture::X86_64 => x86_64::wire_decode_read_page_offset(buffer_offset),
    }
}

/// Byte offset of the OK (sticky flag) page address materialization inside
/// both wire decodes (relocated to the ok slot's region symbol).
pub fn wire_decode_ok_page_offset(
    architecture: Architecture,
    buffer_offset: usize,
    read_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::wire_decode_ok_page_offset(buffer_offset, read_offset),
        Architecture::X86_64 => x86_64::wire_decode_ok_page_offset(buffer_offset, read_offset),
    }
}

/// Byte offset of the TARGET page address materialization inside the varint
/// decode (relocated to the field's region symbol).
pub fn wire_decode_varint_target_page_offset(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    zigzag: bool,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::wire_decode_varint_target_page_offset(
            buffer_offset,
            buffer_length,
            read_offset,
            zigzag,
        ),
        Architecture::X86_64 => x86_64::wire_decode_varint_target_page_offset(
            buffer_offset,
            buffer_length,
            read_offset,
            zigzag,
        ),
    }
}

pub fn wire_decode_byte_slice_target_page_offset(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    predicate_mask: u8,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::wire_decode_byte_slice_target_page_offset(
            buffer_offset,
            buffer_length,
            read_offset,
            predicate_mask,
        ),
        Architecture::X86_64 => x86_64::wire_decode_byte_slice_target_page_offset(
            buffer_offset,
            buffer_length,
            read_offset,
            predicate_mask,
        ),
    }
}

pub fn read_wire_nested_open_width(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    end_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::read_wire_nested_open_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            end_offset,
        ),
        Architecture::X86_64 => x86_64::read_wire_nested_open_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            end_offset,
        ),
    }
}

pub fn read_wire_nested_close_width(
    architecture: Architecture,
    buffer_offset: usize,
    read_offset: usize,
    ok_offset: usize,
    end_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::read_wire_nested_close_width(buffer_offset, read_offset, ok_offset, end_offset)
        }
        Architecture::X86_64 => {
            x86_64::read_wire_nested_close_width(buffer_offset, read_offset, ok_offset, end_offset)
        }
    }
}

/// Byte offset of the END-slot page address materialization inside both
/// nested decodes (relocated to the end slot's region symbol).
pub fn wire_decode_nested_end_page_offset(
    architecture: Architecture,
    buffer_offset: usize,
    read_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::wire_decode_nested_end_page_offset(buffer_offset, read_offset)
        }
        Architecture::X86_64 => {
            x86_64::wire_decode_nested_end_page_offset(buffer_offset, read_offset)
        }
    }
}
