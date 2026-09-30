//! The x86-64 hosted kernels whose console and exit leaves enter through
//! `syscall`: Linux and Intel macOS.
//!
//! Both kernels share the instruction, the argument registers (`rdi`, `rsi`,
//! `rdx`), the result register (`rax`) and the clobbers (`rcx`, `r11`,
//! `rflags`), so one register contract serves both. They differ in two exact
//! facts. Darwin numbers BSD calls in the Unix class `0x2000000`, and it
//! reports failure by setting the carry flag with a positive errno in `rax`,
//! where Linux returns a negative errno. A Darwin leaf therefore branches on
//! carry before any comparison replaces the flags; a positive errno such as
//! `EPERM` (1) would otherwise read as a one-byte transfer.
//!
//! <https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/mach/i386/syscall_sw.h>
//! <https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/syscalls.master>

use diagnostics::Diagnostic;
use target::NativeTarget;

/// Exact syscall facts of one hosted x86-64 kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HostedKernel {
    pub(crate) write: u32,
    pub(crate) exit: u32,
    /// Darwin reports failure through the carry flag.
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

/// Darwin `read` in the Unix class.
const DARWIN_READ: u32 = DARWIN_UNIX_CLASS | 3;

/// The hosted kernel an exact x86-64 target enters, or `None` for targets
/// whose boundary is exclusively C-called (Windows, UEFI).
pub(crate) fn hosted_kernel(target: NativeTarget) -> Option<HostedKernel> {
    if target == NativeTarget::linux_x64() {
        Some(LINUX)
    } else if target == NativeTarget::macos_x64() {
        Some(DARWIN)
    } else {
        None
    }
}

/// Whether the exact target realizes hosted byte and exit leaves through
/// `syscall`.
pub(crate) fn hosted_syscall_target(target: NativeTarget) -> bool {
    hosted_kernel(target).is_some()
}

/// The process-exit call number the exact target's hosted exit leaf enters.
pub(crate) fn hosted_exit_syscall_number(target: NativeTarget) -> Option<u32> {
    hosted_kernel(target).map(|kernel| kernel.exit)
}

/// Import-free `exit(value)` for an immediate status on either hosted
/// kernel: `mov edi, imm32; mov eax, nr; syscall; ud2`.
pub fn encode_hosted_exit_process_i32_for_target(
    target: NativeTarget,
    value: i32,
) -> Result<Vec<u8>, Diagnostic> {
    let kernel = hosted_kernel(target).ok_or_else(|| {
        Diagnostic::error("x86-64 hosted exit has no syscall kernel for this target")
    })?;
    let mut bytes = Vec::with_capacity(14);
    bytes.push(0xbf); // mov edi, imm32
    bytes.extend_from_slice(&value.to_le_bytes());
    bytes.push(0xb8); // mov eax, imm32
    bytes.extend_from_slice(&kernel.exit.to_le_bytes());
    bytes.extend_from_slice(&[0x0f, 0x05]); // syscall
    bytes.extend_from_slice(&[0x0f, 0x0b]); // ud2 if exit returns
    Ok(bytes)
}

/// Import-free Darwin `read(0, &byte, 1)` into one canonical
/// `ByteRead = Eof | Byte(i32)` stack home. The home is zeroed first, so an
/// exact zero-byte read leaves the ordinal-zero `Eof` value; an exact
/// one-byte read writes ordinal one after the kernel filled the payload
/// byte. Carry, and every other count, traps.
pub fn encode_darwin_read_byte_to_stack(
    home_byte_offset: u32,
    payload_byte_offset: u32,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(DARWIN_READ_BYTE_LEAF_BYTES);
    bytes.extend_from_slice(&[0x31, 0xc0]); // xor eax, eax
    for offset in [home_byte_offset, payload_byte_offset] {
        bytes.extend_from_slice(&[0x89, 0x84, 0x24]); // mov [rsp+disp32], eax
        bytes.extend_from_slice(&offset.to_le_bytes());
    }
    bytes.extend_from_slice(&[0x31, 0xff]); // xor edi, edi (stdin)
    bytes.extend_from_slice(&[0x48, 0x8d, 0xb4, 0x24]); // lea rsi, [rsp+disp32]
    bytes.extend_from_slice(&payload_byte_offset.to_le_bytes());
    bytes.extend_from_slice(&[0xba, 1, 0, 0, 0]); // mov edx, 1
    bytes.push(0xb8); // mov eax, SYS_read | Unix class
    bytes.extend_from_slice(&DARWIN_READ.to_le_bytes());
    bytes.extend_from_slice(&[0x0f, 0x05]); // syscall
    let carry_branch = bytes.len();
    bytes.extend_from_slice(&[0x72, 0]); // jc trap: consume carry before CMP
    bytes.extend_from_slice(&[0x48, 0x83, 0xf8, 0]); // cmp rax, 0
    let eof_branch = bytes.len();
    bytes.extend_from_slice(&[0x74, 0]); // je done
    bytes.extend_from_slice(&[0x48, 0x83, 0xf8, 1]); // cmp rax, 1
    let count_branch = bytes.len();
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
            .map_err(|_| Diagnostic::error("Darwin x86-64 read-byte branch is out of range"))
    };
    bytes[carry_branch + 1] = rel8(trap, carry_branch + 2)?;
    bytes[eof_branch + 1] = rel8(done, eof_branch + 2)?;
    bytes[count_branch + 1] = rel8(trap, count_branch + 2)?;
    bytes[done_branch + 1] = rel8(done, done_branch + 2)?;
    Ok(bytes)
}

/// Exact byte width of [`encode_darwin_read_byte_to_stack`].
pub(crate) const DARWIN_READ_BYTE_LEAF_BYTES: usize = 67;

/// Decode the complete Darwin read leaf independently of its encoder,
/// returning its structural home byte offset.
pub(crate) fn decode_darwin_read_byte(bytes: &[u8]) -> Option<u32> {
    if bytes.len() != DARWIN_READ_BYTE_LEAF_BYTES {
        return None;
    }
    let word = |start: usize| -> Option<u32> {
        Some(u32::from_le_bytes(
            bytes.get(start..start + 4)?.try_into().ok()?,
        ))
    };
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
            // mov eax, 0x2000003; syscall; jc trap -- carry is consumed
            // before `cmp rax, 0` replaces the flags.
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
