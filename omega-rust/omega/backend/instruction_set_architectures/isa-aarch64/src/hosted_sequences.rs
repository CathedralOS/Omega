//! Hosted AArch64 instruction sequences realized without imports: process
//! exit, byte read and write through the host syscall interface, and a
//! literal line write, each encoded from raw MOVZ/MOVK/SVC words for Linux and
//! Darwin. Selected-form encodings live in `selected_form_encoding`.

use diagnostics::Diagnostic;

/// Exact import-free hosted AArch64 realization of `exit_process(i32)`.
/// Darwin uses syscall 1 in x16 and SVC 0x80; Linux uses exit_group 94 in x8.
/// See Apple's xnu `bsd/kern/syscalls.master` and `libsyscall/custom/SYS.h`:
/// <https://github.com/apple-oss-distributions/xnu>.
pub fn encode_hosted_exit_process_i32(
    target: target::NativeTarget,
    value: i32,
) -> Result<Vec<u8>, Diagnostic> {
    let (syscall_register, syscall_number, supervisor_call) =
        if target == target::NativeTarget::linux_arm64() {
            (8, 94, 0)
        } else if target == target::NativeTarget::macos_arm64() {
            (16, 1, 0x80)
        } else {
            return Err(Diagnostic::error(
                "unsupported hosted AArch64 process-exit target",
            ));
        };
    let mut bytes = Vec::new();
    append_unsigned_immediate(&mut bytes, 0, i64::from(value) as u64);
    append_unsigned_immediate(&mut bytes, syscall_register, syscall_number);
    bytes.extend(encode_svc(supervisor_call));
    bytes.extend(encode_brk(0));
    Ok(bytes)
}

/// Import-free Linux `write(1, &byte, 1)` realization. The caller places the
/// low byte of the exact `i32` source in `w9`; this encoder owns the stack slot.
pub fn encode_hosted_write_byte_i32_from_w9() -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0xd100_43ff_u32.to_le_bytes());
    bytes.extend_from_slice(&0x3900_03e9_u32.to_le_bytes());
    bytes.extend(encode_movz(0, 1));
    bytes.extend_from_slice(&0x9100_03e1_u32.to_le_bytes());
    bytes.extend(encode_movz(2, 1));
    bytes.extend(encode_movz(8, 64));
    bytes.extend(encode_svc(0));
    bytes.extend(encode_compare_x_immediate(0, 0)?);
    bytes.extend_from_slice(&0x5400_006d_u32.to_le_bytes());
    bytes.extend_from_slice(&0x9100_43ff_u32.to_le_bytes());
    bytes.extend_from_slice(&0x1400_0002_u32.to_le_bytes());
    bytes.extend(encode_brk(0));
    Ok(bytes)
}

/// Import-free Linux `read(0, &byte, 1)` realization into one canonical
/// `ByteRead = Eof | Byte(i32)` stack home.
pub fn encode_linux_read_byte_to_stack(
    home_byte_offset: u32,
    payload_byte_offset: u32,
) -> Result<Vec<u8>, Diagnostic> {
    if !home_byte_offset.is_multiple_of(4)
        || !payload_byte_offset.is_multiple_of(4)
        || home_byte_offset / 4 > 0xfff
        || payload_byte_offset / 4 > 0xfff
        || payload_byte_offset > 0xfff
    {
        return Err(Diagnostic::error(
            "Linux AArch64 read-byte home is not directly addressable",
        ));
    }

    let mut words = Vec::with_capacity(14);
    let str_w = |register: u8, offset: u32| {
        0xb900_0000 | ((offset / 4) << 10) | (31 << 5) | u32::from(register)
    };
    words.push(str_w(31, home_byte_offset)); // canonical Eof tag
    words.push(str_w(31, payload_byte_offset)); // zero unused payload bytes
    words.push(0x9100_03e1 | (payload_byte_offset << 10)); // add x1, sp, #payload
    words.push(u32::from_le_bytes(encode_movz(0, 0)));
    words.push(u32::from_le_bytes(encode_movz(2, 1)));
    words.push(u32::from_le_bytes(encode_movz(8, 63))); // SYS_read
    words.push(u32::from_le_bytes(encode_svc(0)));
    words.push(0xb400_0000 | (7 << 5)); // cbz x0, done
    words.push(u32::from_le_bytes(encode_compare_x_immediate(0, 1)?));
    words.push(0x5400_0001 | (4 << 5)); // b.ne trap
    words.push(0x5280_0029); // mov w9, #1
    words.push(str_w(9, home_byte_offset));
    words.push(0x1400_0002); // b done
    words.push(u32::from_le_bytes(encode_brk(0)));
    Ok(words.into_iter().flat_map(u32::to_le_bytes).collect())
}

/// Import-free Darwin read into caller-owned storage, without changing SP.
pub fn encode_macos_read_byte_to_stack(
    home_byte_offset: u32,
    payload_byte_offset: u32,
) -> Result<Vec<u8>, Diagnostic> {
    if !home_byte_offset.is_multiple_of(4)
        || home_byte_offset.checked_add(4) != Some(payload_byte_offset)
        || payload_byte_offset > 0xfff
    {
        return Err(Diagnostic::error(
            "Darwin AArch64 read-byte home is not directly addressable",
        ));
    }
    let store =
        |register: u32, offset: u32| 0xb900_0000 | ((offset / 4) << 10) | (31 << 5) | register;
    let words = [
        store(31, home_byte_offset),
        store(31, payload_byte_offset),
        0x9100_03e1 | (payload_byte_offset << 10),
        u32::from_le_bytes(encode_movz(0, 0)),
        u32::from_le_bytes(encode_movz(2, 1)),
        u32::from_le_bytes(encode_movz(16, 3)),
        u32::from_le_bytes(encode_svc(0x80)),
        0x5400_00e2, // b.cs trap: consume syscall carry before CMP replaces NZCV
        0xb400_00e0, // cbz x0, done
        u32::from_le_bytes(encode_compare_x_immediate(0, 1)?),
        0x5400_0081, // b.ne trap
        0x5280_0029, // mov w9, #1
        store(9, home_byte_offset),
        0x1400_0002, // b done
        u32::from_le_bytes(encode_brk(0)),
    ];
    Ok(words.into_iter().flat_map(u32::to_le_bytes).collect())
}

/// Import-free Linux `write_line` over one immutable literal.
pub fn encode_linux_write_line_literal(
    literal: &[u8],
) -> Result<(Vec<u8>, std::ops::Range<usize>), Diagnostic> {
    let payload_len = literal
        .len()
        .checked_add(1)
        .and_then(|len| u64::try_from(len).ok())
        .ok_or_else(|| Diagnostic::error("Linux AArch64 write_line literal is too large"))?;
    if payload_len >= (1 << 20) {
        return Err(Diagnostic::error(
            "Linux AArch64 write_line literal exceeds the PC-relative carrier",
        ));
    }
    let mut bytes = Vec::new();
    let adr_offset = bytes.len();
    bytes.extend_from_slice(&[0; 4]); // adr x1, data
    append_unsigned_immediate(&mut bytes, 2, payload_len);
    bytes.extend(encode_movz(8, 64)); // x8 = SYS_write
    let loop_offset = bytes.len();
    bytes.extend(encode_movz(0, 1)); // x0 = STDOUT_FILENO
    bytes.extend(encode_svc(0));
    bytes.extend(encode_compare_x_immediate(0, 0)?);
    let trap_branch_offset = bytes.len();
    bytes.extend_from_slice(&[0; 4]); // b.le trap
    bytes.extend(encode_add_x_register(1, 1, 0));
    bytes.extend(encode_subs_x_register(2, 2, 0));
    let loop_branch_offset = bytes.len();
    bytes.extend_from_slice(&[0; 4]); // b.ne loop
    let data_skip_offset = bytes.len();
    bytes.extend_from_slice(&[0; 4]); // b after_data
    let trap_offset = bytes.len();
    bytes.extend(encode_brk(0));
    let data_offset = bytes.len();
    bytes.extend_from_slice(literal);
    bytes.push(b'\n');
    let data_end = bytes.len();
    while bytes.len() % 4 != 0 {
        bytes.push(0);
    }
    let after_data = bytes.len();

    let adr_distance = i32::try_from(data_offset as i128 - adr_offset as i128)
        .map_err(|_| Diagnostic::error("Linux AArch64 write_line ADR is out of range"))?;
    if !(-(1 << 20)..(1 << 20)).contains(&adr_distance) {
        return Err(Diagnostic::error(
            "Linux AArch64 write_line ADR is out of range",
        ));
    }
    let immediate = adr_distance as u32 & 0x1f_ffff;
    let adr = 0x1000_0000 | ((immediate & 0x3) << 29) | (((immediate >> 2) & 0x7ffff) << 5) | 1;
    bytes[adr_offset..adr_offset + 4].copy_from_slice(&adr.to_le_bytes());
    bytes[trap_branch_offset..trap_branch_offset + 4].copy_from_slice(
        &encode_conditional_branch_less_or_equal(
            isize::try_from(trap_offset).unwrap() - isize::try_from(trap_branch_offset).unwrap(),
        )?,
    );
    bytes[loop_branch_offset..loop_branch_offset + 4].copy_from_slice(
        &encode_conditional_branch_not_equal(
            isize::try_from(loop_offset).unwrap() - isize::try_from(loop_branch_offset).unwrap(),
        )?,
    );
    bytes[data_skip_offset..data_skip_offset + 4].copy_from_slice(&encode_unconditional_branch(
        isize::try_from(after_data).unwrap() - isize::try_from(data_skip_offset).unwrap(),
    )?);
    Ok((bytes, data_offset..data_end))
}

fn instruction(word: u32) -> [u8; 4] {
    word.to_le_bytes()
}

fn encode_movz(register: u8, immediate: u16) -> [u8; 4] {
    instruction(0xD2800000 | (u32::from(immediate) << 5) | u32::from(register))
}

fn encode_movk(register: u8, immediate: u16, halfword_shift: u8) -> [u8; 4] {
    instruction(
        0xF2800000
            | (u32::from(halfword_shift) << 21)
            | (u32::from(immediate) << 5)
            | u32::from(register),
    )
}

fn append_unsigned_immediate(bytes: &mut Vec<u8>, register: u8, value: u64) {
    bytes.extend(encode_movz(register, halfword(value, 0)));
    for halfword_shift in 1..4 {
        let immediate = halfword(value, halfword_shift);
        if immediate != 0 {
            bytes.extend(encode_movk(register, immediate, halfword_shift));
        }
    }
}

fn halfword(value: u64, halfword_shift: u8) -> u16 {
    ((value >> (u64::from(halfword_shift) * 16)) & 0xffff) as u16
}

fn encode_svc(immediate: u16) -> [u8; 4] {
    instruction(0xD4000001 | (u32::from(immediate) << 5))
}

fn encode_brk(immediate: u16) -> [u8; 4] {
    instruction(0xD4200000 | (u32::from(immediate) << 5))
}

fn encode_compare_x_immediate(register: u8, value: u32) -> Result<[u8; 4], Diagnostic> {
    if value > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot compare value `{value}` yet"
        )));
    }
    Ok(instruction(
        0xF100001F | (value << 10) | (u32::from(register) << 5),
    ))
}

fn encode_add_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    instruction(
        0x8B000000
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

fn encode_subs_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    instruction(
        0xEB000000
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

fn encode_conditional_branch_not_equal(byte_distance: isize) -> Result<[u8; 4], Diagnostic> {
    encode_conditional_branch(byte_distance, 0x1, "b.ne")
}

fn encode_conditional_branch_less_or_equal(byte_distance: isize) -> Result<[u8; 4], Diagnostic> {
    encode_conditional_branch(byte_distance, 0xd, "b.le")
}

fn encode_conditional_branch(
    byte_distance: isize,
    condition: u32,
    instruction_name: &str,
) -> Result<[u8; 4], Diagnostic> {
    let distance = checked_instruction_distance(byte_distance, 19, instruction_name)?;
    Ok(instruction(
        0x54000000 | ((distance as u32 & 0x7ffff) << 5) | condition,
    ))
}

fn encode_unconditional_branch(byte_distance: isize) -> Result<[u8; 4], Diagnostic> {
    let distance = checked_instruction_distance(byte_distance, 26, "b")?;
    Ok(instruction(0x14000000 | (distance as u32 & 0x03ff_ffff)))
}

fn checked_instruction_distance(
    byte_distance: isize,
    immediate_bits: u8,
    instruction_name: &str,
) -> Result<isize, Diagnostic> {
    if byte_distance % 4 != 0 {
        return Err(Diagnostic::error(format!(
            "AArch64 {instruction_name} target is not instruction aligned: {byte_distance} byte(s)"
        )));
    }
    let distance = byte_distance / 4;
    let min = -(1isize << (immediate_bits - 1));
    let max = (1isize << (immediate_bits - 1)) - 1;
    if distance < min || distance > max {
        return Err(Diagnostic::error(format!(
            "AArch64 {instruction_name} target is out of range: {distance} instruction(s)"
        )));
    }
    Ok(distance)
}

#[cfg(test)]
mod tests {
    use super::{
        encode_brk, encode_hosted_exit_process_i32, encode_hosted_write_byte_i32_from_w9,
        encode_linux_read_byte_to_stack, encode_linux_write_line_literal,
    };

    #[test]
    fn hosted_exit_process_uses_exact_darwin_trap_abi() {
        use target::NativeTarget;
        let bytes = encode_hosted_exit_process_i32(NativeTarget::macos_arm64(), 37).unwrap();
        assert_eq!(
            bytes,
            [0xd280_04a0_u32, 0xd280_0030, 0xd400_1001, 0xd420_0000]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>()
        );
        for value in [0, 1, 255, 256, -1, i32::MIN, i32::MAX] {
            let darwin =
                encode_hosted_exit_process_i32(NativeTarget::macos_arm64(), value).unwrap();
            let linux = encode_hosted_exit_process_i32(NativeTarget::linux_arm64(), value).unwrap();
            assert_eq!(&darwin[..darwin.len() - 12], &linux[..linux.len() - 12]);
            assert_eq!(&darwin[darwin.len() - 4..], &0xd420_0000_u32.to_le_bytes());
            assert_ne!(darwin, linux);
        }
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::windows_x64(),
            NativeTarget {
                pointer_size: 4,
                ..NativeTarget::macos_arm64()
            },
        ] {
            assert!(encode_hosted_exit_process_i32(target, 37).is_err());
        }
    }

    #[test]
    fn linux_exit_and_write_literal_keep_exact_bytes() {
        let bytes =
            encode_hosted_exit_process_i32(target::NativeTarget::linux_arm64(), 37).unwrap();
        assert_eq!(bytes.len(), 16);
        assert_eq!(&bytes[0..4], &0xd280_04a0_u32.to_le_bytes());
        assert_eq!(&bytes[4..8], &0xd280_0bc8_u32.to_le_bytes());
        assert_eq!(&bytes[8..12], &0xd400_0001_u32.to_le_bytes());
        assert_eq!(&bytes[12..16], &0xd420_0000_u32.to_le_bytes());

        let (bytes, data) = encode_linux_write_line_literal(&[0, 0x80, 0xff]).unwrap();
        assert_eq!(&bytes[data.clone()], &[0, 0x80, 0xff, b'\n']);
        assert_eq!(bytes.len() % 4, 0);
        assert_eq!(
            &bytes[data.start - 4..data.start],
            &0xd420_0000_u32.to_le_bytes()
        );

        let byte_write = encode_hosted_write_byte_i32_from_w9().unwrap();
        assert_eq!(byte_write.len(), 48);
        assert_eq!(&byte_write[..4], &0xd100_43ff_u32.to_le_bytes());
        assert_eq!(&byte_write[36..40], &0x9100_43ff_u32.to_le_bytes());
        assert_eq!(&byte_write[40..44], &0x1400_0002_u32.to_le_bytes());
        assert_eq!(&byte_write[44..], &0xd420_0000_u32.to_le_bytes());

        let byte_read = encode_linux_read_byte_to_stack(16, 20).unwrap();
        assert_eq!(byte_read.len(), 14 * 4);
        assert_eq!(&byte_read[byte_read.len() - 4..], &encode_brk(0));
        assert!(encode_linux_read_byte_to_stack(2, 6).is_err());
    }
}
