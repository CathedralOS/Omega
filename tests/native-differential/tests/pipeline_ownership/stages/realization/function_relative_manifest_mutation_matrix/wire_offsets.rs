//! Structural offsets for both V11 optional-transformation shapes.

#[derive(Debug, Clone, Copy)]
pub(super) struct WireOffsets {
    pub stage: usize,
    pub selected_completion_status: usize,
    pub x86_relaxation_status: usize,
    pub post_allocation_status: usize,
    pub post_allocation_optimization: Option<usize>,
    pub architecture: usize,
    pub object_format: usize,
    pub pointer_size: usize,
    pub pointer_alignment: usize,
    pub layout_policy: usize,
    pub scope: usize,
    pub frame_disposition: usize,
    pub unavailable: [usize; 7],
}

pub(super) fn wire_offsets(bytes: &[u8]) -> WireOffsets {
    let mut offset = 44;
    let stage = offset;
    offset += 1 + 32 + 32;
    let selected_completion_status = offset;
    let selected_completion_present = bytes[offset] == 1;
    offset += 1 + usize::from(selected_completion_present) * 32;
    offset += 32 + 11 * 32;
    let x86_relaxation_status = offset;
    let x86_relaxation_present = bytes[offset] == 1;
    offset += 1 + usize::from(x86_relaxation_present) * 32;
    let post_allocation_status = offset;
    let post_allocation_present = bytes[offset] == 1;
    offset += 1;
    let post_allocation_optimization = post_allocation_present.then_some(offset);
    if post_allocation_present {
        offset += 1 + 4 * 32 + 3 * 8;
    }
    offset += 32;
    let architecture = offset;
    let object_format = offset + 1;
    let pointer_size = offset + 2;
    let pointer_alignment = offset + 10;
    offset += 18;
    let layout_policy = offset;
    let scope = offset + 1;
    offset += 2 + 10 * 8;
    let frame_disposition = offset;
    offset += 1 + usize::from(bytes[offset] == 2) * 64;
    let unavailable = [
        offset,
        offset + 1,
        offset + 2,
        offset + 3,
        offset + 4,
        offset + 5,
        offset + 6,
    ];
    assert_eq!(bytes.len(), unavailable[6] + 1);
    WireOffsets {
        stage,
        selected_completion_status,
        x86_relaxation_status,
        post_allocation_status,
        post_allocation_optimization,
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
        layout_policy,
        scope,
        frame_disposition,
        unavailable,
    }
}
