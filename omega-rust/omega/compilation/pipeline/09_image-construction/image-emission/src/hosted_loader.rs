//! Inspect the ABI frame, body call and completion of an emitted hosted loader entry.
use target::NativeTarget;

// mov edi, 0; mov eax, __NR_exit_group; syscall; ud2
pub(crate) const LINUX_X64_EXIT_ZERO: &[u8] =
    &[0xbf, 0, 0, 0, 0, 0xb8, 231, 0, 0, 0, 0x0f, 5, 0x0f, 0x0b];

pub(crate) fn matches_entry(
    target: NativeTarget,
    start: usize,
    entry: usize,
    bytes: &[u8],
) -> bool {
    if target.architecture == target::Architecture::Aarch64 {
        let (prefix, suffix): (&[u32], &[u32]) = if target == NativeTarget::macos_arm64() {
            (
                &[0xa9bf_7bfd, 0x9100_03fd],
                &[0xa8c1_7bfd, 0x5280_0000, 0xd65f_03c0],
            )
        } else if target == NativeTarget::linux_arm64() {
            (&[], &[0xd280_0000, 0xd280_0bc8, 0xd400_0001, 0xd420_0000])
        } else {
            return false;
        };
        if !start.is_multiple_of(4) || bytes.len() != 4 * (prefix.len() + suffix.len() + 1) {
            return false;
        }
        let (opening, remaining) = bytes.split_at(4 * prefix.len());
        let (branch, closing) = remaining.split_at(4);
        let word = u32::from_le_bytes(branch.try_into().expect("four-byte AArch64 branch"));
        let words_match = |actual: &[u8], expected: &[u32]| {
            actual
                .chunks_exact(4)
                .zip(expected)
                .all(|(bytes, word)| bytes == word.to_le_bytes())
        };
        // Sign-extend imm26 before scaling to bytes.
        let displacement = i64::from(((word << 6) as i32) >> 6) * 4;
        return words_match(opening, prefix)
            && words_match(closing, suffix)
            && word >> 26 == 0b100101
            && start as i128 + opening.len() as i128 + i128::from(displacement) == entry as i128;
    }

    let (prefix, suffix): (&[u8], &[u8]) = if target == NativeTarget::linux_x64() {
        (&[], LINUX_X64_EXIT_ZERO)
    } else if target == NativeTarget::windows_x64() {
        (
            &[0x48, 0x83, 0xec, 40],
            &[0x48, 0x83, 0xc4, 40, 0x31, 0xc0, 0xc3],
        )
    } else if target == NativeTarget::macos_x64() {
        (
            &[0x48, 0x83, 0xec, 8],
            &[0x48, 0x83, 0xc4, 8, 0x31, 0xc0, 0xc3],
        )
    } else {
        return false;
    };
    let Some(call) = bytes
        .strip_prefix(prefix)
        .and_then(|rest| rest.strip_suffix(suffix))
    else {
        return false;
    };
    let &[0xe8, a, b, c, d] = call else {
        return false;
    };
    let displacement = i32::from_le_bytes([a, b, c, d]);
    start as i128 + prefix.len() as i128 + 5 + i128::from(displacement) == entry as i128
}
