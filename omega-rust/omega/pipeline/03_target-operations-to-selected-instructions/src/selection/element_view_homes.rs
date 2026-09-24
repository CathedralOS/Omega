//! Invocation-local element-view homes; calls additionally materialize a descriptor.
use selected_instructions::VirtualRegisterId;
use semantic_vocabulary::{PlaceId, ValueId};

/// Element views share the byte-view {base, extent} descriptor ABI; extent is
/// an element count and `element_stride` scales element indices into byte
/// offsets at each use site. Each producer establishes
/// byte_offset + element_index * element_stride <= root bound in mathematical
/// integers, so subsequent byte-offset sums are exact U64. Construction and
/// replay establish this invariant separately from the admitted source chain;
/// this record itself grants no proof or memory-read authority.
#[derive(Clone, Copy)]
pub(super) struct ElementViewHomes {
    pub place: PlaceId,
    pub backing_pointer: VirtualRegisterId,
    /// Byte offset of the first viewed element inside the root storage.
    pub byte_offset: VirtualRegisterId,
    /// Element count carried in the descriptor's extent word.
    pub element_length: VirtualRegisterId,
    /// The unviewed root's element length in U64.
    pub root_length: ValueId,
    /// Aligned byte width of one element.
    pub element_stride: u32,
}
