//! Unsigned offsets realized as 64-bit `add`/`sub` (immediate) steps.
//!
//! One A64 `add`/`sub` immediate carries twelve bits, optionally shifted left
//! by twelve, so a shifted-then-unshifted pair reaches 16 MiB - 1. Frame
//! extents, record fields and pointer displacements are `u32`-sized, so an
//! offset past that pair continues with further shifted steps of at most 4095
//! pages each before the unshifted remainder. The steps need no scratch
//! register: the first reads the base and every later step reads and writes
//! the destination, so a destination that aliases its base — the allocator's
//! `AddressOffset` aliasing, or `sp` adjusting itself — stays exact.
//!
//! Every offset has exactly one step sequence: full 4095-page steps, then the
//! partial page step, then the unshifted remainder when nonzero. Decoding
//! enforces that shape, so a replayed sequence cannot carry the same sum in a
//! different split.
//!
//! Register number 31 names `sp` in both the base and destination fields of
//! these instructions; the frame protocol and frame forms rely on that.

/// `add xD, xN, #imm` (64-bit, unshifted, flags preserved).
pub(crate) const ADD_IMMEDIATE: u32 = 0x9100_0000;
/// `sub xD, xN, #imm` (64-bit, unshifted, flags preserved).
pub(crate) const SUBTRACT_IMMEDIATE: u32 = 0xd100_0000;
/// The `sh` bit selecting `#imm, lsl #12`.
const SHIFT_TWELVE: u32 = 1 << 22;
/// The largest value one twelve-bit immediate field carries.
const IMMEDIATE_LIMIT: u64 = 4095;

/// One shifted or unshifted step of an offset sequence.
const fn step(opcode: u32, shifted: bool, immediate: u64, destination: u8, base: u8) -> u32 {
    opcode
        | if shifted { SHIFT_TWELVE } else { 0 }
        | ((immediate as u32) << 10)
        | ((base as u32) << 5)
        | destination as u32
}

/// Append `destination = base ± pages * 4096` as shifted steps of at most
/// 4095 pages each. Nothing is appended for zero pages.
pub(crate) fn append_page_steps(
    words: &mut Vec<u32>,
    opcode: u32,
    destination: u8,
    base: u8,
    pages: u64,
) {
    let mut remaining = pages;
    let mut source = base;
    while remaining != 0 {
        let chunk = remaining.min(IMMEDIATE_LIMIT);
        words.push(step(opcode, true, chunk, destination, source));
        remaining -= chunk;
        source = destination;
    }
}

/// Append `destination = base ± amount`: the page steps, then the unshifted
/// remainder when nonzero. A zero amount appends nothing.
pub(crate) fn append_offset_steps(
    words: &mut Vec<u32>,
    opcode: u32,
    destination: u8,
    base: u8,
    amount: u64,
) {
    let before = words.len();
    append_page_steps(words, opcode, destination, base, amount >> 12);
    let low = amount & IMMEDIATE_LIMIT;
    if low != 0 {
        let source = if words.len() == before {
            base
        } else {
            destination
        };
        words.push(step(opcode, false, low, destination, source));
    }
}

/// `destination = base + amount` as an address: the offset steps, and one
/// `add xD, xN, #0` for a zero amount so the destination is always written.
pub(crate) fn address_words(destination: u8, base: u8, amount: u64) -> Vec<u32> {
    let mut words = Vec::with_capacity(address_word_count(amount) as usize);
    append_offset_steps(&mut words, ADD_IMMEDIATE, destination, base, amount);
    if words.is_empty() {
        words.push(step(ADD_IMMEDIATE, false, 0, destination, base));
    }
    words
}

/// Align an address within a padded, fixed frame. SP itself is unchanged.
/// The member offset follows the mask so fields keep their declared offsets.
pub(crate) fn aligned_frame_address_words(
    destination: u8,
    frame_offset: u32,
    alignment: u16,
    member_offset: u32,
) -> Option<Vec<u32>> {
    if alignment <= 16 || !alignment.is_power_of_two() || !frame_offset.is_multiple_of(16) {
        return None;
    }
    let shift = alignment.trailing_zeros();
    let mut words = address_words(
        destination,
        31,
        u64::from(frame_offset) + u64::from(alignment - 1),
    );
    // AND (not ANDS): preserve NZCV and all registers except the destination.
    words.push(
        0x9240_0000
            | ((64 - shift) << 16)
            | ((63 - shift) << 10)
            | (u32::from(destination) << 5)
            | u32::from(destination),
    );
    append_offset_steps(
        &mut words,
        ADD_IMMEDIATE,
        destination,
        destination,
        u64::from(member_offset),
    );
    Some(words)
}

pub(crate) fn validate_aligned_frame_address_words(
    words: &[u32],
    destination: u8,
    frame_offset: u32,
    alignment: u16,
    member_offset: u32,
) -> bool {
    if alignment <= 16 || !alignment.is_power_of_two() || !frame_offset.is_multiple_of(16) {
        return false;
    }
    let Some(mask_position) = words
        .iter()
        .position(|word| word & 0xffc0_0000 == 0x9240_0000)
    else {
        return false;
    };
    let mask = words[mask_position];
    let ones = (mask >> 10) & 63;
    let rotation = (mask >> 16) & 63;
    let decoded_mask = (u64::MAX >> (63 - ones)).rotate_right(rotation);
    let suffix = &words[mask_position + 1..];
    mask & 31 == u32::from(destination)
        && (mask >> 5) & 31 == u32::from(destination)
        && decoded_mask == !u64::from(alignment - 1)
        && decode_address_words(&words[..mask_position], destination, 31)
            == Some(u64::from(frame_offset) + u64::from(alignment - 1))
        && if member_offset == 0 {
            suffix.is_empty()
        } else {
            decode_address_words(suffix, destination, destination) == Some(u64::from(member_offset))
        }
}

/// How many words [`address_words`] emits for `amount`.
pub(crate) const fn address_word_count(amount: u64) -> u64 {
    let pages = (amount >> 12).div_ceil(IMMEDIATE_LIMIT);
    if amount & IMMEDIATE_LIMIT != 0 || amount == 0 {
        pages + 1
    } else {
        pages
    }
}

/// Largest byte size an address sequence over any `u32` offset encodes to:
/// 257 page steps and one remainder step.
pub(crate) const MAX_ADDRESS_BYTES: u16 = (address_word_count(u32::MAX as u64) * 4) as u16;

/// Largest byte size of a scratch-addressed 64-bit frame store: the page
/// steps of the last page-aligned `u32` offset, then the store itself.
pub(crate) const MAX_STORE64_BYTES: u16 =
    ((address_word_count(u32::MAX as u64 & !4095) + 1) * 4) as u16;

/// Decode one canonical address sequence for `destination = base + offset`.
/// Every word must be a 64-bit unshifted or shifted `add` immediate writing
/// the destination; only the first reads the base. Shifted steps come first,
/// all but the last carry exactly 4095 pages, and at most one unshifted
/// remainder follows them. A lone unshifted step may carry zero.
pub(crate) fn decode_address_words(words: &[u32], destination: u8, base: u8) -> Option<u64> {
    let mut offset = 0_u64;
    let mut previous_pages = None;
    let mut remainder_seen = false;
    for (position, word) in words.iter().copied().enumerate() {
        let source = if position == 0 { base } else { destination };
        if word & 0xff80_0000 != ADD_IMMEDIATE
            || word & 31 != u32::from(destination)
            || (word >> 5) & 31 != u32::from(source)
            || remainder_seen
        {
            return None;
        }
        let immediate = u64::from((word >> 10) & 4095);
        if word & SHIFT_TWELVE != 0 {
            if immediate == 0 || previous_pages.is_some_and(|pages| pages != IMMEDIATE_LIMIT) {
                return None;
            }
            previous_pages = Some(immediate);
            offset += immediate << 12;
        } else {
            if immediate == 0 && position != 0 {
                return None;
            }
            remainder_seen = true;
            offset += immediate;
        }
    }
    (!words.is_empty()).then_some(offset)
}

#[cfg(test)]
mod tests {
    use super::{
        ADD_IMMEDIATE, MAX_ADDRESS_BYTES, SUBTRACT_IMMEDIATE, address_word_count, address_words,
        append_offset_steps, decode_address_words, step,
    };

    #[test]
    fn short_offsets_keep_the_one_and_two_word_forms() {
        // add x9, sp, #0 / #17; add x9, sp, #1, lsl #12; then the pair.
        assert_eq!(address_words(9, 31, 0), vec![0x9100_03e9]);
        assert_eq!(address_words(9, 31, 17), vec![0x9100_47e9]);
        assert_eq!(address_words(9, 31, 4096), vec![0x9140_07e9]);
        assert_eq!(address_words(9, 31, 4192), vec![0x9140_07e9, 0x9101_8129]);
        assert_eq!(
            address_words(9, 31, 16_777_215),
            vec![0x917f_ffe9, 0x913f_fd29]
        );
    }

    #[test]
    fn offsets_past_sixteen_mebibytes_continue_with_page_steps() {
        // 32 MiB - 1 = 8191 pages + 4095: 4095 + 4095 + 1 pages, then 4095.
        let words = address_words(3, 7, 33_554_431);
        assert_eq!(
            words,
            vec![0x917f_fce3, 0x917f_fc63, 0x9140_0463, 0x913f_fc63]
        );
        assert_eq!(words.len() as u64, address_word_count(33_554_431));
        assert_eq!(decode_address_words(&words, 3, 7), Some(33_554_431));
        // Aliased base and destination: every step reads the register.
        let aliased = address_words(5, 5, 33_554_432);
        assert_eq!(decode_address_words(&aliased, 5, 5), Some(33_554_432));
    }

    #[test]
    fn every_u32_offset_round_trips_within_the_declared_bound() {
        for offset in [
            0_u64,
            1,
            4095,
            4096,
            16_777_215,
            16_777_216,
            16_773_120,
            33_554_432,
            1 << 31,
            u64::from(u32::MAX),
        ] {
            let words = address_words(1, 31, offset);
            assert_eq!(words.len() as u64, address_word_count(offset), "{offset}");
            assert!(
                words.len() * 4 <= usize::from(MAX_ADDRESS_BYTES),
                "{offset}"
            );
            assert_eq!(
                decode_address_words(&words, 1, 31),
                Some(offset),
                "{offset}"
            );
        }
        assert_eq!(MAX_ADDRESS_BYTES, 258 * 4);
    }

    #[test]
    fn decoding_rejects_noncanonical_splits_and_registers() {
        let words = address_words(3, 7, 33_554_431);
        // 4094 + 4095 + 2 pages is the same sum in a different split.
        let split = vec![
            step(ADD_IMMEDIATE, true, 4094, 3, 7),
            step(ADD_IMMEDIATE, true, 4095, 3, 3),
            step(ADD_IMMEDIATE, true, 2, 3, 3),
            words[3],
        ];
        assert_eq!(decode_address_words(&split, 3, 7), None);
        // A later step reading the original base loses the running sum.
        let mut reread = words.clone();
        reread[1] = step(ADD_IMMEDIATE, true, 4095, 3, 7);
        assert_eq!(decode_address_words(&reread, 3, 7), None);
        // A shifted step after the remainder is out of order, and a zero
        // step after the first pads the sequence.
        let reordered = vec![
            step(ADD_IMMEDIATE, false, 4095, 3, 7),
            step(ADD_IMMEDIATE, true, 1, 3, 3),
        ];
        assert_eq!(decode_address_words(&reordered, 3, 7), None);
        let padded = vec![words[0], step(ADD_IMMEDIATE, false, 0, 3, 3)];
        assert_eq!(decode_address_words(&padded, 3, 7), None);
        // Subtraction is not an address step.
        let mut subtract = Vec::new();
        append_offset_steps(&mut subtract, SUBTRACT_IMMEDIATE, 3, 7, 17);
        assert_eq!(decode_address_words(&subtract, 3, 7), None);
        assert_eq!(decode_address_words(&[], 3, 7), None);
    }

    #[test]
    fn stack_adjustments_emit_nothing_for_zero_and_alias_sp() {
        let mut words = Vec::new();
        append_offset_steps(&mut words, SUBTRACT_IMMEDIATE, 31, 31, 0);
        assert!(words.is_empty());
        append_offset_steps(&mut words, ADD_IMMEDIATE, 31, 31, 33_554_448);
        // add sp, sp, #4095, lsl #12 (x2); add sp, sp, #2, lsl #12; add sp, sp, #16
        assert_eq!(
            words,
            vec![0x917f_ffff, 0x917f_ffff, 0x9140_0bff, 0x9100_43ff]
        );
    }
}
