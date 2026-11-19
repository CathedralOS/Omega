//! Inspect the Linux write loop, its address/branch words and its aligned literal.
use std::ops::Range;

pub fn layout(literal_bytes: usize) -> Option<(Range<usize>, usize)> {
    let payload = literal_bytes.checked_add(1)?;
    if payload >= 1 << 20 {
        return None;
    }
    let start = if payload > u16::MAX as usize { 52 } else { 48 };
    let end = start + payload;
    Some((start..end, end.next_multiple_of(4)))
}

pub fn decode(bytes: &[u8]) -> Option<&[u8]> {
    let instruction = |n: usize| {
        let word: [u8; 4] = bytes.get(n * 4..n * 4 + 4)?.try_into().ok()?;
        Some(u32::from_le_bytes(word))
    };
    let adr = instruction(0)?;
    let low = instruction(1)?;
    if adr & 0x9f00_001f != 0x1000_0001 || low & 0xffe0_001f != 0xd280_0002 {
        return None;
    }
    let mut length = (low >> 5) & 0xffff;
    let high = instruction(2)?;
    let call = if high & 0xffe0_001f == 0xf2a0_0002 {
        length |= ((high >> 5) & 0xffff) << 16;
        3
    } else {
        2
    };
    let (data, end) = layout(usize::try_from(length.checked_sub(1)?).ok()?)?;
    let data_offset = (((adr >> 5) & 0x7ffff) << 2) | ((adr >> 29) & 3);
    if bytes.len() != end || data_offset as usize != data.start || (call + 10) * 4 != data.start {
        return None;
    }

    // The failing write branches to BRK, while the retry branches back to the
    // file descriptor load. The unconditional branch skips all aligned data.
    let loop_words = [
        0xd280_0808,
        0xd280_0020,
        0xd400_0001,
        0xf100_001f,
        0x5400_00ad,
        0x8b00_0021,
        0xeb00_0042,
        0x54ff_ff41,
    ];
    if loop_words
        .into_iter()
        .enumerate()
        .any(|(i, word)| instruction(call + i) != Some(word))
    {
        return None;
    }
    let skip = call + 8;
    let branch = instruction(skip)?;
    if branch & 0xfc00_0000 != 0x1400_0000
        || (branch & 0x03ff_ffff) as usize != end / 4 - skip
        || instruction(skip + 1)? != 0xd420_0000
        || bytes.get(data.end..end)?.iter().any(|byte| *byte != 0)
    {
        return None;
    }
    bytes.get(data)?.strip_suffix(b"\n")
}

#[cfg(test)]
mod tests {
    #[test]
    fn final_loop_checks_short_and_wide_counts_and_zero_padding() {
        for count in [0, 1, 2, 3, 4, 65534, 65535, (1 << 20) - 2] {
            let literal = vec![0x81; count];
            let (mut encoded, range) =
                isa_aarch64::encode_linux_write_line_literal(&literal).unwrap();
            assert_eq!(super::layout(count), Some((range.clone(), encoded.len())));
            assert_eq!(super::decode(&encoded), Some(literal.as_slice()));
            for index in (0..range.start).chain(range.end - 1..encoded.len()) {
                for bit in 0..8 {
                    encoded[index] ^= 1 << bit;
                    assert_ne!(
                        super::decode(&encoded),
                        Some(literal.as_slice()),
                        "length {count}, byte {index}, bit {bit}"
                    );
                    encoded[index] ^= 1 << bit;
                }
            }
        }
        assert!(super::layout((1 << 20) - 1).is_none());
    }
}
