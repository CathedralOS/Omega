//! Attempt one exchange without a retry loop or an LSE feature assumption.
//! Keep the exclusive load's observation. A mismatch clears its reservation;
//! a matching comparison attempts exactly one store. The store's status,
//! rather than equality, distinguishes Exchanged from Uncommitted. Full-system
//! barriers conservatively preserve the source ordering on every exit.

use super::Aarch64SelectedFormEncodingError;
use selected_instructions::AtomicWidth;

pub(super) const BYTE_COUNT: u32 = 40;
const FULL_SYSTEM_BARRIER: u32 = 0xd503_3fbf;
const CLEAR_EXCLUSIVE: u32 = 0xd503_3f5f;

pub(super) fn encode(
    width: AtomicWidth,
    registers: &[u8],
) -> Result<Vec<u8>, Aarch64SelectedFormEncodingError> {
    let [address, expected, replacement, observed, outcome] = registers else {
        return Err(Aarch64SelectedFormEncodingError::OperandCountMismatch);
    };
    let address = u32::from(*address);
    let expected = u32::from(*expected);
    let replacement = u32::from(*replacement);
    let observed = u32::from(*observed);
    let outcome = u32::from(*outcome);
    let size = u32::from(width.ordinal()) << 30;
    let comparison = if width == AtomicWidth::Bits64 {
        0xeb00_001f
    } else {
        0x6b00_001f
    };
    Ok([
        FULL_SYSTEM_BARRIER,
        size | 0x085f_fc00 | address << 5 | observed,
        comparison | expected << 16 | observed << 5,
        0x5400_0081, // b.ne to the mismatch path at word 7.
        size | 0x0800_fc00 | outcome << 16 | address << 5 | replacement,
        0x1100_0400 | outcome << 5 | outcome, // status 0/1 becomes tag 1/2.
        0x1400_0003, // Both result paths join at the final barrier, word 9.
        CLEAR_EXCLUSIVE,
        0x5280_0000 | outcome, // Mismatched = 0; no store was attempted.
        FULL_SYSTEM_BARRIER,
    ]
    .into_iter()
    .flat_map(u32::to_le_bytes)
    .collect())
}

/// Decode every opcode, register and internal branch independently. In
/// particular, a backward branch or a changed status/tag mapping cannot
/// borrow this form's bounded-work or atomic-outcome contract.
pub(super) fn validate(
    width: AtomicWidth,
    registers: &[u8],
    bytes: &[u8],
) -> Result<(), Aarch64SelectedFormEncodingError> {
    let [address, expected, replacement, observed, outcome] = registers else {
        return Err(Aarch64SelectedFormEncodingError::OperandCountMismatch);
    };
    let (encoded, tail) = bytes.as_chunks::<4>();
    if encoded.len() != 10 || !tail.is_empty() {
        return Err(Aarch64SelectedFormEncodingError::MalformedEncoding);
    }
    let words = encoded
        .iter()
        .copied()
        .map(u32::from_le_bytes)
        .collect::<Vec<_>>();
    let register = |word: u32, shift: u32| ((word >> shift) & 31) as u8;
    let comparison = if width == AtomicWidth::Bits64 {
        0xeb00_001f
    } else {
        0x6b00_001f
    };
    let matched = words[0] == FULL_SYSTEM_BARRIER
        && words[9] == FULL_SYSTEM_BARRIER
        && words[1] & !0xc000_03ff == 0x085f_fc00
        && (words[1] >> 30) as u8 == width.ordinal()
        && register(words[1], 5) == *address
        && register(words[1], 0) == *observed
        && words[2] & !0x001f_03e0 == comparison
        && register(words[2], 16) == *expected
        && register(words[2], 5) == *observed
        && words[3] == 0x5400_0081
        && words[4] & !0xc01f_03ff == 0x0800_fc00
        && (words[4] >> 30) as u8 == width.ordinal()
        && register(words[4], 16) == *outcome
        && register(words[4], 5) == *address
        && register(words[4], 0) == *replacement
        && words[5] & !0x0000_03ff == 0x1100_0400
        && register(words[5], 5) == *outcome
        && register(words[5], 0) == *outcome
        && words[6] == 0x1400_0003
        && words[7] == CLEAR_EXCLUSIVE
        && words[8] & !0x0000_001f == 0x5280_0000
        && register(words[8], 0) == *outcome;
    if matched {
        Ok(())
    } else {
        Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
    }
}
