use psi_diagnostics::Diagnostic;

use super::super::primitives::{
    append_add_x_constant, append_unsigned_immediate, encode_add_page_offset_placeholder,
    encode_add_x_immediate, encode_adrp_placeholder, encode_branch_link_placeholder, encode_cbnz_x,
    encode_cbz_x, encode_compare_w_immediate, encode_conditional_branch_equal,
    encode_conditional_branch_not_equal, encode_load_byte_w_from_x, encode_move_x_register,
    encode_movz, encode_store_byte_w_to_x, encode_store_x_to_x, encode_subs_x_immediate,
    encode_svc, encode_unconditional_branch,
};
use super::super::widths::{
    runtime_text_line_read_carrier_import_width, runtime_text_line_read_carrier_syscall_width,
    runtime_text_line_read_import_width, runtime_text_line_read_syscall_width,
};
use super::{Aarch64SyscallRegisters, aarch64_syscall_registers};

#[derive(Clone, Copy)]
enum RuntimeTextReadCall {
    Import,
    Syscall {
        number: u32,
        registers: Aarch64SyscallRegisters,
        supervisor_call: u16,
    },
}

/// Which storage shape the read lands in: a `{ptr, len}` String descriptor
/// backed by a separate line-buffer data object, or an owned `[u8; N]` carrier
/// whose inline bytes ARE the read destination (`{len @ 0, bytes @ +8}` in the
/// target region itself -- no buffer object, no descriptor store, one
/// relocation). The byte-at-a-time read loop is identical for both; only the
/// prologue (where the write cursor starts) and the epilogue (what gets stored
/// after the line ends) differ. Mirrors the x86_64 `build_runtime_text_line_read`
/// carrier flag.
#[derive(Clone, Copy, PartialEq)]
enum RuntimeTextReadTarget {
    StringDescriptor,
    BoundedByteCarrier,
    RawFixedArray,
}

pub fn encode_runtime_text_line_read_import(
    target_offset: usize,
    byte_capacity: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_text_line_read(
        target_offset,
        byte_capacity,
        RuntimeTextReadCall::Import,
        RuntimeTextReadTarget::StringDescriptor,
    )
}

pub fn encode_runtime_text_line_read_syscall(
    target_offset: usize,
    byte_capacity: usize,
    number: u32,
    parameter_registers: &[omega_calling_conventions::MachineRegister],
    result_register: omega_calling_conventions::MachineRegister,
    number_register: omega_calling_conventions::MachineRegister,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    let registers =
        aarch64_syscall_registers(parameter_registers, result_register, number_register)?;
    encode_runtime_text_line_read(
        target_offset,
        byte_capacity,
        RuntimeTextReadCall::Syscall {
            number,
            registers,
            supervisor_call,
        },
        RuntimeTextReadTarget::StringDescriptor,
    )
}

pub fn encode_runtime_text_line_read_carrier_import(
    target_offset: usize,
    byte_capacity: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_text_line_read(
        target_offset,
        byte_capacity,
        RuntimeTextReadCall::Import,
        RuntimeTextReadTarget::BoundedByteCarrier,
    )
}

pub fn encode_runtime_text_line_read_carrier_syscall(
    target_offset: usize,
    byte_capacity: usize,
    number: u32,
    parameter_registers: &[omega_calling_conventions::MachineRegister],
    result_register: omega_calling_conventions::MachineRegister,
    number_register: omega_calling_conventions::MachineRegister,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    let registers =
        aarch64_syscall_registers(parameter_registers, result_register, number_register)?;
    encode_runtime_text_line_read(
        target_offset,
        byte_capacity,
        RuntimeTextReadCall::Syscall {
            number,
            registers,
            supervisor_call,
        },
        RuntimeTextReadTarget::BoundedByteCarrier,
    )
}

pub fn encode_runtime_text_line_read_fixed_array_import(
    target_offset: usize,
    byte_capacity: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_text_line_read(
        target_offset,
        byte_capacity,
        RuntimeTextReadCall::Import,
        RuntimeTextReadTarget::RawFixedArray,
    )
}

pub fn encode_runtime_text_line_read_fixed_array_syscall(
    target_offset: usize,
    byte_capacity: usize,
    number: u32,
    parameter_registers: &[omega_calling_conventions::MachineRegister],
    result_register: omega_calling_conventions::MachineRegister,
    number_register: omega_calling_conventions::MachineRegister,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    let registers =
        aarch64_syscall_registers(parameter_registers, result_register, number_register)?;
    encode_runtime_text_line_read(
        target_offset,
        byte_capacity,
        RuntimeTextReadCall::Syscall {
            number,
            registers,
            supervisor_call,
        },
        RuntimeTextReadTarget::RawFixedArray,
    )
}

fn encode_runtime_text_line_read(
    target_offset: usize,
    byte_capacity: usize,
    call: RuntimeTextReadCall,
    target: RuntimeTextReadTarget,
) -> Result<Vec<u8>, Diagnostic> {
    // The descriptor flavor reserves one buffer byte for its NUL terminator;
    // the carrier is length-delimited by its leading len word (no NUL, exactly
    // like the x86_64 carrier flavor), so all N bytes hold content.
    let max_payload_bytes = match target {
        RuntimeTextReadTarget::StringDescriptor => byte_capacity.saturating_sub(1),
        RuntimeTextReadTarget::BoundedByteCarrier | RuntimeTextReadTarget::RawFixedArray => {
            byte_capacity
        }
    };
    let capacity = u32::try_from(max_payload_bytes).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 runtime line read cannot encode capacity `{byte_capacity}` yet"
        ))
    })?;
    if capacity > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 runtime line read cannot compare capacity `{byte_capacity}` yet"
        )));
    }
    let encoded_capacity = match (target, call) {
        (RuntimeTextReadTarget::StringDescriptor, RuntimeTextReadCall::Import) => {
            runtime_text_line_read_import_width(byte_capacity, target_offset)
        }
        (RuntimeTextReadTarget::StringDescriptor, RuntimeTextReadCall::Syscall { number, .. }) => {
            runtime_text_line_read_syscall_width(byte_capacity, number, target_offset)
        }
        (RuntimeTextReadTarget::BoundedByteCarrier, RuntimeTextReadCall::Import) => {
            runtime_text_line_read_carrier_import_width(target_offset)
        }
        (
            RuntimeTextReadTarget::BoundedByteCarrier,
            RuntimeTextReadCall::Syscall { number, .. },
        ) => runtime_text_line_read_carrier_syscall_width(number, target_offset),
        (RuntimeTextReadTarget::RawFixedArray, RuntimeTextReadCall::Import) => {
            super::super::widths::runtime_text_line_read_fixed_array_import_width(target_offset)
        }
        (RuntimeTextReadTarget::RawFixedArray, RuntimeTextReadCall::Syscall { number, .. }) => {
            super::super::widths::runtime_text_line_read_fixed_array_syscall_width(
                number,
                target_offset,
            )
        }
    };
    let mut bytes = Vec::with_capacity(encoded_capacity);
    // x20 = the read destination base. Descriptor: the relocated line-buffer
    // data object. Carrier: the relocated target REGION advanced past the len
    // word to the inline bytes (`region + target_offset + 8`) -- the extra add
    // is a FIXED 4-byte instruction so the import-call relocation offset stays
    // a constant (the planner has no target_offset to hand).
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    match target {
        RuntimeTextReadTarget::StringDescriptor => {}
        RuntimeTextReadTarget::BoundedByteCarrier => {
            append_add_x_constant(&mut bytes, 20, 20, target_offset + 8, 19)?;
        }
        RuntimeTextReadTarget::RawFixedArray => {
            append_add_x_constant(&mut bytes, 20, 20, target_offset, 19)?;
        }
    }
    bytes.extend(encode_move_x_register(21, 20));
    bytes.extend(encode_movz(22, 0));

    let read_loop_offset = bytes.len();
    let argument_registers = match call {
        RuntimeTextReadCall::Import => [0, 1, 2],
        RuntimeTextReadCall::Syscall { registers, .. } => registers.parameters,
    };
    bytes.extend(encode_movz(argument_registers[0], 0));
    bytes.extend(encode_move_x_register(argument_registers[1], 21));
    bytes.extend(encode_movz(argument_registers[2], 1));
    match call {
        RuntimeTextReadCall::Import => {
            bytes.extend(encode_branch_link_placeholder());
        }
        RuntimeTextReadCall::Syscall {
            number,
            registers,
            supervisor_call,
        } => {
            append_unsigned_immediate(&mut bytes, registers.number, u64::from(number));
            bytes.extend(encode_svc(supervisor_call));
        }
    }
    let result_register = match call {
        RuntimeTextReadCall::Import => 0,
        RuntimeTextReadCall::Syscall { registers, .. } => registers.result,
    };
    bytes.extend(encode_cbz_x(result_register, 64)?);
    bytes.extend(encode_load_byte_w_from_x(24, 21, 0)?);
    // A '\n'/'\r' delimiter terminates the line only once content is present
    // (x22 > 0); a LEADING one is skipped (branch back to the read loop without
    // accepting it), so CRLF acts as a single terminator -- the '\n' trailing a
    // '\r'-ended line never surfaces as a phantom empty line to the next
    // read_line. Mirrors the x86_64 reference loop.
    for (delimiter, finish_line_distance) in [(10, 48isize), (13, 32isize)] {
        bytes.extend(encode_compare_w_immediate(24, delimiter)?);
        bytes.extend(encode_conditional_branch_not_equal(12)?);
        bytes.extend(encode_cbnz_x(22, finish_line_distance)?);
        let skip_leading_delimiter_distance = read_loop_offset as isize - bytes.len() as isize;
        bytes.extend(encode_unconditional_branch(
            skip_leading_delimiter_distance,
        )?);
    }
    bytes.extend(encode_compare_w_immediate(24, 0)?);
    bytes.extend(encode_conditional_branch_equal(20)?);
    bytes.extend(encode_add_x_immediate(21, 21, 1)?);
    bytes.extend(encode_add_x_immediate(22, 22, 1)?);
    bytes.extend(encode_compare_w_immediate(22, capacity)?);
    let repeat_read_distance = read_loop_offset as isize - bytes.len() as isize;
    bytes.extend(encode_conditional_branch_not_equal(repeat_read_distance)?);

    match target {
        RuntimeTextReadTarget::StringDescriptor => {
            bytes.extend(encode_store_byte_w_to_x(31, 21, 0)?);
            bytes.extend(encode_adrp_placeholder(16));
            bytes.extend(encode_add_page_offset_placeholder(16));
            // Store the String descriptor (ptr in x20, length in x22). Both slots go DIRECT when
            // they fit the STR scaled immediate (8-aligned, /8 <= 4095, i.e. target_offset+8 <=
            // 32760 -- offset in the immediate = free). For a String field after a big array the
            // offset overflows that; materialize the base into x16 ONCE (scratch x9) then store at
            // 0/8. The width side (`line_read_descriptor_store_extra`) tracks this in lockstep; the
            // x16 adrp above is at a fixed position, so its relocation offset is unchanged.
            if (target_offset + 8).is_multiple_of(8) && (target_offset + 8) / 8 <= 4095 {
                bytes.extend(encode_store_x_to_x(20, 16, target_offset)?);
                bytes.extend(encode_store_x_to_x(22, 16, target_offset + 8)?);
            } else {
                append_add_x_constant(&mut bytes, 16, 16, target_offset, 9)?;
                bytes.extend(encode_store_x_to_x(20, 16, 0)?);
                bytes.extend(encode_store_x_to_x(22, 16, 8)?);
            }
        }
        RuntimeTextReadTarget::BoundedByteCarrier => {
            // The bytes are already in place (x21 wrote straight into the inline
            // storage). Store only the length at the leading len word, which sits
            // 8 bytes BEFORE the bytes base still held in x20 -- no NUL (the
            // carrier is length-delimited) and no second relocation, mirroring
            // the x86_64 carrier epilogue's `mov [r14-8], r15`.
            bytes.extend(encode_subs_x_immediate(20, 20, 8)?);
            bytes.extend(encode_store_x_to_x(22, 20, 0)?);
        }
        RuntimeTextReadTarget::RawFixedArray => {
            // The input is disposable scratch. Bytes already landed directly
            // in the array and there is no descriptor or length word to store.
        }
    }

    let expected_width = match (target, call) {
        (RuntimeTextReadTarget::StringDescriptor, RuntimeTextReadCall::Import) => {
            runtime_text_line_read_import_width(byte_capacity, target_offset)
        }
        (RuntimeTextReadTarget::StringDescriptor, RuntimeTextReadCall::Syscall { number, .. }) => {
            runtime_text_line_read_syscall_width(byte_capacity, number, target_offset)
        }
        (RuntimeTextReadTarget::BoundedByteCarrier, RuntimeTextReadCall::Import) => {
            runtime_text_line_read_carrier_import_width(target_offset)
        }
        (
            RuntimeTextReadTarget::BoundedByteCarrier,
            RuntimeTextReadCall::Syscall { number, .. },
        ) => runtime_text_line_read_carrier_syscall_width(number, target_offset),
        (RuntimeTextReadTarget::RawFixedArray, RuntimeTextReadCall::Import) => {
            super::super::widths::runtime_text_line_read_fixed_array_import_width(target_offset)
        }
        (RuntimeTextReadTarget::RawFixedArray, RuntimeTextReadCall::Syscall { number, .. }) => {
            super::super::widths::runtime_text_line_read_fixed_array_syscall_width(
                number,
                target_offset,
            )
        }
    };
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}
