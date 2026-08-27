use psi_diagnostics::Diagnostic;

use super::super::primitives::{
    append_add_x_constant, append_unsigned_immediate, encode_add_page_offset_placeholder,
    encode_add_x_immediate, encode_adrp_placeholder, encode_branch_link_placeholder, encode_cbz_x,
    encode_movz, encode_store_w_to_x, encode_svc,
};
use super::super::widths::{
    runtime_byte_read_import_width, runtime_byte_read_syscall_width,
    runtime_byte_write_import_width, runtime_byte_write_syscall_width,
};
use super::{Aarch64SyscallRegisters, aarch64_syscall_registers};

#[derive(Clone, Copy)]
enum RuntimeByteCall {
    Import,
    Syscall {
        number: u32,
        registers: Aarch64SyscallRegisters,
        supervisor_call: u16,
    },
}

/// One stdin byte into a `ByteRead` sum slot -- ZII-driven, no scratch data
/// object (see the abstract kind's doc). x20 (callee-saved: it must survive
/// the read call, and darwin's syscall number register is x16) holds the
/// relocated target-region base:
///
///   adrp+add x20, <region>            ; relocation pair at instruction start
///   str  wzr, [x20, tag]              ; tag = 0 -- the zero state IS Eof
///   str  wzr, [x20, payload]          ; pre-zero: the read writes ONE byte
///   movz x0, #0                       ; fd 0 (stdin)
///   add  x1, x20, #payload            ; read destination = the payload word
///   movz x2, #1                       ; count 1
///   <bl _read | movz+svc>             ; import call at fixed offset 28
///   cbz  x0, #+12                     ; count == 0 -> leave the Eof zeroes
///   movz x9, #1
///   str  w9, [x20, tag]               ; tag = 1 (Byte); payload holds the
///                                     ; kernel-written low byte (LE)
pub fn encode_runtime_byte_read_import(
    target_offset: usize,
    payload_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_byte_read(target_offset, payload_offset, RuntimeByteCall::Import)
}

pub fn encode_runtime_byte_read_syscall(
    target_offset: usize,
    payload_offset: usize,
    number: u32,
    parameter_registers: &[omega_calling_conventions::MachineRegister],
    result_register: omega_calling_conventions::MachineRegister,
    number_register: omega_calling_conventions::MachineRegister,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    let registers =
        aarch64_syscall_registers(parameter_registers, result_register, number_register)?;
    encode_runtime_byte_read(
        target_offset,
        payload_offset,
        RuntimeByteCall::Syscall {
            number,
            registers,
            supervisor_call,
        },
    )
}

fn encode_runtime_byte_read(
    target_offset: usize,
    payload_offset: usize,
    call: RuntimeByteCall,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width = match call {
        RuntimeByteCall::Import => runtime_byte_read_import_width(),
        RuntimeByteCall::Syscall { number, .. } => runtime_byte_read_syscall_width(number),
    };
    let payload_absolute = target_offset + payload_offset;
    let mut bytes = Vec::with_capacity(expected_width);
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_store_w_to_x(31, 20, target_offset, 4)?);
    bytes.extend(encode_store_w_to_x(31, 20, payload_absolute, 4)?);
    let argument_registers = match call {
        RuntimeByteCall::Import => [0, 1, 2],
        RuntimeByteCall::Syscall { registers, .. } => registers.parameters,
    };
    bytes.extend(encode_movz(argument_registers[0], 0));
    bytes.extend(encode_add_x_immediate(
        argument_registers[1],
        20,
        payload_absolute,
    )?);
    bytes.extend(encode_movz(argument_registers[2], 1));
    match call {
        RuntimeByteCall::Import => bytes.extend(encode_branch_link_placeholder()),
        RuntimeByteCall::Syscall {
            number,
            registers,
            supervisor_call,
        } => {
            append_unsigned_immediate(&mut bytes, registers.number, u64::from(number));
            bytes.extend(encode_svc(supervisor_call));
        }
    }
    let result_register = match call {
        RuntimeByteCall::Import => 0,
        RuntimeByteCall::Syscall { registers, .. } => registers.result,
    };
    bytes.extend(encode_cbz_x(result_register, 12)?);
    bytes.extend(encode_movz(9, 1));
    bytes.extend(encode_store_w_to_x(9, 20, target_offset, 4)?);
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

/// One byte to stdout straight from its storage (the first byte of an
/// integer place IS its low byte on little-endian; a literal argument's
/// relocation binds the same adrp pair to a 1-byte data object and
/// `source_offset` is 0):
///
///   adrp+add x20, <source>            ; relocation pair at instruction start
///   movz x0, #1                       ; fd 1 (stdout)
///   add  x1, x20, #source_offset      ; write source = the byte's address
///   movz x2, #1                       ; count 1
///   <bl _write | movz+svc>            ; import call at fixed offset 20
pub fn encode_runtime_byte_write_import(source_offset: usize) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_byte_write(source_offset, RuntimeByteCall::Import)
}

pub fn encode_runtime_byte_write_syscall(
    source_offset: usize,
    number: u32,
    parameter_registers: &[omega_calling_conventions::MachineRegister],
    result_register: omega_calling_conventions::MachineRegister,
    number_register: omega_calling_conventions::MachineRegister,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    let registers =
        aarch64_syscall_registers(parameter_registers, result_register, number_register)?;
    encode_runtime_byte_write(
        source_offset,
        RuntimeByteCall::Syscall {
            number,
            registers,
            supervisor_call,
        },
    )
}

fn encode_runtime_byte_write(
    source_offset: usize,
    call: RuntimeByteCall,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width = match call {
        RuntimeByteCall::Import => runtime_byte_write_import_width(source_offset),
        RuntimeByteCall::Syscall { number, .. } => {
            runtime_byte_write_syscall_width(number, source_offset)
        }
    };
    let mut bytes = Vec::with_capacity(expected_width);
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    let argument_registers = match call {
        RuntimeByteCall::Import => [0, 1, 2],
        RuntimeByteCall::Syscall { registers, .. } => registers.parameters,
    };
    bytes.extend(encode_movz(argument_registers[0], 1));
    append_add_x_constant(&mut bytes, argument_registers[1], 20, source_offset, 9)?;
    bytes.extend(encode_movz(argument_registers[2], 1));
    match call {
        RuntimeByteCall::Import => bytes.extend(encode_branch_link_placeholder()),
        RuntimeByteCall::Syscall {
            number,
            registers,
            supervisor_call,
        } => {
            append_unsigned_immediate(&mut bytes, registers.number, u64::from(number));
            bytes.extend(encode_svc(supervisor_call));
        }
    }
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::super::super::widths::{
        runtime_byte_read_import_call_offset, runtime_byte_read_import_width,
        runtime_byte_read_syscall_width, runtime_byte_write_import_call_offset,
        runtime_byte_write_import_width, runtime_byte_write_syscall_width,
    };
    use super::*;
    use omega_calling_conventions::MachineRegister;

    const PARAMETERS: [MachineRegister; 3] = [
        MachineRegister::Aarch64X(0),
        MachineRegister::Aarch64X(1),
        MachineRegister::Aarch64X(2),
    ];

    // The width fns and the relocation call offsets are consumed WITHOUT the
    // encoders (layout planning + relocation records), so emission and width
    // must agree in release builds too -- the isa width tests rotted behind a
    // green canary suite once already (2026-07 lesson).
    #[test]
    fn byte_read_widths_match_emission() {
        for (target_offset, payload_offset) in [(0, 4), (8, 4), (48, 4)] {
            let import = encode_runtime_byte_read_import(target_offset, payload_offset).unwrap();
            assert_eq!(import.len(), runtime_byte_read_import_width());
            // darwin read = 3, linux_arm64 read = 63: one- and one-halfword numbers.
            for number in [3u32, 63] {
                let syscall = encode_runtime_byte_read_syscall(
                    target_offset,
                    payload_offset,
                    number,
                    &PARAMETERS,
                    MachineRegister::Aarch64X(0),
                    MachineRegister::Aarch64X(16),
                    0x80,
                )
                .unwrap();
                assert_eq!(syscall.len(), runtime_byte_read_syscall_width(number));
            }
        }
        assert!(runtime_byte_read_import_call_offset() < runtime_byte_read_import_width());
    }

    #[test]
    fn byte_write_widths_match_emission() {
        for source_offset in [0usize, 8, 48, 4129] {
            let import = encode_runtime_byte_write_import(source_offset).unwrap();
            assert_eq!(import.len(), runtime_byte_write_import_width(source_offset));
            for number in [4u32, 64] {
                let syscall = encode_runtime_byte_write_syscall(
                    source_offset,
                    number,
                    &PARAMETERS,
                    MachineRegister::Aarch64X(0),
                    MachineRegister::Aarch64X(16),
                    0x80,
                )
                .unwrap();
                assert_eq!(
                    syscall.len(),
                    runtime_byte_write_syscall_width(number, source_offset)
                );
            }
        }
        assert!(
            runtime_byte_write_import_call_offset(4129) < runtime_byte_write_import_width(4129)
        );
    }

    #[test]
    fn byte_write_syscall_uses_normalized_registers() {
        let parameters = [
            MachineRegister::Aarch64X(3),
            MachineRegister::Aarch64X(4),
            MachineRegister::Aarch64X(5),
        ];
        let bytes = encode_runtime_byte_write_syscall(
            24,
            64,
            &parameters,
            MachineRegister::Aarch64X(7),
            MachineRegister::Aarch64X(12),
            5,
        )
        .unwrap();

        assert_eq!(&bytes[8..12], &encode_movz(3, 1));
        assert_eq!(&bytes[12..16], &encode_add_x_immediate(4, 20, 24).unwrap());
        assert_eq!(&bytes[16..20], &encode_movz(5, 1));
        assert_eq!(&bytes[20..24], &encode_movz(12, 64));
        assert_eq!(&bytes[24..28], &encode_svc(5));
    }
}
