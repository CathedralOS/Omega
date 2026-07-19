use crate::aarch64_call_operand;
use omega_calling_conventions::{
    CallSignature, CallingPolicy, EntryControl, HostOperationKey, ValueLocation, ValueShape,
    evaluate_call_plan,
};
use omega_core::diagnostics::Diagnostic;
use omega_isa_aarch64::aarch64;
use omega_isa_x86_64 as x86_64;
use omega_target::Architecture;
use omega_target_operations::InstructionOperandLike;

/// A VtableSlot call (provides-sourced, per-object dispatch). x86_64 only; an
/// aarch64 vtable call awaits its stub.
pub fn encode_vtable_call_sequence<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    index: i64,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => Err(Diagnostic::error(
            "AArch64 vtable-slot dispatch is not implemented (x86_64 only)",
        )),
        Architecture::X86_64 => x86_64::encode_win64_vtable_call(operands, index),
    }
}

/// The FIELD-MODEL flavor (extern brief SS12.1): the byte offset came from
/// the vtable struct's layout via the backend's vtable-field pass. When
/// `result_present`, operand 0 is the RESULT place and the store tail runs.
pub fn encode_vtable_call_sequence_at_offset<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    byte_offset: usize,
    result_present: bool,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => Err(Diagnostic::error(
            "AArch64 vtable-field dispatch is not implemented (x86_64 only)",
        )),
        Architecture::X86_64 => x86_64::encode_win64_vtable_call_at_offset(
            operands,
            i64::try_from(byte_offset)
                .map_err(|_| Diagnostic::error("vtable field offset overflows i64"))?,
            result_present,
        ),
    }
}

/// A SERVICE-TABLE function call: field-model dispatch where the table
/// pointer is dispatch-only, never a wire argument (EFI table services take
/// no This; protocol/COM methods do).
pub fn encode_table_function_call_sequence<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    byte_offset: usize,
    result_present: bool,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => Err(Diagnostic::error(
            "AArch64 table-function dispatch is not implemented (x86_64 only)",
        )),
        Architecture::X86_64 => x86_64::encode_win64_table_function_call(
            operands,
            i64::try_from(byte_offset)
                .map_err(|_| Diagnostic::error("service table field offset overflows i64"))?,
            result_present,
        ),
    }
}

pub fn encode_host_call_sequence<T: InstructionOperandLike>(
    architecture: Architecture,
    operation_key: HostOperationKey,
    operands: &[T],
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        // Deref-result ops (errno) must be checked before the plain
        // value-returning arm: they share `returns_value()` but insert an extra
        // `ldr` to deref the returned pointer.
        Architecture::Aarch64 if operation_key.dereferences_result() => {
            aarch64::encode_host_call_sequence_value_returning_deref_from_operands(
                operands.iter().map(aarch64_call_operand),
            )
        }
        // Stack-mode ops (`open_create`) also share `returns_value()` but bracket
        // the call with `sub sp`/`str [sp]`/`add sp` to pass the variadic `mode`
        // on the stack; checked before the plain value-returning arm.
        Architecture::Aarch64 if operation_key.passes_trailing_mode_on_stack() => {
            aarch64::encode_host_call_sequence_value_returning_open_create_from_operands(
                operands.iter().map(aarch64_call_operand),
            )
        }
        // Float-returning ops (sqrt/hypot) also share `returns_value()` but the
        // result comes back in `d0`; the encoder inserts `fmov x0, d0` before the
        // result store. Checked before the plain value-returning arm.
        Architecture::Aarch64 if operation_key.returns_float() => {
            aarch64::encode_host_call_sequence_value_returning_float_from_operands(
                operands.iter().map(aarch64_call_operand),
            )
        }
        // Constant-result ops have NO call (and no import relocation): the
        // constant materializes into x0 and stores through the normal result
        // tail. Checked before the plain value-returning arm (they share
        // `returns_value()`).
        Architecture::Aarch64 if operation_key.lowers_to_constant_result() => {
            aarch64::encode_host_call_sequence_constant_result_from_operands(
                operands.iter().map(aarch64_call_operand),
            )
        }
        Architecture::Aarch64 if operation_key.returns_value() => {
            aarch64::encode_host_call_sequence_value_returning_from_operands(
                operands.iter().map(aarch64_call_operand),
            )
        }
        Architecture::Aarch64 => aarch64::encode_host_call_sequence_from_operands(
            operands.iter().map(aarch64_call_operand),
        ),
        Architecture::X86_64 => x86_64::encode_host_call_sequence(operation_key, operands),
    }
}

/// A provides-authored / via-leaf IMPORT call (custom capability): the
/// emission-planning blocker enforces the result-binding shape, so the call
/// ALWAYS carries a leading result operand -- on aarch64 it routes to the
/// value-returning sequence directly (the capability-keyed returns_value()
/// catalog cannot know authored operations; routing by catalog sent the
/// result place into x0 and shifted every real argument -- the
/// import_call_argument_lost class). x86_64's encoder handles the key
/// itself (windows-session verified).
pub fn encode_authored_import_call_sequence<T: InstructionOperandLike>(
    architecture: Architecture,
    operation_key: HostOperationKey,
    operands: &[T],
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_host_call_sequence_value_returning_from_operands(
                operands.iter().map(aarch64_call_operand),
            )
        }
        Architecture::X86_64 => x86_64::encode_host_call_sequence(operation_key, operands),
    }
}

pub fn encode_syscall_sequence<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
) -> Result<Vec<u8>, Diagnostic> {
    let policy = match architecture {
        Architecture::Aarch64 => CallingPolicy::LinuxSyscallAarch64,
        Architecture::X86_64 => CallingPolicy::LinuxSyscallX86_64,
    };
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8); operands.len()],
        result: None,
    };
    let plan = evaluate_call_plan(policy, &signature)
        .map_err(|error| Diagnostic::error(format!("cannot evaluate syscall call plan: {error}")))?;
    let argument_registers = plan
        .parameters
        .iter()
        .enumerate()
        .map(|(index, placement)| match placement.locations.as_slice() {
            [ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size: 8,
            }] => Ok(*register),
            locations => Err(Diagnostic::error(format!(
                "normalized syscall parameter {index} did not resolve to one full-width register: {locations:?}"
            ))),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let EntryControl::SupervisorCall {
        number_register,
        immediate: supervisor_call,
    } = plan.entry_control
    else {
        return Err(Diagnostic::error(
            "normalized syscall plan did not select supervisor-call entry control",
        ));
    };

    match architecture {
        Architecture::Aarch64 => aarch64::encode_syscall_sequence_from_operands(
            operands.iter().map(aarch64_call_operand),
            syscall_number,
            &argument_registers,
            number_register,
            supervisor_call,
        ),
        Architecture::X86_64 => x86_64::encode_syscall_sequence(
            operands,
            syscall_number,
            &argument_registers,
            number_register,
            supervisor_call,
        ),
    }
}

pub fn encode_function_enter_bytes(
    architecture: Architecture,
) -> Result<(Vec<u8>, usize), Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            let bytes = aarch64::encode_function_enter_bytes().to_vec();
            let byte_count = bytes.len();
            Ok((bytes, byte_count))
        }
        Architecture::X86_64 => Ok((Vec::new(), 0)),
    }
}

pub fn encode_return_bytes(architecture: Architecture) -> Result<(Vec<u8>, usize), Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            let bytes = aarch64::encode_return_bytes().to_vec();
            let byte_count = bytes.len();
            Ok((bytes, byte_count))
        }
        Architecture::X86_64 => {
            let bytes = x86_64::encode_return_bytes().to_vec();
            let byte_count = bytes.len();
            Ok((bytes, byte_count))
        }
    }
}

/// The privileged `hlt` (`asm { hlt }`) as target bytes: `hlt` (0xF4) on
/// x86_64, its idle analog `wfi` on AArch64. Position-independent, no
/// relocation site.
pub fn encode_machine_halt_bytes(architecture: Architecture) -> Vec<u8> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_machine_halt_bytes().to_vec(),
        Architecture::X86_64 => x86_64::encode_machine_halt_bytes().to_vec(),
    }
}

pub fn encode_memory_fence_bytes(
    architecture: Architecture,
    kind: omega_core::inline_assembly::AsmFenceKind,
) -> Option<Vec<u8>> {
    match architecture {
        Architecture::Aarch64 => None,
        Architecture::X86_64 => Some(x86_64::encode_memory_fence_bytes(kind).to_vec()),
    }
}

pub fn encode_interrupt_control_bytes(
    architecture: Architecture,
    kind: omega_core::inline_assembly::AsmInterruptControlKind,
) -> Option<Vec<u8>> {
    match architecture {
        Architecture::Aarch64 => None,
        Architecture::X86_64 => {
            Some(x86_64::encode_interrupt_control_bytes(kind).to_vec())
        }
    }
}

pub fn encode_runtime_storage_copy_to_return_register_bytes(
    architecture: Architecture,
    register: omega_calling_conventions::MachineRegister,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if register.architecture() != architecture {
        return Err(Diagnostic::error(format!(
            "result register {register:?} does not belong to target architecture {architecture:?}"
        )));
    }
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_storage_copy_to_return_register_bytes(
                register,
                byte_offset,
                byte_size,
            )
        }
        Architecture::X86_64 => {
            x86_64::encode_runtime_storage_copy_to_return_register_bytes(
                register,
                byte_offset,
                byte_size,
            )
        }
    }
}

/// The entry prologue's inbound argument unmarshal. The normalized call plan
/// names the exact register; target encoders only realize that selection.
pub fn encode_entry_argument_register_write_bytes(
    register: omega_calling_conventions::MachineRegister,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match register.architecture() {
        Architecture::Aarch64 => aarch64::encode_entry_argument_register_write_bytes(
            register,
            byte_offset,
            byte_size,
        ),
        Architecture::X86_64 => x86_64::encode_entry_argument_register_write_bytes(
            register,
            byte_offset,
            byte_size,
        ),
    }
}

/// Copy one normalized incoming stack-argument fragment into its entry-frame
/// destination. Target encoders add their ABI return-address/prologue bias.
pub fn encode_entry_stack_argument_write_bytes(
    architecture: Architecture,
    stack_byte_offset: u32,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_entry_stack_argument_write_bytes(
            stack_byte_offset,
            byte_offset,
            byte_size,
        ),
        Architecture::X86_64 => x86_64::encode_entry_stack_argument_write_bytes(
            stack_byte_offset,
            byte_offset,
            byte_size,
        ),
    }
}

/// The entry prologue's `args: &[u8]` descriptor write (x86_64 only).
pub fn encode_entry_arguments_slice_descriptor_write_bytes(
    architecture: Architecture,
    descriptor_offset: usize,
    spill_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_entry_arguments_slice_descriptor_write_bytes(
            descriptor_offset,
            spill_offset,
            byte_length,
        ),
        Architecture::X86_64 => x86_64::encode_entry_arguments_slice_descriptor_write_bytes(
            descriptor_offset,
            spill_offset,
            byte_length,
        ),
    }
}

pub fn encode_return_register_integer_write_bytes(
    architecture: Architecture,
    register: omega_calling_conventions::MachineRegister,
    byte_size: usize,
    value: i64,
) -> Result<(Vec<u8>, usize), Diagnostic> {
    if register.architecture() != architecture {
        return Err(Diagnostic::error(format!(
            "result register {register:?} does not belong to target architecture {architecture:?}"
        )));
    }
    match architecture {
        Architecture::Aarch64 => {
            let bytes = aarch64::encode_return_register_integer_write_bytes(
                register,
                byte_size,
                value,
            )?
            .to_vec();
            let byte_count = bytes.len();
            Ok((bytes, byte_count))
        }
        Architecture::X86_64 => {
            let bytes = x86_64::encode_return_register_integer_write_bytes(
                register,
                byte_size,
                value,
            )?;
            let byte_count = bytes.len();
            Ok((bytes, byte_count))
        }
    }
}

#[cfg(test)]
mod result_register_architecture_tests {
    use super::*;
    use omega_calling_conventions::MachineRegister;

    #[test]
    fn result_registers_cannot_cross_target_architectures() {
        let error = encode_return_register_integer_write_bytes(
            Architecture::Aarch64,
            MachineRegister::X86Rax,
            4,
            0,
        )
        .expect_err("foreign result register must reject");
        assert!(error.message.contains("does not belong to target architecture"));
    }
}
