//! Import-free Linux x86-64 hosted realizations: exit, single-byte write
//! and read, and the literal write-line encoder.

use diagnostics::Diagnostic;

/// Exact import-free Linux x86-64 realization of `exit_process(i32)`.
pub fn encode_hosted_exit_process_i32(value: i32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(14);
    bytes.push(0xbf); // mov edi, imm32
    bytes.extend_from_slice(&value.to_le_bytes());
    bytes.push(0xb8); // mov eax, imm32
    bytes.extend_from_slice(&231_u32.to_le_bytes());
    bytes.extend_from_slice(&[0x0f, 0x05]); // syscall
    bytes.extend_from_slice(&[0x0f, 0x0b]); // ud2 if exit_group returns
    bytes
}

/// Import-free Linux `write(1, &byte, 1)` realization. The caller places the
/// low byte of the exact `i32` source in `r11b`; this closed encoder owns the
/// private stack slot and traps if the kernel does not consume that byte.
pub fn encode_hosted_write_byte_i32_from_r11() -> Vec<u8> {
    let mut bytes = Vec::with_capacity(44);
    bytes.extend_from_slice(&[0x48, 0x83, 0xec, 0x10]);
    bytes.extend_from_slice(&[0x44, 0x88, 0x1c, 0x24]);
    bytes.extend_from_slice(&[0xbf, 1, 0, 0, 0]);
    bytes.extend_from_slice(&[0x48, 0x89, 0xe6]);
    bytes.extend_from_slice(&[0xba, 1, 0, 0, 0]);
    bytes.extend_from_slice(&[0xb8, 1, 0, 0, 0]);
    bytes.extend_from_slice(&[0x0f, 0x05, 0x48, 0x85, 0xc0]);
    bytes.extend_from_slice(&[0x7e, 0x06]);
    bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x10]);
    bytes.extend_from_slice(&[0xeb, 0x02, 0x0f, 0x0b]);
    bytes
}

/// Import-free Linux `read(0, &byte, 1)` realization into one canonical
/// `ByteRead = Eof | Byte(i32)` stack home. The home is zeroed first, so an
/// exact zero-byte read leaves the ordinal-zero `Eof` value; an exact one-byte
/// read writes ordinal one after the kernel has filled the payload byte.
pub fn encode_linux_read_byte_to_stack(
    home_byte_offset: u32,
    payload_byte_offset: u32,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(72);
    bytes.extend_from_slice(&[0x31, 0xc0]); // xor eax, eax
    for offset in [home_byte_offset, payload_byte_offset] {
        bytes.extend_from_slice(&[0x89, 0x84, 0x24]); // mov [rsp+disp32], eax
        bytes.extend_from_slice(&offset.to_le_bytes());
    }
    bytes.extend_from_slice(&[0x31, 0xff]); // xor edi, edi (stdin)
    bytes.extend_from_slice(&[0x48, 0x8d, 0xb4, 0x24]); // lea rsi, [rsp+disp32]
    bytes.extend_from_slice(&payload_byte_offset.to_le_bytes());
    bytes.extend_from_slice(&[0xba, 1, 0, 0, 0]); // mov edx, 1
    bytes.extend_from_slice(&[0x31, 0xc0]); // xor eax, eax (SYS_read)
    bytes.extend_from_slice(&[0x0f, 0x05]); // syscall
    bytes.extend_from_slice(&[0x48, 0x83, 0xf8, 0]); // cmp rax, 0
    let eof_branch = bytes.len();
    bytes.extend_from_slice(&[0x74, 0]); // je done
    bytes.extend_from_slice(&[0x48, 0x83, 0xf8, 1]); // cmp rax, 1
    let trap_branch = bytes.len();
    bytes.extend_from_slice(&[0x75, 0]); // jne trap
    bytes.extend_from_slice(&[0xc7, 0x84, 0x24]); // mov dword [rsp+disp32], 1
    bytes.extend_from_slice(&home_byte_offset.to_le_bytes());
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    let done_branch = bytes.len();
    bytes.extend_from_slice(&[0xeb, 0]); // jmp done
    let trap = bytes.len();
    bytes.extend_from_slice(&[0x0f, 0x0b]); // ud2
    let done = bytes.len();

    let rel8 = |target: usize, end: usize| {
        i8::try_from(target as i128 - end as i128)
            .map(|value| value as u8)
            .map_err(|_| Diagnostic::error("Linux x86-64 read-byte branch is out of range"))
    };
    bytes[eof_branch + 1] = rel8(done, eof_branch + 2)?;
    bytes[trap_branch + 1] = rel8(trap, trap_branch + 2)?;
    bytes[done_branch + 1] = rel8(done, done_branch + 2)?;
    Ok(bytes)
}

/// Import-free Linux `write_line` over one immutable literal.
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
mod tests {
    use super::{
        encode_hosted_exit_process_i32, encode_hosted_write_byte_i32_from_r11,
        encode_linux_read_byte_to_stack, encode_linux_write_line_literal,
    };

    #[test]
    fn linux_exit_and_write_literal_keep_exact_bytes() {
        assert_eq!(
            encode_hosted_exit_process_i32(0x1234_5678),
            [
                0xbf, 0x78, 0x56, 0x34, 0x12, 0xb8, 0xe7, 0x00, 0x00, 0x00, 0x0f, 0x05, 0x0f, 0x0b,
            ]
        );
        let (bytes, data) = encode_linux_write_line_literal(&[0, 0x80, 0xff]).unwrap();
        assert_eq!(&bytes[data.clone()], &[0, 0x80, 0xff, b'\n']);
        assert_eq!(&bytes[data.start - 2..data.start], &[0x0f, 0x0b]);
        let retry = bytes
            .windows(2)
            .position(|window| window == [0x0f, 0x85])
            .expect("retry branch");
        let displacement = i32::from_le_bytes(bytes[retry + 2..retry + 6].try_into().unwrap());
        let retry_target = i64::try_from(retry + 6).unwrap() + i64::from(displacement);
        let loop_start = bytes
            .windows(7)
            .position(|window| window == [0xb8, 1, 0, 0, 0, 0x0f, 0x05])
            .unwrap();
        assert_eq!(retry_target, i64::try_from(loop_start).unwrap());

        let byte_write = encode_hosted_write_byte_i32_from_r11();
        assert_eq!(&byte_write[..4], &[0x48, 0x83, 0xec, 0x10]);
        assert_eq!(&byte_write[33..37], &[0x48, 0x83, 0xc4, 0x10]);
        assert_eq!(&byte_write[39..], &[0x0f, 0x0b]);
        let trap_target = 33_i64 + i64::from(byte_write[32] as i8);
        assert_eq!(trap_target, 39);
        let success_target = 39_i64 + i64::from(byte_write[38] as i8);
        assert_eq!(success_target, i64::try_from(byte_write.len()).unwrap());

        let byte_read = encode_linux_read_byte_to_stack(16, 20).unwrap();
        assert_eq!(&byte_read[..2], &[0x31, 0xc0]);
        assert_eq!(&byte_read[5..9], &16_u32.to_le_bytes());
        assert!(byte_read.windows(2).any(|window| window == [0x0f, 0x05]));
        assert_eq!(&byte_read[byte_read.len() - 2..], &[0x0f, 0x0b]);
    }
}
