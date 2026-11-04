//! A decisive exchange retries failed exclusive stores, never a mismatching
//! comparison. The returned prior belongs to the attempt that either commits
//! or observes the mismatch. Expected/replacement remain unchanged across all
//! attempts, and narrow comparisons use only their stored representation.

use super::Aarch64SelectedFormEncodingError;
use selected_instructions::AtomicWidth;

pub(super) const REGISTERS: [u8; 4] = [9, 10, 11, 12];
const BARRIER: u32 = 0xd503_3fbf;
const CLEAR_EXCLUSIVE: u32 = 0xd503_3f5f;

pub(super) const fn byte_count(width: AtomicWidth) -> u16 {
    if matches!(width, AtomicWidth::Bits8 | AtomicWidth::Bits16) {
        40
    } else {
        36
    }
}

pub(super) fn encode(width: AtomicWidth) -> Vec<u8> {
    let size = u32::from(width.ordinal()) << 30;
    let mut words = vec![BARRIER, size | 0x085f_fc00 | 9 << 5 | 12];
    let expected = match width {
        AtomicWidth::Bits8 => {
            words.push(0x5300_1c00 | 10 << 5 | 16);
            16
        }
        AtomicWidth::Bits16 => {
            words.push(0x5300_3c00 | 10 << 5 | 16);
            16
        }
        AtomicWidth::Bits32 | AtomicWidth::Bits64 => 10,
    };
    let comparison = if width == AtomicWidth::Bits64 {
        0xeb00_001f
    } else {
        0x6b00_001f
    };
    let retry = if expected == 16 {
        0x35ff_ff71
    } else {
        0x35ff_ff91
    };
    words.extend([
        comparison | expected << 16 | 12 << 5,
        0x5400_0081, // B.NE reaches CLREX without attempting a store.
        size | 0x0800_fc00 | 17 << 16 | 9 << 5 | 11,
        retry,       // CBNZ W17 returns to LDAXR after a lost reservation.
        0x1400_0002, // Successful store skips CLREX and joins at the barrier.
        CLEAR_EXCLUSIVE,
        BARRIER,
    ]);
    words.into_iter().flat_map(u32::to_le_bytes).collect()
}

/// Independently decode exact opcodes, width, register homes and both exits.
/// The retry edge must reload; no stale observation can supply completion.
pub(super) fn validate(
    width: AtomicWidth,
    bytes: &[u8],
) -> Result<(), Aarch64SelectedFormEncodingError> {
    let narrow = matches!(width, AtomicWidth::Bits8 | AtomicWidth::Bits16);
    let (encoded, tail) = bytes.as_chunks::<4>();
    if encoded.len() != 9 + usize::from(narrow) || !tail.is_empty() {
        return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
    }
    let words = encoded
        .iter()
        .copied()
        .map(u32::from_le_bytes)
        .collect::<Vec<_>>();
    let register = |word: u32, shift: u32| (word >> shift) & 31;
    let normalized = !narrow
        || (words[2] & 0xffff_0000 == 0x5300_0000
            && (words[2] >> 10) & 63 == u32::from(width.bytes()) * 8 - 1
            && register(words[2], 5) == 10
            && register(words[2], 0) == 16);
    let suffix = &words[2 + usize::from(narrow)..];
    let comparison = if width == AtomicWidth::Bits64 {
        0xeb00_001f
    } else {
        0x6b00_001f
    };
    let matches = normalized
        && words[0] == BARRIER
        && words[1] & !0xc000_03ff == 0x085f_fc00
        && (words[1] >> 30) as u8 == width.ordinal()
        && register(words[1], 5) == 9
        && register(words[1], 0) == 12
        && suffix[0] & !0x001f_03e0 == comparison
        && register(suffix[0], 16) == if narrow { 16 } else { 10 }
        && register(suffix[0], 5) == 12
        && suffix[1] == 0x5400_0081
        && suffix[2] & !0xc01f_03ff == 0x0800_fc00
        && (suffix[2] >> 30) as u8 == width.ordinal()
        && register(suffix[2], 16) == 17
        && register(suffix[2], 5) == 9
        && register(suffix[2], 0) == 11
        && suffix[3] == if narrow { 0x35ff_ff71 } else { 0x35ff_ff91 }
        && suffix[4] == 0x1400_0002
        && suffix[5] == CLEAR_EXCLUSIVE
        && suffix[6] == BARRIER;
    if matches {
        Ok(())
    } else {
        Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
    }
}
