//! Materializing a 64-bit immediate by the canonical zero-seeded MOVZ/MOVK
//! sequence.

/// Append the canonical zero-seeded selected-form materialization of one
/// exact 64-bit value onto `words`.
pub(crate) fn append_canonical_materialization(words: &mut Vec<u32>, register: u8, value: u64) {
    let chunks = std::array::from_fn::<_, 4, _>(|index| ((value >> (index * 16)) & 0xffff) as u16);
    words.push(0xd280_0000 | (u32::from(chunks[0]) << 5) | u32::from(register));
    for (index, chunk) in chunks.into_iter().enumerate().skip(1) {
        if chunk != 0 {
            words.push(
                0xf280_0000
                    | (u32::try_from(index).expect("halfword index fits u32") << 21)
                    | (u32::from(chunk) << 5)
                    | u32::from(register),
            );
        }
    }
}
