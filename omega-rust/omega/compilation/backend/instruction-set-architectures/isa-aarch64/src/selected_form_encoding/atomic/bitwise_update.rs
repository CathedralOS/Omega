//! Repeat a baseline exclusive update until its store commits, returning that
//! successful attempt's observed value. Address and mask stay unchanged; the
//! replacement and store status use declared scratch registers. Full barriers
//! preserve the source ordering, and the effect record retains retry work.

use super::Aarch64SelectedFormEncodingError;
use selected_instructions::{AtomicForm, AtomicOperation, AtomicWidth};

pub(super) const BYTE_COUNT: u16 = 24;
pub(super) const REGISTERS: [u8; 3] = [9, 10, 11];
const BARRIER: u32 = 0xd503_3fbf;

pub(super) fn encode(form: AtomicForm) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
    let logical = match form.operation {
        AtomicOperation::FetchAnd => 0x0a00_0000,
        AtomicOperation::FetchOr => 0x2a00_0000,
        AtomicOperation::FetchXor => 0x4a00_0000,
        _ => return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch),
    };
    let size = u32::from(form.width.ordinal()) << 30;
    let wide = u32::from(form.width == AtomicWidth::Bits64) << 31;
    let words = [
        BARRIER,
        size | 0x085f_fc00 | 9 << 5 | 11, // LDAXR observed, [address].
        wide | logical | 10 << 16 | 11 << 5 | 16, // replacement = observed OP mask.
        size | 0x0800_fc00 | 17 << 16 | 9 << 5 | 16, // STLXR status, replacement, [address].
        0x35ff_ffb1, // CBNZ W17, back to the exclusive load, not the old replacement.
        BARRIER,
    ];
    Ok(words.into_iter().flat_map(u32::to_le_bytes).collect())
}

/// Authenticate the complete instruction stream, including its exact retry
/// edge and preserved input homes. A separate load or a stale replacement
/// cannot borrow the successful-attempt observation contract.
pub(super) fn validate(
    form: AtomicForm,
    bytes: &[u8],
) -> Result<(), Aarch64SelectedFormEncodingError> {
    let opcode = match form.operation {
        AtomicOperation::FetchAnd => 0x0a00_0000,
        AtomicOperation::FetchOr => 0x2a00_0000,
        AtomicOperation::FetchXor => 0x4a00_0000,
        _ => return Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch),
    };
    let (encoded, tail) = bytes.as_chunks::<4>();
    if encoded.len() != 6 || !tail.is_empty() {
        return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
    }
    let words: [u32; 6] = std::array::from_fn(|index| u32::from_le_bytes(encoded[index]));
    let register = |word: u32, shift: u32| (word >> shift) & 31;
    let matches = words[0] == BARRIER
        && words[1] & !0xc000_03ff == 0x085f_fc00
        && (words[1] >> 30) as u8 == form.width.ordinal()
        && register(words[1], 5) == 9
        && register(words[1], 0) == 11
        && words[2] & !0x801f_03ff == opcode
        && (words[2] >> 31 != 0) == (form.width == AtomicWidth::Bits64)
        && register(words[2], 16) == 10
        && register(words[2], 5) == 11
        && register(words[2], 0) == 16
        && words[3] & !0xc01f_03ff == 0x0800_fc00
        && (words[3] >> 30) as u8 == form.width.ordinal()
        && register(words[3], 16) == 17
        && register(words[3], 5) == 9
        && register(words[3], 0) == 16
        && words[4] == 0x35ff_ffb1
        && words[5] == BARRIER;
    if matches {
        Ok(())
    } else {
        Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusive_words_match_baseline_armv8_assembler_vectors() {
        // clang -target arm64-apple-macosx -march=armv8-a. These encodings
        // require no LSE extension. The status branch reloads after failure.
        for (width, load, store) in [
            (AtomicWidth::Bits8, 0x085f_fd2b, 0x0811_fd30),
            (AtomicWidth::Bits16, 0x485f_fd2b, 0x4811_fd30),
            (AtomicWidth::Bits32, 0x885f_fd2b, 0x8811_fd30),
            (AtomicWidth::Bits64, 0xc85f_fd2b, 0xc811_fd30),
        ] {
            for (operation, narrow, wide) in [
                (AtomicOperation::FetchAnd, 0x0a0a_0170, 0x8a0a_0170),
                (AtomicOperation::FetchOr, 0x2a0a_0170, 0xaa0a_0170),
                (AtomicOperation::FetchXor, 0x4a0a_0170, 0xca0a_0170),
            ] {
                let form = AtomicForm { operation, width };
                let logical = if width == AtomicWidth::Bits64 {
                    wide
                } else {
                    narrow
                };
                let expected = [BARRIER, load, logical, store, 0x35ff_ffb1, BARRIER]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>();
                assert_eq!(encode(form).unwrap(), expected);
                validate(form, &expected).unwrap();
            }
        }
    }
}
