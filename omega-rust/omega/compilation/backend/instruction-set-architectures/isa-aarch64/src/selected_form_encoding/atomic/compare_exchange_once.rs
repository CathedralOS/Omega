//! Attempt one exchange without a retry loop or an LSE feature assumption.
//! Keep the exclusive load's observation. A mismatch clears its reservation;
//! a matching comparison attempts exactly one store. The store's status,
//! rather than equality, distinguishes Exchanged from Uncommitted. Full-system
//! barriers conservatively preserve the source ordering on every exit.

use super::Aarch64SelectedFormEncodingError;
use selected_instructions::AtomicWidth;

const FULL_SYSTEM_BARRIER: u32 = 0xd503_3fbf;
const CLEAR_EXCLUSIVE: u32 = 0xd503_3f5f;

pub(super) const fn byte_count(width: AtomicWidth) -> u16 {
    if matches!(width, AtomicWidth::Bits8 | AtomicWidth::Bits16) {
        44
    } else {
        40
    }
}

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
    let mut words = vec![
        FULL_SYSTEM_BARRIER,
        size | 0x085f_fc00 | address << 5 | observed,
    ];
    // Narrow exclusive loads zero-extend their raw representation. A signed
    // expected carrier may be sign-extended, so compare only the stored bits.
    // The outcome register is scratch until STLXR supplies its actual status.
    let normalization = match width {
        AtomicWidth::Bits8 => Some(0x5300_1c00), // UXTB Woutcome, Wexpected.
        AtomicWidth::Bits16 => Some(0x5300_3c00), // UXTH Woutcome, Wexpected.
        AtomicWidth::Bits32 | AtomicWidth::Bits64 => None,
    };
    let comparison_expected = if let Some(normalization) = normalization {
        words.push(normalization | expected << 5 | outcome);
        outcome
    } else {
        expected
    };
    words.extend([
        comparison | comparison_expected << 16 | observed << 5,
        0x5400_0081, // b.ne skips the store and reaches CLREX four words later.
        size | 0x0800_fc00 | outcome << 16 | address << 5 | replacement,
        0x1100_0400 | outcome << 5 | outcome, // status 0/1 becomes tag 1/2.
        0x1400_0003,                          // Both result paths join at the final barrier.
        CLEAR_EXCLUSIVE,
        0x5280_0000 | outcome, // Mismatched = 0; no store was attempted.
        FULL_SYSTEM_BARRIER,
    ]);
    Ok(words.into_iter().flat_map(u32::to_le_bytes).collect())
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
    let narrow = matches!(width, AtomicWidth::Bits8 | AtomicWidth::Bits16);
    let (encoded, tail) = bytes.as_chunks::<4>();
    if encoded.len() != 10 + usize::from(narrow) || !tail.is_empty() {
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
    // UBFM with immr=0 and imms=width-1 is the zero-extension alias. Check
    // its fields rather than asking the encoder to reproduce expected bytes.
    let normalized = !narrow
        || (words[2] & 0xffff_0000 == 0x5300_0000
            && (words[2] >> 10) & 63 == u32::from(width.bytes()) * 8 - 1
            && register(words[2], 5) == *expected
            && register(words[2], 0) == *outcome);
    let completion = &words[2 + usize::from(narrow)..];
    let comparison_expected = if narrow { *outcome } else { *expected };
    let matched = normalized
        && words[0] == FULL_SYSTEM_BARRIER
        && words[1] & !0xc000_03ff == 0x085f_fc00
        && (words[1] >> 30) as u8 == width.ordinal()
        && register(words[1], 5) == *address
        && register(words[1], 0) == *observed
        && completion[0] & !0x001f_03e0 == comparison
        && register(completion[0], 16) == comparison_expected
        && register(completion[0], 5) == *observed
        && completion[1] == 0x5400_0081
        && completion[2] & !0xc01f_03ff == 0x0800_fc00
        && (completion[2] >> 30) as u8 == width.ordinal()
        && register(completion[2], 16) == *outcome
        && register(completion[2], 5) == *address
        && register(completion[2], 0) == *replacement
        && completion[3] & !0x0000_03ff == 0x1100_0400
        && register(completion[3], 5) == *outcome
        && register(completion[3], 0) == *outcome
        && completion[4] == 0x1400_0003
        && completion[5] == CLEAR_EXCLUSIVE
        && completion[6] & !0x0000_001f == 0x5280_0000
        && register(completion[6], 0) == *outcome
        && completion[7] == FULL_SYSTEM_BARRIER;
    if matched {
        Ok(())
    } else {
        Err(Aarch64SelectedFormEncodingError::EncodedFormMismatch)
    }
}
