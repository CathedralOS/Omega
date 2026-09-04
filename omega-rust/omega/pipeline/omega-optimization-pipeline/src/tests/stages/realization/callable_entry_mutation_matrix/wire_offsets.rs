//! Independent traversal of the canonical one-parameter callable-record fixture.

#[derive(Debug, Clone, Copy)]
pub(super) struct RecordWireOffsets {
    pub vocabulary: usize,
    pub architecture: usize,
    pub object_format: usize,
    pub pointer_size: usize,
    pub pointer_alignment: usize,
    pub semantic_entry: usize,
    pub semantic_entry_symbol: usize,
    pub symbol_name_byte: usize,
    pub calling_policy: usize,
    pub parameter_ordinal: usize,
    pub parameter_value: usize,
    pub scalar_tag: usize,
    pub integer_scalar_tag: usize,
    pub integer_carrier: usize,
    pub integer_sign: usize,
    pub integer_bits: usize,
    pub shape_tag: usize,
    pub register_tag: usize,
    pub register_index: usize,
    pub exit_policy: usize,
    pub hardening: usize,
    pub entry_assumption: usize,
    pub disposition: usize,
}

fn length(bytes: &[u8], offset: usize) -> usize {
    usize::try_from(u64::from_le_bytes(
        bytes[offset..offset + 8].try_into().unwrap(),
    ))
    .unwrap()
}

fn skip_units(bytes: &[u8], offset: &mut usize) {
    let count = length(bytes, *offset);
    *offset += 8 + count * 2;
}

fn skip_scalar(bytes: &[u8], offset: &mut usize) {
    match bytes[*offset] {
        1 => *offset += 1,
        2 => *offset += 5,
        3 => *offset += 2,
        tag => panic!("fixture contains unknown scalar tag {tag}"),
    }
}

pub(super) fn record_wire_offsets(bytes: &[u8]) -> RecordWireOffsets {
    let mut offset = 44;
    offset += 64;
    let vocabulary = offset;
    offset += 34;
    offset += 32;
    let architecture = offset;
    let object_format = offset + 1;
    let pointer_size = offset + 2;
    let pointer_alignment = offset + 10;
    offset += 18;
    let semantic_entry = offset;
    offset += 8 + 32 * 6;
    let semantic_entry_symbol = offset;
    offset += 8;
    let name_length = length(bytes, offset);
    offset += 8;
    assert_ne!(name_length, 0);
    let symbol_name_byte = offset;
    offset += name_length;
    offset += 16;
    let calling_policy = offset;
    offset += 1;
    assert_eq!(length(bytes, offset), 1);
    offset += 8;
    let parameter_ordinal = offset;
    offset += 8;
    let parameter_value = offset;
    offset += 8;
    let scalar_tag = offset;
    assert_eq!(bytes[scalar_tag], 1);
    skip_scalar(bytes, &mut offset);
    let shape_tag = offset;
    offset += 5;
    offset += 6;
    let register_tag = offset;
    let register_index = offset + 1;
    offset += 6;
    skip_units(bytes, &mut offset);
    offset += 8;
    let integer_scalar_tag = offset;
    assert_eq!(bytes[integer_scalar_tag], 2);
    let integer_carrier = offset + 1;
    let integer_sign = offset + 2;
    let integer_bits = offset + 3;
    offset += 5;
    offset += 5 + 2 + 2;
    skip_units(bytes, &mut offset);
    let return_count = length(bytes, offset);
    assert_eq!(return_count, 2);
    offset += 8;
    for _ in 0..return_count {
        offset += 8 + 8 + 4 + 4 + 2;
        skip_units(bytes, &mut offset);
    }
    let exit_policy = offset;
    offset += 1;
    let hardening = offset;
    offset += 1;
    let entry_assumption = offset;
    assert_eq!(bytes[entry_assumption], 1);
    offset += 1;
    offset += 2 + 2 + 2;
    let disposition = offset;
    offset += 1;
    assert_eq!(offset, bytes.len());
    RecordWireOffsets {
        vocabulary,
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
        semantic_entry,
        semantic_entry_symbol,
        symbol_name_byte,
        calling_policy,
        parameter_ordinal,
        parameter_value,
        scalar_tag,
        integer_scalar_tag,
        integer_carrier,
        integer_sign,
        integer_bits,
        shape_tag,
        register_tag,
        register_index,
        exit_policy,
        hardening,
        entry_assumption,
        disposition,
    }
}
