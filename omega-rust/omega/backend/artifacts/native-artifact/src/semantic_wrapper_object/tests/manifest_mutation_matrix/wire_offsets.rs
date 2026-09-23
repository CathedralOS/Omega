//! Structural offsets for the fixed ProgramStorage wrapper manifest envelope.

#[derive(Debug, Clone, Copy)]
pub(super) struct WireOffsets {
    pub stage: usize,
    pub vocabulary: usize,
    pub architecture: usize,
    pub object_format: usize,
    pub pointer_size: usize,
    pub pointer_alignment: usize,
    pub wrapper_symbol: usize,
    pub continuation_symbol: usize,
    pub unavailable: [usize; 4],
}

pub(super) fn wire_offsets(bytes: &[u8]) -> WireOffsets {
    let stage = 44;
    let vocabulary = stage + 1 + 7 * 32;
    let architecture = vocabulary + 2 + 32;
    let object_format = architecture + 1;
    let pointer_size = object_format + 1;
    let pointer_alignment = pointer_size + 8;
    let wrapper_symbol = pointer_alignment + 8;
    let continuation_symbol = wrapper_symbol + 8;
    let unavailable_start = continuation_symbol + 8 + 3 * 8;
    let unavailable = [
        unavailable_start,
        unavailable_start + 1,
        unavailable_start + 2,
        unavailable_start + 3,
    ];
    assert_eq!(bytes.len(), unavailable[3] + 1);
    WireOffsets {
        stage,
        vocabulary,
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
        wrapper_symbol,
        continuation_symbol,
        unavailable,
    }
}
