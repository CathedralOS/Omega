//! Invocation-local homes for a derived view; no addressable descriptor layout.
use selected_instructions::VirtualRegisterId;
use semantic_vocabulary::{PlaceId, ValueId};

/// Each producer establishes offset + length <= root_length in mathematical
/// integers. The root's length is U64, so subsequent offset sums are exact U64.
/// Construction and replay establish this invariant separately from the admitted
/// source chain; this record itself grants no proof or memory-read authority.
#[derive(Clone, Copy)]
pub(super) struct ByteViewHomes {
    pub place: PlaceId,
    pub backing_pointer: VirtualRegisterId,
    pub byte_offset: VirtualRegisterId,
    pub byte_length: VirtualRegisterId,
    pub root_length: ValueId,
}
