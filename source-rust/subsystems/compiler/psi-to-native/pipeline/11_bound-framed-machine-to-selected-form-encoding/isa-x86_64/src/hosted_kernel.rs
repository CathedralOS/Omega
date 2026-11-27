/*
We encode and decode import-free hosted x86-64 `syscall` leaves for Linux and
Intel macOS (`encode_darwin_read_byte_to_stack`, `encode_linux_read_byte_to_stack`,
and `encode_linux_write_line_literal`), handling Darwin's `0x0200_0000` Unix
syscall class and carry-flag error reporting alongside Linux's negative-errno
contract.
*/

use diagnostics::Diagnostic;

pub(crate) use target_x86_64::hosted_kernel::*;

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

#[cfg(test)]
fn decode_darwin_read_byte(bytes: &[u8]) -> Option<u32> {
    target_x86_64::inspection::decode_x86_64_selected_hosted_read_byte(
        target::NativeTarget::macos_x64(),
        bytes,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        DARWIN_READ_BYTE_LEAF_BYTES, decode_darwin_read_byte, encode_darwin_read_byte_to_stack,
        hosted_exit_syscall_number, hosted_kernel,
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
    use super::{encode_linux_read_byte_to_stack, encode_linux_write_line_literal};

    #[test]
    fn linux_write_literal_and_read_byte_keep_exact_bytes() {
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

        let byte_read = encode_linux_read_byte_to_stack(16, 20).unwrap();
        assert_eq!(&byte_read[..2], &[0x31, 0xc0]);
        assert_eq!(&byte_read[5..9], &16_u32.to_le_bytes());
        assert!(byte_read.windows(2).any(|window| window == [0x0f, 0x05]));
        assert_eq!(&byte_read[byte_read.len() - 2..], &[0x0f, 0x0b]);
    }
}
