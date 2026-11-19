//! Inspect the Linux write loop, its relative branches and the following literal.
use std::ops::Range;

const CODE_BYTES: usize = 52;

pub fn layout(literal_bytes: usize) -> Option<(Range<usize>, usize)> {
    let payload = literal_bytes.checked_add(1)?;
    // The jump over the trap and payload must fit a signed relative displacement.
    i32::try_from(payload.checked_add(2)?).ok()?;
    let end = CODE_BYTES.checked_add(payload)?;
    Some((CODE_BYTES..end, end))
}

pub fn decode(bytes: &[u8]) -> Option<&[u8]> {
    let code = bytes.first_chunk::<CODE_BYTES>()?;
    let length = usize::try_from(::wire::codec_bytes::u32_at(code, 13)?).ok()?;
    let (data, end) = layout(length.checked_sub(1)?)?;
    let target = |field: usize| {
        let offset = i32::from_le_bytes(code.get(field..field + 4)?.try_into().ok()?);
        field.checked_add(4)?.checked_add_signed(offset as isize)
    };
    if bytes.len() != end
        || code[..8] != [0xbf, 1, 0, 0, 0, 0x48, 0x8d, 0x35]
        || target(8) != Some(data.start)
        || code[12] != 0xba
        || code[17..29] != [0xb8, 1, 0, 0, 0, 0x0f, 0x05, 0x48, 0x85, 0xc0, 0x0f, 0x8e]
        || target(29) != Some(50)
        || code[33..41] != [0x48, 0x01, 0xc6, 0x48, 0x29, 0xc2, 0x0f, 0x85]
        || target(41) != Some(17)
        || code[45] != 0xe9
        || target(46) != Some(end)
        || code[50..] != [0x0f, 0x0b]
    {
        return None;
    }
    bytes.get(data)?.strip_suffix(b"\n")
}

#[cfg(test)]
mod tests {
    #[test]
    fn final_branch_targets_and_literal_match_the_real_encoder() {
        for literal in [b"".as_slice(), b"\0binary\xff\n"] {
            let (encoded, range) = isa_x86_64::encode_linux_write_line_literal(literal).unwrap();
            assert_eq!(
                super::layout(literal.len()),
                Some((range.clone(), encoded.len()))
            );
            assert_eq!(super::decode(&encoded), Some(literal));
            for bit in 0..encoded.len() * 8 {
                let mut changed = encoded.clone();
                changed[bit / 8] ^= 1 << (bit % 8);
                assert_ne!(super::decode(&changed), Some(literal), "bit {bit}");
            }
        }
        assert!(super::layout(i32::MAX as usize - 2).is_none());
    }
}
