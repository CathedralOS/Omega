use super::{X86_64RelocationSite, X86_64RelocationSiteKind, append_mov_r11_imm64, disp32};
use omega_calling_conventions::MachineRegister;
use omega_target_operations::InstructionOperandLike;
use psi_diagnostics::Diagnostic;

/// A `mov <arg-reg>, imm64` is 10 bytes (2-byte REX.W+B8 prefix, then the imm64), and
/// for both an immediate/data-address argument (`mov arg, imm64`) and a runtime-storage
/// argument (whose first instruction is `mov r11, imm64=0` for the relocated region base)
/// the relocated imm64 sits at the argument's start + 2.
pub const SYSCALL_ARG_MOV_WIDTH: usize = 10;

/// Byte width of marshalling a single syscall argument into its register. Simple
/// arguments (immediate, byte-length, data-address) are a direct `mov arg, imm64`;
/// runtime-storage arguments stage the value through r11/rax (see `encode_syscall_sequence`).
fn syscall_arg_operand_width<T: InstructionOperandLike>(operand: &T) -> usize {
    if operand.runtime_pointee_string_pointer().is_some()
        || operand.runtime_pointee_string_length().is_some()
    {
        // mov r11,imm64 (10) + mov r11,[r11+off] (7) + mov rax,[r11+disp] (7) + mov arg,rax (3)
        SYSCALL_ARG_MOV_WIDTH + 7 + 7 + 3
    } else if operand.runtime_string_pointer().is_some()
        || operand.runtime_string_length().is_some()
        || operand.runtime_scalar_integer().is_some()
        || operand.runtime_storage_address().is_some()
    {
        // mov r11,imm64 (10) + mov rax,[r11+disp] (7) + mov arg,rax (3)
        SYSCALL_ARG_MOV_WIDTH + 7 + 3
    } else {
        // mov arg,imm64
        SYSCALL_ARG_MOV_WIDTH
    }
}

/// Byte offset (within the syscall sequence) of the relocated imm64 for the argument at
/// `operand_index`: the sum of the widths of all preceding arguments, plus the 2-byte
/// prefix before the imm64. Applies to both data-address and runtime-storage arguments,
/// whose relocated `mov`/`mov r11` is always the argument's first instruction.
pub fn syscall_data_relocation_byte_offset<T: InstructionOperandLike>(
    operands: &[T],
    operand_index: usize,
) -> usize {
    operands
        .iter()
        .take(operand_index)
        .map(syscall_arg_operand_width)
        .sum::<usize>()
        + 2
}

/// Total byte width of a Linux syscall sequence: each argument's marshalling, plus
/// `mov rax, imm64` (the syscall number) and the 2-byte `syscall`.
pub fn syscall_sequence_width<T: InstructionOperandLike>(operands: &[T]) -> usize {
    operands
        .iter()
        .map(syscall_arg_operand_width)
        .sum::<usize>()
        + SYSCALL_ARG_MOV_WIDTH
        + 2
}

/// x86_64 Linux (System V) syscall sequence: marshal each argument into the syscall
/// argument registers in order (RDI, RSI, RDX, R10, R8, R9), load the syscall number
/// into RAX, then `syscall` (0F 05).
///
/// Simple arguments emit a direct `mov arg, imm64` (data-address arguments use imm64=0
/// fixed up by an Absolute64 relocation). Runtime-storage arguments (a String descriptor
/// in a statically-allocated frame/machine/data region) stage through r11 and rax: load
/// the relocated region base into r11, read the pointer/length field (descriptor layout:
/// pointer at +0, length at +8) into rax, then `mov arg, rax`. Both scratch registers are
/// in the normalized syscall plan's ordinary-clobber set; no callee-saved register is
/// silently destroyed by the marshaller.
pub fn encode_syscall_sequence<T: InstructionOperandLike>(
    operands: &[T],
    syscall_number: u32,
    argument_registers: &[omega_calling_conventions::MachineRegister],
    number_register: omega_calling_conventions::MachineRegister,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    if operands.len() != argument_registers.len() {
        return Err(Diagnostic::error(format!(
            "X86_64 syscall plan supplied {} argument registers for {} operands",
            argument_registers.len(),
            operands.len()
        )));
    }
    if supervisor_call != 0 {
        return Err(Diagnostic::error(format!(
            "X86_64 `syscall` has no supervisor-call immediate, but the normalized plan supplied {supervisor_call}"
        )));
    }
    let mut bytes = Vec::with_capacity(syscall_sequence_width(operands));
    for (operand, register) in operands.iter().zip(argument_registers.iter().copied()) {
        if let Some((_, byte_offset)) = operand.runtime_pointee_string_pointer() {
            append_mov_r11_imm64(&mut bytes, 0); // relocated region base
            append_load_r11_qword_from_r11(&mut bytes, byte_offset)?; // r11 = &descriptor
            append_load_rax_from_r11(&mut bytes, 0)?; // rax = descriptor.pointer
            append_mov_syscall_arg_from_rax(&mut bytes, register)?;
        } else if let Some((_, byte_offset)) = operand.runtime_pointee_string_length() {
            append_mov_r11_imm64(&mut bytes, 0);
            append_load_r11_qword_from_r11(&mut bytes, byte_offset)?;
            append_load_rax_from_r11(&mut bytes, 8)?; // rax = descriptor.length
            append_mov_syscall_arg_from_rax(&mut bytes, register)?;
        } else if let Some((_, byte_offset)) = operand.runtime_string_pointer() {
            append_mov_r11_imm64(&mut bytes, 0);
            if operand.runtime_string_is_bounded_buffer() {
                // Owned carrier: content pointer = base + byte_offset + pointer_size.
                bytes.extend([0x49, 0x8d, 0x83]); // lea rax, [r11 + disp32]
                bytes.extend(disp32(byte_offset + 8)?.to_le_bytes());
            } else {
                append_load_rax_from_r11(&mut bytes, byte_offset)?; // rax = descriptor.pointer
            }
            append_mov_syscall_arg_from_rax(&mut bytes, register)?;
        } else if let Some((_, byte_offset)) = operand.runtime_string_length() {
            append_mov_r11_imm64(&mut bytes, 0);
            if operand.runtime_string_is_bounded_buffer() {
                append_load_rax_from_r11(&mut bytes, byte_offset)?; // carrier len @ offset 0
            } else {
                append_load_rax_from_r11(&mut bytes, byte_offset + 8)?; // rax = descriptor.length
            }
            append_mov_syscall_arg_from_rax(&mut bytes, register)?;
        } else if let Some((_, byte_offset, _)) = operand.runtime_scalar_integer() {
            append_mov_r11_imm64(&mut bytes, 0); // relocated region base
            append_load_rax_from_r11(&mut bytes, byte_offset)?; // rax = scalar value
            append_mov_syscall_arg_from_rax(&mut bytes, register)?;
        } else if let Some((_, byte_offset)) = operand.runtime_storage_address() {
            append_mov_r11_imm64(&mut bytes, 0); // relocated region base
            bytes.extend([0x49, 0x8d, 0x83]); // lea rax, [r11+disp32]
            bytes.extend(disp32(byte_offset)?.to_le_bytes());
            append_mov_syscall_arg_from_rax(&mut bytes, register)?;
        } else {
            let opcode = syscall_arg_mov_imm64_opcode(register)?;
            let value = if let Some(value) = operand.immediate_integer() {
                value as u64
            } else if let Some(value) = operand.byte_length() {
                value as u64
            } else if operand.data_address().is_some() {
                0 // relocated to the data symbol's address
            } else {
                return Err(Diagnostic::error(
                    "X86_64 syscall encoder cannot marshal this argument yet (expected \
                     immediate, byte-length, data-address, or runtime-storage)",
                ));
            };
            bytes.extend(opcode);
            bytes.extend(value.to_le_bytes());
        }
    }
    append_mov_syscall_register_imm64(&mut bytes, number_register, u64::from(syscall_number))?;
    bytes.extend([0x0f, 0x05]); // syscall
    debug_assert_eq!(bytes.len(), syscall_sequence_width(operands));
    Ok(bytes)
}

/// A value-returning Linux syscall. `operands[0]` is the Omega result place;
/// only the remaining operands are marshalled as syscall arguments.
pub fn encode_value_syscall_sequence<T: InstructionOperandLike>(
    operands: &[T],
    syscall_number: u32,
    argument_registers: &[omega_calling_conventions::MachineRegister],
    result_register: omega_calling_conventions::MachineRegister,
    number_register: omega_calling_conventions::MachineRegister,
    supervisor_call: u16,
) -> Result<(Vec<u8>, usize), Diagnostic> {
    let Some((result, arguments)) = operands.split_first() else {
        return Err(Diagnostic::error(
            "X86_64 value-returning syscall has no result storage operand",
        ));
    };
    if result_register != MachineRegister::X86Rax {
        return Err(Diagnostic::error(format!(
            "X86_64 value-returning syscall cannot realize result register {result_register:?}"
        )));
    }
    let Some((_, byte_offset, byte_count)) = result.runtime_scalar_integer() else {
        return Err(Diagnostic::error(
            "X86_64 value-returning syscall result did not lower to runtime scalar storage",
        ));
    };
    let mut bytes = encode_syscall_sequence(
        arguments,
        syscall_number,
        argument_registers,
        number_register,
        supervisor_call,
    )?;
    let result_relocation_byte_offset = bytes.len() + 2;
    append_mov_r11_imm64(&mut bytes, 0);
    match byte_count {
        1 => bytes.extend([0x41, 0x88, 0x83]), // mov [r11+disp32], al
        2 => bytes.extend([0x66, 0x41, 0x89, 0x83]), // mov [r11+disp32], ax
        4 => bytes.extend([0x41, 0x89, 0x83]), // mov [r11+disp32], eax
        8 => bytes.extend([0x49, 0x89, 0x83]), // mov [r11+disp32], rax
        other => {
            return Err(Diagnostic::error(format!(
                "X86_64 value-returning syscall cannot store a {other}-byte result"
            )));
        }
    }
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    Ok((bytes, result_relocation_byte_offset))
}

/// Linux `clock_gettime(clock_id, &timespec)` composite lowering. The Omega
/// operation returns nanoseconds, while the kernel writes two signed 64-bit
/// fields into caller storage and returns a status in RAX. This sequence owns
/// a 16-byte temporary, traps on a non-zero status (the clock id and pointer
/// are compiler-controlled), combines the fields, and stores the semantic
/// result. No `timespec` representation escapes into Omega's ABI.
pub fn encode_linux_timespec_syscall<T: InstructionOperandLike>(
    operands: &[T],
    syscall_number: u32,
    argument_registers: &[MachineRegister],
    result_register: MachineRegister,
    number_register: MachineRegister,
    supervisor_call: u16,
) -> Result<(Vec<u8>, X86_64RelocationSite), Diagnostic> {
    let [result, clock_id] = operands else {
        return Err(Diagnostic::error(
            "X86_64 Linux timespec lowering requires [result place, clock id]",
        ));
    };
    let Some((_, result_offset, result_width)) = result.runtime_scalar_integer() else {
        return Err(Diagnostic::error(
            "X86_64 Linux timespec result did not lower to runtime scalar storage",
        ));
    };
    if result_width != 8 {
        return Err(Diagnostic::error(
            "X86_64 Linux timespec result must be an eight-byte nanosecond value",
        ));
    }
    let Some(clock_id) = clock_id.immediate_integer() else {
        return Err(Diagnostic::error(
            "X86_64 Linux timespec clock id must be a plan-injected immediate",
        ));
    };
    if argument_registers != [MachineRegister::X86Rdi, MachineRegister::X86Rsi]
        || result_register != MachineRegister::X86Rax
        || number_register != MachineRegister::X86Rax
        || supervisor_call != 0
    {
        return Err(Diagnostic::error(format!(
            "X86_64 Linux timespec encoder cannot realize parameters={argument_registers:?}, \
             result={result_register:?}, number={number_register:?}, immediate={supervisor_call}"
        )));
    }

    let mut bytes = Vec::with_capacity(80);
    bytes.extend([0x48, 0x83, 0xec, 0x10]); // sub rsp, 16
    bytes.extend([0x48, 0xbf]); // mov rdi, imm64
    bytes.extend((clock_id as u64).to_le_bytes());
    bytes.extend([0x48, 0x8d, 0x34, 0x24]); // lea rsi, [rsp]
    bytes.extend([0x48, 0xb8]); // mov rax, syscall_number
    bytes.extend(u64::from(syscall_number).to_le_bytes());
    bytes.extend([0x0f, 0x05]); // syscall
    bytes.extend([0x48, 0x85, 0xc0]); // test rax, rax
    bytes.extend([0x74, 0x02]); // jz success
    bytes.extend([0x0f, 0x0b]); // ud2: fixed-input syscall failure
    bytes.extend([0x48, 0x8b, 0x04, 0x24]); // mov rax, [rsp]
    bytes.extend([0x48, 0x69, 0xc0]); // imul rax, rax, 1_000_000_000
    bytes.extend(1_000_000_000_i32.to_le_bytes());
    bytes.extend([0x48, 0x03, 0x44, 0x24, 0x08]); // add rax, [rsp+8]
    bytes.extend([0x48, 0x83, 0xc4, 0x10]); // add rsp, 16

    let result_relocation_byte_offset = bytes.len() + 2;
    append_mov_r11_imm64(&mut bytes, 0); // relocated result-region base
    bytes.extend([0x49, 0x89, 0x83]); // mov [r11 + disp32], rax
    bytes.extend(disp32(result_offset)?.to_le_bytes());

    Ok((
        bytes,
        X86_64RelocationSite {
            operand_index: Some(0),
            byte_offset: result_relocation_byte_offset,
            byte_width: 8,
            kind: X86_64RelocationSiteKind::Absolute64,
        },
    ))
}

/// Linux `nanosleep(&timespec, NULL)` adapter for Omega's millisecond sleep
/// operation. The private two-word request lives on the stack. EINTR remains
/// the platform provider's ordinary early-return behavior, matching the
/// existing Darwin poll adapter; no unbounded retry loop is hidden here.
pub fn encode_linux_timespec_argument_syscall<T: InstructionOperandLike>(
    operands: &[T],
    syscall_number: u32,
    argument_registers: &[MachineRegister],
    result_register: MachineRegister,
    number_register: MachineRegister,
    supervisor_call: u16,
) -> Result<(Vec<u8>, Option<X86_64RelocationSite>), Diagnostic> {
    let [milliseconds] = operands else {
        return Err(Diagnostic::error(
            "X86_64 Linux timespec-argument lowering requires one millisecond operand",
        ));
    };
    if argument_registers != [MachineRegister::X86Rdi, MachineRegister::X86Rsi]
        || result_register != MachineRegister::X86Rax
        || number_register != MachineRegister::X86Rax
        || supervisor_call != 0
    {
        return Err(Diagnostic::error(format!(
            "X86_64 Linux timespec-argument encoder cannot realize \
             parameters={argument_registers:?}, result={result_register:?}, \
             number={number_register:?}, immediate={supervisor_call}"
        )));
    }

    let mut bytes = Vec::with_capacity(96);
    bytes.extend([0x48, 0x83, 0xec, 0x10]); // sub rsp, 16
    let relocation =
        if let Some((_, byte_offset, byte_count)) = milliseconds.runtime_scalar_integer() {
            let relocation_byte_offset = bytes.len() + 2;
            append_mov_r11_imm64(&mut bytes, 0);
            match byte_count {
                4 => bytes.extend([0x41, 0x8b, 0x83]), // mov eax, [r11+disp32]
                8 => bytes.extend([0x49, 0x8b, 0x83]), // mov rax, [r11+disp32]
                other => {
                    return Err(Diagnostic::error(format!(
                        "X86_64 Linux sleep milliseconds must be 4 or 8 bytes, got {other}"
                    )));
                }
            }
            bytes.extend(disp32(byte_offset)?.to_le_bytes());
            Some(X86_64RelocationSite {
                operand_index: Some(0),
                byte_offset: relocation_byte_offset,
                byte_width: 8,
                kind: X86_64RelocationSiteKind::Absolute64,
            })
        } else if let Some(value) = milliseconds.immediate_integer() {
            if value < 0 {
                return Err(Diagnostic::error(
                    "X86_64 Linux sleep milliseconds cannot be negative",
                ));
            }
            bytes.extend([0x48, 0xb8]);
            bytes.extend((value as u64).to_le_bytes());
            None
        } else {
            return Err(Diagnostic::error(
                "X86_64 Linux sleep milliseconds must be an immediate or runtime scalar",
            ));
        };

    bytes.extend([0x31, 0xd2]); // xor edx, edx
    bytes.extend([0xb9]);
    bytes.extend(1000_u32.to_le_bytes()); // mov ecx, 1000
    bytes.extend([0x48, 0xf7, 0xf1]); // div rcx: rax=seconds, rdx=millisecond remainder
    bytes.extend([0x48, 0x89, 0x04, 0x24]); // mov [rsp], rax
    bytes.extend([0x48, 0x69, 0xd2]);
    bytes.extend(1_000_000_i32.to_le_bytes()); // imul rdx, rdx, 1_000_000
    bytes.extend([0x48, 0x89, 0x54, 0x24, 0x08]); // mov [rsp+8], rdx
    bytes.extend([0x48, 0x8d, 0x3c, 0x24]); // lea rdi, [rsp]
    bytes.extend([0x31, 0xf6]); // xor esi, esi
    bytes.extend([0x48, 0xb8]);
    bytes.extend(u64::from(syscall_number).to_le_bytes());
    bytes.extend([0x0f, 0x05]);
    bytes.extend([0x48, 0x83, 0xc4, 0x10]); // add rsp, 16
    Ok((bytes, relocation))
}

fn append_load_r11_qword_from_r11(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
) -> Result<(), Diagnostic> {
    bytes.extend([0x4d, 0x8b, 0x9b]);
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    Ok(())
}

fn append_load_rax_from_r11(bytes: &mut Vec<u8>, byte_offset: usize) -> Result<(), Diagnostic> {
    bytes.extend([0x49, 0x8b, 0x83]);
    bytes.extend(disp32(byte_offset)?.to_le_bytes());
    Ok(())
}

/// `mov <plan-selected-register>, imm64` (REX.W + B8+rd).
fn syscall_arg_mov_imm64_opcode(
    register: omega_calling_conventions::MachineRegister,
) -> Result<[u8; 2], Diagnostic> {
    use omega_calling_conventions::MachineRegister::*;
    Ok(match register {
        X86Rax => [0x48, 0xb8],
        X86Rcx => [0x48, 0xb9],
        X86Rdx => [0x48, 0xba],
        X86Rbx => [0x48, 0xbb],
        X86Rsp => [0x48, 0xbc],
        X86Rbp => [0x48, 0xbd],
        X86Rsi => [0x48, 0xbe],
        X86Rdi => [0x48, 0xbf],
        X86R8 => [0x49, 0xb8],
        X86R9 => [0x49, 0xb9],
        X86R10 => [0x49, 0xba],
        X86R11 => [0x49, 0xbb],
        X86R12 => [0x49, 0xbc],
        X86R13 => [0x49, 0xbd],
        X86R14 => [0x49, 0xbe],
        X86R15 => [0x49, 0xbf],
        other => {
            return Err(Diagnostic::error(format!(
                "X86_64 syscall plan selected non-GPR argument register {other:?}"
            )));
        }
    })
}

/// `mov <plan-selected-register>, rax` (opcode 89 /r, source rax = reg field 0).
fn append_mov_syscall_arg_from_rax(
    bytes: &mut Vec<u8>,
    register: omega_calling_conventions::MachineRegister,
) -> Result<(), Diagnostic> {
    let [rex, opcode] = syscall_arg_mov_imm64_opcode(register)?;
    let register_code = opcode - 0xb8;
    bytes.extend([rex, 0x89, 0xc0 | register_code]);
    Ok(())
}

fn append_mov_syscall_register_imm64(
    bytes: &mut Vec<u8>,
    register: omega_calling_conventions::MachineRegister,
    value: u64,
) -> Result<(), Diagnostic> {
    bytes.extend(syscall_arg_mov_imm64_opcode(register)?);
    bytes.extend(value.to_le_bytes());
    Ok(())
}

/// Exact import-free Linux x86-64 realization of `exit_process(i32)`.
///
/// `edi` receives the exact i32 bit pattern, `eax` receives syscall 231
/// (`exit_group`), and `ud2` closes the impossible return path.
pub fn encode_linux_exit_group_i32(value: i32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(14);
    bytes.push(0xbf); // mov edi, imm32
    bytes.extend_from_slice(&value.to_le_bytes());
    bytes.push(0xb8); // mov eax, imm32
    bytes.extend_from_slice(&231_u32.to_le_bytes());
    bytes.extend_from_slice(&[0x0f, 0x05]); // syscall
    bytes.extend_from_slice(&[0x0f, 0x0b]); // ud2 if exit_group returns
    bytes
}

/// Import-free Linux `write_line` over one immutable literal. The returned
/// data range names the exact literal followed by one `\n`; all preceding
/// bytes are executable and branch over that inline data. Short writes retry,
/// while zero/negative results trap rather than silently changing meaning.
pub fn encode_linux_write_line_literal(
    literal: &[u8],
) -> Result<(Vec<u8>, std::ops::Range<usize>), Diagnostic> {
    let payload_len = literal
        .len()
        .checked_add(1)
        .and_then(|len| u32::try_from(len).ok())
        .ok_or_else(|| Diagnostic::error("Linux x86-64 write_line literal is too large"))?;
    let mut bytes = Vec::with_capacity(52 + payload_len as usize);
    bytes.extend_from_slice(&[0xbf, 1, 0, 0, 0]); // mov edi, STDOUT_FILENO
    let lea_offset = bytes.len();
    bytes.extend_from_slice(&[0x48, 0x8d, 0x35, 0, 0, 0, 0]); // lea rsi, [rip+data]
    bytes.push(0xba); // mov edx, payload_len
    bytes.extend_from_slice(&payload_len.to_le_bytes());
    let loop_offset = bytes.len();
    bytes.extend_from_slice(&[0xb8, 1, 0, 0, 0]); // mov eax, SYS_write on every retry
    bytes.extend_from_slice(&[0x0f, 0x05]); // syscall
    bytes.extend_from_slice(&[0x48, 0x85, 0xc0]); // test rax, rax
    let trap_branch_offset = bytes.len();
    bytes.extend_from_slice(&[0x0f, 0x8e, 0, 0, 0, 0]); // jle trap
    bytes.extend_from_slice(&[0x48, 0x01, 0xc6]); // add rsi, rax
    bytes.extend_from_slice(&[0x48, 0x29, 0xc2]); // sub rdx, rax
    let loop_branch_offset = bytes.len();
    bytes.extend_from_slice(&[0x0f, 0x85, 0, 0, 0, 0]); // jne loop
    let data_skip_offset = bytes.len();
    bytes.extend_from_slice(&[0xe9, 0, 0, 0, 0]); // jmp after_data
    let trap_offset = bytes.len();
    bytes.extend_from_slice(&[0x0f, 0x0b]); // ud2
    let data_offset = bytes.len();
    bytes.extend_from_slice(literal);
    bytes.push(b'\n');
    let data_end = bytes.len();

    let relative = |target: usize, instruction_end: usize| -> Result<[u8; 4], Diagnostic> {
        i32::try_from(target as i128 - instruction_end as i128)
            .map(i32::to_le_bytes)
            .map_err(|_| Diagnostic::error("Linux x86-64 write_line branch is out of range"))
    };
    bytes[lea_offset + 3..lea_offset + 7].copy_from_slice(&relative(data_offset, lea_offset + 7)?);
    bytes[trap_branch_offset + 2..trap_branch_offset + 6]
        .copy_from_slice(&relative(trap_offset, trap_branch_offset + 6)?);
    bytes[loop_branch_offset + 2..loop_branch_offset + 6]
        .copy_from_slice(&relative(loop_offset, loop_branch_offset + 6)?);
    bytes[data_skip_offset + 1..data_skip_offset + 5]
        .copy_from_slice(&relative(data_end, data_skip_offset + 5)?);
    Ok((bytes, data_offset..data_end))
}

#[cfg(test)]
mod syscall_plan_register_tests {
    use super::*;
    use omega_calling_conventions::MachineRegister;
    use omega_target_operations::{InstructionOperandKind, TargetInstructionOperand};

    #[test]
    fn linux_exit_group_i32_has_exact_nonreturning_sequence() {
        assert_eq!(
            encode_linux_exit_group_i32(0x1234_5678),
            [
                0xbf, 0x78, 0x56, 0x34, 0x12, 0xb8, 0xe7, 0x00, 0x00, 0x00, 0x0f, 0x05, 0x0f, 0x0b,
            ]
        );
    }

    #[test]
    fn linux_write_line_literal_has_exact_data_and_closed_retry_loop() {
        let (bytes, data) = encode_linux_write_line_literal(&[0, 0x80, 0xff]).unwrap();
        assert_eq!(&bytes[data.clone()], &[0, 0x80, 0xff, b'\n']);
        assert!(!bytes[..data.start].is_empty());
        assert_eq!(&bytes[data.start - 2..data.start], &[0x0f, 0x0b]);
        let loop_reset = bytes
            .windows(7)
            .position(|window| window == [0xb8, 1, 0, 0, 0, 0x0f, 0x05])
            .expect("retry loop resets eax immediately before syscall");
        let retry = bytes
            .windows(2)
            .position(|window| window == [0x0f, 0x85])
            .expect("retry loop has a backward jne");
        let displacement = i32::from_le_bytes(bytes[retry + 2..retry + 6].try_into().unwrap());
        let retry_target = i64::try_from(retry + 6).unwrap() + i64::from(displacement);
        assert_eq!(retry_target, i64::try_from(loop_reset).unwrap());
        assert_eq!(&bytes[loop_reset..loop_reset + 5], &[0xb8, 1, 0, 0, 0]);
    }

    #[test]
    fn syscall_arguments_use_the_plan_selected_register() {
        let operands = [TargetInstructionOperand {
            kind: InstructionOperandKind::ImmediateInteger(7),
        }];
        let bytes = encode_syscall_sequence(
            &operands,
            60,
            &[MachineRegister::X86R10],
            MachineRegister::X86Rax,
            0,
        )
        .expect("noncanonical syscall register should encode");

        assert_eq!(&bytes[..2], &[0x49, 0xba], "argument must target r10");
        assert_eq!(&bytes[2..10], &7u64.to_le_bytes());
        assert_eq!(&bytes[10..12], &[0x48, 0xb8], "number must target rax");
        assert_eq!(&bytes[12..20], &60u64.to_le_bytes());
        assert_eq!(&bytes[20..], &[0x0f, 0x05]);
    }

    #[test]
    fn runtime_syscall_arguments_use_only_volatile_plan_scratch() {
        let operands = [TargetInstructionOperand {
            kind: InstructionOperandKind::RuntimeScalarInteger {
                region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 24,
                byte_count: 8,
            },
        }];
        let bytes = encode_syscall_sequence(
            &operands,
            1,
            &[MachineRegister::X86Rdi],
            MachineRegister::X86Rax,
            0,
        )
        .expect("runtime syscall argument");

        assert_eq!(&bytes[..2], &[0x49, 0xbb], "base must use volatile r11");
        assert_eq!(&bytes[10..13], &[0x49, 0x8b, 0x83]);
        assert_eq!(&bytes[17..20], &[0x48, 0x89, 0xc7]);
        assert!(!bytes.windows(2).any(|window| window == [0x49, 0xbf]));
    }

    #[test]
    fn linux_timespec_syscall_owns_the_composite_temporary_and_result_site() {
        let operands = [
            TargetInstructionOperand {
                kind: InstructionOperandKind::RuntimeScalarInteger {
                    region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 24,
                    byte_count: 8,
                },
            },
            TargetInstructionOperand {
                kind: InstructionOperandKind::ImmediateInteger(1),
            },
        ];
        let (bytes, site) = encode_linux_timespec_syscall(
            &operands,
            228,
            &[MachineRegister::X86Rdi, MachineRegister::X86Rsi],
            MachineRegister::X86Rax,
            MachineRegister::X86Rax,
            0,
        )
        .expect("clock_gettime composite lowering");

        assert_eq!(&bytes[..4], &[0x48, 0x83, 0xec, 0x10]);
        assert!(bytes.windows(2).any(|window| window == [0x0f, 0x05]));
        assert!(bytes.windows(2).any(|window| window == [0x0f, 0x0b]));
        assert!(
            bytes
                .windows(4)
                .any(|window| { window == 1_000_000_000_i32.to_le_bytes().as_slice() })
        );
        assert_eq!(site.operand_index, Some(0));
        assert_eq!(site.kind, X86_64RelocationSiteKind::Absolute64);
        assert_eq!(&bytes[site.byte_offset..site.byte_offset + 8], &[0; 8]);
        assert_eq!(&bytes[bytes.len() - 4..], &24i32.to_le_bytes());
    }

    #[test]
    fn linux_sleep_materializes_a_private_timespec_from_milliseconds() {
        let operands = [TargetInstructionOperand {
            kind: InstructionOperandKind::RuntimeScalarInteger {
                region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 32,
                byte_count: 4,
            },
        }];
        let (bytes, site) = encode_linux_timespec_argument_syscall(
            &operands,
            35,
            &[MachineRegister::X86Rdi, MachineRegister::X86Rsi],
            MachineRegister::X86Rax,
            MachineRegister::X86Rax,
            0,
        )
        .expect("nanosleep composite lowering");

        let site = site.expect("runtime milliseconds need one region relocation");
        assert_eq!(site.operand_index, Some(0));
        assert_eq!(&bytes[site.byte_offset..site.byte_offset + 8], &[0; 8]);
        assert!(
            bytes
                .windows(4)
                .any(|window| { window == 1_000_000_i32.to_le_bytes().as_slice() })
        );
        assert!(bytes.windows(2).any(|window| window == [0x0f, 0x05]));
        assert_eq!(&bytes[bytes.len() - 4..], &[0x48, 0x83, 0xc4, 0x10]);
    }

    #[test]
    fn linux_value_syscall_stores_rax_after_marshalling_only_arguments() {
        let operands = [
            TargetInstructionOperand {
                kind: InstructionOperandKind::RuntimeScalarInteger {
                    region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: 24,
                    byte_count: 4,
                },
            },
            TargetInstructionOperand {
                kind: InstructionOperandKind::ImmediateInteger(7),
            },
        ];
        let (bytes, result_site) = encode_value_syscall_sequence(
            &operands,
            3,
            &[MachineRegister::X86Rdi],
            MachineRegister::X86Rax,
            MachineRegister::X86Rax,
            0,
        )
        .expect("value-returning close syscall");

        assert_eq!(&bytes[result_site..result_site + 8], &[0; 8]);
        assert_eq!(&bytes[bytes.len() - 4..], &24_i32.to_le_bytes());
        assert!(bytes.windows(2).any(|window| window == [0x0f, 0x05]));
    }
}
