/*
We encode and decode import-free hosted x86-64 `syscall` leaves for Linux and
Intel macOS (`encode_hosted_exit_process_i32_for_target`,
`encode_darwin_read_byte_to_stack`, `encode_hosted_exit_process_i32`,
`encode_hosted_write_byte_i32_from_r11`, `encode_linux_read_byte_to_stack`, and
`encode_linux_write_line_literal`), handling Darwin's `0x0200_0000` Unix syscall
class and carry-flag error reporting alongside Linux's negative-errno contract.
*/

use diagnostics::Diagnostic;
use target::NativeTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HostedKernel {
    pub(crate) write: u32,
    pub(crate) exit: u32,
    pub(crate) carry_reports_failure: bool,
}

const DARWIN_UNIX_CLASS: u32 = 0x0200_0000;

const LINUX: HostedKernel = HostedKernel {
    write: 1,
    exit: 231,
    carry_reports_failure: false,
};

const DARWIN: HostedKernel = HostedKernel {
    write: DARWIN_UNIX_CLASS | 4,
    exit: DARWIN_UNIX_CLASS | 1,
    carry_reports_failure: true,
};

const DARWIN_READ: u32 = DARWIN_UNIX_CLASS | 3;

pub(crate) fn hosted_kernel(target: NativeTarget) -> Option<HostedKernel> {
    if target == NativeTarget::linux_x64() {
        Some(LINUX)
    } else if target == NativeTarget::macos_x64() {
        Some(DARWIN)
    } else {
        None
    }
}

pub(crate) fn hosted_syscall_target(target: NativeTarget) -> bool {
    hosted_kernel(target).is_some()
}

pub(crate) fn hosted_exit_syscall_number(target: NativeTarget) -> Option<u32> {
    hosted_kernel(target).map(|kernel| kernel.exit)
}

pub fn encode_hosted_exit_process_i32_for_target(
    target: NativeTarget,
    value: i32,
) -> Result<Vec<u8>, Diagnostic> {
    let kernel = hosted_kernel(target).ok_or_else(|| {
        Diagnostic::error("x86-64 hosted exit has no syscall kernel for this target")
    })?;
    let mut bytes = Vec::with_capacity(14);
    bytes.push(0xbf);
    crate::codec_bytes::push_encoded(&mut bytes, value.to_le_bytes());
    bytes.push(0xb8);
    crate::codec_bytes::push_encoded(&mut bytes, kernel.exit.to_le_bytes());
    bytes.extend_from_slice(&[0x0f, 0x05]);
    bytes.extend_from_slice(&[0x0f, 0x0b]);
    Ok(bytes)
}

pub fn encode_darwin_read_byte_to_stack(
    home_byte_offset: u32,
    payload_byte_offset: u32,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(DARWIN_READ_BYTE_LEAF_BYTES);
    bytes.extend_from_slice(&[0x31, 0xc0]);
    for offset in [home_byte_offset, payload_byte_offset] {
        bytes.extend_from_slice(&[0x89, 0x84, 0x24]);
        crate::codec_bytes::push_encoded(&mut bytes, offset.to_le_bytes());
    }
    bytes.extend_from_slice(&[0x31, 0xff]);
    bytes.extend_from_slice(&[0x48, 0x8d, 0xb4, 0x24]);
    crate::codec_bytes::push_encoded(&mut bytes, payload_byte_offset.to_le_bytes());
    bytes.extend_from_slice(&[0xba, 1, 0, 0, 0]);
    bytes.push(0xb8);
    crate::codec_bytes::push_encoded(&mut bytes, DARWIN_READ.to_le_bytes());
    bytes.extend_from_slice(&[0x0f, 0x05]);
    let carry_branch = bytes.len();
    bytes.extend_from_slice(&[0x72, 0]);
    bytes.extend_from_slice(&[0x48, 0x83, 0xf8, 0]);
    let eof_branch = bytes.len();
    bytes.extend_from_slice(&[0x74, 0]);
    bytes.extend_from_slice(&[0x48, 0x83, 0xf8, 1]);
    let count_branch = bytes.len();
    bytes.extend_from_slice(&[0x75, 0]);
    bytes.extend_from_slice(&[0xc7, 0x84, 0x24]);
    crate::codec_bytes::push_encoded(&mut bytes, home_byte_offset.to_le_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, 1_u32.to_le_bytes());
    let done_branch = bytes.len();
    bytes.extend_from_slice(&[0xeb, 0]);
    let trap = bytes.len();
    bytes.extend_from_slice(&[0x0f, 0x0b]);
    let done = bytes.len();

    let rel8 = |target: usize, end: usize| {
        i8::try_from(target as i128 - end as i128)
            .map(|value| value as u8)
            .map_err(|_| Diagnostic::error("Darwin x86-64 read-byte branch is out of range"))
    };
    bytes[carry_branch + 1] = rel8(trap, carry_branch + 2)?;
    bytes[eof_branch + 1] = rel8(done, eof_branch + 2)?;
    bytes[count_branch + 1] = rel8(trap, count_branch + 2)?;
    bytes[done_branch + 1] = rel8(done, done_branch + 2)?;
    Ok(bytes)
}

pub(crate) const DARWIN_READ_BYTE_LEAF_BYTES: usize = 67;

pub(crate) fn decode_darwin_read_byte(bytes: &[u8]) -> Option<u32> {
    if bytes.len() != DARWIN_READ_BYTE_LEAF_BYTES {
        return None;
    }
    let word = |start: usize| -> Option<u32> { crate::codec_bytes::u32_at(bytes, start) };
    let home = word(5)?;
    let payload = home.checked_add(4)?;
    if !home.is_multiple_of(4)
        || home > 2147483640
        || bytes[..5] != [0x31, 0xc0, 0x89, 0x84, 0x24]
        || bytes[9..12] != [0x89, 0x84, 0x24]
        || word(12)? != payload
        || bytes[16..22] != [0x31, 0xff, 0x48, 0x8d, 0xb4, 0x24]
        || word(22)? != payload
        || bytes[26..55]
            != [
                0xba, 1, 0, 0, 0, 0xb8, 3, 0, 0, 2, 0x0f, 0x05, 0x72, 25, 0x48, 0x83, 0xf8, 0,
                0x74, 21, 0x48, 0x83, 0xf8, 1, 0x75, 13, 0xc7, 0x84, 0x24,
            ]
        || word(55)? != home
        || bytes[59..] != [1, 0, 0, 0, 0xeb, 2, 0x0f, 0x0b]
    {
        return None;
    }
    Some(home)
}

#[cfg(test)]
mod tests {
    use super::{
        DARWIN_READ_BYTE_LEAF_BYTES, decode_darwin_read_byte, encode_darwin_read_byte_to_stack,
        encode_hosted_exit_process_i32_for_target, hosted_exit_syscall_number, hosted_kernel,
    };
    use target::NativeTarget;

    #[test]
    fn hosted_kernels_are_exactly_linux_and_intel_macos() {
        assert_eq!(
            hosted_exit_syscall_number(NativeTarget::linux_x64()),
            Some(231)
        );
        assert_eq!(
            hosted_exit_syscall_number(NativeTarget::macos_x64()),
            Some(0x0200_0001)
        );
        assert_eq!(
            hosted_kernel(NativeTarget::macos_x64()).unwrap().write,
            0x0200_0004
        );
        assert!(
            hosted_kernel(NativeTarget::macos_x64())
                .unwrap()
                .carry_reports_failure
        );
        assert!(
            !hosted_kernel(NativeTarget::linux_x64())
                .unwrap()
                .carry_reports_failure
        );
        for other in [
            NativeTarget::windows_x64(),
            NativeTarget::uefi_x64(),
            NativeTarget::macos_arm64(),
            NativeTarget::linux_arm64(),
        ] {
            assert_eq!(hosted_kernel(other), None, "{other:?}");
        }
    }

    #[test]
    fn immediate_exit_keeps_linux_bytes_and_enters_darwin_unix_class() {
        assert_eq!(
            encode_hosted_exit_process_i32_for_target(NativeTarget::linux_x64(), 0x1234_5678)
                .unwrap(),
            crate::encode_hosted_exit_process_i32(0x1234_5678)
        );
        assert_eq!(
            encode_hosted_exit_process_i32_for_target(NativeTarget::macos_x64(), 7).unwrap(),
            [
                0xbf, 7, 0, 0, 0, 0xb8, 0x01, 0x00, 0x00, 0x02, 0x0f, 0x05, 0x0f, 0x0b
            ]
        );
        assert!(encode_hosted_exit_process_i32_for_target(NativeTarget::windows_x64(), 0).is_err());
    }

    #[test]
    fn darwin_read_traps_on_carry_before_comparing_the_count() {
        for home in [0_u32, 32, 2147483640] {
            let bytes = encode_darwin_read_byte_to_stack(home, home + 4).unwrap();
            assert_eq!(bytes.len(), DARWIN_READ_BYTE_LEAF_BYTES);
            assert_eq!(decode_darwin_read_byte(&bytes), Some(home));
            assert_eq!(&bytes[31..40], &[0xb8, 3, 0, 0, 2, 0x0f, 0x05, 0x72, 25]);
            assert_eq!(&bytes[40 + 25..40 + 27], &[0x0f, 0x0b]);
            for bit in 0..bytes.len() * 8 {
                let mut changed = bytes.clone();
                changed[bit / 8] ^= 1 << (bit % 8);
                assert_eq!(decode_darwin_read_byte(&changed), None, "bit {bit}");
            }
            let linux = crate::encode_linux_read_byte_to_stack(home, home + 4).unwrap();
            assert_eq!(decode_darwin_read_byte(&linux), None);
        }
    }
}

pub fn encode_hosted_exit_process_i32(value: i32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(14);
    bytes.push(0xbf);
    crate::codec_bytes::push_encoded(&mut bytes, value.to_le_bytes());
    bytes.push(0xb8);
    crate::codec_bytes::push_encoded(&mut bytes, 231_u32.to_le_bytes());
    bytes.extend_from_slice(&[0x0f, 0x05]);
    bytes.extend_from_slice(&[0x0f, 0x0b]);
    bytes
}

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

pub fn encode_linux_read_byte_to_stack(
    home_byte_offset: u32,
    payload_byte_offset: u32,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(72);
    bytes.extend_from_slice(&[0x31, 0xc0]);
    for offset in [home_byte_offset, payload_byte_offset] {
        bytes.extend_from_slice(&[0x89, 0x84, 0x24]);
        crate::codec_bytes::push_encoded(&mut bytes, offset.to_le_bytes());
    }
    bytes.extend_from_slice(&[0x31, 0xff]);
    bytes.extend_from_slice(&[0x48, 0x8d, 0xb4, 0x24]);
    crate::codec_bytes::push_encoded(&mut bytes, payload_byte_offset.to_le_bytes());
    bytes.extend_from_slice(&[0xba, 1, 0, 0, 0]);
    bytes.extend_from_slice(&[0x31, 0xc0]);
    bytes.extend_from_slice(&[0x0f, 0x05]);
    bytes.extend_from_slice(&[0x48, 0x83, 0xf8, 0]);
    let eof_branch = bytes.len();
    bytes.extend_from_slice(&[0x74, 0]);
    bytes.extend_from_slice(&[0x48, 0x83, 0xf8, 1]);
    let trap_branch = bytes.len();
    bytes.extend_from_slice(&[0x75, 0]);
    bytes.extend_from_slice(&[0xc7, 0x84, 0x24]);
    crate::codec_bytes::push_encoded(&mut bytes, home_byte_offset.to_le_bytes());
    crate::codec_bytes::push_encoded(&mut bytes, 1_u32.to_le_bytes());
    let done_branch = bytes.len();
    bytes.extend_from_slice(&[0xeb, 0]);
    let trap = bytes.len();
    bytes.extend_from_slice(&[0x0f, 0x0b]);
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

pub fn encode_linux_write_line_literal(
    literal: &[u8],
) -> Result<(Vec<u8>, std::ops::Range<usize>), Diagnostic> {
    let payload_len = literal
        .len()
        .checked_add(1)
        .and_then(|len| u32::try_from(len).ok())
        .ok_or_else(|| Diagnostic::error("Linux x86-64 write_line literal is too large"))?;
    let mut bytes = Vec::with_capacity(52 + payload_len as usize);
    bytes.extend_from_slice(&[0xbf, 1, 0, 0, 0]);
    let lea_offset = bytes.len();
    bytes.extend_from_slice(&[0x48, 0x8d, 0x35, 0, 0, 0, 0]);
    bytes.push(0xba);
    crate::codec_bytes::push_encoded(&mut bytes, payload_len.to_le_bytes());
    let loop_offset = bytes.len();
    bytes.extend_from_slice(&[0xb8, 1, 0, 0, 0]);
    bytes.extend_from_slice(&[0x0f, 0x05]);
    bytes.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let trap_branch_offset = bytes.len();
    bytes.extend_from_slice(&[0x0f, 0x8e, 0, 0, 0, 0]);
    bytes.extend_from_slice(&[0x48, 0x01, 0xc6]);
    bytes.extend_from_slice(&[0x48, 0x29, 0xc2]);
    let loop_branch_offset = bytes.len();
    bytes.extend_from_slice(&[0x0f, 0x85, 0, 0, 0, 0]);
    let data_skip_offset = bytes.len();
    bytes.extend_from_slice(&[0xe9, 0, 0, 0, 0]);
    let trap_offset = bytes.len();
    bytes.extend_from_slice(&[0x0f, 0x0b]);
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
mod linux_tests {
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
